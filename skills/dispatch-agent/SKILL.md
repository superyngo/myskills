---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--list] [--show-config] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---

# dispatch-agent

Dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) with tier-based fallback and round-robin rotation.

## Recursion Guard

If `DISPATCH_AGENT_DEPTH` env var is set and >= 5, stop immediately:

```bash
python3 -c "
import os, sys
depth = int(os.environ.get('DISPATCH_AGENT_DEPTH', 0))
if depth >= 5:
    print('dispatch recursion limit reached (depth=5)', file=sys.stderr)
    sys.exit(1)
"
```

## Find Config

Check in order (use first found):
1. `--config PATH` argument (if provided)
2. `<git-root>/.config/dispatch-agent.toml` (use `git rev-parse --show-toplevel` to find git root; fall back to cwd if not in a repo)
3. `~/.config/dispatch-agent.toml`

```bash
# Find git root or cwd
PROJECT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
CONFIG_PATH="$PROJECT_ROOT/.config/dispatch-agent.toml"
USER_CONFIG="$HOME/.config/dispatch-agent.toml"
```

## Routing

**If argument is `init`, or no config found:**
Load `references/init-guide.md` and follow the init flow.

**Otherwise — dispatch:**
Translate arguments and run:
```bash
python3 scripts/dispatch.py \
  [-p "<prompt>" | -f "<file>"] \
  [--timeout N] \
  [--tier ID] \
  [--agent ID] \
  [--config PATH] \
  [--dry-run] [--list] [--show-config] [--verbose]
```

If neither `-p` nor `-f` is provided (and not `--list`/`--show-config`/`--dry-run`):
Use `AskUserQuestion` to collect the prompt before dispatching.

**For `--help` or errors:** load `references/dispatch-guide.md`.
