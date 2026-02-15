# Dev Prompt Skill Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a `dev-prompt` skill that sets up a development context by combining a base prompt with language-specific rules (Python, Rust, JS/TS, etc.), featuring auto-detection and file generation.

**Architecture:** A standard Claude Skill structure. `SKILL.md` orchestrates the flow. Python scripts handle language detection and prompt composition. Markdown files in `references/` store the modular prompt content.

**Tech Stack:** Python (standard library) for scripts, Markdown for content.

---

### Task 1: Skill Initialization and Structure

**Files:**
- Create: `dev-prompt/SKILL.md`
- Create: `dev-prompt/references/base.md`
- Create: `dev-prompt/references/python.md`
- Create: `dev-prompt/references/rust.md`
- Create: `dev-prompt/references/javascript.md`
- Create: `dev-prompt/references/scripting.md`
- Create: `dev-prompt/scripts/detect_language.py`
- Create: `dev-prompt/scripts/compose_prompt.py`

**Step 1: Create directory structure**

```bash
mkdir -p dev-prompt/scripts dev-prompt/references
```

**Step 2: Create reference files**

Create `dev-prompt/references/base.md` with general coding standards.
Create `dev-prompt/references/python.md` with Python specific rules.
Create `dev-prompt/references/rust.md` with Rust specific rules.
Create `dev-prompt/references/javascript.md` with JS/TS specific rules.
Create `dev-prompt/references/scripting.md` with Shell/PowerShell rules.

**Step 3: Commit**

```bash
git add dev-prompt/references/
git commit -m "feat: add reference prompt modules"
```

### Task 2: Language Detection Script

**Files:**
- Create: `dev-prompt/scripts/detect_language.py`

**Step 1: Write detection logic**

Create `dev-prompt/scripts/detect_language.py`:
- Scan current directory for key files (`requirements.txt`, `Cargo.toml`, `package.json`, `*.py`, `*.rs`, etc.).
- Return a JSON object with `detected_language` (string or null) and `confidence` (high/medium/low).

**Step 2: Test detection**

Run the script in the current directory (should probably return nothing or low confidence if no code files are present).
Create a dummy `test.py` and run it again to verify it detects "python".

**Step 3: Commit**

```bash
git add dev-prompt/scripts/detect_language.py
git commit -m "feat: add language detection script"
```

### Task 3: Prompt Composition Script

**Files:**
- Create: `dev-prompt/scripts/compose_prompt.py`

**Step 1: Write composition logic**

Create `dev-prompt/scripts/compose_prompt.py`:
- Accept a language argument (e.g., "python", "rust").
- Read `references/base.md`.
- Read `references/[language].md` (if exists).
- Return the concatenated string.

**Step 2: Test composition**

Run: `python3 dev-prompt/scripts/compose_prompt.py python`
Expected: Output containing both base rules and Python rules.

**Step 3: Commit**

```bash
git add dev-prompt/scripts/compose_prompt.py
git commit -m "feat: add prompt composition script"
```

### Task 4: SKILL.md Orchestration

**Files:**
- Create: `dev-prompt/SKILL.md`

**Step 1: Write SKILL.md**

Define the skill metadata and instructions:
- Use `detect_language.py` to guess language.
- Ask user to confirm or select if ambiguous.
- Use `compose_prompt.py` to generate the text.
- Output the text to the conversation.
- Offer to save to a file.

**Step 2: Commit**

```bash
git add dev-prompt/SKILL.md
git commit -m "feat: add SKILL.md orchestration"
```

### Task 5: Packaging and Validation

**Files:**
- None (Running existing scripts)

**Step 1: Verify structure**

Ensure all files are in place.

**Step 2: Manual Test Run**

Simulate a user flow:
1. Create a dummy dir with `Cargo.toml`.
2. "Invoke" the logic (manually run scripts).
3. Verify output.

**Step 3: Final Commit**

```bash
git commit --allow-empty -m "chore: complete dev-prompt skill implementation"
```
