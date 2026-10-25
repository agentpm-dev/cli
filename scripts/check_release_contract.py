"""Check Registry release-schema/vector snapshots against the Rust SDK source."""

import argparse
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry-root", type=Path, required=True)
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1] / "crates/agentpm-sdk/contracts"
    target = args.registry_root / "app/tools/domain/tools/contracts"
    for name in ("package-release-v1.schema.json", "package-release-v1.fixtures.json"):
        if (source / name).read_bytes() != (target / name).read_bytes():
            raise SystemExit(f"Release contract snapshot differs: {name}")
    print("Release schema and shared vectors agree across CLI and Registry.")


if __name__ == "__main__":
    main()
