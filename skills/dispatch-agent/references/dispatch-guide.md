# `dispatch-agent dispatch` — flag reference and examples

Loaded by the skill when it routes a dispatch invocation (the user supplied `-p` / `-f`, or the skill collected a prompt via AskUserQuestion). Also loaded for `--help` and for troubleshooting after a non-zero CLI exit.

## Flag reference

| Flag | Meaning |
|---|---|
| `-p <prompt>` | Inline prompt string. |
| `-f <file>` | Read prompt from a file. Contents become the prompt. |
| `--timeout <N>` | Per-agent timeout in seconds. `-1` (default) = no timeout. |
| `--tier <ID>` | Force a specific tier; skip the normal `tier_order` resolution. |
| `--agent <ID>` | Force a specific agent by `id`; bypass tier logic entirely. |
| `--config <PATH>` | Use a specific config file instead of the resolved default. |
| `--dry-run` | Print the resolved command without running the agent. No agent invocation happens. |
| `--verbose` | Extra logging on stderr. |

`-p` and `-f` are mutually exclusive in practice. Without either flag (and without `--dry-run`), `dispatch` enters interactive mode and will hang inside the Bash tool. The skill prevents this by collecting a prompt via AskUserQuestion before forwarding.

## How the skill routes you here

| Argv shape | Skill action |
|---|---|
| `-p "..."` or `-f path` supplied | Forward as-is. |
| Neither, and not `--dry-run` | Collect prompt via AskUserQuestion; re-prompt on empty input; abort if still empty. Then forward with `-p`. |
| `--dry-run` only | Forward as-is. CLI prints the command template with a literal `<prompt>` placeholder. Safe — no hang. |

## Examples

```bash
# Simplest: inline prompt, default tier rotation.
dispatch-agent dispatch -p "Summarise this PR diff."

# From a file.
dispatch-agent dispatch -f /tmp/prompt.md

# Force a specific agent.
dispatch-agent dispatch --agent claude-default -p "..."

# Force a tier (e.g. fallback tier).
dispatch-agent dispatch --tier secondary -p "..."

# Inspect the resolved command without running.
dispatch-agent dispatch --dry-run -p "..."

# Use a one-off config.
dispatch-agent dispatch --config /tmp/test.toml -p "..."
```

## Troubleshooting

| Symptom | Likely cause | Where to look |
|---|---|---|
| `agent not found` / unexpected agent selected | Config tier order or agent enabled flag. | `dispatch-agent config show`; see config-guide.md. |
| Agent stalls / no output | Missing permission-bypass flag for that CLI. | config-guide.md "Permission-bypass flags" table. |
| Detect lists an agent but dispatch reports it as unavailable | `verified == false` in the upstream template, or `callable == false`. | `dispatch-agent detect`; see detect-guide.md. |
| `Device not configured (os error 6)` | A `config` subcommand was forwarded that requires a TTY. | This shouldn't happen via the skill — it intercepts `config edit` and bare `config`. If you see it, you ran the CLI directly. |
| `invalid JSON: EOF` from `init` | `init` was invoked with no JSON on stdin. | Use the skill's `init` route, which pipes a JSON payload. |

See `config-guide.md` for the TOML schema, tier semantics, and env injection. See `detect-guide.md` for the JSON output of `detect`.
