# Installing the agd CLI

This file is loaded by the `agent-dispatcher` skill when the binary is not on `PATH`.

- **Repository:** https://github.com/superyngo/agd
- **README:** https://raw.githubusercontent.com/superyngo/agd/refs/heads/main/README.md
- **Tested against upstream main as of 2026-05-20.** The CLI has a `-V` / `--version` flag; verify presence with `agd --version`.

When the skill cannot find `agd` on `PATH`, it asks the user to pick one of: **Install (user)**, **Install (system)**, **Show instructions only**, or **Cancel**. On a chosen install it runs the matching one-liner below via the Bash tool, refreshes the shell cache (`hash -r || rehash || true`), and re-detects. If install fails or elevation is declined, the skill prints the system command verbatim for the user to run manually and stops.

## Windows (PowerShell)

User install:

```powershell
$env:APP_NAME="agd"; $env:REPO="superyngo/agd"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.ps1 | iex
```

User uninstall:

```powershell
$env:APP_NAME="agd"; $env:REPO="superyngo/agd"; $env:UNINSTALL="true"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.ps1 | iex
```

System install (Administrator):

```powershell
Start-Process pwsh -Verb RunAs -ArgumentList "-NoExit","-Command","`$env:APP_NAME='agd'; `$env:REPO='superyngo/agd'; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.ps1 | iex"
```

## Linux / macOS (Bash)

User install:

```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.sh \
  | APP_NAME="agd" REPO="superyngo/agd" bash
```

User uninstall:

```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.sh \
  | APP_NAME="agd" REPO="superyngo/agd" bash -s uninstall
```

System install (root):

```bash
sudo -E bash -c 'curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.sh \
  | APP_NAME="agd" REPO="superyngo/agd" bash'
```
