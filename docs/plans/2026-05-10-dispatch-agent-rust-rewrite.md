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
        mod.rs          # subcommand entry, tier traversal, rr-state mutation
        command.rs      # build_command + wrap_with_sources (argv construction)
        process.rs      # spawn, process group, signal forwarding, timeout, verbose tick
        display.rs      # format_list, format_show_config (shared with config_cmd)
      config_cmd.rs     # config subcommand (edit / show / path); editor inlined
      env.rs            # resolve_env_var, get_source_files, build_env
      rr_state.rs       # round-robin state load/store with file lock
      fsutil.rs         # write_atomic (mkdir-p, temp, chmod 0600, rename)
    tests/
      detect.rs
      init.rs
      dispatch.rs
      config_cmd.rs
  scripts/              # untouched in this rewrite
  data/cli-templates.toml  # rewritten with comments
```

Binary name: `dispatch-agent` (set via `[[bin]]` in Cargo.toml). Pre-built artifact will be committed under `skills/dispatch-agent/bin/<target-triple>/dispatch-agent` in a follow-up PR; this PR delivers source + tests only.

### 2.1 Module dependency graph (DAG, leaves at bottom)

```
main.rs        → cli, detect, init, dispatch::mod, config_cmd
detect.rs      → templates, types
init.rs        → types, fsutil, config (find_git_root)
dispatch/mod   → types, templates, env, rr_state, fsutil, dispatch::{command,process,display}
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
pub struct Template { pub detect_binary: Option<String>, pub subcommand: Option<String>,
                      pub prompt_flag: Option<String>, pub prompt_positional: bool,
                      pub model_flag: Option<String>, pub extra_args: Vec<String>,
                      pub version_flag: Option<String>, pub file_input_mode: Option<String>,
                      pub verified: bool /* default true */ }
pub struct DetectInfo { pub path: Option<String>, pub version: Option<String>,
                        pub callable: bool, pub verified: bool }

// fsutil.rs
pub fn write_atomic(path: &Path, content: &[u8]) -> anyhow::Result<()>;

// templates.rs — owns the §7 resolution chain end-to-end.
pub fn load_templates() -> anyhow::Result<IndexMap<String, Template>>;

// config.rs
pub fn find_git_root() -> PathBuf;             // git rev-parse, fallback to cwd
pub fn find_config(arg: Option<&Path>) -> Option<PathBuf>;
pub fn load_config(path: &Path) -> anyhow::Result<Config>;

// dispatch/command.rs
impl Template { pub fn build_command(&self, agent: &Agent, prompt: &str) -> Option<Vec<String>>; }
pub fn wrap_with_sources(cmd: Vec<String>, sources: &[String]) -> Vec<String>;

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

