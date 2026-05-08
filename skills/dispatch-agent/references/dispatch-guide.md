# dispatch-agent Reference Guide

---

## Quick Reference

| Flag | Description |
|------|-------------|
| `-p "prompt"` | Prompt text (mutually exclusive with -f) |
| `-f FILE` | Read prompt from file (max 256KB) |
| `--timeout N` | Per-agent timeout in seconds (-1 = no timeout, default) |
| `--tier ID` | Start from named tier (mutually exclusive with --agent) |
| `--agent ID` | Force specific agent by agent.id (bypass tier logic) |
| `--config PATH` | Explicit config file path |
| `--dry-run` | Show command without executing |
| `--list` | List agents and availability |
| `--show-config` | Print resolved config |
| `--verbose` | Show per-agent attempt and wait status |

---

## Config Schema

**Location (first found wins):**
1. `--config PATH`
2. `<git-root>/.config/dispatch-agent.toml`
3. `~/.config/dispatch-agent.toml`

```toml
version = 1

[[tiers]]
id = "primary"            # tier label (TOML order = fallback order)

  [[tiers.agents]]
  id = "claude-default"   # unique across all agents; [a-zA-Z0-9_-] only
  cli = "claude"          # must match a key in data/cli-templates.toml
  model = "default"       # "default" = omit --model flag
  args = []               # string array, appended after template.extra_args

  [[tiers.agents.env]]
  name = "GITHUB_TOKEN"   # env var name to inject
  type = "file"           # "file": read path contents; "env": forward from shell
  path = "~/.config/gh/token"

  [[tiers.agents.env]]
  name = "OPENAI_KEY"
  type = "env"
  var = "OPENAI_KEY"
```

### Permission Bypass Flags (Required for Non-Interactive Use)

Dispatched agents run non-interactively and will stall on permission prompts unless bypassed:

| CLI | Flag | Notes |
|-----|------|-------|
| `claude` | `--dangerously-skip-permissions` | Skips all tool permission prompts |
| `codex` | `--dangerously-bypass-approvals-and-sandbox` | Skips approval and sandbox |
| `copilot` | `--allow-all` | Allows all tool operations |
| `gemini-npx` | `--skip-trust` | Already in template `extra_args` — do **not** add to agent `args` |

Add these to each agent's `args` array in the config. Example:

```toml
[[tiers.agents]]
id = "claude-default"
cli = "claude"
model = "default"
args = ["--dangerously-skip-permissions"]
```

---

## cli-templates.toml Format

Located at `data/cli-templates.toml`. User-editable.

| Field | Description |
|-------|-------------|
| `prompt_flag` | Flag used to pass prompt (e.g. `-p`). Empty = skip agent. |
| `model_flag` | Flag for model selection (e.g. `--model`). Empty = no model flag. |
| `file_input_mode` | `"arg"`: pass file contents via prompt_flag. `"stdin"` reserved for v2. |
| `version_flag` | Flag for version detection. Empty = skip version check. |
| `extra_args` | Args always prepended before agent.args. |
| `verified` | `false` = agent skipped at dispatch (unverified non-interactive mode). |

**Adding a new CLI:** add `[cli-name]` section. No Python changes needed.

---

## Tier Fallback Logic

1. Tiers are tried in TOML file order.
2. Within a tier, agents are tried round-robin (starting from last-used + 1).
3. An agent is skipped (with warning) if: `prompt_flag = ""`, or CLI not in templates.
4. If all agents in a tier fail/skip, the next tier is tried. rr-state pointer is NOT updated on tier exhaustion.
5. rr-state pointer updates only on success.

---

## rr-state

**Location:** `~/.cache/dispatch-agent/rr-state.json`
**Format:** `{ "tier-id": "next-agent-id" }`
**Reset:** delete the file manually.

On load: if stored agent id not found in config (agent removed/renamed), start from index 0.

---

## Output Formats

**--dry-run:**
```
[DRY RUN] tier=primary  agent=claude-default
  command: ['claude', '-p', 'your prompt']
```

**--list (with config):**
```
TIER primary
  [✓] claude-default   cli=claude   model=default    /usr/local/bin/claude
  [✗] copilot-sonnet   cli=copilot  model=sonnet-4.6  (not found)
```

**--list (no config):**
```
[SYSTEM CLIs — no config loaded, run 'init' to configure]
  [✓] claude    /usr/local/bin/claude   v1.2.3
  [!] opencode  /usr/local/bin/opencode  v0.5.0  (verified=false)
  [✗] codex     (not found)
```

**--show-config:**
```
Config: /project/.config/dispatch-agent.toml  (project layer)

TIER primary
  agent: claude-default   cli=claude  model=default  args=[]
```

**--verbose:**
```
[attempting claude-default]
[waiting: claude-default — 10s elapsed]
[claude-default] (tier: primary)
```

---

## Error Reference

| Message | Cause |
|---------|-------|
| `Config parse error: ...` | Invalid TOML in config file |
| `use -1 for no timeout` | `--timeout 0` passed |
| `dispatch recursion limit reached` | DISPATCH_AGENT_DEPTH >= 5 |
| `file not found: ...` | `-f FILE` path doesn't exist |
| `file ... exceeds 256KB limit` | `-f FILE` too large |
| `invalid env type ...` | config env.type not "file" or "env" |
| `cli-templates.toml not found` | data/cli-templates.toml missing |

**Warnings (agent skipped, dispatch continues):**
- `CLI ... not in cli-templates.toml` — add entry to data/cli-templates.toml
- `agent ... has empty prompt_flag` — CLI non-interactive mode unverified
- `env var ... not set` — set the env var in your shell
- `env file ... not found` — create the file or update config path

---

## Recursion Guard

`DISPATCH_AGENT_DEPTH` env var tracks dispatch nesting. Set to 0 by default, incremented before each subprocess call. At depth >= 5, dispatch exits with error. Prevents infinite recursion when an agent dispatches back to dispatch-agent.
