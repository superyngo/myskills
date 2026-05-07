# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07  
**Revised:** 2026-05-07 (v7 — final)

**Platform:** macOS / Linux only. Windows not supported.

---

## Overview

A skill that dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) using a TOML config file. Supports tier-based fallback, round-robin rotation within tiers, and an interactive init flow.

---

## File Structure

```
skills/dispatch-agent/
  SKILL.md
  data/
    cli-templates.toml     # per-CLI default call syntax (runtime data, user-extensible)
  references/
    init-guide.md          # AskUserQuestion prompts for AI-guided init
    dispatch-guide.md      # dispatch rules, schema reference, output formats
  scripts/
    detect.py              # detect available CLIs, outputs JSON
    init.py                # write TOML config via minimal hand-written TOML serializer
    dispatch.py            # main dispatch logic
```

**Dependencies:** Python 3.11+ stdlib only (`tomllib`, `json`, `subprocess`, `shutil`, `fcntl`, `os`, `signal`, `threading`).

---

## SKILL.md

**Frontmatter:**
```yaml
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--list] [--show-config] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---
```

**Instruction prose (core routing logic):**
1. Find config (priority: `--config` > `<project>/.config/dispatch-agent.toml` > `~/.config/dispatch-agent.toml`). Project root = git root; fallback to cwd if not in a git repo.
2. If no config found, or argument is `init`: load `references/init-guide.md` and run init flow
3. Otherwise: translate skill arguments to dispatch.py flags and run:
   - `-p <prompt>` → `python3 scripts/dispatch.py -p "<prompt>"`
   - `-f <file>` → `python3 scripts/dispatch.py -f "<file>"`
   - `--timeout N` → append `--timeout N`
   - `--tier ID` → append `--tier ID`
   - `--agent ID` → append `--agent ID`
   - `--config PATH` → append `--config PATH`
   - `--dry-run`, `--list`, `--show-config`, `--verbose` → pass through as-is
   - If no prompt flag provided: ask user for prompt via `AskUserQuestion` before dispatching
4. For `--help` or errors: load `references/dispatch-guide.md`

**Recursion guard:** if env var `DISPATCH_AGENT_DEPTH` is set and >= 5, exit with error "dispatch recursion limit reached". dispatch.py sets `DISPATCH_AGENT_DEPTH = current + 1` before each subprocess call.

---

## Config File

**Format:** TOML (read via `tomllib`; written by `init.py` via hand-written minimal TOML serializer).  
**Permissions:** `0600` (set by `init.py` on write).

**Config layer priority:** the first config found is used exclusively. There is no merging between project and user layers.

**Search order:** `--config PATH` → `<project>/.config/dispatch-agent.toml` → `~/.config/dispatch-agent.toml` → trigger init

**Schema:**
```toml
version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "claude-default"
  cli = "claude"
  model = "default"
  args = []

  [[tiers.agents]]
  id = "gemini-default"
  cli = "gemini"
  model = "default"
  args = []

[[tiers]]
id = "fallback"

  [[tiers.agents]]
  id = "copilot-sonnet"
  cli = "copilot"
  model = "sonnet-4.6"
  args = []

  [[tiers.agents.env]]
  name = "GITHUB_TOKEN"
  type = "file"
  path = "~/.config/gh/token"
```

**Key rules:**
- `agent.id`: globally unique; chars restricted to `[a-zA-Z0-9_-]`; used as rr-state key
- Tier fallback order = TOML appearance order; `tier.id` is label only
- `model = "default"` → omit `--model` flag (let CLI decide)
- `model_flag = ""` in cli-templates → omit model flag regardless; if agent's `model != "default"`, log stderr warning
- env vars resolved at dispatch time via `os.path.expanduser` + `os.environ`
- `version` missing → stderr warning, assume v1

**env var semantics:**
- `type = "file"` → `os.path.expanduser(path)` → read file, use stripped contents
- `type = "env"` → forward named var from current process environment

**Config overwrite (init on existing config):**
AI asks user: overwrite or backup (`dispatch-agent.toml.bak`) before writing.

---

## data/cli-templates.toml

```toml
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"    # "arg": contents via prompt_flag. "stdin": pipe (v2, unverified)
version_flag = "--version"
extra_args = []             # e.g. ["--no-stream"] for flags always appended

[gemini]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[codex]
prompt_flag = "-q"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[copilot]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[opencode]
prompt_flag = ""           # non-interactive mode unverified — agent will be skipped at dispatch
model_flag = ""
file_input_mode = "arg"
version_flag = "--version"
verified = false
extra_args = []
```