No tokio, no tracing. All blocking I/O in main thread; subprocess I/O via `std::process::Command` + threads/`select` equivalent.

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
- Prints destination path on success.
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
- `--dry-run` makes `-p`/`-f` optional. With no prompt, the printed command substitutes the literal placeholder `<PROMPT>`.
- `--agent ID` not found in config → exit 1 + `error: agent 'ID' not found in config`. Bypasses tier traversal entirely (single attempt).
- `--tier ID` not found → exit 1 + `error: tier 'ID' not found in config`. Otherwise: traversal starts at that tier and continues to subsequent tiers on failure.
- Recursion guard: `DISPATCH_AGENT_DEPTH` parsed as int (unset/unparseable → 0). If current depth ≥ 5 → exit 1 + `error: recursion depth limit (5) reached`. Before spawning child, set the var to `current + 1` in the child's env only.
- Per-agent process group (`setsid` via `pre_exec` on unix) so timeout/signal kills children.
- SIGINT/SIGTERM forwarded to child group via `signal-hook` watcher thread, then dispatcher exits 1.
- Verbose ticker every 10 s while child runs. `--verbose` also prints `[attempting <agent.id>]` before each attempt and `[<agent.id>] (tier: <tier.id>)` on success (matches Python). It does NOT change `--list`/`--show-config`/`--dry-run` output.
- `failure code == -SIGKILL` reported as `timeout` in the failure summary.
- **Exit code propagation:** on the success branch, dispatcher exits with the child's exit code (0 if child returned 0; otherwise the child's code is treated as failure and triggers fallback). All-agents-failed → exit 1. (Python's behaviour: a non-zero child rc triggers fallback, so "success" here means child rc == 0; dispatcher exit 0 on success.)
- rr-state file `~/.cache/dispatch-agent/rr-state.json`. If missing, treat as `{}`; create parent dirs on first write. Advisory exclusive lock held during read+mutate+write; pointer updated **only on success**.
- Source env files wrapped in `bash -c 'set -a; source X; set +a; exec "$@"' -- <cmd>`. Hardcoded bash is intentional (parity with Python); do NOT shell-detect.
- Mutex groups implemented via `clap::ArgGroup { multiple: false, required: false }` for `-p`/`-f` and `--tier`/`--agent`.
- `verified = false` agents are skipped at dispatch with a stderr warning: `warning: agent 'ID' uses unverified template 'NAME', skipping`.
- Template lookup per agent: `templates.get(agent.template.as_deref().unwrap_or(&agent.cli))`. Missing template → stderr warning, skip agent (does not abort tier).

#### `config`
```
dispatch-agent [--config PATH] config [edit | show | path]
```
- `config` (no arg) → `edit`.
- `edit` — resolve config path (create empty file at user location if none exists; ask for confirmation? — **decision in §10**). Open with `$EDITOR`, then `$VISUAL`, then platform default (`vi` unix, `notepad` windows).
- `show` — same output as `dispatch --show-config`, but requires only `config` subcommand (no `dispatch` semantics).
- `path` — print resolved path (or non-zero exit + message if none found and `--config` not given). When `--config PATH` is given, print PATH literally even if the file doesn't exist (the user explicitly asked).
- `--config PATH` overrides resolution. For `edit` with `--config PATH`, the path is created if missing (user opted in by naming it). Without `--config`, if no config exists, exit 1 and suggest `dispatch-agent init`. (No `--create` flag.)

---

## 5. Annotated `cli-templates.toml`

Replace the current minimal file with a fully commented reference. Outline:

```toml
# cli-templates.toml — describes how dispatch-agent invokes each agent CLI.
#
# Each top-level table is a TEMPLATE keyed by the name used in your
# config's `cli = "..."` (or `template = "..."`) field.
#
# === Field reference ===
#
# detect_binary      (string, default = template name)
#   The executable name searched on PATH for `detect` and availability
#   checks. Use this when the actual binary differs from the logical
#   template name (e.g. gemini-npx → npx).
#
# subcommand         (string, default = "")
#   Inserted right after the binary, before any args/flags.
#   Example: opencode uses `opencode run <prompt>`.
#
# prompt_flag        (string, default = "")
#   Flag used to pass the prompt non-interactively (e.g. "-p", "-q").
#   Empty AND prompt_positional=false → agent is skipped at dispatch.
#
# prompt_positional  (bool, default = false)
#   If true, the prompt is appended as a positional arg right after
#   the subcommand. Used together with subcommand for run-style CLIs.
#
# model_flag         (string, default = "")
#   Flag for `--model`. If empty and the agent's model != "default",
#   the model is silently dropped (warning emitted).
#
# extra_args         (string array, default = [])
#   Args ALWAYS prepended to the agent's own `args` list. Use this to
#   bake in package selectors or CLI feature flags (see gemini-npx).
#
# version_flag       (string, default = "--version")
#   Flag passed during `detect` to obtain a version string. Empty
#   disables the version probe (e.g. when the binary is launched via npx
#   and we don't want to download the package just to read a version).
#
# file_input_mode    ("arg", default = "arg")
#   Reserved for future stdin support; currently only "arg" is honored
#   (file contents passed inline via prompt_flag).
#
# verified           (bool, default = true)
#   Marks whether non-interactive mode for this CLI has been verified.
#   `false` → agent is shown but skipped at dispatch. Use during triage
#   for new CLIs whose -p/--print contract is not yet trusted.
#
# === Examples ===

# 1) Simplest: binary == template name, --model + -p prompt, default version probe.
[claude]
prompt_flag = "-p"
model_flag = "--model"
# version_flag defaults to "--version"; file_input_mode defaults to "arg".

# 2) Subcommand + positional prompt (no -p flag at all).
[opencode]
subcommand = "run"
prompt_positional = true
model_flag = "--model"
verified = true

# 3) Different binary on PATH from template name; bake in package selector.
[gemini-npx]
detect_binary = "npx"           # `which npx` powers detection
prompt_flag = "-p"
model_flag = "--model"
version_flag = ""               # don't `npx ... --version` during detect
extra_args = ["@google/gemini-cli@latest", "--skip-trust"]

# 4) Hypothetical unverified CLI — kept in templates so detect can list it,
#    but blocked from dispatch until the user sets verified = true.
# [some-new-cli]
# prompt_flag = "-p"
# model_flag = "--model"
# verified = false
```

The annotated file is intended both as runtime data and as user documentation. The README/dispatch-guide will simply point to it.

---

## 6. Dispatch internals (port notes)

### Subprocess I/O loop
Python uses `select` on stdout+stderr fds. Rust port:
- Spawn child with stdout+stderr piped, in a new process group (`pre_exec` setsid on unix).
- Two reader threads: one drains stdout → host stdout, one buffers stderr.
- Main thread waits with timeout (`Condvar` + `Mutex<Option<ExitStatus>>` updated by a `wait()` thread, OR simply `wait_timeout` via the `wait-timeout` crate — **see §10 D6**).
- On timeout: kill process group with SIGKILL.
- On SIGINT/SIGTERM (parent): use `signal-hook` or raw `libc::signal` — **decision §10 D7** — forward to child group then exit 1.
- Verbose ticker thread: prints `[waiting: <id> — Ns elapsed]` every 10s.

### rr-state locking
Match Python's `fcntl.flock` semantics with `fs2::FileExt::lock_exclusive` on a file at `~/.cache/dispatch-agent/rr-state.json`. Read → mutate → atomic-rename write while holding the lock; release on close.

### Env injection
`build_env(agent, depth)`:
1. Inherit current env.
2. For each non-`source` env entry, resolve and overwrite.
3. Bump `DISPATCH_AGENT_DEPTH`.
Source files are NOT injected as env vars in-process; they go into the bash wrapper, identical to Python.

---

## 7. Locating `data/cli-templates.toml` from a binary

Python uses `__file__`. For a compiled binary we resolve in this order:
1. `DISPATCH_AGENT_TEMPLATES` env var (explicit override; used by tests).
2. `<exe_dir>/../data/cli-templates.toml` (when shipped under skill dir).
3. `<exe_dir>/data/cli-templates.toml` (when binary placed alongside data).
4. Fallback: `cargo run` development — relative to `CARGO_MANIFEST_DIR/../data/cli-templates.toml` if `cfg!(debug_assertions)` and the env var `CARGO_MANIFEST_DIR` is set at runtime (it isn't — so use a `build.rs`-baked constant only in debug builds).

Distribution-time concern is deferred; in this rewrite we ship source only and tests use option (1).

---

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

### Unit tests (in-source `#[cfg(test)]`)
- `escape_toml_string`, `validate_agent_id`, `validate_unique_ids`.
- `build_command` matrix: positional vs flag, with/without subcommand, default vs explicit model, model with no model_flag (warn), extra_args ordering.
- `wrap_with_sources` argv shape.
- `find_config` with tempdir + fake git root.
- rr-state load/save round trip.

### Integration tests (`rust/tests/*.rs`)
- `init.rs`: feed canonical JSON via stdin, assert generated TOML re-parses to expected `Config`, file mode is 0600 on unix.
- `detect.rs`: with `DISPATCH_AGENT_TEMPLATES` pointing at a fixture and `PATH` rigged with `tempdir` containing fake binaries, assert JSON output.
- `dispatch.rs`:
  - `--dry-run` happy path prints expected command.
  - `--dry-run` with NO prompt: stdout contains literal `<PROMPT>`, exit 0.
  - `--list` no-config falls back to detect.
  - `--list` with config marks agents (`[✓]`/`[✗]`).
  - `--show-config` prints both layers correctly.
  - `--show-config` no config → exit 1, stderr suggests `init`.
  - `--agent BAD` → exit 1, stderr contains "not found".
  - `--tier BAD` → exit 1, stderr contains "not found".
  - Child exit propagation: fake agent exits 0 (success branch) → dispatcher exits 0; fake agent exits 42 in single-tier → dispatcher exits 1 after fallback exhausted.
  - Recursion guard: invoke with `DISPATCH_AGENT_DEPTH=5`, expect immediate exit 1 with depth-limit message.
  - timeout: spawn `sleep 30`, set `--timeout 1`, expect failure marked `timeout` and rc != 0.
  - rr-state pointer advances on success, not on failure (use `true` and `false` shell builtins as fake agents).
  - rr-state file missing on first run: dispatch succeeds and creates the file.
- `config_cmd.rs`:
  - `config path` outputs resolved path; non-zero when none.
  - `config show` matches `dispatch --show-config`.
  - `config edit` invokes `$EDITOR` with the right path (use a stub editor that writes a marker, assert file content + return 0).

### Parity tests
For each Python integration test under `tests/`, port the assertions into Rust integration tests so behaviour is line-for-line equivalent where feasible. Diff the JSON/TOML byte streams against captured Python output for representative inputs.

---

## 10. Open Decisions (resolved during review rounds)

- **D1** Should `init` round-trip-parse the TOML and also deserialise to the same `Config` struct used by `dispatch` (stronger guarantee than Python's `tomllib.loads`)? — **Tentative: yes.**
- **D2** Default editor on Windows: `notepad` vs first-found of `code -w`/`notepad++`? — **Tentative: notepad (no surprise).**
- **D3** `config edit` when no config exists: create empty stub at user location, or refuse and tell user to run `init`? — **Tentative: refuse + suggest `init`** (avoids partial configs that would crash `dispatch`).
- **D4 Resolved:** `--config` accepted at both root and subcommand level. **Subcommand-level wins** on conflict (matches argparse/clap convention; root is fallback). Python only accepts it at the dispatch subcommand level, so subcommand-wins preserves parity.
- **D5** Detection of CLI templates path in shipped binary (see §7) — defer concrete decision to distribution PR; tests use env override.
- **D6** Use `wait-timeout` crate or hand-rolled `Condvar`? — **Tentative: hand-rolled to keep dep set small.**
- **D7** Signal forwarding architecture. **Resolved: use `signal-hook` on unix.** Raw libc `signal()` handlers cannot safely call `killpg` (async-signal-unsafe). Architecture: spawn a `signal_hook::iterator::Signals` iterator on a watcher thread that holds an `Arc<Mutex<Option<Pid>>>` of the current child group; on signal, lock and `killpg`, then set an exit-requested flag the main loop checks after the child exits. Windows: ctrl-c handler kills child by handle.
- **D8** Output of `detect` JSON: stable key order. **Resolved: use `indexmap::IndexMap` end-to-end** so template insertion order (and therefore `KNOWN_CLIS` order) is preserved automatically without hand-serialization.
- **D9** `init`'s `DEFAULT_MODELS` — keep as Rust `const`? Or move into `cli-templates.toml` as a `default_model` field? — **Tentative: keep in code for parity; refactor in follow-up.**
- **D10** Behaviour when required field is omitted in env entry of type `file`/`env` — Python KeyErrors. **Resolved: `EnvEntry` is a tagged enum so missing fields are caught at deserialize-time with a clear serde error.**
- **D11** `init.rs` TOML emission. **Resolved: use `toml::to_string_pretty`** with `Serialize` derives on the types in `types.rs`. Existing tests assert parse-equivalence, not formatting parity, so we lose nothing and avoid hand-rolling `escape_toml_string`. Round-trip validation (re-parse the emitted TOML to `Config`) still runs before atomic rename.

---

## 11. Rollout

This PR delivers: Rust crate + tests + annotated cli-templates.toml + this plan.
Follow-up PRs (out of scope here):
1. Build & commit binaries; update SKILL.md and references to call binary.
2. Remove Python scripts and tests once binary verified across CI matrix.
3. Optional: GitHub Actions release workflow for binaries.

---

## 12. Acceptance criteria for this PR

- `cargo build --release` succeeds.
- `cargo test` green (unit + integration).
- `cargo clippy -- -D warnings` clean.
- `cargo fmt --check` clean.
- Annotated `cli-templates.toml` parses identically to the old one (tested).
- Python scripts and existing Python tests still pass (no regression).
- Plan committed under `docs/plans/`.
