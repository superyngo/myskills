# `dispatch-agent init` — JSON stdin schema and skill orchestration

Loaded by the skill when the user invokes `init`. The CLI's `init` subcommand reads a JSON payload from stdin and writes a TOML config to either `~/.config/dispatch-agent.toml` (`save_location: "user"`) or `<git-root>/.config/dispatch-agent.toml` (`save_location: "project"`).

**The CLI overwrites existing config without warning.** The skill's orchestration adds an overwrite check (Backup / Overwrite / Cancel) before piping. Users who run `dispatch-agent init` directly, outside the skill, do NOT get this guard.

## JSON schema

```json
{
  "save_location": "user",
  "agents": [
    {
      "id": "claude-default",
      "cli": "claude",
      "model": "default",
      "args": [],
      "tier": "primary",
      "env": []
    }
  ],
  "tier_order": ["primary"]
}
```

### Field reference

| Field | Type | Notes |
|---|---|---|
| `save_location` | `"user"` \| `"project"` | Only control over output file. `--config PATH` does NOT redirect init output. |
| `agents[].id` | string | One or more of: ASCII letter, digit, underscore, hyphen. Must be unique across all tiers. |
| `agents[].cli` | string | Should match a `dispatch-agent detect` output key. **The CLI does NOT validate** — any string is accepted. |
| `agents[].model` | string | Concrete model name, or `"default"`. The CLI resolves `"default"` to a concrete model in the output TOML (see resolution table below). |
| `agents[].args` | string[] | Extra args appended to the agent invocation. Init seeds this as `[]`; users may opt into permission-bypass flags during orchestration. |
| `agents[].tier` | string | Must appear in `tier_order`. |
| `agents[].env` | object[] | Optional env injection entries; see config-guide.md for `file` / `env` / `source` types. |
| `tier_order` | string[] | Order in which tiers are tried during dispatch. |

### Default-model resolution (observed at refactor time)

These are the values the CLI substitutes when input is `"default"`. They come from upstream and may change.

| Input `cli` | Resolved `model` |
|---|---|
| `claude` | `claude-sonnet-4-5` |
| `gemini` | `gemini-2.0-flash` |
| `codex` | (record what `config show` reports after init) |
| `copilot` | (record what `config show` reports after init) |
| `opencode` | (record what `config show` reports after init) |

Implementers should fill in observed values when running init verification for the first time.

## Skill orchestration (what happens when you run `init` via the skill)

1. Load this guide.
2. Run `dispatch-agent detect`, parse the JSON.
3. **Zero-agent abort:** if no agent has `callable == true`, the skill informs you and aborts without writing anything. Install an agent CLI first.
4. Build a default JSON payload: one entry per callable agent, `id = "<cli>-default"`, `model = "default"`, `args = []`, `tier = "primary"`. Non-callable agents are listed in the confirmation step as opt-in choices.
5. Ask you to confirm:
   - `save_location`: user or project.
   - which agents to include (callable ones pre-selected).
   - **permission-bypass flags** (off by default, with an explicit risk note). On selection: append the matching flag from config-guide.md's table to each agent's `args[]`.
6. **Overwrite check** (runs after step 5 because it needs the chosen `save_location`):
   - Compute target path: `~/.config/dispatch-agent.toml` for user, `<git-root>/.config/dispatch-agent.toml` for project (`git rev-parse --show-toplevel`, fall back to cwd).
   - If the file exists: ask Overwrite / Backup first / Cancel.
   - Backup copies to `<path>.bak.<UTC-timestamp>` with format `YYYYMMDDTHHMMSSZ` (e.g. `20260519T143000Z`).
7. Pipe the JSON via stdin: `printf '%s' "<json>" | dispatch-agent init`.
   - `--config PATH` in argv is **stripped** before forwarding (it does not redirect init output). The skill warns you once if you supplied it.
8. Run `dispatch-agent config show` and surface the resulting file.
   - If you opted into bypass flags, the skill also runs `dispatch-agent dispatch --dry-run` to show the resulting command shape.

## Tuning after init

Init seeds a minimal config (no extra `args` beyond bypass flags, no `env`). To add more args or env injection:

- Run `dispatch-agent config path` to get the config location.
- Edit the file directly with `$EDITOR <path>` in your terminal, or with Read/Edit tools.
- See `config-guide.md` for the TOML schema, tier semantics, and env block types.

The skill's `config edit` route does this for you (prints the path and edit guidance).

## Multi-tier setups

Init seeds a single `"primary"` tier. Multi-tier setups (e.g. `primary` falls back to `secondary`) are not generated automatically — add additional `[[tiers]]` blocks manually in the config after init.