**args merge order:** `template.extra_args` first, then `agent.args` from config.  
**`prompt_flag = ""`:** agent is skipped at dispatch time with stderr warning. Users are informed of this during init (see `references/init-guide.md`).  
**`file_input_mode = "stdin"`:** reserved for v2; do not use in v1.  
**Missing `data/cli-templates.toml`:** dispatch.py exits with error — no fallback.

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt text" [--timeout 30] [--tier primary] [--agent claude-default] [--config path] [--dry-run] [--list] [--show-config] [--verbose]
python3 dispatch.py -f prompt.txt    [--timeout -1]
```

**Flags:**
- `-p` / `-f`: mutually exclusive prompt input
  - `-f FILE`: read file; if not found → stderr error, exit 1; if size > 256KB → stderr error, exit 1 (ARG_MAX risk); contents passed via `prompt_flag`
- `--timeout N`: per-agent wall-clock seconds (each attempt gets N seconds independently); `-1` = no timeout (default); `0` → exit with error
- `--tier ID` / `--agent ID`: mutually exclusive; `--agent` accepts `agent.id` (e.g. `claude-default`), bypasses tier logic; multiple id matches → first + stderr warning
- `--help`: handled by argparse within dispatch.py; does not route to AI
- `--dry-run`: print exact subprocess args without executing
- `--list`: with config → agents + availability; without config → detect-only (runs detect.py)
- `--show-config`: print config (see output format below), then exit
- `--verbose`: print per-agent attempt and periodic wait status (every 10s) to stderr

**subprocess safety:** `shell=False`, args as Python list always.

**Streaming:** `subprocess.Popen` with `start_new_session=True` (creates new process group). Subprocess **stdout forwarded in real time to dispatch.py stdout** (caller receives output as it arrives). Use `select` + non-blocking read loop to avoid blocking on `readline()`; this also handles clean pipe state after SIGKILL. Subprocess stderr captured in memory during run; forwarded to dispatch.py stderr on failure only (included in failure summary).

**Timeout implementation:** `threading.Timer(N, lambda: os.killpg(os.getpgid(process.pid), signal.SIGKILL))` started after `Popen`. Kills entire process group (prevents orphan children). `select` loop exits when pipe closes after kill. Any in-flight stdout is discarded; treated as failure.

**Verbose wait status:** separate background thread prints `[waiting: agent-id — Xs elapsed]` every 10s to stderr. Thread is stopped when agent completes.

**Recursion guard:** read `DISPATCH_AGENT_DEPTH` from env (default 0); if >= 5, exit 1. Set `DISPATCH_AGENT_DEPTH = current + 1` in subprocess env before each call.

**Exit code:** propagate subprocess exit code on success. Exit 1 on all-tiers-exhausted. Exit code 0 = success (no stdout content inspection).

**Signal handling:** `SIGINT` and `SIGTERM` → `os.killpg` on subprocess group, exit cleanly. rr-state NOT written.

**Stderr on success:** always print `[agent-id] (tier: tier-id)` to stderr.

**Round-robin algorithm:**
```python
validate config: env.type must be "file"|"env" for all agents; exit 1 on invalid

# Phase 1: read rr_state under lock, release immediately
with fcntl.flock(rr_state_fd, LOCK_EX):
    rr_state = load_rr_state()  # {} if file missing or unreadable

failures = []

for tier in tiers:
    agents = tier.agents            # list of agent dicts from config
    next_id = rr_state.get(tier.id)
    start = index_of(next_id, agents) if next_id in [a.id for a in agents] else 0
    n = len(agents)

    for i in range(n):
        agent = agents[(start + i) % n]
        template = cli_templates.get(agent.cli)
        if template is None: warn, continue
        if template.prompt_flag == "": warn, continue
        if -f used and template.prompt_flag == "": warn, continue

        # resolve env vars (at dispatch time)
        env = os.environ.copy()
        for ev in agent.env:
            if ev.type == "file": env[ev.name] = open(expanduser(ev.path)).read().strip()
            elif ev.type == "env": env[ev.name] = os.environ[ev.var]
        env["DISPATCH_AGENT_DEPTH"] = str(current_depth + 1)

        # call agent — lock is NOT held during this call
        result = call_agent(agent, template, prompt, env)
        # call_agent: Popen(start_new_session=True, shell=False)
        #             + select loop for stdout forward + stderr capture
        #             + threading.Timer(timeout, killpg) if timeout != -1

        if result.success:
            # Phase 2: write updated rr_state under lock
            next_agent_id = agents[(start + i + 1) % n].id
            with fcntl.flock(rr_state_fd, LOCK_EX):
                rr_state = load_rr_state()  # re-read in case another process wrote
                rr_state[tier.id] = next_agent_id
                atomic_write(rr_state)
            return result
        else:
            failures.append((agent.id, result.exit_code or "timeout", result.stderr_snippet))
    # tier exhausted — do NOT update rr pointer

