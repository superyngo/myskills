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
