---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | detect | config <show|list|path|edit> | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---

# dispatch-agent

Thin shell over the `dispatch-agent` CLI (https://github.com/superyngo/dispatch-agent). The skill detects the binary, guides install when missing, orchestrates two subcommands that need pre-CLI work (`init`, `dispatch` without prompt), intercepts TTY-requiring `config` subcommands, and forwards everything else verbatim. All subcommand knowledge lives in `references/`.

## 1. Detect the CLI

Run:

```bash
command -v dispatch-agent >/dev/null 2>&1
```

If it succeeds, skip to **Section 3 — Route**.

## 2. Install when missing

1. Load `references/install-guide.md` (Read the file in full — the user may need to see the repo link or the manual command).
2. Ask the user via `AskUserQuestion`:
   - **Install (user)** — non-elevated install for the current account.
   - **Install (system)** — requires Administrator (Windows) or sudo (Linux/macOS).
   - **Show instructions only** — print the one-liner, do nothing.
   - **Cancel** — abort the skill.
3. Detect OS for command selection:
   - macOS / Linux: `uname -s` returns `Darwin` or `Linux`.
   - Windows: `uname` absent, or `$OS == "Windows_NT"`.
4. Execute the matching one-liner from `install-guide.md` via Bash.
5. After install, refresh the shell cache and re-detect:

   ```bash
   hash -r 2>/dev/null || rehash 2>/dev/null || true
   command -v dispatch-agent
   ```

6. On success: run `dispatch-agent detect` and show the user which agent CLIs are ready.
7. On install failure (non-zero exit, declined elevation, network error): print the system-install command verbatim from `install-guide.md` and stop. The user runs it manually and re-invokes the skill.

The skill must not claim success unless the second `command -v` returns 0.

## 3. Route

Inspect the first non-flag argv token.

### `init` — orchestrated

The CLI's `init` reads a JSON payload from stdin. The skill assembles it:

1. Load `references/init-guide.md`.
2. Run `dispatch-agent detect`, parse the JSON.
3. **Zero-agent abort:** if no agent has `callable == true`, tell the user no agent CLI is installed, suggest installing one, and stop. Do not call `init`.
4. Build a default payload from callable agents:

   ```json
   {
     "save_location": "user",
     "agents": [
       { "id": "<cli>-default", "cli": "<cli>", "model": "default",
         "args": [], "tier": "primary", "env": [] }
     ],
     "tier_order": ["primary"]
   }
   ```

   Non-callable agents are shown to the user in the next step as opt-in only.
5. `AskUserQuestion` to confirm:
   - `save_location`: user vs project.
   - Which agents to include (callable ones pre-selected; non-callable shown unchecked).
   - **Permission-bypass flags** — off by default, with the explicit risk note: "These flags (e.g. `--dangerously-skip-permissions`) let the agent run tools without prompting — required for unattended dispatch but they skip safety checks." Options: **Off (safe default)** / **On (I understand the risk)**.
     - On: append the matching flag to each agent's `args[]` per the table in `references/config-guide.md`. Skip agents with no known bypass flag (currently `opencode`); log a note.
6. **Overwrite check** (runs AFTER step 5 because it needs the chosen `save_location`):
   - Compute target path:
     - `user` → `~/.config/dispatch-agent.toml`.
     - `project` → `$(git rev-parse --show-toplevel 2>/dev/null || pwd)/.config/dispatch-agent.toml`.
   - If the file exists, `AskUserQuestion` Overwrite / Backup first / Cancel.
     - **Backup:** `cp "<path>" "<path>.bak.$(date -u +%Y%m%dT%H%M%SZ)"` before continuing.
     - **Cancel:** stop here.
7. Pipe the JSON:

   ```bash
   printf '%s' "<json>" | dispatch-agent init
   ```

   If the user supplied `--config PATH` in argv, **strip it** before forwarding — `--config` does not redirect init output. Warn the user once that `--config` was ignored.
8. Run `dispatch-agent config show` and surface the resulting file. If bypass flags were appended, also run `dispatch-agent dispatch --dry-run` to confirm the command shape.

### `detect`

Load `references/detect-guide.md`, then run:

```bash
dispatch-agent detect
```

Pass `--config PATH` through verbatim if supplied.

### `config edit` and bare `config` — intercepted

Both require a TTY and fail with `Device not configured (os error 6)` inside the Bash tool. Do NOT forward.

1. Load `references/config-guide.md`.
2. Run `dispatch-agent config path` and capture stdout.
3. Tell the user: "Edit this file in your terminal: `$EDITOR <path>`, or with the Read/Edit tools."

### `config <show|list|path>`

Load `references/config-guide.md`, then forward:

```bash
dispatch-agent config <sub> [--config PATH]
```

### Dispatch with `-p` or `-f`

Load `references/dispatch-guide.md`, then forward verbatim:

```bash
dispatch-agent dispatch <argv>
```

### Dispatch without `-p` / `-f` and without `--dry-run` — prompt collection

The CLI would enter interactive mode and hang. Prevent this:

1. Load `references/dispatch-guide.md`.
2. `AskUserQuestion` to collect prompt text.
3. If the response is empty or whitespace-only, re-ask once with a hint. If still empty, abort with a clear message. Do NOT forward.
4. Forward as:

   ```bash
   dispatch-agent dispatch -p "<collected>" <rest-of-argv>
   ```

### Dispatch with `--dry-run` and no prompt

Forward as-is. The CLI prints the resolved command template with a literal `<prompt>` placeholder. No error, no hang.

### `--help`

1. Run `dispatch-agent --help` and surface the output.
2. Load `references/dispatch-guide.md` for skill-level notes (install, init, etc.).

## 4. CLI errors

On non-zero CLI exit: print stderr unchanged, then load the matching reference file for the route as troubleshooting context. Do not classify exit codes — upstream semantics are not documented.

## 5. `--config PATH` rules

- Passed through verbatim at subcommand level for every forwarded route.
- **Stripped** before forwarding `dispatch-agent init`, with a one-time warning to the user (init's output location is controlled only by JSON `save_location`).
