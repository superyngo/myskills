# `dispatch-agent config` — schema, semantics, and skill behaviour

Loaded when the skill routes any `config` subcommand. `config edit` and bare `config` are intercepted by the skill (see "Editing the config" below); `show`, `list`, `path` are forwarded to the CLI verbatim.

## Where the config lives

- **User-level:** `~/.config/dispatch-agent.toml`
- **Project-level:** `<git-root>/.config/dispatch-agent.toml` (if cwd is in a git repo)

The CLI's resolution order is: `--config PATH` if supplied → project-level if in a repo → user-level. The `dispatch-agent init` JSON `save_location` field (`"user"` or `"project"`) selects which file `init` writes to. **`--config` does not redirect `init` output** — only `save_location` does.

## TOML schema

```toml
version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "claude-default"          # [a-zA-Z0-9_-]+, unique across all tiers
  cli = "claude"                  # must match a detect() output key
  model = "claude-sonnet-4-5"     # resolved value, may be "default" in input JSON
  args = ["--dangerously-skip-permissions"]   # extra CLI args
  enabled = true                  # written by init, default true

    [[tiers.agents.env]]
    type = "file"                 # or "env", or "source"
    name = "ANTHROPIC_API_KEY"    # for "file" / "env"
    path = "~/.secrets/anthropic" # for "file" / "source"
    # var = "UPSTREAM_VAR_NAME"   # for "env"
```

### `env` block types

- `type = "file"` — load the variable named `name` from a file at `path` (one `KEY=VALUE` per line, or single-value file).
- `type = "env"` — copy the variable named `name` from the parent env var `var`.
- `type = "source"` — `source` the shell file at `path` before executing the agent (loads everything the file exports).

## Tier resolution and round-robin

Tiers are ordered. A dispatch request resolves the first tier in `tier_order` that has at least one enabled agent. Within that tier, the CLI rotates across agents in round-robin order, persisted in CLI-managed state.

`--tier ID` forces a specific tier. `--agent ID` forces a specific agent and bypasses tier logic entirely.

## Permission-bypass flags (required for unattended dispatch)

These flags let an agent run tools without prompting. They skip safety checks and are **off by default in init orchestration**. Authoritative table:

| CLI | Flag |
|---|---|
| `claude` | `--dangerously-skip-permissions` |
| `codex` | `--dangerously-bypass-approvals-and-sandbox` |
| `copilot` | `--allow-all` |
| `gemini` | (no standalone flag — use `gemini-npx`; its `--skip-trust` is already baked into the CLI template's `extra_args` and must NOT be re-added) |
| `gemini-npx` | `--skip-trust` is already in the template; do NOT add to `args[]` |
| `opencode` | no known bypass flag at refactor time |

When the user opts into bypass flags during init, the skill appends the matching flag to each agent's `args[]`. Agents with no known flag (e.g. `opencode`) are left alone and the skill logs a note.

## Subcommands

| Command | Behaviour |
|---|---|
| `dispatch-agent config show` | Forwarded. Prints the resolved TOML to stdout. |
| `dispatch-agent config list` | Forwarded. Lists agents and their callable / enabled status. |
| `dispatch-agent config path` | Forwarded. Prints the resolved config path. |
| `dispatch-agent config edit` | **Intercepted by the skill.** Requires a TTY; fails with `Device not configured (os error 6)` inside the Bash tool. |
| `dispatch-agent config` (no action) | **Intercepted by the skill.** Same TTY failure mode. |

## Editing the config

When you ask for `config edit` (or bare `config`), the skill instead:

1. Runs `dispatch-agent config path` to capture the resolved path.
2. Tells you the path and suggests one of:
   - `$EDITOR <path>` in your terminal.
   - The Read/Edit tools in this conversation.

The skill does NOT forward `config edit` to the CLI.
