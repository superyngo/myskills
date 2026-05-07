#!/usr/bin/env python3
"""Detect available agent CLIs. Outputs JSON to stdout."""
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

KNOWN_CLIS = ["claude", "gemini", "codex", "copilot", "opencode"]
TEMPLATES_PATH = Path(__file__).parent.parent / "data" / "cli-templates.toml"


def load_templates() -> dict:
    """Load templates from TOML file. Returns empty dict if file doesn't exist."""
    if not TEMPLATES_PATH.exists():
        return {}
    try:
        # Try Python 3.11+ tomllib first
        import tomllib
        with open(TEMPLATES_PATH, "rb") as f:
            return tomllib.load(f)
    except ImportError:
        # Fallback: parse TOML manually for basic key=value structure
        templates = {}
        try:
            with open(TEMPLATES_PATH, "r") as f:
                current_section = None
                for line in f:
                    line = line.strip()
                    if not line or line.startswith("#"):
                        continue
                    if line.startswith("[") and line.endswith("]"):
                        current_section = line[1:-1]
                        templates[current_section] = {}
                    elif "=" in line and current_section:
                        key, val = line.split("=", 1)
                        key = key.strip()
                        val = val.strip().strip('"\'')
                        # Parse boolean strings
                        if val.lower() == "true":
                            val = True
                        elif val.lower() == "false":
                            val = False
                        templates[current_section][key] = val
        except Exception:
            pass
        return templates


def check_cli(name: str, templates: dict) -> dict:
    path = shutil.which(name)
    if path is None or not os.access(path, os.X_OK):
        return {"path": None, "version": None, "callable": False, "verified": True}

    tmpl = templates.get(name)
    # If no template exists for this CLI, we can't safely determine version or verified status
    if tmpl is None:
        return {"path": path, "version": None, "callable": True, "verified": True}

    verified = tmpl.get("verified", True)
    version_flag = tmpl.get("version_flag", "--version")

    version = None
    if version_flag:
        try:
            result = subprocess.run(
                [name, version_flag],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0 and result.stdout.strip():
                version = result.stdout.strip().splitlines()[0]
        except Exception:
            pass

    return {"path": path, "version": version, "callable": True, "verified": verified}


def main():
    templates = load_templates()
    output = {cli: check_cli(cli, templates) for cli in KNOWN_CLIS}
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
