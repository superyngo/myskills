# dispatch-agent — Rust Rewrite Plan

**Date:** 2026-05-10
**Scope:** Replace the three Python scripts (`detect.py`, `init.py`, `dispatch.py`) with a single Rust binary exposing them as subcommands, plus a new `config` subcommand. Annotate `data/cli-templates.toml` so it doubles as user-facing reference.

---

## 1. Goals & Non-Goals

### Goals
1. One Rust binary `dispatch-agent` with subcommands `detect`, `init`, `dispatch`, `config`.
2. Full functional parity with current Python scripts — same JSON output (detect), same TOML output (init), same dispatch semantics (tier fallback, round-robin, env injection, source-file wrapping, recursion guard, signal handling, timeout, verbose ticks).
3. New `config` subcommand: open editor (`config edit` is default), `config show`, `config path`; respect `--config PATH` global flag and `EDITOR` / `VISUAL` env vars with sensible fallback.
4. Rewrite `data/cli-templates.toml` as a richly commented reference covering every supported field (`detect_binary`, `subcommand`, `prompt_flag`, `prompt_positional`, `model_flag`, `file_input_mode`, `version_flag`, `verified`, `extra_args`).
5. Keep Python scripts in place for now; do not break existing SKILL.md routing in this PR.

### Non-Goals
- No async runtime, no logging framework.
- No packaging/distribution pipeline yet (handled in a follow-up).
- No new dispatch features (e.g. parallel fan-out) — strict port.
- SKILL.md / references rewrite to point at the binary is a follow-up phase.
- **v1 targets unix (macOS, Linux) only.** All process-group, signal, and `flock` code is gated behind `#[cfg(unix)]`. On Windows the binary still compiles; `detect`/`init`/`config` work, but `dispatch` exits 1 with `error: dispatch is unix-only in v1`. Full Windows support (CREATE_NEW_PROCESS_GROUP, SetConsoleCtrlHandler, byte-range locks) is a follow-up.

---

## 2. Layout

```
skills/dispatch-agent/
  rust/
    Cargo.toml
    src/
      main.rs           # clap parser, top-level subcommand dispatch
      cli.rs            # clap derive structs
      types.rs          # leaf: Config, Tier, Agent, EnvEntry, Template, DetectInfo
      config.rs         # find_config, find_git_root, load Config TOML
      templates.rs      # load cli-templates.toml (owns §7 resolution chain)
      detect.rs         # detect subcommand
      init.rs           # init subcommand (stdin JSON → TOML)
      dispatch/
        mod.rs            # subcommand entry, tier traversal, rr-state mutation
        command.rs        # build_command + wrap_with_sources (argv construction)
        display.rs        # format_list, format_show_config (shared with config_cmd)
        process/
          mod.rs          # spawn, pipe threads, wait loop, verbose ticker, ChildState FSM
          unix.rs         # #[cfg(unix)]: setsid, killpg, signal_hook setup
          windows.rs      # #[cfg(windows)]: stub returning "dispatch is unix-only" error
      config_cmd.rs     # config subcommand (edit / show / path); editor inlined
      env.rs            # resolve_env_var, get_source_files, build_env
      rr_state.rs       # round-robin state load/store with file lock
      fsutil.rs         # write_atomic (mkdir-p, temp, chmod 0600, rename)
    tests/
      bin/
        fake_agent.rs       # subprocess test harness (FAKE_AGENT_MODE env)
      fixtures/
        inputs/             # canonical inputs used by golden-file tests
        golden/             # Python-generated expected outputs (parity)
      snapshots/            # insta snapshot files (committed)
      detect.rs
      init.rs
      dispatch.rs
      config_cmd.rs
    scripts/
      regen_golden.sh       # regenerate fixtures/golden/ from Python
      parity_check.sh       # run Python + Rust on each fixture, diff outputs
  scripts/              # Python scripts — untouched in this rewrite
  data/cli-templates.toml  # rewritten with comments
```

Binary name: `dispatch-agent` (set via `[[bin]]` in Cargo.toml). Pre-built artifact will be committed under `skills/dispatch-agent/bin/<target-triple>/dispatch-agent` in a follow-up PR; this PR delivers source + tests only.

### 2.1 Module dependency graph (DAG, leaves at bottom)

```
main.rs        → cli, detect, init, dispatch::mod, config_cmd
detect.rs      → templates, types
init.rs        → types, fsutil, config (find_git_root)
dispatch/mod   → types, templates, env, rr_state, fsutil, detect (for --list no-config), dispatch::{command,process,display}
config_cmd.rs  → config, types, templates, dispatch::display
env.rs         → types
rr_state.rs    → fsutil, types
templates.rs   → types
config.rs      → types, fsutil
fsutil.rs      → (leaf — std only)
types.rs       → (leaf — serde derives only, no I/O, no business logic)
```

Direction is strictly downward; no cycles possible by construction.

### 2.2 Public API signatures (key entry points)

