#!/usr/bin/env python3
"""Write dispatch-agent TOML config from JSON read on stdin."""
import json
import os
import re
import sys
import tempfile
import tomllib
from pathlib import Path

DEFAULT_MODELS = {
    "claude": "default",
    "gemini": "default",
    "codex": "default",
    "copilot": "sonnet-4.6",
    "opencode": "glm-5.1",
}

_ID_RE = re.compile(r"^[a-zA-Z0-9_-]+$")


def escape_toml_string(s: str) -> str:
    s = s.replace("\\", "\\\\")
    s = s.replace('"', '\\"')
    s = s.replace("\n", "\\n")
    s = s.replace("\t", "\\t")
    return s


def validate_agent_id(agent_id: str) -> None:
    if not _ID_RE.match(agent_id):
        print(f"Error: agent id {agent_id!r} contains invalid chars (allowed: [a-zA-Z0-9_-])", file=sys.stderr)
        sys.exit(1)


def validate_unique_ids(agents: list) -> None:
    seen = set()
    for a in agents:
        if a["id"] in seen:
            print(f"Error: duplicate agent id {a['id']!r}", file=sys.stderr)
            sys.exit(1)
        seen.add(a["id"])


def _toml_str(s: str) -> str:
    return f'"{escape_toml_string(s)}"'


def _toml_str_array(lst: list) -> str:
    items = ", ".join(_toml_str(x) for x in lst)
    return f"[{items}]"


def build_toml(data: dict) -> str:
    agents_by_tier: dict[str, list] = {}
    for tier_id in data["tier_order"]:
        agents_by_tier[tier_id] = []
    for agent in data["agents"]:
        tier = agent["tier"]
        if tier not in agents_by_tier:
            agents_by_tier[tier] = []
        agents_by_tier[tier].append(agent)

    lines = ["version = 1", ""]
    for tier_id in data["tier_order"]:
        lines.append("[[tiers]]")
        lines.append(f'id = {_toml_str(tier_id)}')
        lines.append("")
        for agent in agents_by_tier.get(tier_id, []):
            model = agent.get("model") or DEFAULT_MODELS.get(agent["cli"], "default")
            lines.append("  [[tiers.agents]]")
            lines.append(f'  id = {_toml_str(agent["id"])}')
            lines.append(f'  cli = {_toml_str(agent["cli"])}')
            lines.append(f'  model = {_toml_str(model)}')
            lines.append(f'  args = {_toml_str_array(agent.get("args", []))}')
            for ev in agent.get("env", []):
                lines.append("")
                lines.append("  [[tiers.agents.env]]")
                lines.append(f'  name = {_toml_str(ev["name"])}')
                lines.append(f'  type = {_toml_str(ev["type"])}')
                if ev["type"] == "file":
                    lines.append(f'  path = {_toml_str(ev["path"])}')
                elif ev["type"] == "env":
                    lines.append(f'  var = {_toml_str(ev["var"])}')
            lines.append("")

    return "\n".join(lines)


def write_config(data: dict, dest_path: str) -> None:
    dest = Path(dest_path)
    dest.parent.mkdir(parents=True, exist_ok=True)

    toml_str = build_toml(data)

    try:
        tomllib.loads(toml_str)
    except tomllib.TOMLDecodeError as e:
        print(f"Error: generated TOML failed validation: {e}", file=sys.stderr)
        sys.exit(1)

    with tempfile.NamedTemporaryFile(
        mode="w", dir=dest.parent, suffix=".tmp", delete=False
    ) as tmp:
        tmp.write(toml_str)
        tmp_path = tmp.name

    os.chmod(tmp_path, 0o600)
    os.replace(tmp_path, dest)

    print(str(dest))


def main():
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(f"Error: invalid JSON on stdin: {e}", file=sys.stderr)
        sys.exit(1)

    agents = data.get("agents", [])
    for a in agents:
        validate_agent_id(a["id"])
        if not isinstance(a.get("args", []), list) or not all(isinstance(x, str) for x in a.get("args", [])):
            print(f"Error: agent {a['id']} args must be a list of strings", file=sys.stderr)
            sys.exit(1)
    validate_unique_ids(agents)

    save_location = data.get("save_location", "user")
    if save_location == "project":
        import subprocess
        result = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
        project_root = result.stdout.strip() if result.returncode == 0 else os.getcwd()
        dest = os.path.join(project_root, ".config", "dispatch-agent.toml")
    else:
        dest = os.path.expanduser("~/.config/dispatch-agent.toml")

    write_config(data, dest)


if __name__ == "__main__":
    main()
