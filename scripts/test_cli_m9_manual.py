"""Guard fixture setup safety and the optional mock's distinct response cases."""

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import cli_m9_manual as manual


class ManualFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "fixtures"
        self.cli = Path(self.temp.name) / "agentpm"
        self.cli.write_bytes(b"fixture CLI bytes")

    def setup(self, reset=False):
        with patch.object(
            manual.subprocess, "check_output", return_value="agentpm test"
        ) as version, patch.object(manual.subprocess, "run") as command, patch(
            "builtins.print"
        ):
            manual.setup(self.root, self.cli, reset)
        version.assert_called_once_with(
            [str(self.cli.resolve()), "--version"], text=True
        )
        command.assert_not_called()

    def test_setup_creates_empty_cases_not_manifests_or_results(self):
        self.setup()
        for name in manual.DIRECTORIES:
            self.assertEqual(list((self.root / name).iterdir()), [])
        self.assertEqual(list(self.root.rglob("agent.json")), [])
        metadata = json.loads((self.root / "fixture.json").read_text())
        self.assertEqual(metadata["cli"], str(self.cli.resolve()))
        self.assertEqual(len(metadata["sha256"]), 64)
        self.assertNotIn("AGENTPM_TOKEN=", (self.root / "env.sh").read_text())
        self.assertNotIn("export HOME=", (self.root / "env.sh").read_text())

    def test_existing_results_require_explicit_owned_reset(self):
        self.setup()
        result = self.root / "runs" / "keep.txt"
        result.write_text("operator evidence")
        with self.assertRaises(RuntimeError):
            self.setup()
        self.assertTrue(result.exists())
        self.setup(reset=True)
        self.assertFalse(result.exists())

    def test_reset_refuses_unowned_directory(self):
        self.root.mkdir()
        keep = self.root / "keep.txt"
        keep.write_text("not a fixture")
        with self.assertRaises(RuntimeError):
            self.setup(reset=True)
        self.assertTrue(keep.exists())

    def test_reset_refuses_symlink_root_and_active_server(self):
        self.setup()
        link = Path(self.temp.name) / "link"
        link.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(RuntimeError):
            manual.setup(link, self.cli, reset=True)
        (self.root / "mock.pid").write_text(str(manual.os.getpid()))
        with self.assertRaisesRegex(RuntimeError, "Stop the optional mock"):
            self.setup(reset=True)
        self.assertTrue((self.root / "fixture.json").exists())

    def test_failed_binary_probe_preserves_previous_evidence(self):
        self.setup()
        result = self.root / "runs" / "keep.txt"
        result.write_text("operator evidence")
        with patch.object(
            manual.subprocess, "check_output", side_effect=OSError("not executable")
        ), self.assertRaises(OSError):
            manual.setup(self.root, self.cli, reset=True)
        self.assertEqual(result.read_text(), "operator evidence")

    def test_mock_auth_status_and_decode_cases_are_distinct(self):
        for label, expected in (
            ("rejected", 401),
            ("forbidden", 403),
            ("missing", 404),
            ("server", 500),
        ):
            with self.subTest(label=label):
                code, _ = manual.fixture_response(
                    "POST",
                    "/v1/tools/install/resolve",
                    f"Bearer apm_m9_{label}.fixture",
                )
                self.assertEqual(code, expected)
        ok = manual.fixture_response("GET", "/v1/whoami", "Bearer apm_m9_ok.fixture")
        bad = manual.fixture_response(
            "GET", "/v1/whoami", "Bearer apm_m9_decode.fixture"
        )
        self.assertEqual(ok[0], 200)
        self.assertIn("email", ok[1])
        self.assertEqual(bad[0], 200)
        self.assertNotIn("email", bad[1])
        self.assertEqual(manual.fixture_response("POST", "/unexpected", "")[0], 400)


if __name__ == "__main__":
    unittest.main()
