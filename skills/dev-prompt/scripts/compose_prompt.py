#!/usr/bin/env python3
import sys
import os
from pathlib import Path

def compose_prompt(language=None):
    """
    Reads references/base.md and references/[language].md and returns the combined prompt.
    """
    # Determine the directory where the script is located to find references/ relative to it
    script_dir = Path(__file__).resolve().parent
    references_dir = script_dir.parent / "references"

    combined_content = ""

    # 1. Read Base Rules
    base_file = references_dir / "base.md"
    if base_file.exists():
        try:
            with open(base_file, "r", encoding="utf-8") as f:
                combined_content += f.read() + "\n\n"
        except Exception as e:
            print(f"Error reading base.md: {e}", file=sys.stderr)
            return ""
    else:
        print(f"Warning: base.md not found at {base_file}", file=sys.stderr)

    # 2. Read Language Rules
    if language:
        lang_file = references_dir / f"{language}.md"
        if lang_file.exists():
            try:
                with open(lang_file, "r", encoding="utf-8") as f:
                    combined_content += f.read() + "\n\n"
            except Exception as e:
                print(f"Error reading {language}.md: {e}", file=sys.stderr)
        else:
            # It's okay if a specific language file doesn't exist, we just skip it
            # But maybe we should warn?
            pass

    return combined_content.strip()

if __name__ == "__main__":
    language_arg = sys.argv[1] if len(sys.argv) > 1 else None
    prompt = compose_prompt(language_arg)
    print(prompt)
