"""Check Registry release-schema/vector snapshots against the Rust SDK source."""

import argparse
from pathlib import Path

CONTRACT_FILES = (
    "package-release-v1.schema.json",
    "package-release-v1.fixtures.json",
    "signing-canonicalization-v1.fixtures.json",
)


def check_contracts(source: Path, target: Path) -> None:
    for name in CONTRACT_FILES:
        if (source / name).read_bytes() != (target / name).read_bytes():
            raise ValueError(f"Release contract snapshot differs: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry-root", type=Path, required=True)
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1] / "crates/agentpm-sdk/contracts"
    target = args.registry_root / "app/tools/domain/tools/contracts"
    try:
        check_contracts(source, target)
    except (OSError, ValueError) as error:
        raise SystemExit(str(error)) from error
    print("Release schema, signing and shared vectors agree across CLI and Registry.")


if __name__ == "__main__":
    main()
