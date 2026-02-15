#!/usr/bin/env python3
import os
import json
import sys
from pathlib import Path

def detect_language():
    """
    Scans the current directory for specific files to detect the programming language.
    Returns a JSON object with 'detected_language' and 'confidence'.
    """
    cwd = Path.cwd()

    # Rules: (language_key, [filenames], [extensions])
    rules = [
        ("rust", ["Cargo.toml"], [".rs"]),
        ("python", ["requirements.txt", "pyproject.toml", "Pipfile", "setup.py"], [".py"]),
        ("javascript", ["package.json", "tsconfig.json", "jsconfig.json"], [".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"]),
        ("scripting", [], [".sh", ".ps1", ".bat", ".cmd"]),
    ]

    scores = {lang: 0 for lang, _, _ in rules}

    # Check for specific configuration files (Strong signal)
    for lang, files, _ in rules:
        for filename in files:
            if (cwd / filename).exists():
                scores[lang] += 10

    # Check for file extensions (Weak signal, accumulative)
    # Limit search to top-level and one level deep to save time/resources
    # and avoid checking node_modules or venv
    try:
        for file in cwd.glob("*"):
            if file.is_file():
                for lang, _, extensions in rules:
                    if file.suffix in extensions:
                        scores[lang] += 2
            elif file.is_dir() and not file.name.startswith("."):
                # Shallow check in subdirs
                try:
                    for subfile in file.glob("*"):
                         if subfile.is_file():
                            for lang, _, extensions in rules:
                                if subfile.suffix in extensions:
                                    scores[lang] += 1
                except (PermissionError, OSError):
                    pass
    except (PermissionError, OSError):
        pass

    # Determine winner
    best_lang = None
    max_score = 0

    for lang, score in scores.items():
        if score > max_score:
            max_score = score
            best_lang = lang

    # Calculate confidence
    confidence = "low"
    if max_score >= 10:
        confidence = "high"
    elif max_score >= 4:
        confidence = "medium"

    result = {
        "detected_language": best_lang,
        "confidence": confidence,
        "details": scores
    }

    return result

if __name__ == "__main__":
    print(json.dumps(detect_language(), indent=2))
