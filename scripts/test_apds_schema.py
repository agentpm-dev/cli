"""Independent Python structural checks; semantic conformance is a separate suite."""

import copy
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

from check_apds_schema import BUNDLE, COMPATIBILITY, ROOT, SCHEMA, synchronize


class ApdsSchemaTests(unittest.TestCase):
    def setUp(self):
        self.schema = json.loads((ROOT / SCHEMA).read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(self.schema)
        self.validator = Draft202012Validator(self.schema)
        self.agent = json.loads(
            (
                ROOT / BUNDLE / "conformance/fixtures/valid/minimal-agent.json"
            ).read_text()
        )

    def test_all_kinds_and_stage_one_additions(self):
        paths = sorted((ROOT / BUNDLE / "conformance/fixtures/valid").glob("*.json"))
        self.assertEqual(len(paths), 9)
        for path in paths:
            with self.subTest(path=path.name):
                self.validator.validate(json.loads(path.read_text(encoding="utf-8")))

    def test_exact_description_whitespace_contract(self):
        cases = json.loads(
            (ROOT / BUNDLE / "conformance/structural-cases.json").read_text()
        )
        for case in cases["description_cases"]:
            with self.subTest(value=repr(case["value"])):
                value = dict(self.agent, description=case["value"])
                self.assertEqual(self.validator.is_valid(value), case["valid"])

    def test_selector_identity_closure_and_tool_requirements(self):
        for field in ("standard", "description"):
            value = copy.deepcopy(self.agent)
            del value[field]
            self.assertFalse(self.validator.is_valid(value))
        for selector in [
            None,
            "agentpm/1.0.0",
            {},
            {"id": "other", "version": "1.0.0"},
            {"id": "agentpm", "version": "2.0.0"},
            {"id": "agentpm", "version": "1.0.0", "extra": True},
        ]:
            with self.subTest(selector=selector):
                self.assertFalse(
                    self.validator.is_valid(dict(self.agent, standard=selector))
                )
        self.assertFalse(
            self.validator.is_valid(dict(self.agent, name="@example/name"))
        )
        self.assertFalse(self.validator.is_valid(dict(self.agent, standrad={})))
        tool = json.loads(
            (ROOT / BUNDLE / "conformance/fixtures/valid/tool.json").read_text()
        )
        for field in ("entrypoint", "files", "inputs", "outputs"):
            value = dict(tool)
            del value[field]
            self.assertFalse(self.validator.is_valid(value))
        value = copy.deepcopy(tool)
        value["runtime"]["type"] = "node"
        self.assertFalse(
            self.validator.is_valid(value), "Python dependencies stay Python-only"
        )

    def test_generated_entrypoint_and_bundle_mutations_are_detected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            shutil.copytree(ROOT / BUNDLE, root / BUNDLE)
            (root / COMPATIBILITY).parent.mkdir(parents=True)
            shutil.copyfile(ROOT / COMPATIBILITY, root / COMPATIBILITY)
            synchronize(root)
            for relative in [
                COMPATIBILITY,
                SCHEMA,
                BUNDLE / "conformance/fixtures/valid/minimal-agent.json",
                BUNDLE / "semantics.md",
                BUNDLE / "conformance/rule-index.md",
                BUNDLE / "conformance/semantic-cases.md",
            ]:
                path = root / relative
                original = path.read_bytes()
                path.write_bytes(original + b"\n")
                with self.assertRaises(ValueError):
                    synchronize(root)
                path.write_bytes(original)
            original = (root / SCHEMA).read_bytes()
            for relative in (SCHEMA, COMPATIBILITY):
                (root / relative).write_bytes(original + b"\n")
            with self.assertRaisesRegex(ValueError, "checksum drift"):
                synchronize(root)
            for relative in (SCHEMA, COMPATIBILITY):
                (root / relative).write_bytes(original)
            synchronize(root)


if __name__ == "__main__":
    unittest.main()
