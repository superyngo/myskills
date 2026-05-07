# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07  
**Revised:** 2026-05-07 (v4 — final)

---

## Overview

A skill that dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) using a TOML config file. Supports tier-based fallback, round-robin rotation within tiers, and an interactive init flow.

---

## File Structure

```
skills/dispatch-agent/
  SKILL.md
  references/
    init-guide.md          # init flow details, AskUserQuestion prompts for AI
    dispatch-guide.md      # dispatch rules, schema, error handling, output formats
    cli-templates.toml     # per-CLI default call syntax (user-extensible)
  scripts/
    detect.py              # detect available CLIs (outputs JSON)
    init.py                # write TOML config via string template (no third-party)
    dispatch.py            # main dispatch logic
```

**Dependencies:** Python 3.11+ stdlib only (`tomllib`, `json`, `subprocess`, `shutil`, `fcntl`, `os`, `signal`). No third-party packages required.

---

## SKILL.md Frontmatter

```toml
# (frontmatter is YAML per skill convention)
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier N] [--cli NAME] [--dry-run] [--list] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---
```

---

## Config File

**Format:** TOML. Read via `tomllib` (Python 3.11+ stdlib). Written by `init.py` using string templates.

**Search order (project takes priority):**
1. `<project>/.config/dispatch-agent.toml`
2. `~/.config/dispatch-agent.toml`
3. Not found → trigger init

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

  [tiers.agents.env]
    [tiers.agents.env.GITHUB_TOKEN]
    type = "file"
    path = "~/.config/gh/token"

    [tiers.agents.env.CUSTOM_VAR]
    type = "env"
    var = "CUSTOM_VAR"
```

**Key rules:**
- `agent.id` must be unique across the entire config — used as rr-state key
- `tier.id` is a label only; fallback order follows YAML appearance order
- `version = 1` required at top level for future migration

**env var semantics:**
- `type = "file"` → read file at `path`, use stripped contents as env var value
- `type = "env"` → forward named env var from current process
- Secrets must NOT be written directly into config

---

## references/cli-templates.toml

User-editable. Read by `dispatch.py` at runtime via `tomllib`.

```toml
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"    # "arg" = pass file contents as -p arg; "stdin" = pipe to stdin
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
prompt_flag = ""           # TBD: verify non-interactive mode before impl
model_flag = ""
file_input_mode = "arg"
version_flag = "--version"
extra_args = []
```

Adding a new CLI: create a new `[cli-name]` section. No Python changes needed.

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt text" [--timeout 30] [--tier primary] [--cli claude-default] [--config path/to.toml] [--dry-run] [--list] [--verbose]
python3 dispatch.py -f prompt.txt    [--timeout -1]
```

- `-p` / `-f`: prompt input (mutually exclusive). `-f` reads file and passes contents per `file_input_mode`.
- `--timeout N`: seconds before kill; `-1` = no timeout (default). `0` is invalid → exit with error.
- `--tier ID`: start from named tier (default: first tier in config). Cannot combine with `--cli`.
- `--cli ID`: force specific agent by `agent.id`, bypass all tier logic. Cannot combine with `--tier`.
- `--config PATH`: explicit config path (overrides auto-search)
- `--dry-run`: print the exact subprocess args that would be run, without executing
- `--list`: print all agents with system availability (`shutil.which` check), then exit
- `--verbose`: print per-agent attempt info and wait status to stderr (default: silent except errors)

**subprocess safety:** Always `shell=False`. Build args as a Python list. Never join prompt into a shell string.

**Round-robin state:** `~/.cache/dispatch-agent/rr-state.json`
```json
{
  "primary": { "index": 2, "agents": ["claude-default", "gemini-default"] },
  "fallback": { "index": 0, "agents": ["copilot-sonnet"] }
}
```
- Index advances only on success
- On load: if stored `agents` list differs from config (agent added/removed), reset index to 0 for that tier
- Read-modify-write protected by `fcntl.flock(LOCK_EX)` to prevent TOCTOU race
- Final write uses `os.replace()` (atomic on POSIX) to prevent file corruption

