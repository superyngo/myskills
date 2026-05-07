# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07  
**Revised:** 2026-05-07 (v6 — final)

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
    init.py                # write TOML config via string template (reads --input from stdin)
    dispatch.py            # main dispatch logic
```

**Dependencies:** Python 3.11+ stdlib only (`tomllib`, `json`, `subprocess`, `shutil`, `fcntl`, `os`, `signal`).

---

## SKILL.md

**Frontmatter:**
```yaml
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--cli ID] [--dry-run] [--list] [--show-config] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---
```

**Instruction prose (core routing logic):**
1. Find config: check `<project>/.config/dispatch-agent.toml` (project root = git root, fallback cwd), then `~/.config/dispatch-agent.toml`
2. If no config, or argument is `init`: load `references/init-guide.md` and run init flow
3. Otherwise: `python3 scripts/dispatch.py [args]`
4. For `--help` or errors: load `references/dispatch-guide.md`

---

## Config File

**Format:** TOML (read via `tomllib`; written by `init.py` via string templates).  
**Permissions:** `0600` (set by `init.py` on write).

**Search order:** `<project>/.config/dispatch-agent.toml` → `~/.config/dispatch-agent.toml` → trigger init

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
- `model = "default"` → omit `--model` flag entirely (let CLI decide)
- `model_flag = ""` in cli-templates → omit model flag regardless of model value (log warning if model != "default")
- env vars resolved at dispatch time (not at init time)
- `version` missing → stderr warning, assume v1

**env var semantics:**
- `type = "file"` → read file at `path`, use stripped contents
- `type = "env"` → forward named var from current process

**Config overwrite (init on existing config):**  
AI asks user: overwrite or backup (`dispatch-agent.toml.bak`) before writing new config.

---

## data/cli-templates.toml

```toml
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"    # "arg": pass file contents via prompt_flag; "stdin": pipe to stdin
version_flag = "--version"
extra_args = []

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
prompt_flag = ""           # non-interactive mode unverified
model_flag = ""
file_input_mode = "arg"
version_flag = "--version"
verified = false
extra_args = []
```

**Adding a new CLI:** add `[cli-name]` section. No Python changes needed.

**args merge order:** `template.extra_args` first, then `agent.args` from config.

**`prompt_flag = ""`:** skip agent at dispatch time, log stderr warning.

**Missing `data/cli-templates.toml`:** dispatch.py exits with error and clear message — no fallback.

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt text" [--timeout 30] [--tier primary] [--cli claude-default] [--config path] [--dry-run] [--list] [--show-config] [--verbose]
python3 dispatch.py -f prompt.txt    [--timeout -1]
```

**Flags:**
- `-p` / `-f`: mutually exclusive prompt input
  - `-f FILE`: read file contents; if file not found → stderr error, exit 1
  - contents passed via `prompt_flag` (if `file_input_mode = "arg"`) or piped to stdin (if `"stdin"`)
- `--timeout N`: total wall-clock seconds; `-1` = no timeout (default); `0` → exit with error
- `--tier ID` / `--cli ID`: mutually exclusive; `--cli` bypasses tier logic; multiple matches → first match + stderr warning
- `--dry-run`: print exact subprocess args without executing
- `--list`: with config → agents + availability; without config → detect-only (runs detect.py)
- `--show-config`: print effective merged config (project layer + user layer shown separately), then exit
- `--verbose`: print per-agent attempt start/result and periodic wait status (every 10s) to stderr

**subprocess safety:** `shell=False`, args as Python list always.

**Streaming:** `subprocess.Popen` with line-by-line stdout read. On timeout (wall-clock via `select` or thread), kill process; in-flight stdout is discarded; treated as failure.

**Exit code:** propagate subprocess exit code on success. Exit 1 on all-tiers-exhausted. Exit code 0 from subprocess = success (no stdout content inspection).

**Signal handling:**
- `SIGINT` and `SIGTERM`: kill subprocess, exit cleanly. rr-state NOT written.

**Stderr output on success:** `[agent-id] (tier: tier-id)` — always printed to stderr, regardless of `--verbose`.

