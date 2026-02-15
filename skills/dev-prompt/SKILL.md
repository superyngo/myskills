---
name: dev-prompt
description: Sets up a development context by combining general best practices with language-specific rules (Python, Rust, JS/TS, etc.). Detects the project language automatically or allows manual selection. Use this skill when starting a new session or project to ensure consistent coding standards.
---

# Development Prompt Setup

This skill configures the current session with a comprehensive set of development guidelines. It combines universal "Clean Code" principles with language-specific best practices.

## Usage

1.  **Detection**: The skill will first attempt to detect the programming language of the current directory.
2.  **Confirmation**: It will ask you to confirm the detected language or select a different one.
3.  **Application**: It will generate the guidelines and output them to the conversation, effectively "priming" the session.
4.  **Persistence**: It will offer to save these guidelines to a file (e.g., `CLAUDE.md`, `.cursorrules`, or `PROJECT_RULES.md`) for future use.

## Instructions for Claude

1.  **Run Detection**: Execute `scripts/detect_language.py` to guess the language.
2.  **Interact**:
    *   If confidence is "high" -> Inform the user: "Detected [Language]. Applying development context." (Ask if they want to switch if it looks wrong, but proceed).
    *   If confidence is "low/medium" or result is null -> Ask the user: "I see this might be a [Detected] project, but I'm not sure. Which context should I apply? (Options: Python, Rust, JavaScript, Scripting, General Only)".
3.  **Compose**: Execute `scripts/compose_prompt.py [language]` (or just `scripts/compose_prompt.py` for General Only).
4.  **Display**: Output the content of the generated prompt in a code block or readable format.
5.  **Persist**: Ask the user: "Would you like me to save these rules to a file (e.g., CLAUDE.md or .cursorrules) for this project?"
    *   If yes, use `Write` tool to save it to the requested filename.

## Available Modules

The following modules are available in `references/`:
-   **base.md**: Universal principles (always included).
-   **python.md**: Python (PEP8, pytest, type hints).
-   **rust.md**: Rust (Clippy, idiomatic patterns).
-   **javascript.md**: JS/TS (Modern features, async/await).
-   **scripting.md**: Shell/PowerShell (Safety, portability).
