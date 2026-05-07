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
