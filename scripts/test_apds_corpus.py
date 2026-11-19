"""Test portable corpus integrity and result comparison, not APDS semantic engines."""

import copy
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from jsonschema import ValidationError

from check_apds_schema import BUNDLE, ROOT

# Importing test infrastructure must not add bytecode to the immutable bundle.
sys.dont_write_bytecode = True
DIRECTORY = ROOT / BUNDLE / "conformance"
spec = importlib.util.spec_from_file_location("apds_corpus", DIRECTORY / "run.py")
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)


class ApdsCorpusTests(unittest.TestCase):
    def setUp(self):
        self.index, self.observations, self.schema = corpus.load_corpus(DIRECTORY)
        self.cases = {c["case_id"]: c for c in self.index["cases"]}

    def results(self):
        return [
            dict(case_id=c["case_id"], level=c["level"], **copy.deepcopy(c["expected"]))
            for c in self.cases.values()
        ]

    def test_whole_corpus_and_rule_mapping(self):
        checked = corpus.check_corpus(DIRECTORY, self.index, self.observations)
        self.assertEqual(set(checked), set(self.cases))
        self.assertEqual(
            {c["expected"]["status"] for c in checked.values()},
            {"conformant", "nonconformant", "incomplete", "unsupported"},
        )
        for kind in [
            "agent",
            "tool",
            "skill",
            "knowledge",
            "memory",
            "profile",
            "loop",
            "template",
        ]:
            self.assertIn("valid-" + kind, checked)
            self.assertIn("invalid-" + kind + "-required", checked)
        for case in checked.values():
            if case["expected"]["status"] == "nonconformant":
                self.assertIn("counterpart", case, case["case_id"])
                self.assertEqual(
                    checked[case["counterpart"]]["expected"]["status"], "conformant"
                )
                self.assertTrue(
                    set(case["rules"]) <= set(checked[case["counterpart"]]["rules"]),
                    f"{case['case_id']}: counterpart does not cover the same rules",
                )

    def test_bad_expectation_and_stale_rule_mapping_fail(self):
        for mutate in [
            lambda i: i["cases"].append(copy.deepcopy(i["cases"][0])),
            lambda i: i["cases"][0].update(schema_valid=False),
            lambda i: i["cases"][0]["expected"].update(status="nonconformant"),
            lambda i: i["rule_coverage"][0]["validator_cases"].append("not-a-case"),
            lambda i: i["cases"][0].update(rules=["APDS-NOTREAL-999"]),
            lambda i: i["cases"][0].update(root="../manifest.schema.json"),
        ]:
            index = copy.deepcopy(self.index)
            mutate(index)
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                corpus.check_corpus(DIRECTORY, index, self.observations)

    def test_comparator_accepts_exact_results_and_rejects_false_claims(self):
        corpus.compare_results(
            self.cases, self.results(), self.schema["$defs"]["result"]
        )
        for field, value in [
            ("status", "unsupported"),
            ("level", "resolved"),
            (
                "findings",
                [
                    {
                        "rule_id": "APDS-ID-002",
                        "document": "other.json",
                        "path": "/name",
                        "severity": "error",
                    }
                ],
            ),
            ("unevaluated", ["APDS-ID-002"]),
        ]:
            rows = self.results()
            rows[0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                corpus.compare_results(self.cases, rows, self.schema["$defs"]["result"])
        for rows in [self.results()[:-1], self.results() + [self.results()[0]]]:
            with self.assertRaises(ValueError):
                corpus.compare_results(self.cases, rows, self.schema["$defs"]["result"])

    def test_negative_finding_rule_path_document_and_severity_are_checked(self):
        case = self.cases["description-whitespace"]
        cases = {case["case_id"]: case}
        good = dict(
            case_id=case["case_id"],
            level=case["level"],
            **copy.deepcopy(case["expected"]),
        )
        corpus.compare_results(cases, [good], self.schema["$defs"]["result"])
        falsely_accepted = copy.deepcopy(good)
        falsely_accepted.update(status="conformant", findings=[])
        with self.assertRaisesRegex(ValueError, "expected nonconformant"):
            corpus.compare_results(
                cases, [falsely_accepted], self.schema["$defs"]["result"]
            )
        for field, value in [
            ("rule_id", "APDS-ID-001"),
            ("document", "wrong.json"),
            ("path", "/name"),
            ("severity", "warning"),
        ]:
            row = copy.deepcopy(good)
            row["findings"][0][field] = value
            with self.subTest(field=field), self.assertRaises(
                (ValueError, ValidationError)
            ):
                corpus.compare_results(cases, [row], self.schema["$defs"]["result"])

    def test_checksum_detects_changed_fixture_and_extra_file(self):
        with tempfile.TemporaryDirectory() as folder:
            bundle = Path(folder) / "bundle"
            shutil.copytree(ROOT / BUNDLE, bundle)
            corpus.check_bundle(bundle)
            path = bundle / "conformance" / self.cases["composition-complete"]["root"]
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.assertRaisesRegex(ValueError, "checksum"):
                corpus.check_bundle(bundle)
            path.write_bytes(original)
            (bundle / "unexpected.txt").write_text("unversioned")
            with self.assertRaisesRegex(ValueError, "checksum"):
                corpus.check_bundle(bundle)

    def test_graph_metadata_and_missing_edges_fail_as_corpus_errors(self):
        for mutate in [
            lambda g: g["artifacts"][0].update(version="9.0.0"),
            lambda g: g["edges"][0].update(to="absent"),
            lambda g: g["edges"][0].update(field="/missing"),
        ]:
            index = copy.deepcopy(self.index)
            case = next(
                c for c in index["cases"] if c["case_id"] == "composition-complete"
            )
            mutate(case["graph"])
            with self.assertRaises((ValueError, KeyError)):
                corpus.check_corpus(DIRECTORY, index, self.observations)

    def test_selected_versions_are_edges_not_name_only_candidates(self):
        case = self.cases["composition-two-tool-versions"]
        graph = case["graph"]
        selected = {(e["from"], e["field"]): e["to"] for e in graph["edges"]}
        self.assertEqual(selected[("root", "/tools/0")], "tool:echo@1.0.0")
        self.assertEqual(selected[("root", "/skills/0")], "skill:review@2.0.0")
        self.assertEqual(
            selected[("skill:review@2.0.0", "/tools/0")], "tool:echo@2.0.0"
        )

    def test_stateful_triggers_are_runner_sequences_only(self):
        observations = {
            o["observation_id"]: o for o in self.observations["observations"]
        }
        for trigger in ["record_count", "capacity"]:
            self.assertTrue(
                observations["trigger-initial-" + trigger]["expected"][
                    "count_condition_satisfied"
                ]
            )
            self.assertEqual(
                observations["trigger-unchanged-" + trigger]["expected"][
                    "count_condition_satisfied"
                ],
                [False, False],
            )
            self.assertEqual(
                observations["trigger-rearm-" + trigger]["expected"][
                    "count_condition_satisfied"
                ],
                [False, True],
            )
        interval = observations["interval-baseline"]["expected"]
        self.assertEqual(interval["11:00"], "dormant-no-baseline")
        self.assertEqual(interval["12:10"], "eligible")
        self.assertEqual(interval["12:21"], "ineligible")
        self.assertEqual(interval["12:22"], "eligible")

    def test_legacy_inference_is_outside_normative_authoring_corpus(self):
        adapter = corpus.read_json(DIRECTORY / "compatibility/index.json")["cases"][0]
        self.assertNotIn(adapter["case_id"], self.cases)
        self.assertEqual(adapter["expected"]["conformance"], "not_verified")
        self.assertFalse(adapter["expected"]["explicit_declaration"])
        self.assertNotIn("standard", corpus.read_json(DIRECTORY / adapter["root"]))

    def test_quickstart_runs_from_a_bundle_without_repository_services(self):
        with tempfile.TemporaryDirectory() as folder:
            bundle = Path(folder) / "bundle"
            shutil.copytree(ROOT / BUNDLE, bundle)
            command = [sys.executable, str(bundle / "conformance/run.py")]
            env = dict(
                os.environ,
                PYTHONDONTWRITEBYTECODE="1",
                AGENTPM_BASE_URL="http://127.0.0.1:1",
            )
            for args, expected_exit, text in [
                ([], 0, "Semantic outcomes were not evaluated"),
                (
                    ["--case", "description-whitespace"],
                    0,
                    "PASS corpus/schema: description-whitespace",
                ),
                (["--list"], 0, "graph-unavailable"),
                (["--case", "not-a-case"], 1, "Unknown case"),
            ]:
                result = subprocess.run(
                    command + args,
                    cwd=folder,
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=30,
                )
                self.assertEqual(result.returncode, expected_exit, result.stderr)
                self.assertIn(text, result.stdout + result.stderr)
            case = self.cases["description-whitespace"]
            results = Path(folder) / "results.json"
            results.write_text(
                json.dumps(
                    [
                        dict(
                            case_id=case["case_id"],
                            level=case["level"],
                            **case["expected"],
                        )
                    ]
                ),
                encoding="utf-8",
            )
            result = subprocess.run(
                command + ["--case", case["case_id"], "--results", str(results)],
                cwd=folder,
                env=env,
                capture_output=True,
                text=True,
                timeout=30,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("supplied implementation result(s) match", result.stdout)


if __name__ == "__main__":
    unittest.main()
