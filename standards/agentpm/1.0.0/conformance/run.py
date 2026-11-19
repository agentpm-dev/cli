"""Check the portable APDS corpus, or compare an implementation's supplied results.

This is test infrastructure, not an APDS semantic validator or Runner.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path

from jsonschema import Draft202012Validator


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def local_path(root, relative):
    """Fixture inputs must be local even when the fixture describes an unsafe path."""
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise ValueError(f"Invalid corpus path: {relative!r}")
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"Invalid corpus path: {relative!r}")
    result = (root / path).resolve()
    if not result.is_relative_to(root.resolve()):
        raise ValueError(f"Corpus path escapes its root: {relative}")
    return result


def pointer(value, path):
    if path == "":
        return value
    if not path.startswith("/"):
        raise ValueError(f"Invalid JSON pointer: {path}")
    for token in path[1:].split("/"):
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if isinstance(value, list) else value[token]
    return value


def unique(rows, key):
    result = {row[key]: row for row in rows}
    if len(result) != len(rows):
        raise ValueError(f"Duplicate {key}")
    return result


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_bundle(bundle):
    expected = read_json(bundle / "checksums.json")
    actual = {
        p.relative_to(bundle).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(bundle.rglob("*"))
        if p.is_file() and p != bundle / "checksums.json"
    }
    require(actual == expected, "Bundle checksum inventory differs from supplied files")


def load_corpus(directory):
    index_schema = read_json(directory / "fixture-index.schema.json")
    Draft202012Validator.check_schema(index_schema)
    index = read_json(directory / "fixture-index.json")
    Draft202012Validator(index_schema).validate(index)
    observations = read_json(directory / "runner-observations.json")
    Draft202012Validator(index_schema["$defs"]["observations"]).validate(observations)
    return index, observations, index_schema


def check_corpus(directory, index, observations):
    """Check test-data consistency; never derive semantic results from expectations."""
    directory = directory.resolve()
    schema = read_json(directory.parent / "manifest.schema.json")
    validator = Draft202012Validator(schema)
    cases = unique(index["cases"], "case_id")
    scenarios = unique(observations["observations"], "observation_id")
    rules = set(
        re.findall(
            r"^#{3,4} (APDS-[A-Z]+-\d{3})$",
            (directory.parent / "semantics.md").read_text(encoding="utf-8"),
            re.M,
        )
    )
    referenced_files = set()
    for case_id, case in cases.items():
        root = read_json(local_path(directory, case["root"]))
        require(
            validator.is_valid(root) == case["schema_valid"],
            f"{case_id}: unexpected manifest schema result",
        )
        expected = case["expected"]
        require(bool(case["rules"]), f"{case_id}: no applicable rule")
        require(set(case["rules"]) <= rules, f"{case_id}: unknown rule")
        require(
            set(expected["unevaluated"]) <= rules,
            f"{case_id}: unknown unevaluated rule",
        )
        require(
            bool(expected["findings"]) == (expected["status"] == "nonconformant"),
            f"{case_id}: definite violations and status disagree",
        )
        if expected["status"] == "incomplete":
            require(expected["unevaluated"], f"{case_id}: no unevaluated checks")
        if "counterpart" in case:
            require(case["counterpart"] in cases, f"{case_id}: missing counterpart")
            require(case["counterpart"] != case_id, f"{case_id}: self counterpart")
        documents = {case["root"]}
        referenced_files.add(case["root"])
        graph = case.get("graph")
        if graph:
            nodes = unique(graph["artifacts"], "id")
            manifests = {"root": root}
            for node_id, node in nodes.items():
                value = read_json(local_path(directory, node["manifest"]))
                validator.validate(value)
                require(
                    value["kind"] == node["kind"]
                    and value["version"] == node["version"]
                    and node["identity"].endswith("/" + value["name"]),
                    f"{case_id}: graph artifact metadata does not describe its file",
                )
                manifests[node_id] = value
                documents.add(node["manifest"])
                referenced_files.add(node["manifest"])
            edges = set()
            for edge in graph["edges"]:
                require(edge["from"] in manifests, f"{case_id}: missing edge owner")
                require(edge["to"] in nodes, f"{case_id}: missing edge target")
                # The field must exist, but wrong-kind/version selections are
                # intentional negative data for independent semantic validators.
                pointer(manifests[edge["from"]], edge["field"])
                key = (edge["from"], edge["field"])
                require(key not in edges, f"{case_id}: duplicate selection edge")
                edges.add(key)
        package = case.get("package")
        if package and package["availability"] == "complete":
            require("root" in package, f"{case_id}: complete package has no root")
            package_root = local_path(directory, package["root"])
            require(package_root.is_dir(), f"{case_id}: missing package directory")
            require(
                local_path(directory, case["root"]).is_relative_to(package_root),
                f"{case_id}: package root does not contain its manifest",
            )
            documents.update(
                path.relative_to(directory).as_posix()
                for path in package_root.rglob("*.json")
                if path.is_file()
            )
            referenced_files.update(
                path.relative_to(directory).as_posix()
                for path in package_root.rglob("*")
                if path.is_file()
            )
        for finding in expected["findings"]:
            require(finding["rule_id"] in rules, f"{case_id}: unknown finding rule")
            require(
                finding["document"] in documents,
                f"{case_id}: diagnostic names a document outside its input context",
            )
            # A missing-required-field pointer may intentionally not exist.
            require(
                finding["path"] == "" or finding["path"].startswith("/"),
                f"{case_id}: diagnostic path is not a JSON pointer",
            )
    for observation in scenarios.values():
        require(observation["fixture_case"] in cases, "Unknown observation fixture")
        require(set(observation["rules"]) <= rules, "Unknown observation rule")
    coverage = unique(index["rule_coverage"], "rule_id")
    require(set(coverage) == rules, "Rule coverage differs from semantic reference")
    for rule, row in coverage.items():
        require(
            set(row["validator_cases"])
            == {
                c["case_id"]
                for c in cases.values()
                if rule in c["rules"] or rule in c["expected"]["unevaluated"]
            },
            f"{rule}: stale validator coverage",
        )
        require(
            set(row["runner_observations"])
            == {o["observation_id"] for o in scenarios.values() if rule in o["rules"]},
            f"{rule}: stale Runner coverage",
        )
    letters = set("ABCDEFGHIJKLMNOPQRST")
    mapped = {s for c in cases.values() for s in c["semantic_cases"]} | {
        s for o in scenarios.values() for s in o["semantic_cases"]
    }
    require(mapped == letters, "A–T interpretation mapping is incomplete")
    require(
        not any(set(c["semantic_cases"]) & set("QRST") for c in cases.values()),
        "Stateful Q–T observations cannot be manifest-validation cases",
    )
    compatibility = read_json(directory / "compatibility/index.json")
    for case in compatibility["cases"]:
        require(case["case_id"] not in cases, "Compatibility case in normative suite")
        local_path(directory, case["root"]).read_bytes()
        referenced_files.add(case["root"])
    supplied_files = {
        path.relative_to(directory).as_posix()
        for path in (directory / "fixtures").rglob("*")
        if path.is_file()
    }
    require(supplied_files == referenced_files, "Unindexed or missing fixture files")
    return cases


def compare_results(cases, actual, result_schema):
    """Compare a corpus-local projection, not an APDS diagnostic wire protocol."""
    require(isinstance(actual, list), "Results must be a JSON array")
    validator = Draft202012Validator(result_schema)
    for row in actual:
        validator.validate(row)
    rows = unique(actual, "case_id")
    require(set(rows) == set(cases), "Result IDs must match the selected cases exactly")
    for case_id, case in cases.items():
        row = rows[case_id]
        require(row["level"] == case["level"], f"{case_id}: wrong evaluation level")
        require(
            row["status"] == case["expected"]["status"],
            f"{case_id}: expected {case['expected']['status']}, got {row['status']}",
        )
        key = lambda f: (f["rule_id"], f["document"], f["path"], f["severity"])
        actual_findings = [key(f) for f in row["findings"]]
        require(
            len(actual_findings) == len(set(actual_findings))
            and set(actual_findings) == {key(f) for f in case["expected"]["findings"]},
            f"{case_id}: findings differ (rule/document/path/severity)",
        )
        require(
            len(row["unevaluated"]) == len(set(row["unevaluated"]))
            and set(row["unevaluated"]) == set(case["expected"]["unevaluated"]),
            f"{case_id}: unevaluated checks differ",
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", help="Check one case ID instead of the full corpus")
    parser.add_argument("--list", action="store_true", help="List validation case IDs")
    parser.add_argument(
        "--results", type=Path, help="Compare independently produced results"
    )
    args = parser.parse_args()
    directory = Path(__file__).resolve().parent
    try:
        check_bundle(directory.parent)
        index, observations, schema = load_corpus(directory)
        cases = check_corpus(directory, index, observations)
        if args.case:
            require(args.case in cases, f"Unknown case: {args.case}")
            cases = {args.case: cases[args.case]}
        if args.list:
            for case in cases.values():
                print(
                    f"{case['case_id']}\t{case['level']}\t{case['expected']['status']}"
                )
            return
        if args.results:
            compare_results(cases, read_json(args.results), schema["$defs"]["result"])
            print(f"PASS: {len(cases)} supplied implementation result(s) match.")
        else:
            for case in cases.values():
                print(f"PASS corpus/schema: {case['case_id']}")
            print(f"PASS: {len(cases)} case(s); bundle integrity and corpus checks.")
            print(
                "Semantic outcomes were not evaluated. Supply --results to compare them."
            )
        print(
            f"{len(observations['observations'])} Runner observations are catalogued; "
            "no Runner behavior was executed."
        )
    except Exception as error:
        parser.exit(1, f"FAIL: {error}\n")


if __name__ == "__main__":
    main()
