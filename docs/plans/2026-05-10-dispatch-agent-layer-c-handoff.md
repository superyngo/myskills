# Session Handoff: dispatch-agent Rust Rewrite — Layer (c)

**Date:** 2026-05-10
**Status:** PR 1 layers (a) and (b) complete. Layer (c) is next.

---

## What's done

Rust crate at `skills/dispatch-agent/rust/`. Run `cargo test --bin dispatch-agent` → **60 tests pass**, clippy clean, fmt clean.

### Layer (a) — complete
`types.rs`, `fsutil.rs`, `config.rs`, `templates.rs`

### Layer (b) — complete
`dispatch/display.rs`, `detect.rs`, `init.rs`, `config_cmd.rs`

All other modules (`env.rs`, `rr_state.rs`, `dispatch/command.rs`, `dispatch/process/`, `dispatch/mod.rs`, `cli.rs`, `main.rs`) are stubs containing `// TODO`.

---

## What remains (layer c + CLI wiring)

Implement in this order (respects dependency DAG):

| # | Module | Key deps |
|---|--------|----------|
| 1 | `src/env.rs` | types only |
| 2 | `src/rr_state.rs` | fsutil, types |
| 3 | `src/dispatch/command.rs` | types |
| 4 | `src/dispatch/process/windows.rs` | stub only |
| 5 | `src/dispatch/process/unix.rs` | libc, signal-hook |
| 6 | `src/dispatch/process/mod.rs` | process/unix, process/windows, types |
| 7 | `src/dispatch/mod.rs` | all dispatch/*, env, rr_state, config, templates, display |
| 8 | `src/cli.rs` + `src/main.rs` | clap, all subcommands |
| 9 | `tests/bin/fake_agent.rs` | integration harness |
| 10 | Integration tests + fixtures | fake_agent, insta snapshots |

Full spec: `docs/plans/2026-05-10-dispatch-agent-rust-rewrite.md`

---

## Workflow

Use `/subagent-driven-development`. All subagent work (implementing, spec review, code quality review) must go through `/dispatch-agent` to third-party agents with 30-minute timeouts.

### Dispatch invocation pattern

**Always write prompts to a file first** (never inline with `-p` — code blocks cause shell interpretation):

```bash
# Step 1: write prompt to temp file
# (use Write tool or heredoc — never inline markdown with -p)

# Step 2: dispatch
cd /Volumes/Home/Users/wen/.claude/skills/dispatch-agent
uv run --python 3.12 python3 scripts/dispatch.py \
  -f /tmp/<task_name>.md \
  --timeout 1800 \
  --verbose 2>&1
```

Bash tool `timeout` parameter: **1860000** (31 min in ms).

Config: `~/.config/dispatch-agent.toml` — primary tier uses `claude` CLI agents.

### Per-task loop

```
TaskUpdate(in_progress)
  → write /tmp/impl_<module>.md
  → dispatch (uv run ... -f /tmp/impl_<module>.md --timeout 1800)
  → write /tmp/spec_review_<module>.md
  → dispatch spec review
  → if ✅: write /tmp/quality_review_<module>.md → dispatch quality review
  → if ✅: fix minor issues (cargo fmt, trivial renames) directly
  → TaskUpdate(completed)
  → next task
```

Spec review must pass before quality review starts. Never skip either.

---

## Spec excerpts for layer (c)

(Full spec in `docs/plans/2026-05-10-dispatch-agent-rust-rewrite.md`)

### env.rs (§2.2, §6 env injection)

```rust
pub fn resolve_env_var(ev: &EnvEntry) -> Option<(String, String)>;
// File variant: read file contents → (name, contents.trim())
// Env variant: std::env::var(var) → (name, value); None if var unset
// Source variant: None (handled separately via get_source_files)

pub fn get_source_files(agent: &Agent) -> Vec<String>;
// Returns expanded paths for all Source entries via fsutil::expand_tilde
// Non-expandable paths: warn to stderr, skip

pub fn build_env(agent: &Agent, current_depth: i64) -> HashMap<String, String>;
// 1. Start with std::env::vars() (inherit all)
// 2. For each non-Source EnvEntry: resolve_env_var → overwrite
// 3. Set DISPATCH_AGENT_DEPTH = (current_depth + 1).to_string()
```

Tests: file read roundtrip, env var lookup, Source → None, build_env inherits + overlays + bumps depth.

### rr_state.rs (§6 rr-state locking)

```rust
pub fn load_rr_state(path: &Path) -> IndexMap<String, String>;
// NotFound → silent {} (first-run)
// PermissionDenied → warn stderr, return {}
// Parse error → warn stderr, return {}
// Lock: fs2::FileExt::lock_exclusive on a sidecar ".lock" file

pub fn store_rr_state(path: &Path, state: &IndexMap<String, String>) -> anyhow::Result<()>;
// Serialize to JSON, write via fsutil::write_atomic
// Lock same sidecar before write
```

Tests: roundtrip, NotFound → {}, concurrent load+store doesn't corrupt.

### dispatch/command.rs (§4 dispatch, §7.1 shell injection)

```rust
pub(crate) fn shell_quote(s: &str) -> String;
// Single-quote: "'" + s.replace("'", "'\\''") + "'"
// Tests: empty, plain ascii, single-quote, dollar, backtick, space, newline

impl Template {
    pub fn build_command(&self, agent: &Agent, prompt: &str) -> Option<Vec<String>>;
    // Returns None if prompt_flag.is_empty() && !prompt_positional
    // Shape: [detect_binary|name, subcommand?, extra_args..., agent.args...,
    //         model_flag model (if model != None), prompt_flag prompt | prompt (positional)]
}

pub fn wrap_with_sources(cmd: Vec<String>, sources: &[String]) -> Vec<String>;
// If sources empty: return cmd unchanged
// Otherwise: ["bash", "-c",
//   "set -a; source <q(A)>; source <q(B)>; ...; set +a; exec \"$@\"",
//   "--", cmd[0], cmd[1..]...]
// Each source path is shell_quote()'d before interpolation
// Single bash layer (not nested) regardless of source count
```

Property tests (proptest): prompt appears exactly once; extra_args precede agent.args; subcommand immediately after binary; None returned when no delivery method.

### dispatch/process/windows.rs

```rust
// Entire file:
pub fn dispatch_unix_only() -> anyhow::Result<std::process::ExitStatus> {
    anyhow::bail!("dispatch is unix-only in v1")
}
```

### dispatch/process/unix.rs (§6 subprocess I/O, §D7 signal-hook)

```rust
use std::os::unix::process::CommandExt;
// pre_exec: unsafe { libc::setsid() };

pub fn setup_process_group(cmd: &mut std::process::Command) {
    unsafe { cmd.pre_exec(|| { libc::setsid(); Ok(()) }); }
}

pub fn killpg(pgid: i32, sig: libc::c_int) {
    unsafe { libc::killpg(pgid, sig); }
}

pub fn start_signal_watcher(
    state: Arc<Mutex<ChildState>>,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()>;
// Uses signal_hook::iterator::Signals for SIGINT + SIGTERM
// On signal: lock state, if pid.is_some() → killpg(pid, signum), set interrupted=true
// Checks shutdown AtomicBool each iteration
```

### dispatch/process/mod.rs (§6.1 ChildState FSM)

```rust
pub struct ChildState {
    pub pid: Option<i32>,
    pub killed_by_timeout: bool,
    pub interrupted: bool,
}

// spawn attempt:
// 1. build Command, setup_process_group (unix), stdout+stderr piped
// 2. spawn → set state.pid = Some(child.id() as i32)
// 3. stdout reader thread: write raw bytes to io::stdout()
// 4. stderr reader thread: collect into Vec<u8> behind Mutex
// 5. verbose ticker thread: every 10s prints "[waiting: <id> — Ns elapsed]", checks AtomicBool
// 6. main thread: child.wait_timeout(duration) or child.wait() if no timeout
//    Ok(None) → timeout → lock state, set killed_by_timeout, killpg SIGKILL
//    Ok(Some(status)) → done
// 7. Join order: set ticker shutdown + unpark → child.kill() cleanup → join readers → join ticker
// 8. lock state → set pid=None → return (exit_status, stderr_bytes, state snapshot)
```

### dispatch/mod.rs (§4 dispatch subcommand, §6 internals)

Top-level entry: `pub fn cmd_dispatch(args: &DispatchArgs, config_path: Option<&Path>) -> anyhow::Result<()>`

Key behaviors:
- Recursion guard: parse `DISPATCH_AGENT_DEPTH` env (trim, parse i64); unset → 0; unparseable → exit 1 with message; ≥ 5 → exit 1
- `--list` no-config → `format_list_detect(run_detect(&templates))`
- `--list` with config → `format_list(config)`
- `--show-config` → `format_show_config(config, path)`, no config → exit 1
- `--dry-run` → print command, `-p`/`-f` optional (substitutes `<prompt>` literal)
- `--agent ID` → single attempt, template missing → exit 1 (hard error)
- `--tier ID` → start at that tier, continue to subsequent on failure
- Per-agent processing order: (1) resolve template, (2) check verified, (3) build_command, (4) wrap_with_sources, (5) build_env, (6) spawn
- rr-state: lock → read snapshot before loop; lock → re-read → mutate → write on success
- Signal watcher: registered here ONLY, dropped on dispatch exit
- Exit codes: success (child rc=0) → 0; all failed → 1; recursion/validation → 1
- `--verbose`: `[attempting <id>]` before each, `[<id>] (tier: <tid>)` on success

### cli.rs + main.rs (§4 CLI surface, §D4)

```rust
// cli.rs
#[derive(Parser)]
#[command(name = "dispatch-agent")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
pub struct GlobalArgs {
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    Detect,
    Init,
    Dispatch(DispatchArgs),
    Config(ConfigArgs),
}

#[derive(Args)]
pub struct DispatchArgs {
    #[arg(short = 'p', long, group = "prompt_src")]
    pub prompt: Option<String>,
    #[arg(short = 'f', long, group = "prompt_src", value_name = "FILE")]
    pub file: Option<PathBuf>,
    #[arg(long, default_value = "-1")]
    pub timeout: i64,
    #[arg(long, group = "target")]
    pub tier: Option<String>,
    #[arg(long, group = "target")]
    pub agent: Option<String>,
    #[arg(long)] pub dry_run: bool,
    #[arg(long)] pub list: bool,
    #[arg(long)] pub show_config: bool,
    #[arg(long)] pub verbose: bool,
}

#[derive(Args)]
pub struct ConfigArgs {
    pub action: Option<String>,   // edit | show | path
}
```

`main.rs`: parse Cli, call appropriate `cmd_*` function, `process::exit(1)` on Err.

### tests/bin/fake_agent.rs (§9 test harness)

```rust
fn main() {
    let mode = std::env::var("FAKE_AGENT_MODE").unwrap_or_default();
    match mode.as_str() {
        "exit-0" => std::process::exit(0),
        "exit-N" => {
            let n: i32 = std::env::var("FAKE_AGENT_EXIT_CODE")
                .unwrap_or("1".into()).trim().parse().unwrap_or(1);
            std::process::exit(n);
        }
        "sleep" => {
            if let Ok(f) = std::env::var("READY_FILE") {
                std::fs::write(f, "ready").ok();
            }
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        "print-env" => {
            for (k, v) in std::env::vars().filter(|(k, _)| k.starts_with("TEST_")) {
                println!("{k}={v}");
            }
        }
        _ => std::process::exit(0),
    }
}
```

---

## Acceptance criteria (plan §12)

- `cargo build --release` succeeds on macOS (aarch64-apple-darwin)
- `cargo test` green (all unit + integration + property + snapshot tests)
- `cargo clippy -- -D warnings` clean
- `cargo fmt --check` clean
- `insta` snapshots committed under `tests/snapshots/`
- `tests/fixtures/golden/` populated; `scripts/regen_golden.sh` produces no diff
- Python tests still pass: `cd skills/dispatch-agent && uv run --python 3.12 python3 -m pytest tests/`
- `CHANGELOG.md` Unreleased entry updated

---

## Quick-start commands for new session

```bash
# Verify current state
cargo test --bin dispatch-agent 2>&1 | tail -3
cargo clippy -- -D warnings 2>&1 | tail -2

# Reference the full plan
cat docs/plans/2026-05-10-dispatch-agent-rust-rewrite.md

# Test dispatch works
cd /Volumes/Home/Users/wen/.claude/skills/dispatch-agent
uv run --python 3.12 python3 scripts/dispatch.py --list --verbose 2>&1
```
