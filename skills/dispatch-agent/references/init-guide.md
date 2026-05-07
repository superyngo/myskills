# dispatch-agent Init Guide

Follow this guide when no config is found or the user invokes `init`.

---

## Prerequisites

```bash
python3 --version  # must be 3.11+
python3 scripts/detect.py  # must return valid JSON
```

---

## Step 1: Detect CLIs

```bash
python3 scripts/detect.py
```

Display results to user. **Important:** explicitly tell the user:
> "CLIs marked `verified: false` will be skipped at dispatch even if added to config, because their non-interactive mode is unverified."

---

## Step 2: Existing Config Handling

Check for existing config at:
- `<git-root>/.config/dispatch-agent.toml`
- `~/.config/dispatch-agent.toml`

If found, use `AskUserQuestion`:

```
Question: "A config already exists at <path>. What would you like to do?"
Options:
  - "Overwrite" — delete existing, write new
  - "Backup first" — rename to dispatch-agent.toml.bak, then write new
  - "Cancel" — abort init
```

---

## Step 3: Per-CLI Configuration

For each CLI that is callable AND verified (skip unverified), ask **one at a time**:

```
Question: "For <cli>: set a custom agent id? (default: <cli>-default)"
Options:
  - "Use default: <cli>-default"
  - (Other: let user type custom id matching [a-zA-Z0-9_-])
```

```
Question: "For <cli>: specify extra args? (default: none)"
Options:
  - "No extra args"
  - (Other: comma-separated args, e.g. --no-stream,--debug)
```

```
Question: "For <cli>: need env vars? (default: none)"
Options:
  - "No env vars needed"
  - "Add env var from file (type=file)"
  - "Forward env var from shell (type=env)"
```

If user adds env var, collect:
- `name`: the env var name (e.g. GITHUB_TOKEN)
- `type`: file or env
- `path` (if file): path to token file
- `var` (if env): name of the source env var

Pre-fill model using defaults:
| CLI | Default model |
|-----|--------------|
| claude | default |
| gemini | default |
| gemini-npx | default |
| codex | default |
| copilot | sonnet-4.6 |
| opencode | zai-coding-plan/glm-5.1 |

> **Note:** `gemini-npx` uses `npx` as the underlying binary with `@google/gemini-cli@latest`.
> Set `cli = "npx"` in your config when using this template. Agent args (e.g. `["--thinking"]`)
> are appended after the model flag for `opencode`.

---

## Step 4: Tier Assignment

```
Question: "How many tiers do you want? (e.g. 2: primary + fallback)"
Options: "1", "2", "3", (Other)
```

For each tier, collect a name (e.g. "primary", "fallback") then:

```
Question: "Which agents go in tier '<name>'? (in priority order)"
Options: show all configured agent ids as checkboxes
```

---

## Step 5: Save Location

```
Question: "Save config to:"
Options:
  - "User (~/.config/dispatch-agent.toml)" — shared across all projects
  - "Project (<git-root>/.config/dispatch-agent.toml)" — project-specific
```

---

## Step 6: Write Config

Build the JSON input and pipe to init.py:

```python
payload = {
  "agents": [...],          # collected above
  "tier_order": [...],      # tier names in order
  "save_location": "user" | "project"
}
```

```bash
echo '<json>' | python3 scripts/init.py
```

If init.py exits non-0: show stderr to user, offer to retry from Step 3.
On success: init.py prints the config path — confirm to user with permissions note (0600).

---

## Edge Cases

- **No callable CLIs detected:** inform user, suggest installing at least one CLI from the Default Platforms list.
- **All CLIs unverified:** same as above — no agents can be configured.
- **Duplicate agent ids:** validate before calling init.py; prompt user to choose a different id.
- **env file not found during init:** warn user but allow proceeding — dispatch.py will skip the var at runtime.
