# Update Scripting Guidelines Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Update `dev-prompt/references/scripting.md` to include guidelines for versioning, help messages, and localization support in scripts.

**Architecture:** Modify the existing Markdown file to add new sections and update existing ones.

**Tech Stack:** Markdown.

---

### Task 1: Update Scripting Guidelines

**Files:**
- Modify: `dev-prompt/references/scripting.md`

**Step 1: Read existing content**

Read `dev-prompt/references/scripting.md` to understand current structure.

**Step 2: Rewrite file with new requirements**

Update `dev-prompt/references/scripting.md` to include:
- **Versioning**: Instructions to define a `VERSION` variable at the top and auto-increment strategies (e.g., git hooks or manual).
- **Help Message**: Instructions to implement a usage function that prints version info.
- **Localization**: Instructions to support `-l/--language` for output messages, defaulting to English.

**Step 3: Commit**

```bash
git add dev-prompt/references/scripting.md
git commit -m "docs: update scripting guidelines with versioning and localization"
```
