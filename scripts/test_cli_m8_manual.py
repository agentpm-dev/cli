"""Guard the manual helper against green results from missing or failed work."""

import contextlib
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import cli_m8_manual as manual


class ManualHelperTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name) / "cli-m8-test"
        self.base.mkdir()
        (self.base / "runs").mkdir()
        self.addCleanup(patch.stopall)
        patch.object(manual, "BASE", self.base).start()
        self.suite = manual.Suite({"apm": "/unused/agentpm", "sha256": "test-digest"})
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()
        self.addCleanup(self.output.__exit__, None, None, None)

    def test_expected_failure_requires_the_nonzero_exit(self):
        for actual, expected in ((0, 1), (1, 0)):
            with self.subTest(actual=actual, expected=expected):
                with patch.object(
                    manual.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], actual, "out", "err"),
                ):
                    with self.assertRaisesRegex(RuntimeError, "expected exit"):
                        self.suite.run("failure", self.base, ["lint"], expected)
        with patch.object(
            manual.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 1, "out", "err"),
        ):
            self.suite.run("expected-failure", self.base, ["lint"], 1)

    def test_summary_rejects_missing_sections(self):
        with self.assertRaisesRegex(RuntimeError, "Missing section"):
            manual.summary(self.suite)

    def test_summary_rejects_a_passing_section_missing_one_command(self):
        manual.write_json(
            self.base / "runs/section-scaffolds.json",
            {
                "status": "passed",
                "commands": ["init-tool"],
                "binary_sha256": "test-digest",
            },
        )
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            manual.summary(self.suite)

    def test_commands_cannot_run_outside_fixture_root(self):
        with patch.object(manual.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "outside fixture root"):
                self.suite.run("unsafe", self.base.parent, ["init"])
            run.assert_not_called()

    def test_reset_rejects_an_unowned_directory(self):
        binary = self.base.parent / "agentpm"
        binary.write_text("not executed", encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "not owned"):
            manual.setup(binary, reset=True)
        self.assertTrue((self.base / "runs").is_dir())


if __name__ == "__main__":
    unittest.main()
