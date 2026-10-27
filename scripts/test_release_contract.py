import tempfile
import unittest
from pathlib import Path

from check_release_contract import CONTRACT_FILES, check_contracts


class ReleaseContractParityTests(unittest.TestCase):
    def test_each_snapshot_is_required_and_independently_checked(self):
        with tempfile.TemporaryDirectory() as root:
            source = Path(root) / "source"
            target = Path(root) / "target"
            source.mkdir()
            target.mkdir()
            for name in CONTRACT_FILES:
                (source / name).write_bytes(b"original\n")
                (target / name).write_bytes(b"original\n")
            check_contracts(source, target)
            for name in CONTRACT_FILES:
                (target / name).write_bytes(b"changed\n")
                with self.assertRaisesRegex(ValueError, name):
                    check_contracts(source, target)
                (target / name).unlink()
                with self.assertRaises(FileNotFoundError):
                    check_contracts(source, target)
                (target / name).write_bytes(b"original\n")
            check_contracts(source, target)


if __name__ == "__main__":
    unittest.main()
