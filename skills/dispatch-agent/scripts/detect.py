#!/usr/bin/env python3
"""Detect available agent CLIs. Outputs JSON to stdout."""
import json
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

KNOWN_CLIS = ["claude", "gemini", "gemini-npx", "codex", "copilot", "opencode"]
TEMPLATES_PATH = Path(__file__).parent.parent / "data" / "cli-templates.toml"


def load_templates() -> dict:
    """Load templates from TOML file. Returns empty dict if file doesn't exist."""
    if not TEMPLATES_PATH.exists():
        return {}
    with open(TEMPLATES_PATH, "rb") as f:
        return tomllib.load(f)


def check_cli(name: str, templates: dict) -> dict:
    tmpl = templates.get(name)
    detect_binary = tmpl.get("detect_binary", name) if tmpl else name

    path = shutil.which(detect_binary)
    if path is None or not os.access(path, os.X_OK):
        return {"path": None, "version": None, "callable": False, "verified": True}

    if tmpl is None:
        return {"path": path, "version": None, "callable": True, "verified": True}

    verified = tmpl.get("verified", True)
    version_flag = tmpl.get("version_flag", "--version")

    version = None
    if version_flag:
        try:
            result = subprocess.run(
                [detect_binary, version_flag],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0 and result.stdout.strip():
                version = result.stdout.strip().splitlines()[0]
        except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
            pass

    return {"path": path, "version": version, "callable": True, "verified": verified}


def main():
    templates = load_templates()
    output = {cli: check_cli(cli, templates) for cli in KNOWN_CLIS}
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
