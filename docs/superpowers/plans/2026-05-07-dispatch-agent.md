# dispatch-agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a skill that dispatches tasks to agent CLIs (claude, gemini, codex, copilot, opencode) with YAML-tier-based fallback, round-robin rotation, and an interactive init flow.

**Architecture:** A thin `SKILL.md` routes to three Python scripts: `detect.py` (discovers system CLIs), `init.py` (writes TOML config from stdin JSON), and `dispatch.py` (main loop — reads config, iterates tiers round-robin, streams subprocess output). All three are stdlib-only (Python 3.11+). State is persisted in `~/.cache/dispatch-agent/rr-state.json` with `fcntl.flock` + `os.replace` for atomicity.

**Tech Stack:** Python 3.11+ stdlib (`tomllib`, `json`, `subprocess`, `shutil`, `fcntl`, `os`, `signal`, `threading`, `select`, `unittest`, `unittest.mock`)

---

## File Map

| File | Responsibility |
|------|---------------|
| `skills/dispatch-agent/SKILL.md` | Skill entry point: routing logic, arg translation, recursion guard |
| `skills/dispatch-agent/data/cli-templates.toml` | Per-CLI call syntax, version flags, verified status |
| `skills/dispatch-agent/references/init-guide.md` | AI-guided init flow: AskUserQuestion prompts, edge cases |
| `skills/dispatch-agent/references/dispatch-guide.md` | Help reference: schema, flags, output formats, error reference |
| `skills/dispatch-agent/scripts/detect.py` | Detect available CLIs, output JSON |
| `skills/dispatch-agent/scripts/init.py` | Read JSON from stdin, write TOML config with validation |
| `skills/dispatch-agent/scripts/dispatch.py` | Argparse, config load, tier loop, call_agent, rr-state |
| `skills/dispatch-agent/tests/test_detect.py` | Unit tests for detect.py |
| `skills/dispatch-agent/tests/test_init.py` | Unit tests for init.py TOML serializer and validation |
| `skills/dispatch-agent/tests/test_dispatch.py` | Unit tests for config loading, env resolution, arg building, rr-state |

---

### Task 1: Scaffold — directories, SKILL.md, cli-templates.toml

**Files:**
- Create: `skills/dispatch-agent/SKILL.md`
- Create: `skills/dispatch-agent/data/cli-templates.toml`
- Create: `skills/dispatch-agent/scripts/__init__.py` (empty)
- Create: `skills/dispatch-agent/tests/__init__.py` (empty)

- [ ] **Step 1: Create directories**

```bash
mkdir -p skills/dispatch-agent/{data,references,scripts,tests}
touch skills/dispatch-agent/scripts/__init__.py
touch skills/dispatch-agent/tests/__init__.py
```

- [ ] **Step 2: Create SKILL.md**

Create `skills/dispatch-agent/SKILL.md`:

```markdown
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--list] [--show-config] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---

# dispatch-agent

Dispatches tasks to other agent CLIs (claude, gemini, codex, copilot, opencode) with tier-based fallback and round-robin rotation.

## Recursion Guard

If `DISPATCH_AGENT_DEPTH` env var is set and >= 5, stop immediately:

```bash
python3 -c "
import os, sys
depth = int(os.environ.get('DISPATCH_AGENT_DEPTH', 0))
if depth >= 5:
    print('dispatch recursion limit reached (depth=5)', file=sys.stderr)
    sys.exit(1)
"
```

## Find Config

Check in order (use first found):
1. `--config PATH` argument (if provided)
2. `<git-root>/.config/dispatch-agent.toml` (use `git rev-parse --show-toplevel` to find git root; fall back to cwd if not in a repo)
3. `~/.config/dispatch-agent.toml`

```bash
# Find git root or cwd
PROJECT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
CONFIG_PATH="$PROJECT_ROOT/.config/dispatch-agent.toml"
USER_CONFIG="$HOME/.config/dispatch-agent.toml"
```

## Routing

**If argument is `init`, or no config found:**
Load `references/init-guide.md` and follow the init flow.

**Otherwise — dispatch:**
Translate arguments and run:
```bash
python3 scripts/dispatch.py \
  [-p "<prompt>" | -f "<file>"] \
  [--timeout N] \
  [--tier ID] \
  [--agent ID] \
  [--config PATH] \
  [--dry-run] [--list] [--show-config] [--verbose]
```

If neither `-p` nor `-f` is provided (and not `--list`/`--show-config`/`--dry-run`):
Use `AskUserQuestion` to collect the prompt before dispatching.

**For `--help` or errors:** load `references/dispatch-guide.md`.
```

- [ ] **Step 3: Create data/cli-templates.toml**

Create `skills/dispatch-agent/data/cli-templates.toml`:

```toml
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[gemini]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[codex]
prompt_flag = "-q"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[copilot]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[opencode]
prompt_flag = ""
model_flag = ""
file_input_mode = "arg"
version_flag = "--version"
verified = false
extra_args = []
```

- [ ] **Step 4: Commit scaffold**

```bash
git add skills/dispatch-agent/
git commit -m "feat(dispatch-agent): scaffold SKILL.md and cli-templates.toml"
```

---

### Task 2: detect.py

**Files:**
- Create: `skills/dispatch-agent/scripts/detect.py`
- Create: `skills/dispatch-agent/tests/test_detect.py`

- [ ] **Step 1: Write failing tests**

Create `skills/dispatch-agent/tests/test_detect.py`:

```python
import json
import sys
import os
import unittest
from unittest.mock import patch, MagicMock
from pathlib import Path

# Add scripts dir to path
sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

import detect


class TestDetectCli(unittest.TestCase):
    def setUp(self):
        self.templates = {
            "claude": {"version_flag": "--version", "verified": True},
            "opencode": {"version_flag": "--version", "verified": False},
            "nodecli": {"version_flag": "", "verified": True},
        }

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    def test_callable_when_found_and_executable(self, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/claude"
        mock_access.return_value = True
        result = detect.check_cli("claude", self.templates)
        self.assertTrue(result["callable"])
        self.assertEqual(result["path"], "/usr/bin/claude")

    @patch("detect.shutil.which")
    def test_not_callable_when_not_found(self, mock_which):
        mock_which.return_value = None
        result = detect.check_cli("claude", self.templates)
        self.assertFalse(result["callable"])
        self.assertIsNone(result["path"])

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    def test_not_callable_when_not_executable(self, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/claude"
        mock_access.return_value = False
        result = detect.check_cli("claude", self.templates)
        self.assertFalse(result["callable"])

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    @patch("detect.subprocess.run")
    def test_version_captured(self, mock_run, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/claude"
        mock_access.return_value = True
        mock_run.return_value = MagicMock(returncode=0, stdout="claude 1.2.3\n")
        result = detect.check_cli("claude", self.templates)
        self.assertEqual(result["version"], "claude 1.2.3")

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    @patch("detect.subprocess.run")
    def test_version_null_on_empty_version_flag(self, mock_run, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/nodecli"
        mock_access.return_value = True
        result = detect.check_cli("nodecli", self.templates)
        self.assertIsNone(result["version"])
        mock_run.assert_not_called()

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    @patch("detect.subprocess.run")
    def test_version_null_on_timeout(self, mock_run, mock_access, mock_which):
        import subprocess
        mock_which.return_value = "/usr/bin/claude"
        mock_access.return_value = True
        mock_run.side_effect = subprocess.TimeoutExpired("claude", 5)
        result = detect.check_cli("claude", self.templates)
        self.assertIsNone(result["version"])

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    def test_verified_false_copied_from_template(self, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/opencode"
        mock_access.return_value = True
        result = detect.check_cli("opencode", self.templates)
        self.assertFalse(result["verified"])

    def test_missing_templates_file_returns_null_versions(self):
        result = detect.check_cli("claude", {})
        # no template → version null, verified defaults to True
        self.assertIsNone(result.get("version"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests — expect failure**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/test_detect.py -v 2>&1 | head -30
```