```rust
// types.rs — pure data; all derive Serialize+Deserialize where needed.
pub struct Config { pub version: Option<u32>, pub tiers: Vec<Tier> }
pub struct Tier   { pub id: String, pub agents: Vec<Agent> }
pub struct Agent  { pub id: String, pub cli: String, pub model: Option<String>,
                    pub args: Vec<String>, pub env: Vec<EnvEntry>,
                    pub template: Option<String> }
pub enum   EnvEntry { File{name:String,path:String}, Env{name:String,var:String}, Source{path:String} }
pub enum FileInputMode { Arg }     // serde rename "arg"; rejects unknown variants
pub struct Template { pub detect_binary: Option<String>, pub subcommand: Option<String>,
                      pub prompt_flag: Option<String>, pub prompt_positional: bool,
                      pub model_flag: Option<String>, pub extra_args: Vec<String>,
                      pub version_flag: Option<String>, pub file_input_mode: Option<FileInputMode>,
                      pub verified: bool }
//   Field defaults applied via serde:
//     #[serde(default)]            on prompt_positional, extra_args, etc.
//     #[serde(default = "true_fn")] on `verified` (helper fn returning true)
//   `version_flag = Some("--version")` default applied at use-site (templates.rs
//   load helper), not in serde — easier than a tagged literal default.
pub struct DetectInfo { pub path: Option<String>, pub version: Option<String>,
                        pub callable: bool, pub verified: bool }

// fsutil.rs
pub fn write_atomic(path: &Path, content: &[u8]) -> anyhow::Result<()>;
//   Creates parent dirs (mkdir -p), writes to a unique temp file in the same
//   directory (name = ".{stem}.{pid}.{nanos}.tmp" via File::create_new for
//   O_EXCL atomicity), then rename → dest. On unix:
//     - mode 0600 set atomically at creation via OpenOptionsExt::mode(0o600)
//       (matches Python os.open(...,0o600); no umask visibility window)
//     - O_NOFOLLOW set via OpenOptionsExt::custom_flags(libc::O_NOFOLLOW) so a
//       pre-placed symlink at the temp path causes ELOOP, not link-following
//       (TOCTOU defense)
//   An explicit chmod 0600 follows as defense-in-depth. On any failure after
//   temp creation, the temp file is unlinked before returning Err. (A SIGKILL
//   between create and rename can leave a 0600 .tmp behind; harmless — it'll
//   be overwritten by the next attempt with a different nanos suffix.)
pub fn expand_tilde(path: &str) -> anyhow::Result<PathBuf>;
//   `~` and `~/...` → home_dir() + remainder. Errors if home dir unknown.

// templates.rs — owns the §7 resolution chain end-to-end.
pub fn load_templates() -> anyhow::Result<IndexMap<String, Template>>;

// config.rs
pub fn find_git_root() -> PathBuf;             // git rev-parse, fallback to cwd
pub fn find_config(arg: Option<&Path>) -> Option<PathBuf>;
pub fn load_config(path: &Path) -> anyhow::Result<Config>;

// dispatch/command.rs
impl Template { pub fn build_command(&self, agent: &Agent, prompt: &str) -> Option<Vec<String>>; }
pub fn wrap_with_sources(cmd: Vec<String>, sources: &[String]) -> Vec<String>;
pub(crate) fn shell_quote(s: &str) -> String;  // single-quote with `'` → `'\''`

// detect.rs
pub fn run_detect(templates: &IndexMap<String, Template>) -> IndexMap<String, DetectInfo>;
//   Pure function: probes PATH for each template's detect_binary, optionally
//   runs `--version`, applies `verified` flag. Used by both the `detect`
//   subcommand AND `dispatch --list` no-config fallback.

// env.rs
pub fn resolve_env_var(ev: &EnvEntry) -> Option<(String, String)>;  // None for Source variant
pub fn get_source_files(agent: &Agent) -> Vec<String>;              // expanded paths
pub fn build_env(agent: &Agent, current_depth: i64) -> HashMap<String, String>;
//   Inherits parent env, overlays resolved (non-source) entries, sets
//   DISPATCH_AGENT_DEPTH = current_depth + 1.

// rr_state.rs
pub fn load_rr_state(path: &Path) -> IndexMap<String, String>;
//   Returns {} on NotFound (silent); warns and returns {} on PermissionDenied
//   or parse error. tier_id → next-agent-id.
pub fn store_rr_state(path: &Path, state: &IndexMap<String, String>) -> anyhow::Result<()>;
//   Atomic write via fsutil::write_atomic.

// dispatch/display.rs — pure functions, return formatted String for testability.
pub fn format_list(config: &Config) -> String;
pub fn format_show_config(config: &Config, path: &Path) -> String;
pub fn format_list_detect(detect: &IndexMap<String, DetectInfo>) -> String;
```

