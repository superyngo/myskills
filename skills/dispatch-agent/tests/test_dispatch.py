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
