import copy
import json
import tempfile
import unittest
from pathlib import Path

from check_payload_formats import POLICY, check_documents, sections, synchronize


class PayloadFormatDocumentationTests(unittest.TestCase):
    def test_inventory_and_document_edits_cannot_drift_silently(self):
        policy = json.loads(POLICY.read_text())
        template = "\n".join(
            f"<!-- BEGIN tool-payload-formats:{name} -->\n<!-- END tool-payload-formats:{name} -->"
            for name in sections(policy)
        )
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "spec.md"
            generated = synchronize(template, sections(policy))
            path.write_text(generated)
            check_documents(policy, [path])
            for old, new in [
                ("`.py`", "`.other`"),
                ("`README`", "`OTHER`"),
                ("89 50", "89 51"),
                ("`venv`", "`other-env`"),
                ("`.node`", "`.wrong-native`"),
                ("`.so.`", "`.wrong-fragment.`"),
            ]:
                path.write_text(generated.replace(old, new))
                with self.assertRaises(ValueError):
                    check_documents(policy, [path])
            path.write_text(generated)
            changed = copy.deepcopy(policy)
            changed["text_groups"][0]["suffixes"].append(".new")
            with self.assertRaises(ValueError):
                check_documents(changed, [path])
            check_documents(changed, [path], write=True)
            check_documents(changed, [path])
            collection_changed = copy.deepcopy(changed)
            collection_changed["collection"]["file_basenames"].append(".other-os-data")
            with self.assertRaises(ValueError):
                check_documents(collection_changed, [path])
            for change in ["add", "remove", "fragment"]:
                native_changed = copy.deepcopy(changed)
                if change == "add":
                    native_changed["native_filenames"]["suffixes"].append(".wasm")
                elif change == "remove":
                    native_changed["native_filenames"]["suffixes"].remove(".node")
                else:
                    native_changed["native_filenames"]["basename_contains"] = [
                        ".dylib."
                    ]
                with self.assertRaises(ValueError):
                    check_documents(native_changed, [path])

    def test_missing_or_repeated_generated_regions_fail(self):
        for document in ["", "<!-- BEGIN tool-payload-formats:text -->" * 2]:
            with self.assertRaises(ValueError):
                synchronize(document, {"text": "inventory"})


if __name__ == "__main__":
    unittest.main()
