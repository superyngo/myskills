import json
import sys
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
    @patch("detect.subprocess.run")
    def test_verified_false_copied_from_template(self, mock_run, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/opencode"
        mock_access.return_value = True
        mock_run.return_value = MagicMock(returncode=0, stdout="opencode 0.5\n")
        result = detect.check_cli("opencode", self.templates)
        self.assertFalse(result["verified"])

    @patch("detect.shutil.which")
    @patch("detect.os.access")
    def test_no_template_returns_callable_with_null_version(self, mock_access, mock_which):
        mock_which.return_value = "/usr/bin/claude"
        mock_access.return_value = True
        result = detect.check_cli("claude", {})
        # no template → version null, callable true, verified defaults to True
        self.assertIsNone(result.get("version"))
        self.assertTrue(result["callable"])
        self.assertTrue(result["verified"])


class TestLoadTemplates(unittest.TestCase):
    def test_returns_empty_when_file_missing(self):
        # Temporarily point TEMPLATES_PATH to a nonexistent file
        orig = detect.TEMPLATES_PATH
        detect.TEMPLATES_PATH = Path("/nonexistent/path.toml")
        try:
            result = detect.load_templates()
            self.assertEqual(result, {})
        finally:
            detect.TEMPLATES_PATH = orig

    @patch("detect.tomllib.load")
    def test_loads_valid_templates(self, mock_load):
        mock_load.return_value = {
            "claude": {"prompt_flag": "-p", "version_flag": "--version"}
        }
        result = detect.load_templates()
        self.assertIn("claude", result)
        self.assertEqual(result["claude"]["prompt_flag"], "-p")


if __name__ == "__main__":
    unittest.main()