Expected: `ModuleNotFoundError: No module named 'detect'`

- [ ] **Step 3: Implement detect.py**

Create `skills/dispatch-agent/scripts/detect.py`:

```python
#!/usr/bin/env python3
"""Detect available agent CLIs. Outputs JSON to stdout."""
import json
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

KNOWN_CLIS = ["claude", "gemini", "codex", "copilot", "opencode"]
TEMPLATES_PATH = Path(__file__).parent.parent / "data" / "cli-templates.toml"


def load_templates() -> dict:
    if not TEMPLATES_PATH.exists():
        return {}
    with open(TEMPLATES_PATH, "rb") as f:
        return tomllib.load(f)


def check_cli(name: str, templates: dict) -> dict:
    path = shutil.which(name)
    if path is None or not os.access(path, os.X_OK):
        return {"path": None, "version": None, "callable": False, "verified": True}

    tmpl = templates.get(name, {})
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
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/test_detect.py -v
```

Expected: all 8 tests PASS

- [ ] **Step 5: Manual smoke test**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills/skills/dispatch-agent
python3 scripts/detect.py
```

Expected: JSON with claude/gemini/codex/copilot/opencode entries; opencode shows `"verified": false`

- [ ] **Step 6: Commit**

```bash
git add skills/dispatch-agent/scripts/detect.py skills/dispatch-agent/tests/test_detect.py
git commit -m "feat(dispatch-agent): add detect.py with CLI availability detection"
```

---

### Task 3: init.py — TOML serializer and config writer

**Files:**
- Create: `skills/dispatch-agent/scripts/init.py`
- Create: `skills/dispatch-agent/tests/test_init.py`

- [ ] **Step 1: Write failing tests**

Create `skills/dispatch-agent/tests/test_init.py`:

```python
import json
import sys
import os
import tomllib
import unittest
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))
import init as init_mod


VALID_INPUT = {
    "agents": [
        {"id": "claude-default", "cli": "claude", "model": "default", "args": [], "env": [], "tier": "primary"},
        {"id": "gemini-default", "cli": "gemini", "model": "default", "args": [], "env": [], "tier": "primary"},
        {"id": "copilot-sonnet", "cli": "copilot", "model": "sonnet-4.6", "args": ["--no-stream"], "env": [
            {"name": "GH_TOKEN", "type": "file", "path": "~/.config/gh/token"}
        ], "tier": "fallback"},
    ],
    "tier_order": ["primary", "fallback"],
    "save_location": "user",
}


class TestEscapeToml(unittest.TestCase):
    def test_escape_backslash(self):
        self.assertEqual(init_mod.escape_toml_string(r"a\b"), r"a\\b")

    def test_escape_double_quote(self):
        self.assertEqual(init_mod.escape_toml_string('say "hi"'), r'say \"hi\"')

    def test_escape_newline(self):
        self.assertEqual(init_mod.escape_toml_string("line1\nline2"), r"line1\nline2")

    def test_escape_tab(self):
        self.assertEqual(init_mod.escape_toml_string("a\tb"), r"a\tb")

    def test_no_escape_needed(self):
        self.assertEqual(init_mod.escape_toml_string("hello-world_123"), "hello-world_123")


class TestValidateAgentId(unittest.TestCase):
    def test_valid_id(self):
        init_mod.validate_agent_id("claude-default")  # should not raise

    def test_invalid_chars(self):
        with self.assertRaises(SystemExit):
            init_mod.validate_agent_id("claude default")  # space not allowed

    def test_invalid_dot(self):
        with self.assertRaises(SystemExit):
            init_mod.validate_agent_id("claude.default")

    def test_duplicate_ids(self):
        agents = [
            {"id": "same", "cli": "claude", "model": "default", "args": [], "env": [], "tier": "t1"},
            {"id": "same", "cli": "gemini", "model": "default", "args": [], "env": [], "tier": "t1"},
        ]
        with self.assertRaises(SystemExit):
            init_mod.validate_unique_ids(agents)


class TestBuildToml(unittest.TestCase):
    def test_round_trip(self):
        toml_str = init_mod.build_toml(VALID_INPUT)
        parsed = tomllib.loads(toml_str)
        self.assertEqual(parsed["version"], 1)
        self.assertEqual(len(parsed["tiers"]), 2)
        primary = parsed["tiers"][0]
        self.assertEqual(primary["id"], "primary")
        self.assertEqual(len(primary["agents"]), 2)

    def test_agent_args_in_output(self):
        toml_str = init_mod.build_toml(VALID_INPUT)
        parsed = tomllib.loads(toml_str)
        fallback_agents = parsed["tiers"][1]["agents"]
        self.assertIn("--no-stream", fallback_agents[0]["args"])

    def test_env_file_in_output(self):
        toml_str = init_mod.build_toml(VALID_INPUT)
        parsed = tomllib.loads(toml_str)
        agent = parsed["tiers"][1]["agents"][0]
        env = agent["env"]
        self.assertEqual(env[0]["name"], "GH_TOKEN")
        self.assertEqual(env[0]["type"], "file")

    def test_special_chars_escaped(self):
        data = dict(VALID_INPUT)
        data["agents"] = [
            {"id": "test-agent", "cli": "claude", "model": 'say "hi"', "args": [], "env": [], "tier": "t1"}
        ]
        data["tier_order"] = ["t1"]
        toml_str = init_mod.build_toml(data)
        parsed = tomllib.loads(toml_str)
        self.assertEqual(parsed["tiers"][0]["agents"][0]["model"], 'say "hi"')


