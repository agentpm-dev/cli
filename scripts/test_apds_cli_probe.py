"""The actual-CLI acceptance probe must never count discovery-only success."""

import contextlib
import io
import unittest
from types import SimpleNamespace
from unittest.mock import patch

import check_apds_cli_fixtures as probe


class ApdsCliProbeTests(unittest.TestCase):
    def test_no_files_empty_reports_and_legacy_reports_cannot_pass(self):
        for stdout in ["No agent.json found.", "[]", '[{"ok":true}]']:
            with (
                self.subTest(stdout=stdout),
                patch("sys.argv", ["probe", "--cli", "unused-cli"]),
                patch.object(
                    probe.subprocess,
                    "run",
                    return_value=SimpleNamespace(
                        returncode=0, stdout=stdout, stderr=""
                    ),
                ),
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit) as raised,
            ):
                probe.main()
            self.assertEqual(raised.exception.code, 1)


if __name__ == "__main__":
    unittest.main()