**Execution flow:**
```
Validate args → error if --timeout 0, or --cli + --tier combined
  ↓
Load config.toml (tomllib) → load cli-templates.toml
  ↓
Resolve env vars → build starting tier list (or direct agent if --cli)
  ↓
Acquire flock on rr-state.json → read index for current tier
  ↓
Build subprocess args list (shell=False)
  ↓
Execute (subprocess.run, timeout=N or None)
  ├─ exit 0 → stream stdout to caller
  │           print "[agent-id]" to stderr
  │           advance index, write rr-state, release flock
  │           exit 0
  ├─ non-0 or timeout → record failure (agent-id, exit code or "timeout")
  │                     try next agent in tier (wrap around then move to next tier)
  └─ all tiers exhausted → stderr: failure summary per agent, exit 1
```

**SIGINT handling:** register `signal.SIGINT` handler that kills the running subprocess and exits cleanly. rr-state is NOT written on interrupt (index not advanced).

**--dry-run output example:**
```
[DRY RUN] Would call: ['claude', '-p', 'your prompt', '--model', 'default']
  agent-id: claude-default  tier: primary
```

**--list output example:**
```
TIER primary
  [✓] claude-default   cli=claude   model=default   (found: /usr/local/bin/claude)
  [✓] gemini-default   cli=gemini   model=default   (found: /usr/local/bin/gemini)
TIER fallback
  [✗] copilot-sonnet   cli=copilot  model=sonnet-4.6 (not found)
```

**Error handling:**
| Scenario | Behavior |
|----------|----------|
| TOML parse failure | stderr: "Config parse error: \<detail\>", exit 1 |
| `--timeout 0` | stderr: "Invalid timeout: use -1 for no timeout", exit 1 |
| `--cli` + `--tier` combined | stderr: "Cannot use --cli and --tier together", exit 1 |
| `--cli NAME` matches multiple agents | use first match, print warning to stderr |
| CLI binary not found on system | skip agent, log warning to stderr |
| stdout empty on exit 0 | success (empty output is valid) |
| env file path not found | stderr: warning, skip env var, continue |
| rr-state.json unreadable/corrupt | reset all indices to 0, continue |
| cli-templates entry missing for CLI | stderr: warning, pass prompt as sole arg |
| `version` field missing in config | stderr: warning, continue (assume v1) |

---

## detect.py

**Output:** JSON to stdout
```json
{
  "claude": { "path": "/usr/local/bin/claude", "version": "1.2.3", "callable": true },
  "gemini": { "path": null, "version": null, "callable": false }
}
```

**Detection strategy:**
1. `shutil.which(cli)` → get path; if `None`, `callable: false`
2. Read `version_flag` from `cli-templates.toml`; run `<cli> <version_flag>` → capture version string
3. `callable: true` if binary found. No test prompt sent (avoids API calls/billing).

---

## init.py

**Triggers:** called by AI when no config found, or via `init` argument.

**Interaction model:** AI uses `AskUserQuestion` to collect all data, then calls:
```bash
python3 init.py --input '<json-string>'
```
Script writes TOML via string template. No interactive stdin inside the script.

**File permissions:** config written with `chmod 0o600` (owner read/write only).

**AI-guided flow (detailed in references/init-guide.md):**
1. AI runs `python3 detect.py` → displays callable CLIs to user
2. AI asks via `AskUserQuestion` (one question at a time):
   - For each callable CLI: special args? env vars? custom agent id?
   - Tier assignment (which agents in which tier, in what order)
   - Save location: `[P]` project or `[U]` user
3. AI calls `python3 init.py --input '<json>'`
4. Script writes TOML, AI confirms path to user

**references/init-guide.md content:**
- Exact `AskUserQuestion` option sets per step
- JSON schema for `--input` parameter
- Edge cases: no CLIs detected, duplicate agent ids, env var conflicts

---

## Default Platforms (for init detection)

| CLI | Default Model |
|-----|--------------|
| claude | default |
| gemini | default |
| codex | default |
| copilot | sonnet-4.6 |
| opencode | glm5.1 |

---

## Out of Scope

- Parallel (concurrent) agent calls
- Backoff / cooldown between tier attempts
- Result validation or output format checking
- detect.py result caching
- Result persistence or logging beyond stderr failure summary
- Python < 3.11 support (tomllib requires 3.11+)