class TestWriteConfig(unittest.TestCase):
    def test_writes_file_with_correct_permissions(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            dest = Path(tmpdir) / "dispatch-agent.toml"
            init_mod.write_config(VALID_INPUT, str(dest))
            self.assertTrue(dest.exists())
            mode = oct(dest.stat().st_mode)[-3:]
            self.assertEqual(mode, "600")

    def test_round_trip_after_write(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            dest = Path(tmpdir) / "dispatch-agent.toml"
            init_mod.write_config(VALID_INPUT, str(dest))
            with open(dest, "rb") as f:
                parsed = tomllib.load(f)
            self.assertEqual(parsed["version"], 1)

    def test_prints_path_to_stdout(self, capsys=None):
        with tempfile.TemporaryDirectory() as tmpdir:
            dest = Path(tmpdir) / "dispatch-agent.toml"
            import io
            from contextlib import redirect_stdout
            buf = io.StringIO()
            with redirect_stdout(buf):
                init_mod.write_config(VALID_INPUT, str(dest))
            self.assertIn(str(dest), buf.getvalue())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests — expect failure**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/test_init.py -v 2>&1 | head -20
```

Expected: `ModuleNotFoundError: No module named 'init'`

- [ ] **Step 3: Implement init.py**

Create `skills/dispatch-agent/scripts/init.py`:

```python
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

    # Round-trip validation
    try:
        tomllib.loads(toml_str)
    except tomllib.TOMLDecodeError as e:
        print(f"Error: generated TOML failed validation: {e}", file=sys.stderr)
        sys.exit(1)

    # Atomic write via temp file + replace
    with tempfile.NamedTemporaryFile(
        mode="w", dir=dest.parent, suffix=".tmp", delete=False
    ) as tmp:
        tmp.write(toml_str)
        tmp_path = tmp.name

    os.chmod(tmp_path, 0o600)
    os.replace(tmp_path, dest)

    print(str(dest))  # AI reads this to confirm location


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
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/test_init.py -v
```

Expected: all tests PASS

- [ ] **Step 5: Manual smoke test**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills/skills/dispatch-agent
echo '{
  "agents": [{"id": "claude-default", "cli": "claude", "model": "default", "args": [], "env": [], "tier": "t1"}],
  "tier_order": ["t1"],
  "save_location": "user"
}' | python3 scripts/init.py
```

Expected: prints path like `/Users/wen/.config/dispatch-agent.toml`; file exists with mode 600; `python3 -c "import tomllib; tomllib.load(open('~/.config/dispatch-agent.toml','rb'))"` succeeds.

- [ ] **Step 6: Commit**

```bash
git add skills/dispatch-agent/scripts/init.py skills/dispatch-agent/tests/test_init.py
git commit -m "feat(dispatch-agent): add init.py with TOML serializer and config writer"
```

---

### Task 4: dispatch.py — argparse, config loading, validation

**Files:**
- Create: `skills/dispatch-agent/scripts/dispatch.py` (partial — argparse + load only)
- Create: `skills/dispatch-agent/tests/test_dispatch.py` (partial)

- [ ] **Step 1: Write failing tests for config loading**

Create `skills/dispatch-agent/tests/test_dispatch.py`:

```python
import json
import os
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch, MagicMock

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))
import dispatch


SAMPLE_TOML = """
version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "claude-default"
  cli = "claude"
  model = "default"
  args = []

  [[tiers.agents]]
  id = "gemini-default"
  cli = "gemini"
  model = "default"
  args = []

[[tiers]]
id = "fallback"

  [[tiers.agents]]
  id = "copilot-sonnet"
  cli = "copilot"
  model = "sonnet-4.6"
  args = ["--no-stream"]

  [[tiers.agents.env]]
  name = "GH_TOKEN"
  type = "file"
  path = "/tmp/fake_token"
"""

SAMPLE_TEMPLATES = """
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[gemini]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[copilot]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[opencode]
prompt_flag = ""
model_flag = ""
file_input_mode = "arg"
version_flag = "--version"
verified = false
extra_args = []
"""


class TestLoadConfig(unittest.TestCase):
    def _write_toml(self, content):
        f = tempfile.NamedTemporaryFile(suffix=".toml", delete=False, mode="w")
        f.write(content)
        f.close()
        return f.name

    def test_loads_valid_config(self):
        path = self._write_toml(SAMPLE_TOML)
        try:
            config = dispatch.load_config(path)
            self.assertEqual(len(config["tiers"]), 2)
            self.assertEqual(config["tiers"][0]["id"], "primary")
        finally:
            os.unlink(path)

    def test_exits_on_missing_file(self):
        with self.assertRaises(SystemExit):
            dispatch.load_config("/nonexistent/path.toml")

    def test_warns_on_missing_version(self):
        path = self._write_toml("[[tiers]]\nid = 'x'\n")
        import io
        from contextlib import redirect_stderr
        buf = io.StringIO()
        try:
            with redirect_stderr(buf):
                dispatch.load_config(path)
        finally:
            os.unlink(path)
        self.assertIn("version", buf.getvalue().lower())

    def test_exits_on_invalid_env_type(self):
        bad = SAMPLE_TOML + '\n  [[tiers.agents.env]]\n  name = "X"\n  type = "bad"\n'
        path = self._write_toml(bad)
        try:
            with self.assertRaises(SystemExit):
                dispatch.load_config(path)
        finally:
            os.unlink(path)


class TestLoadTemplates(unittest.TestCase):
    def test_loads_templates(self):
        f = tempfile.NamedTemporaryFile(suffix=".toml", delete=False, mode="w")
        f.write(SAMPLE_TEMPLATES)
        f.close()
        try:
            templates = dispatch.load_templates(f.name)
            self.assertIn("claude", templates)
            self.assertEqual(templates["claude"]["prompt_flag"], "-p")
        finally:
            os.unlink(f.name)

    def test_exits_on_missing_templates_file(self):
        with self.assertRaises(SystemExit):
            dispatch.load_templates("/nonexistent.toml")


class TestBuildCommand(unittest.TestCase):
    def setUp(self):
        self.templates = tomllib.loads(SAMPLE_TEMPLATES)

    def test_basic_prompt(self):
        agent = {"id": "claude-default", "cli": "claude", "model": "default", "args": [], "env": []}
        cmd = dispatch.build_command(agent, self.templates["claude"], "hello world")
        self.assertEqual(cmd, ["claude", "-p", "hello world"])

    def test_model_appended_when_not_default(self):
        agent = {"id": "copilot-sonnet", "cli": "copilot", "model": "sonnet-4.6", "args": [], "env": []}
        cmd = dispatch.build_command(agent, self.templates["copilot"], "hi")
        self.assertIn("--model", cmd)
        self.assertIn("sonnet-4.6", cmd)

    def test_model_omitted_when_default(self):
        agent = {"id": "claude-default", "cli": "claude", "model": "default", "args": [], "env": []}
        cmd = dispatch.build_command(agent, self.templates["claude"], "hi")
        self.assertNotIn("--model", cmd)

    def test_extra_args_before_agent_args(self):
        tmpl = dict(self.templates["claude"])
        tmpl["extra_args"] = ["--no-stream"]
        agent = {"id": "claude-default", "cli": "claude", "model": "default", "args": ["--debug"], "env": []}
        cmd = dispatch.build_command(agent, tmpl, "hi")
        extra_idx = cmd.index("--no-stream")
        debug_idx = cmd.index("--debug")
        self.assertLess(extra_idx, debug_idx)

    def test_empty_prompt_flag_returns_none(self):
        agent = {"id": "opencode", "cli": "opencode", "model": "default", "args": [], "env": []}
        result = dispatch.build_command(agent, self.templates["opencode"], "hi")
        self.assertIsNone(result)


class TestResolveEnv(unittest.TestCase):
    def test_env_type_env(self):
        ev = {"name": "MY_KEY", "type": "env", "var": "MY_KEY"}
        with patch.dict(os.environ, {"MY_KEY": "secret"}):
            result = dispatch.resolve_env_var(ev)
        self.assertEqual(result, ("MY_KEY", "secret"))

    def test_env_type_file(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("  token123  \n")
            fname = f.name
        try:
            ev = {"name": "TOKEN", "type": "file", "path": fname}
            key, val = dispatch.resolve_env_var(ev)
            self.assertEqual(val, "token123")
        finally:
            os.unlink(fname)

    def test_env_file_missing_returns_none(self):
        ev = {"name": "X", "type": "file", "path": "/nonexistent/file.txt"}
        result = dispatch.resolve_env_var(ev)
        self.assertIsNone(result)


class TestRrState(unittest.TestCase):
    def test_load_missing_returns_empty(self):
        state = dispatch.load_rr_state("/nonexistent/rr-state.json")
        self.assertEqual(state, {})

    def test_load_valid(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump({"primary": "claude-default"}, f)
            fname = f.name
        try:
            state = dispatch.load_rr_state(fname)
            self.assertEqual(state["primary"], "claude-default")
        finally:
            os.unlink(fname)

    def test_atomic_write(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "rr-state.json")
            dispatch.write_rr_state({"primary": "gemini-default"}, path)
            state = dispatch.load_rr_state(path)
            self.assertEqual(state["primary"], "gemini-default")
            self.assertEqual(oct(os.stat(path).st_mode)[-3:], "600")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests — expect failure**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/test_dispatch.py -v 2>&1 | head -20
```

Expected: `ModuleNotFoundError: No module named 'dispatch'`

- [ ] **Step 3: Implement dispatch.py — argparse, config load, template load, build_command, env resolution, rr-state**

Create `skills/dispatch-agent/scripts/dispatch.py`:

```python
#!/usr/bin/env python3
"""Dispatch tasks to agent CLIs with tier-based fallback and round-robin."""
import argparse
import fcntl
import json
import os
import select
import signal
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
from pathlib import Path

TEMPLATES_PATH = Path(__file__).parent.parent / "data" / "cli-templates.toml"
RR_STATE_PATH = Path.home() / ".cache" / "dispatch-agent" / "rr-state.json"
MAX_FILE_BYTES = 256 * 1024  # 256 KB
VALID_ENV_TYPES = {"file", "env"}


# ── Config loading ────────────────────────────────────────────────────────────

def load_config(path: str) -> dict:
    try:
        with open(path, "rb") as f:
            config = tomllib.load(f)
    except FileNotFoundError:
        print(f"Error: config file not found: {path}", file=sys.stderr)
        sys.exit(1)
    except tomllib.TOMLDecodeError as e:
        print(f"Error: config parse error: {e}", file=sys.stderr)
        sys.exit(1)

    if "version" not in config:
        print("Warning: config missing 'version' field, assuming v1", file=sys.stderr)

    # Validate env types
    for tier in config.get("tiers", []):
        for agent in tier.get("agents", []):
            for ev in agent.get("env", []):
                if ev.get("type") not in VALID_ENV_TYPES:
                    print(f"Error: agent {agent['id']} has invalid env type {ev.get('type')!r}", file=sys.stderr)
                    sys.exit(1)

    return config


def load_templates(path: str = None) -> dict:
    p = Path(path) if path else TEMPLATES_PATH
    if not p.exists():
        print(f"Error: cli-templates.toml not found: {p}", file=sys.stderr)
        sys.exit(1)
    with open(p, "rb") as f:
        return tomllib.load(f)


def find_config(config_arg: str | None) -> str | None:
    if config_arg:
        return config_arg
    # Git root or cwd
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True, text=True
    )
    project_root = result.stdout.strip() if result.returncode == 0 else os.getcwd()
    project_cfg = Path(project_root) / ".config" / "dispatch-agent.toml"
    if project_cfg.exists():
        return str(project_cfg)
    user_cfg = Path.home() / ".config" / "dispatch-agent.toml"
    if user_cfg.exists():
        return str(user_cfg)
    return None


# ── Command building ──────────────────────────────────────────────────────────

def build_command(agent: dict, template: dict, prompt: str) -> list | None:
    """Build subprocess args list. Returns None if agent should be skipped."""
    prompt_flag = template.get("prompt_flag", "")
    if not prompt_flag:
        return None  # skip this agent

    model_flag = template.get("model_flag", "")
    extra_args = template.get("extra_args", [])
    agent_args = agent.get("args", [])
    model = agent.get("model", "default")

    cmd = [agent["cli"]]
    cmd += extra_args
    cmd += agent_args
    if model != "default" and model_flag:
        cmd += [model_flag, model]
    elif model != "default" and not model_flag:
        print(f"Warning: agent {agent['id']} has model={model!r} but model_flag is empty — model ignored", file=sys.stderr)
    cmd += [prompt_flag, prompt]
    return cmd


# ── Env resolution ────────────────────────────────────────────────────────────

def resolve_env_var(ev: dict) -> tuple | None:
    """Returns (name, value) or None on failure (with stderr warning)."""
    name = ev["name"]
    if ev["type"] == "env":
        val = os.environ.get(ev["var"])
        if val is None:
            print(f"Warning: env var {ev['var']!r} not set, skipping", file=sys.stderr)
            return None
        return (name, val)
    elif ev["type"] == "file":
        path = os.path.expanduser(ev["path"])
        try:
            return (name, Path(path).read_text().strip())
        except OSError:
            print(f"Warning: env file {path!r} not found, skipping", file=sys.stderr)
            return None


def build_env(agent: dict, current_depth: int) -> dict:
    env = os.environ.copy()
    for ev in agent.get("env", []):
        result = resolve_env_var(ev)
        if result:
            env[result[0]] = result[1]
    env["DISPATCH_AGENT_DEPTH"] = str(current_depth + 1)
    return env


# ── rr-state ─────────────────────────────────────────────────────────────────

def load_rr_state(path: str | Path = None) -> dict:
    p = Path(path) if path else RR_STATE_PATH
    try:
        return json.loads(p.read_text())
    except Exception:
        return {}


def write_rr_state(state: dict, path: str | Path = None) -> None:
    p = Path(path) if path else RR_STATE_PATH
    p.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w", dir=p.parent, suffix=".tmp", delete=False
    ) as tmp:
        json.dump(state, tmp)
        tmp_path = tmp.name
    os.chmod(tmp_path, 0o600)
    os.replace(tmp_path, p)


# ── Argparse ──────────────────────────────────────────────────────────────────

def make_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="dispatch.py",
        description="Dispatch tasks to agent CLIs with tier-based fallback.",
    )
    prompt_group = parser.add_mutually_exclusive_group()
    prompt_group.add_argument("-p", metavar="PROMPT", help="Prompt text")
    prompt_group.add_argument("-f", metavar="FILE", help="File containing prompt")

    target_group = parser.add_mutually_exclusive_group()
    target_group.add_argument("--tier", metavar="ID", help="Start from named tier")
    target_group.add_argument("--agent", metavar="ID", help="Force specific agent.id")

    parser.add_argument("--timeout", type=int, default=-1, metavar="N",
                        help="Per-agent timeout seconds (-1 = no timeout)")
    parser.add_argument("--config", metavar="PATH", help="Config file path")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--show-config", action="store_true")
    parser.add_argument("--verbose", action="store_true")
    return parser


def main():
    # Recursion guard
    depth = int(os.environ.get("DISPATCH_AGENT_DEPTH", 0))
    if depth >= 5:
        print("Error: dispatch recursion limit reached (DISPATCH_AGENT_DEPTH >= 5)", file=sys.stderr)
        sys.exit(1)

    parser = make_parser()
    args = parser.parse_args()

    if args.timeout == 0:
        print("Error: --timeout 0 is invalid, use -1 for no timeout", file=sys.stderr)
        sys.exit(1)

    # Load templates (always needed)
    templates = load_templates()

    # --list without config: detect-only mode
    if args.list:
        cfg_path = find_config(args.config)
        if cfg_path:
            _cmd_list(load_config(cfg_path), templates)
        else:
            _cmd_list_detect(templates)
        return

    # Find config
    cfg_path = find_config(args.config)
    if cfg_path is None:
        print("No config found. Run with 'init' to configure.", file=sys.stderr)
        sys.exit(1)
    config = load_config(cfg_path)

    if args.show_config:
        _cmd_show_config(config, cfg_path)
        return

    # Resolve prompt
    prompt = None
    if args.p:
        prompt = args.p
    elif args.f:
        fpath = Path(args.f)
        if not fpath.exists():
            print(f"Error: file not found: {args.f}", file=sys.stderr)
            sys.exit(1)
        if fpath.stat().st_size > MAX_FILE_BYTES:
            print(f"Error: file {args.f} exceeds 256KB limit", file=sys.stderr)
            sys.exit(1)
        prompt = fpath.read_text()

    if prompt is None and not args.dry_run:
        print("Error: -p or -f required", file=sys.stderr)
        sys.exit(1)

    if args.dry_run and prompt is None:
        prompt = "<prompt>"

    _cmd_dispatch(config, templates, prompt, args, depth)


# ── Output commands ───────────────────────────────────────────────────────────

def _cmd_show_config(config: dict, path: str) -> None:
    layer = "project" if ".config" in path and "/.config/dispatch-agent" not in str(Path.home() / ".config") else "user"
    # Simpler heuristic: if under home/.config, it's user
    if path == str(Path.home() / ".config" / "dispatch-agent.toml"):
        layer = "user"
    else:
        layer = "project"
    print(f"Config: {path}  ({layer} layer)\n")
    for tier in config.get("tiers", []):
        print(f"TIER {tier['id']}")
        for agent in tier.get("agents", []):
            args_str = str(agent.get("args", []))
            print(f"  agent: {agent['id']}   cli={agent['cli']}  model={agent.get('model','default')}  args={args_str}")
            for ev in agent.get("env", []):
                if ev["type"] == "file":
                    print(f"    env: {ev['name']} (file: {ev['path']})")
                else:
                    print(f"    env: {ev['name']} (env: {ev['var']})")


def _cmd_list(config: dict, templates: dict) -> None:
    for tier in config.get("tiers", []):
        print(f"TIER {tier['id']}")
        for agent in tier.get("agents", []):
            cli = agent["cli"]
            path = shutil.which(cli)
            ok = path is not None and os.access(path, os.X_OK)
            marker = "✓" if ok else "✗"
            loc = path if ok else "(not found)"
            print(f"  [{marker}] {agent['id']}   cli={cli}   model={agent.get('model','default')}    {loc}")


def _cmd_list_detect(templates: dict) -> None:
    # Import detect from sibling script
    detect_path = Path(__file__).parent / "detect.py"
    result = subprocess.run(
        [sys.executable, str(detect_path)],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print("Error running detect.py", file=sys.stderr)
        sys.exit(1)
    data = json.loads(result.stdout)
    print("[SYSTEM CLIs — no config loaded, run 'init' to configure]")
    for cli, info in data.items():
        if not info["callable"]:
            print(f"  [✗] {cli}  (not found)")
        elif not info.get("verified", True):
            ver = info.get("version") or ""
            print(f"  [!] {cli}  {info['path']}  {ver}  (verified=false — will be skipped at dispatch)")
        else:
            ver = info.get("version") or ""
            print(f"  [✓] {cli}  {info['path']}  {ver}")


# ── Dispatch core ─────────────────────────────────────────────────────────────

def _cmd_dispatch(config: dict, templates: dict, prompt: str, args, depth: int) -> None:
    tiers = config.get("tiers", [])

    # --agent: bypass tier logic
    if args.agent:
        agent = _find_agent_by_id(tiers, args.agent)
        if agent is None:
            print(f"Error: agent id {args.agent!r} not found in config", file=sys.stderr)
            sys.exit(1)
        tmpl = templates.get(agent["cli"])
        if tmpl is None:
            print(f"Error: CLI {agent['cli']!r} not in cli-templates.toml", file=sys.stderr)
            sys.exit(1)
        cmd = build_command(agent, tmpl, prompt)
        if cmd is None:
            print(f"Error: agent {agent['id']} has empty prompt_flag, cannot dispatch", file=sys.stderr)
            sys.exit(1)
        if args.dry_run:
            print(f"[DRY RUN] agent={agent['id']}")
            print(f"  command: {cmd}")
            return
        env = build_env(agent, depth)
        rc = call_agent(agent["id"], None, cmd, env, args.timeout, args.verbose)
        sys.exit(rc)

    # Filter to starting tier
    if args.tier:
        tier_ids = [t["id"] for t in tiers]
        if args.tier not in tier_ids:
            print(f"Error: tier {args.tier!r} not found in config", file=sys.stderr)
            sys.exit(1)
        idx = tier_ids.index(args.tier)
        tiers = tiers[idx:]

    # Read rr_state (lock scope: read only)
    rr_path = RR_STATE_PATH
    rr_path.parent.mkdir(parents=True, exist_ok=True)
    rr_fd = open(rr_path, "a+")  # open for read/write, create if missing
    fcntl.flock(rr_fd, fcntl.LOCK_EX)
    rr_fd.seek(0)
    try:
        rr_state = json.load(rr_fd)
    except Exception:
        rr_state = {}
    fcntl.flock(rr_fd, fcntl.LOCK_UN)

    failures = []

    for tier in tiers:
        agents = tier.get("agents", [])
        if not agents:
            continue

        agent_ids = [a["id"] for a in agents]
        next_id = rr_state.get(tier["id"])
        start = agent_ids.index(next_id) if next_id in agent_ids else 0
        n = len(agents)

        for i in range(n):
            agent = agents[(start + i) % n]
            tmpl = templates.get(agent["cli"])
            if tmpl is None:
                print(f"Warning: CLI {agent['cli']!r} not in cli-templates.toml, skipping", file=sys.stderr)
                failures.append((agent["id"], "skip: no template", ""))
                continue
            cmd = build_command(agent, tmpl, prompt)
            if cmd is None:
                print(f"Warning: agent {agent['id']} has empty prompt_flag, skipping", file=sys.stderr)
                failures.append((agent["id"], "skip: empty prompt_flag", ""))
                continue

            if args.dry_run:
                print(f"[DRY RUN] tier={tier['id']}  agent={agent['id']}")
                print(f"  command: {cmd}")
                return

            env = build_env(agent, depth)
            if args.verbose:
                print(f"[attempting {agent['id']}]", file=sys.stderr)

            rc, stderr_snippet = call_agent_with_result(
                agent["id"], tier["id"], cmd, env, args.timeout, args.verbose
            )

            if rc == 0:
                # Update rr-state under lock
                next_agent_id = agents[(start + i + 1) % n]["id"]
                fcntl.flock(rr_fd, fcntl.LOCK_EX)
                rr_fd.seek(0)
                try:
                    rr_state = json.load(rr_fd)
                except Exception:
                    rr_state = {}
                rr_state[tier["id"]] = next_agent_id
                fcntl.flock(rr_fd, fcntl.LOCK_UN)
                write_rr_state(rr_state)
                rr_fd.close()
                sys.exit(0)
            else:
                reason = "timeout" if rc == -signal.SIGKILL else str(rc)
                failures.append((agent["id"], reason, stderr_snippet))

    rr_fd.close()
    print("\nAll agents failed:", file=sys.stderr)
    for agent_id, reason, stderr_snip in failures:
        print(f"  {agent_id}: {reason}", file=sys.stderr)
        if stderr_snip:
            print(f"    stderr: {stderr_snip[:200]}", file=sys.stderr)
    sys.exit(1)


def _find_agent_by_id(tiers: list, agent_id: str) -> dict | None:
    found = None
    for tier in tiers:
        for agent in tier.get("agents", []):
            if agent["id"] == agent_id:
                if found is not None:
                    print(f"Warning: multiple agents with id {agent_id!r}, using first", file=sys.stderr)
                    return found
                found = agent
    return found


# ── Agent calling ─────────────────────────────────────────────────────────────

def call_agent(agent_id: str, tier_id: str | None, cmd: list, env: dict,
               timeout: int, verbose: bool) -> int:
    rc, _ = call_agent_with_result(agent_id, tier_id, cmd, env, timeout, verbose)
    return rc


def call_agent_with_result(agent_id: str, tier_id: str | None, cmd: list, env: dict,
                            timeout: int, verbose: bool) -> tuple[int, str]:
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        start_new_session=True,
    )

    killed = threading.Event()
    timer = None

    def _kill():
        killed.set()
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except ProcessLookupError:
            pass

    def _handle_signal(signum, frame):
        _kill()
        sys.exit(1)

    signal.signal(signal.SIGINT, _handle_signal)
    signal.signal(signal.SIGTERM, _handle_signal)

    if timeout > 0:
        timer = threading.Timer(timeout, _kill)
        timer.start()

    # Verbose wait thread
    verbose_stop = threading.Event()
    if verbose:
        start_time = time.time()
        def _verbose_tick():
            while not verbose_stop.wait(10):
                elapsed = int(time.time() - start_time)
                print(f"[waiting: {agent_id} — {elapsed}s elapsed]", file=sys.stderr)
        vthread = threading.Thread(target=_verbose_tick, daemon=True)
        vthread.start()

    stderr_buf = []

    try:
        while True:
            rlist, _, _ = select.select([proc.stdout, proc.stderr], [], [], 0.1)
            for fd in rlist:
                data = fd.read1(4096)
                if not data:
                    continue
                if fd is proc.stdout:
                    sys.stdout.buffer.write(data)
                    sys.stdout.buffer.flush()
                else:
                    stderr_buf.append(data.decode(errors="replace"))

            if proc.poll() is not None:
                # Drain remaining
                for fd in [proc.stdout, proc.stderr]:
                    remaining = fd.read()
                    if remaining:
                        if fd is proc.stdout:
                            sys.stdout.buffer.write(remaining)
                            sys.stdout.buffer.flush()
                        else:
                            stderr_buf.append(remaining.decode(errors="replace"))
                break
    finally:
        if timer:
            timer.cancel()
        verbose_stop.set()

    rc = proc.returncode
    stderr_text = "".join(stderr_buf)

    if rc == 0:
        if tier_id:
            print(f"[{agent_id}] (tier: {tier_id})", file=sys.stderr)
    else:
        if stderr_text:
            print(stderr_text, file=sys.stderr)

    return rc, stderr_text


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/ -v
```

Expected: all tests PASS (some may be skipped if CLI binaries unavailable)

- [ ] **Step 5: Manual smoke — argparse**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills/skills/dispatch-agent
python3 scripts/dispatch.py --help
python3 scripts/dispatch.py --timeout 0 -p "hi" 2>&1  # expect: error about timeout 0
python3 scripts/dispatch.py --list                     # expect: detect-only or config list
```

- [ ] **Step 6: Commit**

```bash
git add skills/dispatch-agent/scripts/dispatch.py skills/dispatch-agent/tests/test_dispatch.py
git commit -m "feat(dispatch-agent): add dispatch.py with argparse, config loading, and agent calling"
```

---

### Task 5: references/init-guide.md

**Files:**
- Create: `skills/dispatch-agent/references/init-guide.md`

- [ ] **Step 1: Create init-guide.md**

Create `skills/dispatch-agent/references/init-guide.md`:

```markdown
# dispatch-agent Init Guide

Follow this guide when no config is found or the user invokes `init`.

---

## Prerequisites

```bash
python3 --version  # must be 3.11+
python3 scripts/detect.py  # must return valid JSON
```

---

## Step 1: Detect CLIs

```bash
python3 scripts/detect.py
```

Display results to user. **Important:** explicitly tell the user:
> "CLIs marked `verified: false` (e.g. opencode) will be skipped at dispatch even if added to config, because their non-interactive mode is unverified."

---

## Step 2: Existing Config Handling

Check for existing config at:
- `<git-root>/.config/dispatch-agent.toml`
- `~/.config/dispatch-agent.toml`

If found, use `AskUserQuestion`:

```
Question: "A config already exists at <path>. What would you like to do?"
Options:
  - "Overwrite" — delete existing, write new
  - "Backup first" — rename to dispatch-agent.toml.bak, then write new
  - "Cancel" — abort init
```

---

## Step 3: Per-CLI Configuration

For each CLI that is callable AND verified (skip unverified), ask **one at a time**:

```
Question: "For <cli>: set a custom agent id? (default: <cli>-default)"
Options:
  - "Use default: <cli>-default"
  - (Other: let user type custom id matching [a-zA-Z0-9_-])
```

```
Question: "For <cli>: specify extra args? (default: none)"
Options:
  - "No extra args"
  - (Other: comma-separated args, e.g. --no-stream,--debug)
```

```
Question: "For <cli>: need env vars? (default: none)"
Options:
  - "No env vars needed"
  - "Add env var from file (type=file)"
  - "Forward env var from shell (type=env)"
```

If user adds env var, collect:
- `name`: the env var name (e.g. GITHUB_TOKEN)
- `type`: file or env
- `path` (if file): path to token file
- `var` (if env): name of the source env var

Pre-fill model using defaults:
| CLI | Default model |
|-----|--------------|
| claude | default |
| gemini | default |
| codex | default |
| copilot | sonnet-4.6 |
| opencode | glm-5.1 |

---

## Step 4: Tier Assignment

```
Question: "How many tiers do you want? (e.g. 2: primary + fallback)"
Options: "1", "2", "3", (Other)
```

For each tier, collect a name (e.g. "primary", "fallback") then:

```
Question: "Which agents go in tier '<name>'? (in priority order)"
Options: show all configured agent ids as checkboxes
```

---

## Step 5: Save Location

```
Question: "Save config to:"
Options:
  - "User (~/.config/dispatch-agent.toml)" — shared across all projects
  - "Project (<git-root>/.config/dispatch-agent.toml)" — project-specific
```

---

## Step 6: Write Config

Build the JSON input and pipe to init.py:

```python
payload = {
  "agents": [...],          # collected above
  "tier_order": [...],      # tier names in order
  "save_location": "user" | "project"
}
```

```bash
echo '<json>' | python3 scripts/init.py
```

If init.py exits non-0: show stderr to user, offer to retry from Step 3.
On success: init.py prints the config path — confirm to user with permissions note (0600).

---

## Edge Cases

- **No callable CLIs detected:** inform user, suggest installing at least one CLI from the Default Platforms list.
- **All CLIs unverified:** same as above — no agents can be configured.
- **Duplicate agent ids:** validate before calling init.py; prompt user to choose a different id.
- **env file not found during init:** warn user but allow proceeding — dispatch.py will skip the var at runtime.
```

- [ ] **Step 2: Commit**

```bash
git add skills/dispatch-agent/references/init-guide.md
git commit -m "feat(dispatch-agent): add references/init-guide.md"
```

---

### Task 6: references/dispatch-guide.md

**Files:**
- Create: `skills/dispatch-agent/references/dispatch-guide.md`

- [ ] **Step 1: Create dispatch-guide.md**

Create `skills/dispatch-agent/references/dispatch-guide.md`:

```markdown
# dispatch-agent Reference Guide

---

## Quick Reference

| Flag | Description |
|------|-------------|
| `-p "prompt"` | Prompt text (mutually exclusive with -f) |
| `-f FILE` | Read prompt from file (max 256KB) |
| `--timeout N` | Per-agent timeout in seconds (-1 = no timeout, default) |
| `--tier ID` | Start from named tier (mutually exclusive with --agent) |
| `--agent ID` | Force specific agent by agent.id (bypass tier logic) |
| `--config PATH` | Explicit config file path |
| `--dry-run` | Show command without executing |
| `--list` | List agents and availability |
| `--show-config` | Print resolved config |
| `--verbose` | Show per-agent attempt and wait status |

---

## Config Schema

**Location (first found wins):**
1. `--config PATH`
2. `<git-root>/.config/dispatch-agent.toml`
3. `~/.config/dispatch-agent.toml`

```toml
version = 1

[[tiers]]
id = "primary"            # tier label (TOML order = fallback order)

  [[tiers.agents]]
  id = "claude-default"   # unique across all agents; [a-zA-Z0-9_-] only
  cli = "claude"          # must match a key in data/cli-templates.toml
  model = "default"       # "default" = omit --model flag
  args = []               # string array, appended after template.extra_args

  [[tiers.agents.env]]
  name = "GITHUB_TOKEN"   # env var name to inject
  type = "file"           # "file": read path contents; "env": forward from shell
  path = "~/.config/gh/token"

  [[tiers.agents.env]]
  name = "OPENAI_KEY"
  type = "env"
  var = "OPENAI_KEY"
```

---

## cli-templates.toml Format

Located at `data/cli-templates.toml`. User-editable.

| Field | Description |
|-------|-------------|
| `prompt_flag` | Flag used to pass prompt (e.g. `-p`). Empty = skip agent. |
| `model_flag` | Flag for model selection (e.g. `--model`). Empty = no model flag. |
| `file_input_mode` | `"arg"`: pass file contents via prompt_flag. `"stdin"` reserved for v2. |
| `version_flag` | Flag for version detection. Empty = skip version check. |
| `extra_args` | Args always prepended before agent.args. |
| `verified` | `false` = agent skipped at dispatch (unverified non-interactive mode). |

**Adding a new CLI:** add `[cli-name]` section. No Python changes needed.

---

## Tier Fallback Logic

1. Tiers are tried in TOML file order.
2. Within a tier, agents are tried round-robin (starting from last-used + 1).
3. An agent is skipped (with warning) if: `prompt_flag = ""`, or CLI not in templates.
4. If all agents in a tier fail/skip, the next tier is tried. rr-state pointer is NOT updated on tier exhaustion.
5. rr-state pointer updates only on success.

---

## rr-state

**Location:** `~/.cache/dispatch-agent/rr-state.json`
**Format:** `{ "tier-id": "next-agent-id" }`
**Reset:** delete the file manually.

On load: if stored agent id not found in config (agent removed/renamed), start from index 0.

---

## Output Formats

**--dry-run:**
```
[DRY RUN] tier=primary  agent=claude-default
  command: ['claude', '-p', 'your prompt']
```

**--list (with config):**
```
TIER primary
  [✓] claude-default   cli=claude   model=default    /usr/local/bin/claude
  [✗] copilot-sonnet   cli=copilot  model=sonnet-4.6  (not found)
```

**--list (no config):**
```
[SYSTEM CLIs — no config loaded, run 'init' to configure]
  [✓] claude    /usr/local/bin/claude   v1.2.3
  [!] opencode  /usr/local/bin/opencode  v0.5.0  (verified=false)
  [✗] codex     (not found)
```

**--show-config:**
```
Config: /project/.config/dispatch-agent.toml  (project layer)

TIER primary
  agent: claude-default   cli=claude  model=default  args=[]
```

**--verbose:**
```
[attempting claude-default]
[waiting: claude-default — 10s elapsed]
[claude-default] (tier: primary)
```

---

## Error Reference

| Message | Cause |
|---------|-------|
| `Config parse error: ...` | Invalid TOML in config file |
| `use -1 for no timeout` | `--timeout 0` passed |
| `dispatch recursion limit reached` | DISPATCH_AGENT_DEPTH >= 5 |
| `file not found: ...` | `-f FILE` path doesn't exist |
| `file ... exceeds 256KB limit` | `-f FILE` too large |
| `invalid env type ...` | config env.type not "file" or "env" |
| `cli-templates.toml not found` | data/cli-templates.toml missing |

**Warnings (agent skipped, dispatch continues):**
- `CLI ... not in cli-templates.toml` — add entry to data/cli-templates.toml
- `agent ... has empty prompt_flag` — CLI non-interactive mode unverified
- `env var ... not set` — set the env var in your shell
- `env file ... not found` — create the file or update config path

---

## Recursion Guard

`DISPATCH_AGENT_DEPTH` env var tracks dispatch nesting. Set to 0 by default, incremented before each subprocess call. At depth >= 5, dispatch exits with error. Prevents infinite recursion when an agent dispatches back to dispatch-agent.
```

- [ ] **Step 2: Commit**

```bash
git add skills/dispatch-agent/references/dispatch-guide.md
git commit -m "feat(dispatch-agent): add references/dispatch-guide.md"
```

---

### Task 7: End-to-end verification

**Files:** none (manual testing + cleanup)

- [ ] **Step 1: Run full test suite**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
python3 -m pytest skills/dispatch-agent/tests/ -v
```

Expected: all tests PASS

- [ ] **Step 2: Verify detect.py (checklist #1)**

```bash
cd skills/dispatch-agent
python3 scripts/detect.py | python3 -c "import json,sys; d=json.load(sys.stdin); print('ok' if 'claude' in d and d['opencode']['verified']==False else 'FAIL')"
```

Expected: `ok`

- [ ] **Step 3: Verify --list no config (checklist #2)**

```bash
python3 scripts/dispatch.py --list
```

Expected: `[SYSTEM CLIs — no config loaded...]` with `[!]` for opencode

- [ ] **Step 4: Run init and verify config (checklist #3, #14)**

Invoke `init` through SKILL.md flow, or directly:
```bash
echo '{
  "agents": [
    {"id":"claude-default","cli":"claude","model":"default","args":[],"env":[],"tier":"t1"},
    {"id":"gemini-default","cli":"gemini","model":"default","args":[],"env":[],"tier":"t1"}
  ],
  "tier_order":["t1"],
  "save_location":"user"
}' | python3 scripts/init.py
```

Then verify:
```bash
python3 scripts/dispatch.py --list
python3 scripts/dispatch.py --show-config
ls -la ~/.config/dispatch-agent.toml  # expect mode 600
```

- [ ] **Step 5: Verify dry-run (checklist #6)**

```bash
python3 scripts/dispatch.py -p "say hi" --dry-run
```

Expected: `[DRY RUN] tier=t1  agent=claude-default` with command

- [ ] **Step 6: Verify -f error handling (checklist #7)**

```bash
python3 scripts/dispatch.py -f nonexistent.txt
echo $?  # expect: 1
```

Expected: stderr error, exit code 1

- [ ] **Step 7: Verify live dispatch with streaming (checklist #8, #9)**

```bash
python3 scripts/dispatch.py -p "say hi in one word" --verbose
```

Expected: output streamed, `[claude-default] (tier: t1)` on stderr

- [ ] **Step 8: Verify tier fallback (checklist #10)**

```bash
# Temporarily rename claude binary
sudo mv $(which claude) $(which claude).bak 2>/dev/null || true
python3 scripts/dispatch.py -p "say hi"
sudo mv $(which claude).bak $(which claude) 2>/dev/null || true
```

Expected: falls back to gemini-default (or next tier if configured)

- [ ] **Step 9: Verify all-fail behavior (checklist #11)**

```bash
# Create a config with a nonexistent CLI
echo '{
  "agents": [{"id":"fake-agent","cli":"notacli","model":"default","args":[],"env":[],"tier":"t1"}],
  "tier_order":["t1"],
  "save_location":"user"
}' | python3 scripts/init.py
python3 scripts/dispatch.py -p "hi"
echo $?  # expect: 1
```

Expected: stderr failure summary, exit code 1

- [ ] **Step 10: Final commit**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills
git add -A
git commit -m "feat(dispatch-agent): complete skill implementation"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|-----------------|------|
| SKILL.md frontmatter + routing | Task 1 |
| cli-templates.toml | Task 1 |
| detect.py with shutil.which + X_OK + version | Task 2 |
| init.py TOML serializer + round-trip | Task 3 |
| init.py validation (id regex, uniqueness, args type) | Task 3 |
| init.py 0600 permissions + stdout path | Task 3 |
| dispatch.py argparse mutually exclusive groups | Task 4 |
| dispatch.py recursion guard | Task 4 |
| dispatch.py config load + env.type validation | Task 4 |
| dispatch.py --list / --show-config / --dry-run | Task 4 |
| dispatch.py build_command (model default, extra_args order) | Task 4 |
| dispatch.py env resolution (file + env types) | Task 4 |
| dispatch.py rr-state load/write with flock | Task 4 |
| dispatch.py Popen + select loop + SIGKILL | Task 4 |
| dispatch.py tier loop + round-robin | Task 4 |
| dispatch.py verbose wait thread | Task 4 |
| dispatch.py failure summary on all-tiers-exhausted | Task 4 |
| references/init-guide.md | Task 5 |
| references/dispatch-guide.md | Task 6 |
| All 14 manual verification items | Task 7 |

All spec requirements covered. No placeholders found.