`IndexMap` is used (instead of `HashMap`) wherever stable ordering matters — notably template iteration in `detect` (replaces D8's hand-serialization).

---

## 3. Dependencies

| Crate | Use | Justification |
|-------|-----|---------------|
| `clap` (derive) | Argv parsing, subcommands, help text | Standard, less code than hand-rolled |
| `serde` + `serde_derive` | Struct deserialisation | Consumed by `toml` and `serde_json` |
| `toml` | Read/write TOML | Replaces `tomllib` |
| `serde_json` | Parse stdin JSON in `init`, parse rr-state, emit detect output | Replaces Python `json` |
| `anyhow` | Error wrapping with context | Concise error propagation |
| `which` | `shutil.which` equivalent | Tiny, well-tested |
| `fs2` | `flock` for rr-state lock file | Cross-platform advisory lock |
| `indexmap` | Insertion-ordered map for templates and detect output | Stable JSON/iteration order without hand-serialization |
| `signal-hook` | SIGINT/SIGTERM forwarding with safe pipe-based handlers | Calling `killpg` from a raw libc handler is async-signal-unsafe; signal-hook is the standard safe wrapper |
| `wait-timeout` | `child.wait_timeout(d)` without a wait thread | Replaces hand-rolled Condvar + wait thread; small, reuses `libc` already in tree |
| `libc` | `O_NOFOLLOW`, `setsid`, `killpg`, `flock` direct syscalls on unix | Direct dep needed by `fsutil.rs` and `dispatch/process/unix.rs` independent of `signal-hook`'s transitive use |
| `dirs` | `home_dir()` for `~` expansion and `$HOME` resolution | Mirrors Python `Path.home()` + `os.path.expanduser`; portable home lookup with passwd fallback on unix |
| `insta` (dev) | Snapshot tests for formatted output | Catches whitespace and truncation bugs that `contains()` asserts miss |
| `proptest` (dev) | Property tests for `build_command` invariants | Generates pathological template/agent combinations the example tests miss |

No tokio, no tracing. All blocking I/O in main thread; subprocess I/O via `std::process::Command` + threads/`select` equivalent.

**MSRV:** `rust-version = "1.77"` (for `File::create_new`). All deps compile cleanly under both `x86_64-unknown-linux-gnu` (glibc ≥ 2.17) and `x86_64-unknown-linux-musl`. CI matrix targets glibc by default; musl static builds are deferred to the distribution PR. No glibc-specific assumptions exist anywhere in the dependency tree.

---

## 4. CLI Surface

### Top-level
```
dispatch-agent [--config PATH] <SUBCOMMAND>
```

`--config PATH` is a **global** flag (parsed at root, propagated). It overrides the search order (`<git-root>/.config/dispatch-agent.toml` then `~/.config/dispatch-agent.toml`).

### Subcommands

#### `detect`
- No args.
- Output: same JSON shape as today
  ```json
  {"claude": {"path": "...", "version": "...", "callable": true, "verified": true}, ...}
  ```
- Reads `data/cli-templates.toml` resolved relative to **the binary's installed location**, with `DISPATCH_AGENT_TEMPLATES` env override (see §7).

#### `init`
- Reads JSON from stdin (same schema as today).
- Writes TOML to:
  - `<git-root>/.config/dispatch-agent.toml` if `save_location == "project"`
  - `~/.config/dispatch-agent.toml` otherwise
- Prints destination path on stdout on success. Also prints stderr hint: `hint: run 'dispatch-agent config edit' to fine-tune your configuration`.
- Validation parity: agent id regex `^[a-zA-Z0-9_-]+$`, unique ids, args is `Vec<String>`, env entries match one of `file`/`env`/`source`.
- Atomic write to `*.tmp` in same dir + chmod 0600 + rename. Round-trip-validate generated TOML before rename.

#### `dispatch`
Args (1:1 with Python):
```
dispatch [-p PROMPT | -f FILE]
         [--timeout N] [--tier ID | --agent ID]
         [--config PATH]    # also accepted at root
         [--dry-run] [--list] [--show-config] [--verbose]
```
Behaviours preserved exactly:
- `--timeout 0` → error; `-1` (default) → no timeout. Type: `i64`. Use clap's default validator format for non-numeric input; custom message only for the `0` case (`error: --timeout 0 is invalid; use -1 for no timeout`).
- `-f` rejects > 256 KiB (`error: file exceeds 256 KiB limit`); missing file → `error: file not found: <path>` (exit 1).
- `--list` with no config falls back to detect-style printout (see `format_list_detect`); with config, uses `format_list` which groups by tier and shows `[✓]`/`[✗]` per agent.
- `--show-config` prints layer (`user` / `project`) and resolved tiers/agents/env. **Requires a config file to exist; if none found, exit 1 with `error: no config file found. Run 'dispatch-agent init' to create one.`**
- `--dry-run` makes `-p`/`-f` optional. With no prompt, the printed command substitutes the literal placeholder `<prompt>` (lowercase, matching Python).
- `--agent ID` not found in config → exit 1 + `error: agent 'ID' not found in config`. Bypasses tier traversal entirely (single attempt).
- `--tier ID` not found → exit 1 + `error: tier 'ID' not found in config`. Otherwise: traversal starts at that tier and continues to subsequent tiers on failure.
- Recursion guard: `DISPATCH_AGENT_DEPTH` parsed via `.trim().parse::<i64>()` (matches Python's `int()` which strips whitespace; tolerates trailing `\n` from `echo`-set values). **Unset → 0. Unparseable** (e.g. `"abc"`) → exit 1 + `error: invalid DISPATCH_AGENT_DEPTH value 'abc': expected integer` (silently defaulting would defeat the guard if the env got corrupted). If current depth ≥ 5 → exit 1 + `error: recursion depth limit (5) reached`. Before spawning child, set the var to `current + 1` in the child's env only.
- Per-agent process group (`setsid` via `pre_exec` on unix) so timeout/signal kills children.
- SIGINT/SIGTERM forwarded to child group via `signal-hook` watcher thread, then dispatcher exits 1.
- Verbose ticker every 10 s while child runs. `--verbose` also prints `[attempting <agent.id>]` before each attempt and `[<agent.id>] (tier: <tier.id>)` on success (matches Python). It does NOT change `--list`/`--show-config`/`--dry-run` output.
- `failure code == -SIGKILL` reported as `timeout` in the failure summary.
- **Exit code propagation:** on the success branch, dispatcher exits with the child's exit code (0 if child returned 0; otherwise the child's code is treated as failure and triggers fallback). All-agents-failed → exit 1. (Python's behaviour: a non-zero child rc triggers fallback, so "success" here means child rc == 0; dispatcher exit 0 on success.)
- rr-state file `~/.cache/dispatch-agent/rr-state.json`. If missing, treat as `{}`; create parent dirs on first write. Advisory exclusive lock held during read+mutate+write; pointer updated **only on success**.
- Source env files wrapped in **a single** `bash -c 'set -a; source A; source B; ...; set +a; exec "$@"' -- <cmd>` (one bash layer, not nested). Each `path` is shell-quoted via `shell_quote(s) = "'" + s.replace("'", "'\\''") + "'"` before interpolation — **mandatory** to prevent shell injection from paths containing `'`, `$`, backticks, or spaces (e.g. `/home/alice/d'ev/env`). Helper lives in `dispatch/command.rs`. Hardcoded bash is intentional (parity with Python `shlex.quote`); do NOT shell-detect.
- Mutex groups implemented via `clap::ArgGroup { multiple: false, required: false }` for `-p`/`-f` and `--tier`/`--agent`.
- `verified = false` agents are skipped at dispatch with a stderr warning: `warning: agent 'ID' uses unverified template 'NAME', skipping`.
- Template lookup per agent: `templates.get(agent.template.as_deref().unwrap_or(&agent.cli))`. **Tier mode:** missing template → stderr warning, skip agent (does not abort tier). **`--agent` mode:** missing template → exit 1 + `error: template 'X' for agent 'Y' not found in cli-templates.toml` (no fallback exists, so this is a hard error, matching Python `dispatch.py:362-363`).
- Tiers with zero agents are silently skipped (parity with Python).
- Duplicate `agent.id` across the config: at load time emit a stderr warning `warning: multiple agents with id 'X', using first`; do NOT fail. Matches Python `dispatch.py:209-211`.
- Config missing top-level `version` field: stderr warning `warning: config missing 'version' field, assuming v1` and proceed.
- TOML loaders (`load_config` and `load_templates`) strip a leading UTF-8 BOM (`\u{FEFF}`) before calling `toml::from_str`. Mirrors Python's `tomllib`. CRLF is handled by the `toml` crate.
- `~` expansion applies to: `EnvEntry::File.path`, `EnvEntry::Source.path`, the user config path, AND `--config PATH` values. All routed through `fsutil::expand_tilde` (`dirs::home_dir()` + manual `~` strip). If `$HOME` is unset and we need it, exit 1 + `error: cannot determine home directory ($HOME not set)`.
- **No-config dispatch:** if no config is found AND the invocation is a real dispatch (not `--list`/`--dry-run`/`--show-config`/`--help`), exit 1 with `error: no config file found. Run 'dispatch-agent init' to create one.` (mirrors the `--show-config` path; gives users guidance instead of cryptic "all agents failed").
- **Per-agent processing order in tier mode:** for each candidate (1) resolve template via `agent.template.unwrap_or(agent.cli)`; if absent, warn and skip; (2) check `template.verified`; if false, warn and skip; (3) `build_command`; if `None`, warn and skip; (4) `wrap_with_sources`; (5) `build_env`; (6) spawn.

#### `config`
```
dispatch-agent [--config PATH] config [edit | show | path]
```
- `config` (no arg) → `edit`.
- `edit` — resolve config path. If no config exists and `--config` was NOT given, exit 1 and suggest `dispatch-agent init` (per D3). If `--config PATH` was given and the path is missing, create a minimal stub: `version = 1\n# See cli-templates.toml for available templates and field reference.\n` (user opted in by naming the path).
  - Editor resolution chain: `$EDITOR` → `$VISUAL` → platform default (`vi` on unix, `notepad` on windows). Empty / whitespace-only values are skipped.
  - **`$EDITOR` splitting:** value is split on whitespace (`split_whitespace()`); the first token is the command, remaining tokens are prepended as args before the file path. Handles `code -w`, `vim +10`, `emacs -nw`. **No** shell expansion / glob / tilde — `$EDITOR` is trusted by convention (user-set in their own shell profile), and `Command::new` keeps us out of `sh -c` territory.
  - After the editor exits: if the file's mtime is unchanged and exit code was 0, print stderr hint: `hint: your editor may have returned immediately. For GUI editors, set EDITOR to include a wait flag (e.g. 'code -w', 'subl -w')`.
  - Then re-run `load_config` on the saved file. If TOML/validation fails, print `warning: config has syntax errors: <msg>` to stderr but exit 0 (the user chose to save it; a follow-up dispatch will re-surface the error).
- `show` — uses the **identical** config-resolution + error-reporting code path as `dispatch --show-config` (shared via `dispatch::display::format_show_config` and a shared `resolve_or_error` helper). Error messages are guaranteed identical, not just output format.
- `path` — print resolved path (or non-zero exit + message if none found and `--config` not given). On the no-config error path, also print the default search locations to stderr for discoverability:
  ```
  error: no config file found
  hint: default locations searched:
    <git-root>/.config/dispatch-agent.toml (project)
    ~/.config/dispatch-agent.toml (user)
  ```
  When `--config PATH` is given, print PATH literally even if the file doesn't exist (the user explicitly asked).
- **No `config init` subcommand.** Configuration initialization is the top-level `init` command (which reads JSON from stdin); `config edit` opens an existing config in an editor. The dual paths are intentional and distinct.
- `--config PATH` overrides resolution. For `edit` with `--config PATH`, the path is created if missing (user opted in by naming it). Without `--config`, if no config exists, exit 1 and suggest `dispatch-agent init`. (No `--create` flag.)

---

## 5. Annotated `cli-templates.toml`

Replace the current minimal file with a fully commented reference. Outline:

```toml
# =====================================================================
# cli-templates.toml — describes HOW dispatch-agent invokes each agent CLI.
#
# This file ships with the dispatch-agent binary. It is NOT your
# personal configuration. Your config lives in:
#   ~/.config/dispatch-agent.toml          (user-level, shared)
#   <git-root>/.config/dispatch-agent.toml (project-level)
#
# Run `dispatch-agent init` to generate a config, or
# `dispatch-agent config edit` to open an existing one.
# Each `cli = "..."` (or `template = "..."`) in your config must match
# a top-level table key in THIS file. See §5.1 below for an annotated
# config example.
# =====================================================================

# === Field reference ===
#
# detect_binary      (string, default = template name)
#   Executable name searched on PATH for `detect` and availability
#   checks. Use when the actual binary differs from the logical
#   template name (e.g. gemini-npx → npx).
#
# subcommand         (string, default = "")
#   Inserted right after the binary, before any args/flags.
#   Example: opencode uses `opencode run <prompt>`.
#
# prompt_flag        (string, default = "")
#   Flag used to pass the prompt non-interactively (e.g. "-p", "-q").
#   WARNING: if both `prompt_flag = ""` and `prompt_positional = false`,
#   the agent is **silently skipped** at dispatch (no error printed
#   beyond a stderr warning). Always set one or the other.
#
# prompt_positional  (bool, default = false)
#   If true, prompt is appended as a positional arg right after the
#   subcommand. Used with `subcommand` for run-style CLIs.
#
# model_flag         (string, default = "")
#   Flag for model selection (e.g. "--model"). If empty and the agent's
#   `model` field is not `"default"`, the model is silently dropped
#   (a stderr warning is emitted).
#
# extra_args         (string array, default = [])
#   Args ALWAYS placed before `agent.args`. Both blocks come AFTER the
#   subcommand and BEFORE the model/prompt flags (see command shape).
#
# version_flag       (string, default = "--version")
#   Flag passed during `detect` to read a version string. Empty disables
#   the version probe (e.g. when binary is launched via npx and you
#   don't want to download a package just to read a version). When
#   empty, `detect` reports `"version": null` for this template.
#
# file_input_mode    (string enum {"arg"}, default = "arg")
#   With "arg": when the user passes `-f FILE`, the file is read and
#   its CONTENTS become the prompt string (delivered via `prompt_flag`
#   or positionally). In v1, "arg" is the only valid value and omitting
#   the field is equivalent. Field retained for forward compatibility
#   (e.g. future stdin piping); deprecation TBD.
#
# verified           (bool, default = true)
#   Whether non-interactive mode for this CLI has been verified.
#   `verified = false` → agent is listed by `detect`/`config show`/
#   `--list`, but **skipped at dispatch** with a stderr warning. Use
#   while triaging new CLIs whose -p/--print contract is not trusted.
#
# === Resulting command shape ===
#
#   <detect_binary|template_name>
#       [subcommand]
#       [extra_args …]
#       [agent.args …]
#       [model_flag <model>]              # only if model != "default"
#       { prompt_flag <prompt>            # if prompt_positional = false
#       | <prompt>                        # if prompt_positional = true
#       }
#
# === Examples ===

# 1) Simplest: binary == template name, --model + -p prompt, default version probe.
[claude]
prompt_flag = "-p"
model_flag = "--model"
# detect_binary defaults to "claude" (same as template key).
# version_flag defaults to "--version"; file_input_mode defaults to "arg";
# verified defaults to true.

# 2) Subcommand + positional prompt (no -p flag at all).
[opencode]
subcommand = "run"
prompt_positional = true
model_flag = "--model"
# verified omitted — defaults to true.

# 3) Different binary on PATH from template name; bake in package selector.
[gemini-npx]
detect_binary = "npx"           # `which npx` powers detection
prompt_flag = "-p"
model_flag = "--model"
version_flag = ""               # detect reports version as null
extra_args = ["@google/gemini-cli@latest", "--skip-trust"]
# Combining extra_args with an agent.args of ["--debug"] yields:
#   npx @google/gemini-cli@latest --skip-trust --debug --model X -p "..."

# 4) Real unverified CLI. Listed by `detect` but skipped at dispatch
#    until the user verifies its non-interactive contract works.
[some-new-cli]
prompt_flag = "-p"
model_flag = "--model"
verified = false                # dispatch will warn-and-skip
```

The annotated file is intended both as runtime data and as user documentation. The README/dispatch-guide will simply point to it.

### 5.1 Annotated `dispatch-agent.toml` (user config) reference

Embedded in `dispatch-guide.md` (and emitted commented at the top of any new config produced by `init`):

```toml
# =====================================================================
# dispatch-agent.toml — your personal dispatch configuration.
# Tier traversal is in TOML order; agents within a tier round-robin.
# =====================================================================

version = 1                       # schema version (currently 1)

[[tiers]]
id = "primary"                    # tier label; arbitrary string

  [[tiers.agents]]
  id = "claude-default"           # unique across all agents; [a-zA-Z0-9_-] only
  cli = "claude"                  # MUST match a top-level key in cli-templates.toml
  # template = "claude"           # OPTIONAL override of which template to use;
                                  # falls back to `cli` when omitted (used when two
                                  # agents share a binary but differ in args/model).
  model = "default"               # "default" → omit the model_flag entirely
  args = ["--dangerously-skip-permissions"]
                                  # appended AFTER template.extra_args; see template
                                  # docs for the full command shape.

    [[tiers.agents.env]]          # env injection (zero or more entries per agent)
    type = "file"                 # read file contents → set env var <name>
    name = "GITHUB_TOKEN"
    path = "~/.config/gh/token"

    [[tiers.agents.env]]
    type = "env"                  # forward an env var from the parent shell
    name = "OPENAI_API_KEY"
    var = "OPENAI_API_KEY"        # name in the PARENT shell's environment

    [[tiers.agents.env]]
    type = "source"               # source a shell env file inside a bash wrapper
    path = "~/.zshrc.d/zclaude.env"
                                  # NOTE: name/var fields are not used for `source`;
                                  # the file is loaded via `set -a; source X; set +a`
                                  # so every assignment becomes an exported var.

[[tiers]]
id = "fallback"

  [[tiers.agents]]
  id = "gemini-npx-default"
  cli = "gemini-npx"              # uses the gemini-npx template (binary = npx)
  model = "default"
  args = []
```

---

## 6. Dispatch internals (port notes)

### Subprocess I/O loop
Python uses `select` on stdout+stderr fds. Rust port (unix only):
- Spawn child with stdout+stderr piped, in a new process group (`pre_exec` setsid).
- Two reader threads: one drains stdout → host stdout (calling `flush()` after each write to avoid buffering visible hangs), one drains stderr into a `Vec<u8>` buffer behind a `Mutex`.
- Main thread blocks on `child.wait_timeout(duration)` (see D6 below).
- Verbose ticker thread: parks 10 s at a time, prints `[waiting: <id> — Ns elapsed]` to stderr; checks an `AtomicBool` shutdown flag each wakeup.
- **Thread join order after wait returns:** (1) set ticker shutdown flag and unpark, (2) `child.kill()` if still alive (cleanup safety), (3) join both reader threads (their pipes will EOF as the child is reaped), (4) join ticker thread, (5) consume stderr buffer for the failure summary. Stderr `Vec<u8>` → `String` via `String::from_utf8_lossy()` (matches Python's `errors='replace'`). Stdout reader writes raw `&[u8]` straight to `io::stdout().write_all()` — no UTF-8 validation. All pipe handles and the `Child` are dropped at the end of each attempt; no FDs survive across fallback iterations.
- **`wait_timeout` return handling:** `Ok(Some(status))` → child exited; `Ok(None)` → timeout (trigger kill path); `Err` → wait failure, treat as `Ok(Some(status))` (proceed to reap and inspect — should never occur in practice).
- **Signal handler scope:** signal-hook `Signals` iterator is registered ONLY inside `dispatch/mod.rs` entry, dropped on dispatch exit. Never installed in `main.rs`, `config_cmd.rs`, or any other subcommand path — otherwise `config edit`'s spawned editor would have its SIGINT intercepted.

### 6.1 Child lifecycle state machine

All concurrent access (signal watcher, timeout, main thread) coordinates through a single mutex:

```rust
struct ChildState {
    pid: Option<i32>,           // process-group leader pid; None = no live child
    killed_by_timeout: bool,    // set by timeout branch before SIGKILL
    interrupted: bool,          // set by signal watcher when SIGINT/SIGTERM seen
}
let state = Arc::new(Mutex::new(ChildState::default()));
```

Transitions:
- **Spawn:** main thread sets `pid = Some(pgid)`.
- **Timeout fires:** main thread (after `wait_timeout` returns `None`) locks state, sets `killed_by_timeout = true`, calls `killpg(pgid, SIGKILL)` while still holding the lock, then loops back to `wait_timeout` for reaper to collect the corpse.
- **Signal arrives:** signal-hook watcher thread locks state, if `pid.is_some()` calls `killpg(pid, signum)` (ignoring `ESRCH`), sets `interrupted = true`. Releases lock. Does NOT exit the process — the main thread observes `interrupted` after the wait returns.
- **wait returns:** main thread immediately locks state and sets `pid = None` (closes the use-after-reap window). Then inspects `killed_by_timeout` / `interrupted` to label the failure.
- **Between agents:** the gap between one wait returning and the next spawn has `pid = None`; the watcher's `killpg` is skipped. If `interrupted` is set, the dispatch loop breaks before spawning the next agent and exits 1.

Failure-label precedence: `interrupted` > `killed_by_timeout` > raw exit code.

### rr-state locking
Match Python's `fcntl.flock` cadence (NOT held during child execution):
1. Before the dispatch loop: brief exclusive lock → read JSON → unlock. Use this snapshot to pick the round-robin starting agent.
2. On a successful child exit: brief exclusive lock → re-read JSON (pick up concurrent writers) → mutate the tier's pointer → atomic-rename write → unlock.
On read: distinguish `io::ErrorKind::NotFound` (silent `{}`, first-run case) from `ErrorKind::PermissionDenied` (emit `warning: cannot read rr-state at <path>: permission denied` and treat as `{}` — most likely cause is a previous run as root chowning the file). Parse errors also warn. Other I/O errors warn similarly. Parent dirs are created on first write via `fsutil::write_atomic`. **Operational note:** running `dispatch-agent dispatch` under `sudo` is unsupported — it will create a root-owned state file that breaks subsequent unprivileged runs (mitigated by the warning above so the user can `chown` it back).

**Coexistence guarantee:** `fs2::FileExt::lock_exclusive` wraps `flock(LOCK_EX)` on Linux and macOS — same syscall as Python's `fcntl.flock`. A Python and a Rust dispatcher running side-by-side during migration contend correctly on the same advisory lock.

### Env injection
`build_env(agent, depth)`:
1. Inherit current env.
2. For each non-`source` env entry, resolve and overwrite.
3. Bump `DISPATCH_AGENT_DEPTH`.
Source files are NOT injected as env vars in-process; they go into the bash wrapper, identical to Python.

---

## 7. Locating `data/cli-templates.toml` from a binary

Python uses `__file__`. For a compiled binary we resolve in this order:
1. `DISPATCH_AGENT_TEMPLATES` env var (explicit override; used by all tests).
2. `<exe_dir>/../data/cli-templates.toml` (when shipped under skill dir).
3. `<exe_dir>/data/cli-templates.toml` (when binary placed alongside data).
4. (No `cargo run` development fallback in v1 — see D5. `cargo run` requires the user to set `DISPATCH_AGENT_TEMPLATES` manually.)

Distribution-time concern is deferred; in this rewrite we ship source only and tests use option (1).

---

## 7.1 Security notes

- **Shell injection in source paths:** all `EnvEntry::Source.path` values are shell-quoted before interpolation into the `bash -c` wrapper (see §6). Without this, paths containing `'` are RCE.
- **Symlink TOCTOU on temp files:** `write_atomic` uses `O_NOFOLLOW` so an attacker cannot redirect writes via a pre-placed symlink.
- **`$EDITOR` handling:** values are split on whitespace and passed to `Command::new(first).args(rest).arg(file_path)` — no shell expansion, no glob, no tilde processing. `$EDITOR` is inherently trusted (user-set in their own shell profile). No file lock is held during editing; a concurrent `dispatch` may see a partially-written config (intentional — advisory locks on user-editable files create more problems than they solve).
- **TOML DoS:** the `toml` crate has no documented depth limit. Trusting `--config PATH` with attacker-controlled files is out of scope. cli-templates.toml is shipped data; user configs are user-controlled.
- **Secret leakage in error messages:** env *values* (file contents, env var values) MUST NEVER appear in error messages, log lines, or `--show-config`/`--list` output. `anyhow` context strings include only paths and var names. Reviewer checklist item.
- **Child stderr is forwarded as-is** to the dispatcher's stderr; if a child CLI prints a token in its own error output, that's parity with Python and unavoidable.

## 8. Error handling & exit codes

- `anyhow::Result<()>` returned from each subcommand; `main` prints `Error: {chain}` to stderr and exits 1 on `Err`.
- Specific exit codes preserved:
  - dispatch success → 0
  - all agents failed → 1
  - usage / validation errors → 1 (was `sys.exit(1)` in Python)
  - recursion limit → 1
- Warnings go to stderr without affecting exit code.

---

## 9. Testing

### Test scaffolding

**Fake-agent harness.** Subprocess-based integration tests use a `tests/bin/fake_agent.rs` `[[bin]]` target (registered via `Cargo.toml [[test]]` or built as a test artifact). It reads `FAKE_AGENT_MODE` and branches:
- `exit-0`: exit 0.
- `exit-N`: exit N (parsed from env).
- `sleep`: touch `$READY_FILE` then sleep 60 s.
- `print-env`: print all `TEST_*` env vars to stdout, exit 0.

Tests set `DISPATCH_AGENT_TEMPLATES` to a fixture TOML registering `fake-agent` as a template with `detect_binary = "<path-to-fake_agent>"` and `prompt_flag = "-p"`. Removes any dependency on installed real CLIs in CI.

**Snapshot framework.** Use `insta` for any multi-line formatted-output assertion (`format_list`, `format_show_config`, `format_list_detect`, `--dry-run` stdout, error messages). Snapshots committed under `tests/snapshots/`. Reviewers run `cargo insta review` on changes.

**Property tests.** Use `proptest` on `Template::build_command` to assert: (1) when `prompt_flag.is_empty() && !prompt_positional` → `None`; (2) the prompt string appears at most once in the result; (3) `extra_args` always precede `agent.args`; (4) `subcommand` (if non-empty) appears immediately after the binary.

**Golden parity files.** `tests/fixtures/inputs/` holds canonical inputs; `tests/fixtures/golden/` holds Python-generated expected outputs (regenerated by `scripts/regen_golden.sh`). CI runs the regen script and a `git diff --exit-code` to ensure parity is not silently broken. Covers: `detect` JSON, `init` TOML for canonical input, `dispatch --dry-run` stdout for single-agent config, `--list` formatted output.

### Unit tests (in-source `#[cfg(test)]`)
- `validate_agent_id`, `validate_unique_ids`.
- `Template::build_command` matrix: positional vs flag, with/without subcommand, default vs explicit model, model with no model_flag (warn emitted), extra_args ordering.
- `wrap_with_sources` argv shape — including a path containing `'`, `$`, backtick, and a space; assert the resulting `bash -c` string is syntactically valid (re-parse via `shlex` or by piping to `bash -n`). Also test the multiple-source-files case (single bash layer, not nested).
- `shell_quote` direct unit tests for empty string, plain ascii, single-quote, dollar, backtick, space, embedded newline.
- `find_config` with tempdir + fake git root.
- rr-state load/save round trip.
- `expand_tilde` on `~`, `~/foo`, `/abs`, relative path, empty path.
- BOM stripping: pass `\u{FEFF}version = 1` to the shared TOML load helper, assert success. Same helper underpins both `load_config` and `load_templates`, so one test covers both.

### Integration tests (`rust/tests/*.rs`)
- `init.rs`:
  - Canonical JSON via stdin → generated TOML re-parses to expected `Config`; file mode 0600 on unix.
  - Malformed JSON → exit 1, stderr contains `invalid JSON`.
  - Missing `tier_order` → exit 1, stderr names the field.
  - Invalid `save_location` ("foo") → exit 1.
  - Agent id failing regex → exit 1, stderr contains the failing id.
  - Duplicate agent ids → exit 1 (init fails fast; dispatch only warns).
  - `env` entry of type `file` with no `path` → exit 1, stderr names the agent.
  - Successful init also prints stderr hint containing `config edit`.
- `detect.rs`: with `DISPATCH_AGENT_TEMPLATES` pointing at a fixture and `PATH` rigged with `tempdir` containing fake binaries, assert JSON output.
- `dispatch.rs`:
  - `--dry-run` happy path prints expected command.
  - `--dry-run` with NO prompt: stdout contains literal `<prompt>` (lowercase, matching Python), exit 0.
  - `--list` no-config falls back to detect.
  - `--list` with config marks agents (`[✓]`/`[✗]`).
  - `--show-config` prints both layers correctly.
  - `--show-config` no config → exit 1, stderr suggests `init`.
  - `--agent BAD` → exit 1, stderr contains "not found".
  - `--tier BAD` → exit 1, stderr contains "not found".
  - Child exit propagation: fake agent exits 0 (success branch) → dispatcher exits 0; fake agent exits 42 in single-tier → dispatcher exits 1 after fallback exhausted.
  - Recursion guard: invoke with `DISPATCH_AGENT_DEPTH=5`, expect immediate exit 1 with depth-limit message.
  - timeout: spawn `sleep 30`, set `--timeout 1`, expect failure marked `timeout` and rc != 0.
  - rr-state pointer advances on success, not on failure (use `fake-agent` with `exit-0` / `exit-1`).
  - rr-state file missing on first run: dispatch succeeds and creates the file.
  - Signal forwarding: launch dispatcher with a `sleep` fake-agent that touches a `READY_FILE`; main test waits for readiness, sends SIGINT to dispatcher pid, asserts dispatcher exits ≤ 2 s with rc 1 and stderr mentions `interrupted`.
  - Env injection: register `print-env` fake agent, config has all three env entry types (`file`, `env`, `source`), assert child stdout contains the expected `TEST_*` variables (verifies `build_env` and `wrap_with_sources` end-to-end).
  - `verified = false` agent skipped at dispatch with the documented stderr warning.
  - `--config` edge cases: relative path, `~`-prefixed path, path containing spaces, `--config /nonexistent` (clear error message).
  - `cli-templates.toml` validation test: `load_templates()` against the actual repo file, assert all original entries deserialise with equal field values, assert no entry has an unrecognised `file_input_mode`.
- `config_cmd.rs`:
  - `config path` outputs resolved path; on no-config, exit non-zero AND stderr lists default search locations.
  - `config show` matches `dispatch --show-config` byte-for-byte (snapshot).
  - `config edit` invokes the editor with the right path (stub editor writes a marker, assert file content + return 0).
  - `EDITOR="code -w"` correctly splits into `Command::new("code")` with args `["-w", path]` (unit test on the splitter helper).
  - `EDITOR="  "` (whitespace-only) falls through to `$VISUAL` / platform default.
  - Post-edit validation: stub editor writes invalid TOML; assert stderr contains `warning: config has syntax errors`; exit 0.
  - Mtime-unchanged hint: stub editor returns immediately without modifying the file; assert stderr hint about GUI editors appears.

### Parity tests
Golden files under `tests/fixtures/golden/` (generated by running the Python scripts on `tests/fixtures/inputs/` via `scripts/regen_golden.sh`). CI runs the regen script then `git diff --exit-code tests/fixtures/golden/` before `cargo test`. Covers: `detect` JSON, `init` TOML for canonical input, `dispatch --dry-run` stdout for single-agent config, `--list` formatted output. Concurrent rr-state correctness is **out of scope for this PR** (deferred to a dedicated lock-stress test PR; rationale: CI flake risk; `fs2` is already crate-tested).

---

## 10. Decisions log (all resolved)

- **D1 Resolved: yes.** `init` round-trip-parses the emitted TOML *and* deserialises to the `Config` struct from `types.rs` before atomic rename. Combined with D11 (which mandates `toml::to_string_pretty` from typed structs), this gives strictly stronger guarantees than Python's `tomllib.loads` validation.
- **D2 Resolved: `notepad` on Windows** (no surprise). Moot in practice because dispatch is unix-only in v1; only `config edit` is exercised on Windows.
- **D3 Resolved: refuse + suggest `init`** when no config exists and `--config` is absent. With `--config PATH`, write a minimal `version = 1` stub. Encoded in §4 and tested in §9.
- **D4 Resolved:** `--config` accepted at both root and subcommand level. **Subcommand-level wins** on conflict (matches argparse/clap convention; root is fallback). Python only accepts it at the dispatch subcommand level, so subcommand-wins preserves parity.
- **D5 Resolved: deferred to distribution PR.** Tests use the `DISPATCH_AGENT_TEMPLATES` env override (§7 step 1); the shipped-binary discovery chain (steps 2–3) is implementation-trivial. §7 step 4 (the build.rs / `cargo run` development fallback) is **dropped from v1** — `cargo test` always sets the env override; running `cargo run` interactively is not a supported developer workflow until then.
- **D6 Resolved: use `wait-timeout` crate.** Hand-rolled `Condvar` + wait thread *adds* a thread and synchronization that wouldn't otherwise exist. With `wait-timeout`, the main thread directly calls `child.wait_timeout(duration)`; only the signal watcher remains as an auxiliary thread. ~200 LOC dep, no transitive deps. Marginal cost is negligible.
- **D7 Resolved:** use `signal-hook` on unix. Watcher thread runs `signal_hook::iterator::Signals` for SIGINT/SIGTERM, coordinates with the main thread through the `ChildState` mutex described in §6.1 (no raw signal handlers). Windows path is a no-op (dispatch is unix-only in v1).
- **D8** Output of `detect` JSON: stable key order. **Resolved: use `indexmap::IndexMap` end-to-end** so template insertion order (and therefore `KNOWN_CLIS` order) is preserved automatically without hand-serialization.
- **D9 Resolved: keep `DEFAULT_MODELS` as a `const` in `init.rs`** (parity with Python `init.py:DEFAULT_MODELS`). It maps `cli` → default model string and is consulted only when populating the generated TOML during `init`; the user can override per agent. Migration to a `cli-templates.toml`-side `default_model` field is a follow-up refactor (would require a schema bump).
- **D10 Resolved:** `EnvEntry` is a `#[serde(tag = "type", rename_all = "lowercase")]` tagged enum, so missing/unknown variants are caught at deserialize time. **However**, raw serde messages lack agent-id context. So `load_config` runs a post-deserialize validation pass that walks tiers→agents→env, wrapping any deserialize/validation failure with the agent id to produce Python-equivalent messages: `Error: agent 'X' has invalid env type 'Y'` and `Error: agent 'X' env entry of type 'file' missing 'path'`. Implemented either by parsing into `toml::Value` first then mapping into `EnvEntry`, or by catching the serde error path and re-emitting with context.
- **D11** `init.rs` TOML emission. **Resolved: use `toml::to_string_pretty`** with `Serialize` derives on the types in `types.rs`. Existing tests assert parse-equivalence, not formatting parity, so we lose nothing and avoid hand-rolling `escape_toml_string`. Round-trip validation (re-parse the emitted TOML to `Config`) still runs before atomic rename.

---

## 11. Rollout

### Sequence (4 PRs)

1. **PR 1 (this work):** Rust crate + tests + annotated `cli-templates.toml` + this plan + `Unreleased` CHANGELOG entry. **Binary is dark in production** — Python remains the SKILL.md entry point.
2. **PR 2:** Pre-built binaries committed under `skills/dispatch-agent/bin/<target-triple>/dispatch-agent` for the CI matrix (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`; musl deferred). SKILL.md routing gains an opt-in branch:
   ```bash
   if [ -n "$DISPATCH_AGENT_USE_RUST" ] && [ -x "$BIN" ]; then exec "$BIN" "$@"
   else exec python3 "$SCRIPTS/dispatch.py" "$@"; fi
   ```
   Per-user opt-in. **Gate:** `scripts/parity_check.sh` (see §11.1) green across the CI matrix on every fixture.
3. **PR 3:** After ≥ 2 weeks of clean burn-in via `DISPATCH_AGENT_USE_RUST=1`, flip default-on; remove Python scripts and tests; SKILL.md calls the binary unconditionally.
4. **PR 4 (optional):** GitHub Actions release workflow that produces signed binaries on tag push.

### 11.1 Parity verification harness

`scripts/parity_check.sh` runs the Python and Rust binaries against each fixture in `tests/fixtures/inputs/` and diffs stdout/stderr/exit code. It's the gate for PR 2's CI matrix and the daily check during the PR-3 burn-in window. Reuses the same fixtures as the golden-file tests (§9). Wraps `regen_golden.sh` with a `--diff` mode.

### 11.2 PR strategy

The Rust crate is self-contained (no shared codebase coupling), so a single PR is acceptable. If reviewer load is a concern, the §2.1 DAG enables a clean three-way split:

- **(a)** `types` + `fsutil` + `config` + `templates` (pure, no subprocesses)
- **(b)** `detect` + `init` + `config_cmd`
- **(c)** `dispatch/` (subprocess + signal complexity, highest reviewer surface)

Each layer is independently testable and reviewable.

### 11.3 Doc updates in PR 1

- `CHANGELOG.md`: Unreleased entry summarising the Rust port.
- `references/dispatch-guide.md`: append the §5.1 annotated config example so the doc-side reference matches the in-file annotations. Add a "Future: binary" stub line linking to this plan.
- `references/init-guide.md`: no change in PR 1 (init JSON contract unchanged); revisit in PR 2.
- `skills/dispatch-agent/CLAUDE.md` (if added): not in scope.

---

## 12. Acceptance criteria for this PR

- `cargo build --release` succeeds on `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`.
- `cargo test` green (unit + integration + property + snapshot).
- `cargo clippy -- -D warnings` clean.
- `cargo fmt --check` clean.
- All `insta` snapshots committed under `tests/snapshots/`.
- `tests/fixtures/golden/` populated and `scripts/regen_golden.sh` produces no diff.
- Annotated `cli-templates.toml` parses identically to the old one (tested via `templates_validation` integration test naming the actual repo file via `DISPATCH_AGENT_TEMPLATES`).
- Python scripts and existing Python tests still pass (no regression).
- `CHANGELOG.md` has an `Unreleased` entry summarising the Rust port.
- `references/dispatch-guide.md` updated with the §5.1 annotated config example.
- Plan committed under `docs/plans/`.
