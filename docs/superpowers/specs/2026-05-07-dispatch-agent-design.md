# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07  
**Revised:** 2026-05-07 (v3)

---

## Overview

A skill that dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) using a YAML config file. Supports tier-based fallback, round-robin rotation within tiers, and an interactive init flow.

---

## File Structure

```
skills/dispatch-agent/
  SKILL.md
  references/
    init-guide.md          # init flow details, AskUserQuestion prompts for AI
    dispatch-guide.md      # dispatch rules, YAML schema, error handling
    cli-templates.yml      # per-CLI default call syntax (user-extensible)
  scripts/
    detect.py              # detect available CLIs (outputs JSON)
    init.py                # write YAML config (AI feeds data, script writes)
    dispatch.py            # main dispatch logic
```

---

## SKILL.md Frontmatter

```yaml
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier N] [--cli NAME] [--dry-run] [--list] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---
```

---

## Config File

**Search order (project takes priority):**
1. `<project>/.config/dispatch-agent.yml`
2. `~/.config/dispatch-agent.yml`
3. Not found → trigger init

**Schema:**
```yaml
tiers:
  - id: 1
    agents:
      - cli: claude
        model: default
        args: []
        env: {}
      - cli: gemini
        model: default
        args: []
        env: {}
  - id: 2
    agents:
      - cli: copilot
        model: sonnet-4.6
        args: []
        env:
          GITHUB_TOKEN:
            type: file       # read file contents as env var value
            path: ~/.config/gh/token
          CUSTOM_VAR:
            type: env        # inherit value from current process environment
            var: CUSTOM_VAR
```

**env var semantics:**
- `type: file` → read file at `path`, use contents (stripped) as env var value
- `type: env` → read named env var from current process and forward it
- `type: value` is **not supported** — do not write secrets into YAML
- Omitting `env` or empty `{}` → no extra env vars injected

---

## references/cli-templates.yml

User-editable. Defines default call syntax per CLI. dispatch.py reads this file at runtime.

```yaml
claude:
  prompt_flag: "-p"
  model_flag: "--model"
  extra_args: []

gemini:
  prompt_flag: "-p"
  model_flag: "--model"
  extra_args: []

codex:
  prompt_flag: "-q"
  model_flag: "--model"
  extra_args: []

copilot:
  prompt_flag: "-p"
  model_flag: "--model"
  extra_args: []

opencode:
  prompt_flag: ""       # TBD: verify non-interactive mode at impl time
  model_flag: ""
  extra_args: []
```

User-specified `args` from YAML config are appended after template args.
New CLIs can be added by the user without modifying any Python script.

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt text" [--timeout 30] [--tier 1] [--cli claude] [--config path/to.yml] [--dry-run] [--list] [--verbose]
python3 dispatch.py -f prompt.txt    [--timeout -1] [--tier 2]
```

- `-p` / `-f`: prompt input (mutually exclusive)
- `--timeout N`: seconds before kill; `-1` = no timeout (default: -1). `0` is invalid and exits with error.
- `--tier N`: start from this tier (default: 1)
- `--cli NAME`: force a specific CLI, bypasses all tier logic entirely. Mutually exclusive with `--tier`.
- `--config PATH`: explicit config path (overrides auto-search)
- `--dry-run`: print the command that would be run, without executing
- `--list`: list all agents from config with system availability, then exit
- `--verbose`: print per-agent attempt info to stderr (default: silent except errors)

**subprocess safety:** Always use `shell=False` with args as a list. Never concatenate prompt into a shell string.

**Round-robin state:** persisted via atomic write to `~/.cache/dispatch-agent/rr-state.json`
```json
{ "tier_1": { "index": 2, "agents": ["claude", "gemini"] }, "tier_2": { "index": 0, "agents": ["copilot"] } }
```
State stores agent identity list alongside index. On load, if stored agent list differs from config (agent added/removed), index resets to 0 for that tier.

**Atomic write:** write to `rr-state.json.tmp`, then `os.replace()` — prevents corruption on concurrent writes. No file lock needed (replace is atomic on POSIX).

**Execution flow:**
```
Validate args (--cli and --tier mutually exclusive; --timeout 0 is error)
  ↓
Load YAML → resolve env vars → find starting tier (or direct CLI)
  ↓
Load cli-templates.yml
  ↓
Read rr-state.json → pick agent at current index for this tier
  ↓
Build subprocess args list (shell=False)
  ↓
Execute with optional timeout (subprocess.run with timeout=N or None)
  ├─ exit 0 → stream stdout to caller, print "[cli-name]" to stderr (--verbose or always?), advance rr index atomically, exit 0
  ├─ non-0 exit or timeout → record failure reason, try next agent in tier
  └─ tier exhausted → move to next tier, do NOT advance rr index
  ↓
All tiers exhausted → stderr: per-agent failure summary, exit 1
```

**Output:** on success, `[cli-name]` source label is always printed to stderr (stdout is clean for piping).

**Error handling:**
| Scenario | Behavior |
|----------|----------|
| YAML parse failure | stderr: "Config parse error: \<detail\>", exit 1 |
| `--timeout 0` | stderr: "Invalid timeout: use -1 for no timeout", exit 1 |
| `--cli` and `--tier` both specified | stderr: "Cannot use --cli and --tier together", exit 1 |
| CLI in config not found on system | skip agent, log warning to stderr |
| stdout empty on exit 0 | treated as success (empty output is valid) |
| env file path not found | stderr: warning, skip env var, continue |
| rr-state.json unreadable/corrupt | start from index 0 for all tiers, continue |
| cli-templates.yml missing entry for CLI | stderr: warning, call CLI with prompt as sole arg |

---

## detect.py

**Output:** JSON to stdout
```json
{
  "claude": { "path": "/usr/local/bin/claude", "version": "1.2.3", "callable": true },
  "gemini": { "path": null, "version": null, "callable": false }
}
```

**Detection strategy per CLI:**
1. `which <cli>` → get path; if not found, `callable: false`, skip remaining steps
2. Run `<cli> --version` or `-v` → capture version string (non-zero = unknown version, still callable)
3. No test prompt sent (avoids API calls/billing)

---

## init.py

**Triggers:** called by AI agent when no config found, or via `init` argument.

**Interaction model:** SKILL.md instructs the AI to use `AskUserQuestion` to collect all data, then pass collected data to `init.py --input <json>`. The script only handles file writing — no interactive stdin.

**AI-guided flow (detailed in references/init-guide.md):**
1. AI runs `python3 detect.py` → displays results to user
2. AI asks one question at a time via `AskUserQuestion`:
   - For each callable CLI: any special args? any env vars needed?
   - Tier assignment for each CLI
   - Save location: `[P]` project or `[U]` user
3. AI calls `python3 init.py --input '<json>'`
4. Script writes YAML, AI confirms path to user

**references/init-guide.md content:**
- Exact `AskUserQuestion` option sets for each step
- JSON schema for `--input` parameter
- Edge cases: no CLIs detected, same CLI in multiple tiers, env var conflict

---

## references/dispatch-guide.md Content

- Full YAML schema with field descriptions and examples
- cli-templates.yml format and how to add a new CLI
- env var type semantics (`file`, `env`)
- Round-robin state file format, location, and reset conditions
- Error message reference
- `--dry-run` output format
- `--list` output format
- `--verbose` output format

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
- detect.py caching
- Result persistence or logging beyond stderr failure summary
- Authentication management beyond env var forwarding
