# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07  
**Revised:** 2026-05-07 (v5 — final)

---

## Overview

A skill that dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) using a TOML config file. Supports tier-based fallback, round-robin rotation within tiers, and an interactive init flow.

---

## File Structure

```
skills/dispatch-agent/
  SKILL.md
  data/
    cli-templates.toml     # per-CLI default call syntax (user-extensible runtime data)
  references/
    init-guide.md          # init flow AskUserQuestion prompts for AI
    dispatch-guide.md      # dispatch rules, schema, error handling, output formats
  scripts/
    detect.py              # detect available CLIs (outputs JSON)
    init.py                # write TOML config via string template
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
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--cli ID] [--dry-run] [--list] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---
```

**Instruction prose (core logic AI follows):**
1. Find config: check `<project>/.config/dispatch-agent.toml` then `~/.config/dispatch-agent.toml`
2. If no config found, or argument is `init`: load `references/init-guide.md` and run init flow
3. Otherwise: run `python3 scripts/dispatch.py` with forwarded arguments
4. On error or `--help`: load `references/dispatch-guide.md` for details

---

## Config File

**Format:** TOML. Read via `tomllib`. Written by `init.py` via string templates.

**Search order:** `<project>/.config/dispatch-agent.toml` → `~/.config/dispatch-agent.toml` → trigger init

**Schema:**
```toml
version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "claude-default"    # unique across entire config; used as rr-state key
  cli = "claude"
  model = "default"        # "default" = omit --model flag; let CLI decide
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

    [tiers.agents.env.GITHUB_TOKEN]
    type = "file"
    path = "~/.config/gh/token"
```

**Key rules:**
- `agent.id` must be globally unique — used as rr-state key
- Tier fallback order = YAML appearance order; `tier.id` is label only
- `model = "default"` → dispatch.py omits `--model` flag entirely
- `version = 1` required; missing version → warning + assume v1
- env vars resolved at dispatch time (not at init time)

**env var semantics:**
- `type = "file"` → read file at `path`, use stripped contents as value
- `type = "env"` → forward named env var from current process environment
- Direct secret strings in config are not supported

---

## data/cli-templates.toml

Runtime data file (not an AI reference doc). Read by `dispatch.py` and `detect.py` via `tomllib`.

```toml
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"      # "arg" = pass contents via prompt_flag; "stdin" = pipe
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
prompt_flag = ""             # unverified non-interactive mode
model_flag = ""
file_input_mode = "arg"
version_flag = "--version"
verified = false             # impl must verify before enabling
extra_args = []
```

Adding a new CLI: add a new `[cli-name]` section. No Python changes needed.

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt" [--timeout 30] [--tier primary] [--cli claude-default] [--config path] [--dry-run] [--list] [--verbose]
python3 dispatch.py -f prompt.txt [--timeout -1]
```

- `--timeout N`: `-1` = no timeout (default). `0` → exit with error.
- `--tier ID`: start from named tier. Cannot combine with `--cli`.
- `--cli ID`: force specific agent by `agent.id`. Cannot combine with `--tier`. If ID matches multiple agents, use first match + stderr warning.
- `--list`: with config → show all agents with system availability. Without config → detect-only mode (run detect.py, show system CLIs).

**subprocess safety:** Always `shell=False`. Args as Python list. Never join prompt into shell string.

**Streaming:** Use `subprocess.Popen` with line-by-line stdout read for real-time output. Do not use `subprocess.run` (blocks until completion).

**Exit code:** On success, propagate subprocess exit code. On all-tiers-exhausted, exit 1.

**Round-robin algorithm (pseudocode):**
```
function dispatch(tiers, prompt):
  load rr_state (flock LOCK_EX)
  
  for tier in tiers:
    agents = tier.agents
    start_idx = rr_state[tier.id].index % len(agents)
    
    for i in range(len(agents)):
      agent = agents[(start_idx + i) % len(agents)]
      result = call(agent, prompt)
      
      if result.success:
        rr_state[tier.id].index = (start_idx + i + 1) % len(agents)
        write rr_state (atomic), release flock
        return result
    
    # tier exhausted, try next tier (do not advance rr index)
  
  release flock
  print failure summary to stderr
  exit 1
