"""Check the semantic reference artifacts, not a semantic validator implementation."""

import json
import re
import subprocess
import unittest

from jsonschema import Draft202012Validator

from check_apds_schema import BUNDLE, ROOT, SCHEMA

SEMANTIC_CASE_IDS = list("ABCDEFGHIJKLMNOPQRST")


def anchor(heading):
    return re.sub(r"[^\w -]", "", heading.lower()).replace(" ", "-")


class ApdsSemanticDocumentTests(unittest.TestCase):
    def setUp(self):
        self.bundle = ROOT / BUNDLE
        self.document = (self.bundle / "semantics.md").read_text(encoding="utf-8")
        self.index = (self.bundle / "conformance/rule-index.md").read_text(
            encoding="utf-8"
        )

    def test_bundle_files_are_not_silently_omitted_by_git_ignore(self):
        visible = set(
            subprocess.check_output(
                [
                    "git",
                    "ls-files",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                    "-z",
                    "--",
                    str(BUNDLE),
                ],
                cwd=ROOT,
            ).split(b"\0")
        )
        for path in self.bundle.rglob("*"):
            if path.is_file():
                with self.subTest(file=str(path.relative_to(ROOT))):
                    self.assertIn(path.relative_to(ROOT).as_posix().encode(), visible)

    def test_chapters_and_stable_rule_index_agree(self):
        chapters = re.findall(r"^## (\d+)\. (.+)$", self.document, re.M)
        self.assertEqual([int(number) for number, _ in chapters], list(range(1, 12)))
        rules = re.findall(r"^#{3,4} (APDS-[A-Z]+-\d{3})$", self.document, re.M)
        self.assertEqual(len(rules), len(set(rules)), "Duplicate normative rule ID")
        indexed = re.findall(
            r"^\| \[(APDS-[A-Z]+-\d{3})[^\]]*\]"
            r"\(\.\./semantics\.md#([^\)]+)\)\s*\|([^|]+)\|([^|]+)\|([^|]+)\|$",
            self.index,
            re.M,
        )
        self.assertEqual(set(rules), {row[0] for row in indexed})
        self.assertEqual(len(rules), len(indexed))
        for rule, target, subjects, checks, cases in indexed:
            with self.subTest(rule=rule):
                self.assertEqual(target, rule.lower())
                self.assertTrue(
                    set(subjects.strip().split(", ")) <= {"A", "V", "M", "R"}
                )
                self.assertTrue(set(checks.strip().split(", ")) <= {"S", "C", "P", "R"})
                self.assertTrue(
                    set(cases.strip().split(", ")) <= set(SEMANTIC_CASE_IDS) | {"—"}
                )
        for chapter in re.split(r"^## \d+\. ", self.document, flags=re.M)[1:]:
            self.assertRegex(chapter, r"(?m)^#{3,4} APDS-")

    def test_examples_validate_against_their_actual_schema_definitions(self):
        schema = json.loads((ROOT / SCHEMA).read_text(encoding="utf-8"))
        examples = re.findall(r"^```json\n(.*?)^```", self.document, re.M | re.S)
        # These are explicitly labelled object values, not invented root manifests.
        definitions = ["agentBindings", "loopMetadata", "memoryTransformOperation"]
        self.assertEqual(len(examples), len(definitions))
        for definition, text in zip(definitions, examples):
            with self.subTest(definition=definition):
                validator = Draft202012Validator(
                    {
                        "$schema": schema["$schema"],
                        "$defs": schema["$defs"],
                        "$ref": f"#/$defs/{definition}",
                    }
                )
                value = json.loads(text)
                validator.validate(value)
                # Prove the example check is using the closed definition, not {}.
                self.assertFalse(validator.is_valid(dict(value, unexpected_field=True)))

    def test_all_semantic_cases_have_context_expectation_and_rule_links(self):
        cases = (self.bundle / "conformance/semantic-cases.md").read_text(
            encoding="utf-8"
        )
        rows = re.findall(r"^\| ([A-Z])\s*\|([^|]+)\|([^|]+)\|([^|]+)\|$", cases, re.M)
        self.assertEqual([row[0] for row in rows], SEMANTIC_CASE_IDS)
        rules = set(re.findall(r"^#{3,4} (APDS-[A-Z]+-\d{3})$", self.document, re.M))
        for case, context, expectation, checks in rows:
            with self.subTest(case=case):
                self.assertTrue(context.strip())
                self.assertTrue(expectation.strip())
                targets = re.findall(r"\.\./semantics\.md#(apds-[a-z]+-\d{3})", checks)
                self.assertTrue(targets, "Case has no normative rule reference")
                self.assertTrue({target.upper() for target in targets} <= rules)

    def test_bundle_relative_links_and_rule_anchors_resolve_offline(self):
        for path in self.bundle.rglob("*.md"):
            text = path.read_text(encoding="utf-8")
            for link in re.findall(r"\]\(([^)]+)\)", text):
                if "://" in link:
                    continue
                relative, _, fragment = link.partition("#")
                destination = (path.parent / relative).resolve() if relative else path
                with self.subTest(file=str(path.relative_to(self.bundle)), link=link):
                    self.assertTrue(destination.exists(), "Broken relative link")
                    self.assertTrue(destination.is_relative_to(self.bundle.resolve()))
                    if fragment and destination.suffix == ".md":
                        headings = re.findall(
                            r"^#+ (.+)$", destination.read_text(encoding="utf-8"), re.M
                        )
                        self.assertIn(
                            fragment, {anchor(heading) for heading in headings}
                        )
                    elif fragment and destination.suffix == ".json":
                        value = json.loads(destination.read_text(encoding="utf-8"))
                        for part in fragment.lstrip("/").split("/"):
                            value = value[part.replace("~1", "/").replace("~0", "~")]


if __name__ == "__main__":
    unittest.main()
