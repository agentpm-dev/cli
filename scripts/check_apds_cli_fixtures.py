"""Probe APDS fixtures through AgentPM's lint and known Knowledge representation.

This AgentPM-specific acceptance probe is separate from the portable corpus and
does not claim that lint implements the full APDS semantic suite.
"""

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path

from check_apds_schema import BUNDLE, ROOT


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    cli = args.cli.resolve()
    directory = ROOT / BUNDLE / "conformance"
    index = json.loads((directory / "fixture-index.json").read_text(encoding="utf-8"))
    paths = set()
    for case in index["cases"]:
        if case["expected"]["status"] != "conformant":
            continue
        paths.add(case["root"])
        paths.update(n["manifest"] for n in case.get("graph", {}).get("artifacts", []))
    with tempfile.TemporaryDirectory(prefix="apds-cli-fixtures-") as folder:
        environment = dict(os.environ, AGENTPM_TELEMETRY="0")
        environment["AGENTPM_BASE_URL"] = "http://127.0.0.1:1"
        for key in ["XDG_CONFIG_HOME", "XDG_CACHE_HOME", "APPDATA", "LOCALAPPDATA"]:
            environment[key] = str(Path(folder) / key)
        for relative in sorted(paths):
            result = subprocess.run(
                [
                    str(cli),
                    "lint",
                    "--apds",
                    "standalone",
                    "--format",
                    "json",
                    str(directory / relative),
                ],
                cwd=folder,
                env=environment,
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=30,
            )
            if result.returncode:
                parser.exit(1, f"FAIL {relative}:\n{result.stdout}\n{result.stderr}\n")
            try:
                reports = json.loads(result.stdout)
                if len(reports) != 1 or reports[0]["apds"]["status"] != "conformant":
                    raise ValueError(
                        "Expected one actual conformant standalone APDS report"
                    )
            except (ValueError, KeyError, TypeError) as error:
                parser.exit(1, f"FAIL {relative}: {error}\n{result.stdout}\n")
        print(f"PASS: {len(paths)} positive manifests accepted by actual AgentPM lint.")
        for case_id, should_pass in [
            ("knowledge-vector-files", True),
            ("knowledge-vector-truncated", False),
        ]:
            case = next(c for c in index["cases"] if c["case_id"] == case_id)
            result = subprocess.run(
                [
                    str(cli),
                    "knowledge",
                    "inspect",
                    str(directory / case["package"]["root"]),
                    "--json",
                ],
                cwd=folder,
                env=environment,
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=30,
            )
            if (result.returncode == 0) != should_pass:
                parser.exit(1, f"FAIL {case_id}: {result.stdout}\n{result.stderr}\n")
            if should_pass:
                inspected = json.loads(result.stdout)
                if not (
                    inspected["manifest_metadata_fresh"] and inspected["index"]["fresh"]
                ):
                    parser.exit(1, f"FAIL {case_id}: stale built Knowledge content\n")
            elif "byte length must be divisible by 4" not in result.stderr:
                parser.exit(
                    1, f"FAIL {case_id}: unexpected rejection: {result.stderr}\n"
                )
        print(
            "PASS: actual Knowledge reader accepts fresh vectors and rejects truncation."
        )
        print("This acceptance probe is not the full APDS semantic-validator suite.")


if __name__ == "__main__":
    main()
