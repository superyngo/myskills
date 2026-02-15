# Scripting & Shell Guidelines

## General
-   **Shebang**: Always include a shebang line (e.g., `#!/bin/bash` or `#!/usr/bin/env python3`).
-   **Portability**: Prefer POSIX-compliant syntax when possible for shell scripts.

## Versioning & Metadata
-   **Version Header**: Always define a `VERSION` variable at the top of the script.
    -   Example: `VERSION="1.0.0"`
-   **Auto-increment**: Increment the version number (SemVer recommended) upon every modification or feature addition.
-   **Change Log**: Briefly comment or log version changes near the header if a full changelog is not available.

## Interface & Localization
-   **Help Message**: Implement a `-h` / `--help` flag that prints usage instructions AND the current version info.
-   **Localization**: Support modular output messages.
    -   Default to **English**.
    -   Support a `-l` / `--language` flag (e.g., `-l zh-TW` or `-l cn`) to switch output to Chinese (or other languages).
    -   Use a message function or dictionary/associative array to handle strings.

## Bash/Shell
-   **Safety**: Start scripts with `set -euo pipefail` to fail on errors, unset variables, and pipe failures.
-   **ShellCheck**: Validate scripts with ShellCheck.
-   **Quoting**: Quote all variable expansions (`"$VAR"`) to prevent word splitting and globbing issues.
-   **Naming**: Use `SCREAMING_SNAKE_CASE` for environment variables and `snake_case` for local variables.
-   **Functions**: Use functions to modularize code.

## PowerShell
-   **Naming**: Use `Verb-Noun` convention for functions.
-   **Parameters**: Use `[CmdletBinding()]` and typed `param()` blocks.
-   **Error Handling**: Use `try...catch` blocks.
-   **Output**: Return objects, not text, whenever possible.

## Windows CMD (Batch)
-   **Avoid**: Avoid using Batch files for complex logic. Use PowerShell or Python instead.
-   **Echo**: Start with `@echo off`.
-   **Variables**: Use `setlocal enabledelayedexpansion` for loops.