**Round-robin algorithm:**
```python
load rr_state (fcntl.flock LOCK_EX)

for tier in tiers:
    agents = tier.agents
    n = len(agents)
    start = rr_state[tier.id]["index"] % n

    for i in range(n):
        agent = agents[(start + i) % n]
        if agent has prompt_flag == "": skip, warn, continue
        result = call_agent(agent, prompt)  # Popen + wall-clock timeout
        if result.success:
            rr_state[tier.id]["index"] = (start + i + 1) % n
            write rr_state atomically (os.replace), release flock
            return result
        else:
            record failure(agent.id, exit_code or "timeout")

    # tier exhausted — do NOT advance index

release flock
print failure summary to stderr
exit 1
```

**rr-state:** `~/.cache/dispatch-agent/rr-state.json` (created by dispatch.py on first use; permissions `0600`)

```json
{
  "primary":  { "index": 2, "agents": ["claude-default", "gemini-default"] },
  "fallback": { "index": 0, "agents": ["copilot-sonnet"] }
}
```
On load: if `agents` list differs from config for a tier, reset that tier's index to 0.  
Atomic write: `fcntl.flock(LOCK_EX)` + `os.replace()`.

**--dry-run output:**
```
[DRY RUN] tier=primary  agent=claude-default
  command: ['claude', '-p', 'your prompt']
```

**--list output (with config):**
```
TIER primary
  [✓] claude-default   cli=claude   model=default    /usr/local/bin/claude
  [✓] gemini-default   cli=gemini   model=default    /usr/local/bin/gemini
TIER fallback
  [✗] copilot-sonnet   cli=copilot  model=sonnet-4.6  (not found)
```

**--list output (no config):**
```
[no config — system CLIs detected]
  [✓] claude    /usr/local/bin/claude   v1.2.3
  [✓] gemini    /usr/local/bin/gemini   v0.9.0
  [✗] opencode  (not found)
```

**--verbose additions:** per-agent: `[attempting claude-default]`; wait status: `[waiting: claude-default 10s elapsed]` every 10s.

**Error handling:**
| Scenario | Behavior |
|----------|----------|
| TOML parse failure | stderr error, exit 1 |
| `--timeout 0` | stderr: "use -1 for no timeout", exit 1 |
| `--cli` + `--tier` | stderr error, exit 1 |
| `-f FILE` not found | stderr error, exit 1 |
| `prompt_flag = ""` for agent | skip agent, stderr warning |
| `model_flag = ""` + model != "default" | skip model flag, stderr warning |
| CLI binary not found | skip agent, stderr warning |
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

**Strategy:**
1. `shutil.which(cli)` → path; if None, `callable: false`
2. Read `version_flag` from `data/cli-templates.toml`; if `version_flag = ""`, skip → `version: null`
3. Otherwise run `<cli> <version_flag>` → capture version string
4. Copy `verified` field from template (default `true` if absent)

---

## init.py

**Interface:** reads `--input` JSON from **stdin pipe** (not CLI arg, to avoid shell escaping issues):
```bash
echo '<json>' | python3 init.py --input -
```

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

`env` is a list of `{ "name": "VAR", "type": "file"|"env", "path"|"var": "..." }`.

**TOML escaping:** validate `agent.id` against `[a-zA-Z0-9_-]` before writing. Escape string values (args, paths) via Python `repr`-style quoting in the template.

**Permissions:** config written with `chmod 0o600`. rr-state written with `0600`.

**AI-guided flow (detailed in references/init-guide.md):**
1. AI runs `python3 scripts/detect.py` → shows callable CLIs; warns on `verified: false`
2. AI asks via `AskUserQuestion` one question at a time:
   - If existing config found: overwrite or backup first?
   - For each callable CLI: custom id? extra args? env vars?
   - Tier assignment and order
   - Save location: project or user
3. AI pipes JSON to `python3 scripts/init.py --input -`
4. Script writes TOML, AI confirms path to user

---

## Manual Verification Checklist

1. `python3 scripts/detect.py` → correct JSON for all 5 CLIs
2. `python3 scripts/dispatch.py --list` (no config) → detect-only output
3. Run `init` → config at correct path, permissions `0600`
4. `python3 scripts/dispatch.py --list` → tier/agent/availability
5. `python3 scripts/dispatch.py --show-config` → config printed
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

## Out of Scope

- Parallel concurrent agent invocations (checklist #13 tests rr-state concurrency safety, not parallel dispatch)
- Backoff / cooldown between tier attempts
- Output content validation
- detect.py result caching
- Result persistence beyond stderr failure summary
- Python < 3.11 support
