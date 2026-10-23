"""Validate the generated resolver table against the canonical CLI platform list."""

import argparse
import json
from pathlib import Path

from resolver_pins import ROOT, read_targets


def validate(pins, targets):
    expected = {target["id"] for target in targets}
    if set(pins["artifacts"]) != expected:
        raise ValueError("CLI release targets and resolver pin table disagree")
    for artifact in pins["artifacts"].values():
        digest = artifact["sha256"]
        if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            raise ValueError("resolver pin must be a full lowercase SHA-256 digest")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix", action="store_true")
    parser.add_argument("--registry-pins", type=Path)
    args = parser.parse_args()
    pins = json.loads((ROOT / "platforms/uv-pins.json").read_text())
    targets = read_targets()
    validate(pins, targets)
    if args.registry_pins and pins != json.loads(args.registry_pins.read_text()):
        raise ValueError("CLI and registry resolver inventories disagree")
    if args.matrix:
        matrix = {"include": [{"target": t["id"], "os": t["os"]} for t in targets]}
        print("matrix=" + json.dumps(matrix, separators=(",", ":")))


if __name__ == "__main__":
    main()
