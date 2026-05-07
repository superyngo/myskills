# dispatch-agent Skill — Design Spec

**Date:** 2026-05-07

---

## Overview

A skill that dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) using a YAML config file. Supports tier-based fallback, round-robin rotation within tiers, and an interactive init flow.

---

## File Structure

```
skills/dispatch-agent/
  SKILL.md
  references/
    init-guide.md
    dispatch-guide.md
  scripts/
    detect.py
    init.py
    dispatch.py
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
          GITHUB_TOKEN: ~/.config/gh/token
```

Tiers are fully user-defined (no semantic meaning enforced). Round-robin state is maintained in memory per dispatch run (not persisted to YAML).

---

## dispatch.py

**Interface:**
```bash
python3 dispatch.py -p "prompt text" [--timeout 30] [--tier 1] [--config path/to.yml]
python3 dispatch.py -f prompt.txt   [--timeout 0]  [--tier 2]
```

- `-p` or `-f`: prompt input (mutually exclusive)
- `--timeout`: seconds before a CLI call is killed; `0` = no timeout (default: 0)
- `--tier`: start from this tier (default: 1)
- `--config`: explicit config path (overrides auto-search)

**Execution flow:**
```
Load YAML → find starting tier
  ↓
Round-robin through tier's agents (index tracked in memory)
  ↓
Invoke agent via subprocess + optional timeout
  ├─ exit 0 → stream stdout to caller, exit 0
  ├─ non-0 exit or timeout → try next agent in tier
  └─ tier exhausted → move to next tier
  ↓
All tiers exhausted → stderr: per-agent failure summary, exit 1
```

**Failure criteria:** non-zero exit code OR timeout expiry. No stdout content inspection.

---

## detect.py

Outputs JSON: `{ "claude": { "path": "...", "version": "...", "callable": true }, ... }`

Steps per CLI:
1. `which <cli>` to confirm binary exists
2. Send minimal test prompt to confirm executable works
3. Record version if `--version` / `-v` is supported

---

## init.py

**Triggers:** called automatically when no config found, or via `init` argument.

**Flow:**
1. Run `detect.py` → show detected CLIs and callability status
2. For each callable CLI, ask user:
   - Any special invocation args (model flag, extra flags)?
   - Any required env vars or API key paths?
3. Ask user to assign CLIs to tiers (user decides tier semantics)
4. Ask where to save config:
   - `[P]` project: `<project>/.config/dispatch-agent.yml`
   - `[U]` user: `~/.config/dispatch-agent.yml`
5. Write YAML, display summary

---

## SKILL.md

Kept to ~30 lines. Responsibilities:
- YAML frontmatter (name, description, argument-hint, allowed-tools)
- Config file search logic
- Route `init` → `scripts/init.py`
- Route general call → `scripts/dispatch.py`
- Reference `references/init-guide.md` and `references/dispatch-guide.md` for details

Details NOT in SKILL.md: YAML schema, CLI call templates, error message formats, detect logic.

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

- Result persistence or logging
- Parallel (concurrent) agent calls
- Authentication management beyond env var path recording