print failure summary to stderr
exit 1
```

**rr-state:** `~/.cache/dispatch-agent/rr-state.json` — created by dispatch.py on first use; permissions `0600`.

```json
{ "primary": "gemini-default", "fallback": "copilot-sonnet" }
```
Map of tier-id → **next agent id to call**. On load: find the stored id in current agent list → start from that index. If id not found (agent removed/renamed) → start from index 0.

**Atomic write:** `fcntl.flock(LOCK_EX)` + `os.replace()`.

**--dry-run output:**
```
[DRY RUN] tier=primary  agent=claude-default
  command: ['claude', '-p', 'your prompt']
```

**--show-config output:**
```
Config: /project/.config/dispatch-agent.toml  (project layer)

TIER primary
  agent: claude-default   cli=claude  model=default  args=[]
  agent: gemini-default   cli=gemini  model=default  args=[]
TIER fallback
  agent: copilot-sonnet   cli=copilot  model=sonnet-4.6  args=[]
    env: GITHUB_TOKEN (file: ~/.config/gh/token)
```

**--list output (with config):**
```
TIER primary
  [✓] claude-default   cli=claude   model=default    /usr/local/bin/claude
  [✓] gemini-default   cli=gemini   model=default    /usr/local/bin/gemini
TIER fallback
  [✗] copilot-sonnet   cli=copilot  model=sonnet-4.6  (not found)
```

**--list output (no config — detect-only, different purpose from config mode):**
```
[SYSTEM CLIs — no config loaded, run 'init' to configure]
  [✓] claude    /usr/local/bin/claude   v1.2.3
  [✓] gemini    /usr/local/bin/gemini   v0.9.0
  [!] opencode  /usr/local/bin/opencode  v0.5.0  (verified=false — will be skipped at dispatch)
  [✗] codex     (not found)
```

**--verbose additions:** `[attempting claude-default]` before each call; `[waiting: claude-default — 10s elapsed]` every 10s.

**Error handling:**
| Scenario | Behavior |
|----------|----------|
| TOML parse failure | stderr error, exit 1 |
| `--timeout 0` | stderr: "use -1 for no timeout", exit 1 |
| `--cli` + `--tier` combined | stderr error, exit 1 |
| `-f FILE` not found | stderr error, exit 1 |
| `-f FILE` size > 256KB | stderr error, exit 1 (ARG_MAX risk) |
| `DISPATCH_AGENT_DEPTH` >= 5 | stderr error, exit 1 |
| `env.type` invalid value | stderr error, exit 1 at startup |
| `prompt_flag = ""` for agent | skip agent, stderr warning |
| `-f FILE` + `prompt_flag = ""` | skip agent, stderr warning |
| `cli` in config not found in cli-templates.toml | skip agent, stderr warning |
| `model_flag = ""` + model != "default" | skip model flag, stderr warning |
| CLI binary not found on system | skip agent, stderr warning |
| env file not found | skip env var, stderr warning, continue |
| rr-state unreadable | reset all indices to 0, continue |
| `data/cli-templates.toml` missing | stderr error, exit 1 |
| config version missing | stderr warning, assume v1 |

---

## detect.py

**Output:** JSON to stdout
```json
{
  "claude":   { "path": "/usr/local/bin/claude", "version": "1.2.3", "callable": true, "verified": true },
  "opencode": { "path": "/usr/local/bin/opencode", "version": null, "callable": true, "verified": false }
}
```

**Strategy per CLI:**
1. `shutil.which(cli)` → path; if None, `callable: false`, stop
2. Read `version_flag` from `data/cli-templates.toml`:
   - If file missing → skip version detection for all CLIs, `version: null`
   - If CLI entry missing in templates → `version: null`
   - If `version_flag = ""` → `version: null`
3. Run `<cli> <version_flag>` with 5s timeout; on failure (non-0 exit, timeout, exception) → `version: null`
4. Copy `verified` from template (default `true` if absent)

---

## init.py

**Interface:** reads JSON from stdin:
```bash
echo '<json>' | python3 init.py
```

**TOML serializer:** hand-written minimal serializer covering only the defined schema. No string template.
- Validates `agent.id` against `[a-zA-Z0-9_-]`; exits with stderr error on invalid chars
- Validates global uniqueness of all `agent.id` values; exits with stderr error on duplicates
- `agent.args`: string array only; element type validated at serialize time
- String escaping for all TOML string values: `\` → `\\`, `"` → `\"`, newline → `\n`, tab → `\t`
- **Round-trip validation:** after writing, read back with `tomllib.loads()` and compare; if parse fails, delete file, stderr error, exit 1
- On any validation error: exit 1, plain text stderr message
- To reset rr-state: delete `~/.cache/dispatch-agent/rr-state.json` manually

