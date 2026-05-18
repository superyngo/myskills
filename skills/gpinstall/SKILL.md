---
name: gpinstall
description: >
  Generate README installation instructions using the gpinstall generic GitHub release
  installer scripts (gpinstall.sh / gpinstall.ps1). Use this skill whenever the user
  asks to add installation docs, write an "Installation" section, or document how to
  install a GitHub-released project using these scripts. Triggers on phrases like
  "add install instructions", "write README install section", "document installation
  with gpinstall", or when the user shares a project name/repo and wants install docs.
---

# gpinstall README Skill

Generate a complete, copy-pasteable **Installation** section for a project README using
the generic gpinstall scripts hosted on GitHub Gist.

## Script URLs

| Platform | Script |
|----------|--------|
| Linux / macOS (bash) | `https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh` |
| Windows (PowerShell) | `https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1` |

## Required Variables

Gather these from the current project context before generating:

| Variable | Description | Example |
|----------|-------------|---------|
| `APP_NAME` | The binary / app name | `wedi` |
| `REPO` | GitHub `owner/repo` | `superyngo/wedi` |

## Output Template

Produce the following Markdown block, substituting `APP_NAME` and `REPO`:

````markdown
## Installation

### Linux / macOS

```bash
# Install
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="<APP_NAME>" REPO="<REPO>" bash

# Uninstall
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="<APP_NAME>" REPO="<REPO>" bash -s uninstall

# System-wide install (requires root)
sudo -E bash -c 'curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="<APP_NAME>" REPO="<REPO>" bash'
```

### Windows (PowerShell)

```powershell
# Install
$env:APP_NAME="<APP_NAME>"; $env:REPO="<REPO>"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex

# Uninstall
$env:APP_NAME="<APP_NAME>"; $env:REPO="<REPO>"; $env:UNINSTALL="true"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex

# System-wide install (requires Administrator)
Start-Process pwsh -Verb RunAs -ArgumentList "-NoExit","-Command","`$env:APP_NAME='<APP_NAME>'; `$env:REPO='<REPO>'; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex"
```
````

## Rules

1. **Always substitute** `<APP_NAME>` and `<REPO>` with the actual project values. Never output placeholder text.
2. **Include all three variants** per platform: install, uninstall, and system-wide install.
3. **Use fenced code blocks** with the correct language tag (`bash` / `powershell`).
4. Place the section under an `## Installation` heading unless the user specifies otherwise.
5. If `APP_NAME` or `REPO` cannot be inferred from context, ask the user before generating.