```

Index advances only on success. On tier exhaustion, index is NOT advanced. Full round within a tier is attempted before falling to next tier.

**rr-state.json location:** `~/.cache/dispatch-agent/rr-state.json`  
`dispatch.py` creates `~/.cache/dispatch-agent/` if it does not exist.

**rr-state.json format:**
```json
{
  "primary":  { "index": 2, "agents": ["claude-default", "gemini-default"] },
  "fallback": { "index": 0, "agents": ["copilot-sonnet"] }
}
```
On load: if stored `agents` list differs from config, reset index to 0 for that tier.

**Atomic write:** `fcntl.flock(LOCK_EX)` protects read-modify-write. Final write uses `os.replace()` (atomic on POSIX).

**SIGINT:** `signal.SIGINT` handler kills subprocess, exits cleanly. rr-state NOT written on interrupt.

**--dry-run output:**
```
[DRY RUN] tier=primary  agent=claude-default
  command: ['claude', '-p', 'your prompt']
```

**--list output (with config):**
```
TIER primary
  [✓] claude-default   cli=claude  model=default   /usr/local/bin/claude
  [✓] gemini-default   cli=gemini  model=default   /usr/local/bin/gemini
TIER fallback
  [✗] copilot-sonnet   cli=copilot model=sonnet-4.6  (not found)
```

**--list output (no config — detect-only):**
```
[no config found — showing system-detected CLIs]
  [✓] claude   /usr/local/bin/claude   v1.2.3
  [✓] gemini   /usr/local/bin/gemini   v0.9.0
  [✗] opencode  (not found)
```

**Error handling:**
| Scenario | Behavior |
|----------|----------|
| TOML parse failure | stderr: "Config parse error: \<detail\>", exit 1 |
| `--timeout 0` | stderr: "Invalid: use -1 for no timeout", exit 1 |
| `--cli` + `--tier` combined | stderr: "Cannot combine --cli and --tier", exit 1 |
| `--cli ID` multiple matches | use first, stderr warning |
| CLI binary not found | skip agent, stderr warning |
| stdout empty on success | success (valid) |
| env file path not found | stderr warning, skip env var, continue |
| rr-state unreadable/corrupt | reset all indices to 0, continue |
| cli-template missing for CLI | stderr warning, pass prompt as sole arg |
| version missing in config | stderr warning, assume v1 |

---

## detect.py

**Output:** JSON to stdout
```json
{
  "claude": { "path": "/usr/local/bin/claude", "version": "1.2.3", "callable": true },
  "opencode": { "path": "/usr/local/bin/opencode", "version": null, "callable": true, "verified": false }
}
```

**Strategy per CLI:**
1. `shutil.which(cli)` → path; if None, `callable: false`
2. Read `version_flag` from `data/cli-templates.toml`; run to capture version string
3. Include `verified: false` if template has `verified = false`
4. No test prompt sent

---

## init.py

**Triggers:** no config found, or `init` argument.

**Interface:** `python3 init.py --input '<json>'`

**--input JSON schema:**
```json
{
  "agents": [
    {
      "id": "claude-default",
      "cli": "claude",
      "model": "default",
      "args": [],
      "env": {},
      "tier": "primary"
    }
  ],
  "tier_order": ["primary", "fallback"],
  "save_location": "project" | "user"
}
```

**File permissions:** config written with `chmod 0o600`.

**AI-guided flow (detailed in references/init-guide.md):**
1. AI runs `python3 scripts/detect.py` → shows callable CLIs (warns on `verified: false`)
2. AI asks via `AskUserQuestion` one at a time:
   - For each callable CLI: custom id? special args? env vars?
   - Tier assignment and order
   - Save location: project or user
3. AI calls `python3 scripts/init.py --input '<json>'`
4. Script writes TOML with 0600 permissions, AI confirms path

---

## Manual Verification Checklist

After implementation, verify with `--dry-run` and live calls:

1. `python3 detect.py` → correct JSON for all 5 CLIs
2. `python3 dispatch.py --list` (no config) → detect-only output
3. Run `init` → config written to correct path with 0600 permissions
4. `python3 dispatch.py --list` (with config) → shows tier/agent/availability
5. `python3 dispatch.py -p "say hi" --dry-run` → correct command shown
6. `python3 dispatch.py -p "say hi"` → real call succeeds, output streamed, `[agent-id]` on stderr
7. Disable tier-1 CLIs → verify tier-2 fallback
8. Disable all CLIs → verify stderr failure summary + exit 1
9. `python3 dispatch.py -p "say hi" --cli claude-default` → bypasses tier logic
10. Two concurrent dispatch calls → rr-state consistent (no index corruption)

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

- Parallel (concurrent) agent calls
- Backoff / cooldown between tier attempts
- Output validation or format checking
- detect.py result caching
- Result persistence beyond stderr failure summary
- Python < 3.11 support