**Model defaults:** if user does not specify a model for an agent during init, apply the Default Platforms table value for that CLI. If CLI not in the table, leave model field as `"default"`.

**--input JSON schema:**
```json
{
  "agents": [
    {
      "id": "claude-default",
      "cli": "claude",
      "model": "default",
      "args": [],
      "env": [],
      "tier": "primary"
    }
  ],
  "tier_order": ["primary", "fallback"],
  "save_location": "project" | "user"
}
```

`env` items: `{ "name": "VAR", "type": "file"|"env", "path"|"var": "..." }`.

**Permissions:** config written with `chmod 0o600`.

**AI-guided flow (detailed in references/init-guide.md):**
1. AI runs `python3 scripts/detect.py` → shows callable CLIs
   - Explicitly tells user: CLIs with `verified: false` will be skipped at dispatch even if configured
2. AI asks via `AskUserQuestion` one at a time:
   - If existing config found: overwrite or backup?
   - For each callable + verified CLI: custom id? extra args? env vars? (pre-fill model from Default Platforms table)
   - Tier assignment and order
   - Save location: project or user
3. AI pipes JSON to `python3 scripts/init.py`
4. If init.py exits non-0: AI shows stderr error and offers to retry
5. On success: AI confirms path + permissions to user

---

## references/ File Outlines

### references/init-guide.md sections
1. **Prerequisites** — ensure detect.py is runnable; check Python 3.11+
2. **Step 1: Detect CLIs** — run detect.py, interpret output, explain verified/unverified distinction to user
3. **Step 2: Existing config handling** — check for existing config; AskUserQuestion for overwrite vs backup
4. **Step 3: Per-CLI configuration** — for each callable+verified CLI: id, model (pre-fill default), args, env vars
5. **Step 4: Tier assignment** — AskUserQuestion for tier names and which agents go in which tier + order
6. **Step 5: Save location** — AskUserQuestion for project vs user
7. **Step 6: Write config** — build JSON, pipe to init.py, handle errors
8. **Edge cases** — no CLIs detected; all CLIs unverified; duplicate tier names; env file not found

### references/dispatch-guide.md sections
1. **Quick reference** — flag summary table
2. **Config schema** — full TOML schema with field descriptions and examples
3. **cli-templates.toml format** — field descriptions, how to add a new CLI
4. **env var types** — `file` and `env` semantics with examples
5. **Tier fallback logic** — how tiers and round-robin work together
6. **rr-state** — file location, format, reset conditions
7. **Output formats** — `--dry-run`, `--list`, `--show-config`, `--verbose` with examples
8. **Error reference** — all error messages and their meanings
9. **Recursion guard** — DISPATCH_AGENT_DEPTH explanation

---

## Manual Verification Checklist

1. `python3 scripts/detect.py` → correct JSON for all 5 CLIs; opencode shows `verified: false`
2. `python3 scripts/dispatch.py --list` (no config) → detect-only, opencode shows `[!]`
3. Run `init` → config at correct path, permissions `0600`, opencode warning shown
4. `python3 scripts/dispatch.py --list` → tier/agent/availability
5. `python3 scripts/dispatch.py --show-config` → correct formatted output
6. `python3 scripts/dispatch.py -p "say hi" --dry-run` → correct command shown
7. `python3 scripts/dispatch.py -f nonexistent.txt` → exit 1 with error
8. `python3 scripts/dispatch.py -p "say hi"` → output streamed, `[agent-id] (tier: ...)` on stderr
9. `python3 scripts/dispatch.py -p "say hi" --verbose` → per-attempt logs on stderr
10. Disable tier-1 CLIs → tier-2 fallback triggered
11. Disable all CLIs → stderr failure summary, exit 1
12. `--cli claude-default` → bypasses tier logic
13. Two concurrent dispatch calls → rr-state index consistent (no corruption)
14. Run `init` on existing config → backup/overwrite prompt honored

---

## Default Platforms

| CLI | Default Model |
|-----|--------------|
| claude | default |
| gemini | default |
| codex | default |
| copilot | sonnet-4.6 |
| opencode | glm-5.1 |

---

## Out of Scope (v1)

- Parallel concurrent agent invocations (checklist #13 tests rr-state concurrency safety, not parallel dispatch)
- Backoff / cooldown between tier attempts
- Output content validation
- detect.py result caching
- Result persistence beyond stderr failure summary
- `file_input_mode = "stdin"` (deferred to v2)
- rr-state `last_failure_time` tracking (deferred to v2)
- Windows support
- Python < 3.11
