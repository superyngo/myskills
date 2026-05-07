# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07  
**Revised:** 2026-05-07

---

## Overview

A skill that dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) using a YAML config file. Supports tier-based fallback, round-robin rotation within tiers, and an interactive init flow.

---

## File Structure

```
skills/dispatch-agent/
  SKILL.md
  references/
    init-guide.md        # init flow details, AskUserQuestion scripts for AI
    dispatch-guide.md    # dispatch rules, YAML schema, error handling
  scripts/
    detect.py            # detect available CLIs (outputs JSON)
    init.py              # write YAML config (AI feeds data, script writes)
    dispatch.py          # main dispatch logic
    cli_templates.py     # per-CLI call syntax definitions
```

---

## SKILL.md Frontmatter

```yaml
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier N] [--cli NAME] [--dry-run] [--list]"
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
        args: ["--no-stream"]
        env:
          GITHUB_TOKEN:
            type: file          # read file contents as env var value
            path: ~/.config/gh/token
          OPENAI_API_KEY:
            type: value         # use string directly as env var value
            value: "sk-..."
```

**env var semantics:**
- `type: file` → read file at `path`, use contents (stripped) as env var value
- `type: value` → use `value` string directly as env var value
- Omitting `env` or empty `{}` → no extra env vars injected

---

## cli_templates.py

Defines default call syntax per CLI. Used by dispatch.py to construct subprocess commands.

| CLI | Non-interactive prompt flag | Model flag | Notes |
|-----|-----------------------------|------------|-------|
| claude | `-p "prompt"` | `--model MODEL` | |
| gemini | `-p "prompt"` | `--model MODEL` | |
| codex | `-q "prompt"` | `--model MODEL` | quiet mode |
| copilot | `-p "prompt"` | `--model MODEL` | actual flags TBD at impl time |
| opencode | TBD at impl time | TBD | verify non-interactive mode |

User-specified `args` from YAML are appended after the default template args.

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt text" [--timeout 30] [--tier 1] [--cli claude] [--config path/to.yml] [--dry-run] [--list]
python3 dispatch.py -f prompt.txt   [--timeout 0]  [--tier 2]
```

- `-p` / `-f`: prompt input (mutually exclusive)
- `--timeout`: seconds before kill; `0` = no timeout (default: 0)
- `--tier N`: start from this tier (default: 1)
- `--cli NAME`: force a specific CLI, skip tier logic (for debugging)
- `--config PATH`: explicit config path (overrides auto-search)
- `--dry-run`: show which CLI would be called without executing
- `--list`: list all agents from config with availability status, then exit

**Round-robin state:** persisted to `~/.cache/dispatch-agent/rr-state.json`
```json
{ "tier_1": 2, "tier_2": 0 }
```
Index increments after each successful call (not on failure). On failure, next agent in same tier is tried without advancing the persistent index.

**Execution flow:**
```
Load YAML → resolve env vars → find starting tier
  ↓
Read rr-state.json → pick agent at current index for this tier
  ↓
Build subprocess command from cli_templates + YAML args
  ↓
Execute with optional timeout
  ├─ exit 0 → stream stdout to caller, advance rr index, exit 0
  ├─ non-0 exit or timeout → record failure reason, try next agent in tier
  └─ tier exhausted → move to next tier, do NOT advance rr index
  ↓
All tiers exhausted → stderr: per-agent failure summary, exit 1
```

**Error handling:**
| Scenario | Behavior |
|----------|----------|
| YAML parse failure | stderr: "Config parse error: <detail>", exit 1 |
| CLI in config not found on system | skip agent, log warning to stderr |
| stdout empty on exit 0 | treated as success (empty output is valid) |
| env file path not found | stderr: warning, skip env var, continue |
| rr-state.json unreadable | start from index 0, continue |

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
1. `which <cli>` → get path
2. Run `<cli> --version` or `-v` → capture version (non-zero exit = unknown version, not failure)
3. `callable: true` if binary found; no test prompt sent (avoids API calls/billing)

---

## init.py

**Triggers:** called by AI agent when no config found, or via `init` argument.

**Interaction model:** SKILL.md instructs the AI to use `AskUserQuestion` to collect all data, then pass collected data to `init.py` via args or stdin JSON. The script only handles file writing — no stdin interaction inside the script.

**AI-guided flow (defined in references/init-guide.md):**
1. AI runs `detect.py` → displays results to user
2. AI asks (one question at a time via `AskUserQuestion`):
   - For each detected CLI: special args? env vars needed?
   - Tier assignment for each CLI
   - Save location: project or user
3. AI calls `init.py --input <json>` with collected data
4. Script writes YAML, AI confirms to user

**references/init-guide.md content outline:**
- Exact `AskUserQuestion` prompts for each step
- JSON schema for `--input` parameter
- Edge cases (no CLIs detected, duplicate tier assignments)

---

## references/dispatch-guide.md Content Outline

- Full YAML schema with field descriptions
- CLI templates table (mirrors cli_templates.py)
- env var type semantics
- Round-robin state file format and location
- Error message formats
- `--dry-run` output format
- `--list` output format

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
- Result persistence or logging beyond stderr failure summary
- Authentication management beyond env var path recording
- Relationship with `dispatching-parallel-agents` skill (different use case)
