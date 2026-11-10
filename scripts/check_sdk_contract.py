#!/usr/bin/env python3
"""Compare SDK snapshots with the CLI-owned shared contract (no network)."""
import argparse
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--cli-root", type=Path, default=Path(__file__).resolve().parents[1])
parser.add_argument("--sdk-root", type=Path, action="append", required=True)
args = parser.parse_args()
source = args.cli_root / "crates/agentpm-sdk/contracts/sdk-v1"
for sdk in args.sdk_root:
    tests = "tests" if (sdk / "pyproject.toml").exists() else "test"
    target = sdk / tests / "fixtures/sdk-v1"
    expected = {p.name for p in source.iterdir() if p.is_file()}
    actual = {p.name for p in target.iterdir() if p.is_file()}
    if expected != actual:
        raise SystemExit(f"SDK fixture inventory differs: {sdk}")
    for name in expected:
        if (source / name).read_bytes() != (target / name).read_bytes():
            raise SystemExit(f"SDK fixture drift: {sdk}/{name}")
    print(f"PASS: shared SDK contract matches {sdk.name}")
