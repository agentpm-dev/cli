"""The cross-repository snapshot check must fail on missing or changed vectors."""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class SdkContractTests(unittest.TestCase):
    def test_snapshot_and_mutations(self):
        with tempfile.TemporaryDirectory() as directory:
            sdk = Path(directory)
            (sdk / "pyproject.toml").write_text('[project]\nname="fixture"\n')
            snapshot = sdk / "tests/fixtures/sdk-v1"
            shutil.copytree(ROOT / "crates/agentpm-sdk/contracts/sdk-v1", snapshot)
            args = [
                sys.executable,
                str(ROOT / "scripts/check_sdk_contract.py"),
                "--cli-root",
                str(ROOT),
                "--sdk-root",
                str(sdk),
            ]
            result = subprocess.run(args, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            fixture = snapshot / "compatibility.json"
            fixture.write_bytes(fixture.read_bytes() + b"\n")
            result = subprocess.run(args, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("SDK fixture drift", result.stderr)
            fixture.unlink()
            result = subprocess.run(args, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("inventory differs", result.stderr)


if __name__ == "__main__":
    unittest.main()
