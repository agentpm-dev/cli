"""Check the canonical APDS bundle and its generated compatibility entrypoint."""

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = Path("standards/agentpm/1.0.0")
SCHEMA = BUNDLE / "manifest.schema.json"
COMPATIBILITY = Path("schemas/agentpm.manifest.schema.json")
CHECKSUMS = BUNDLE / "checksums.json"
SCHEMA_ID = (
    "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/"
    "standards/agentpm/1.0.0/manifest.schema.json"
)


def inventory(root):
    bundle = root / BUNDLE
    return {
        path.relative_to(bundle)
        .as_posix(): hashlib.sha256(path.read_bytes())
        .hexdigest()
        for path in sorted(bundle.rglob("*"))
        if path.is_file() and path != root / CHECKSUMS
    }


def synchronize(root, write=False):
    """Generate only derived files; schema and prose remain the authored source."""
    source = (root / SCHEMA).read_bytes()
    schema = json.loads(source)
    if schema["$id"] != SCHEMA_ID:
        raise ValueError("APDS schema identity must use the pinned version tag")
    if schema["properties"]["standard"]["properties"] != {
        "id": {"const": "agentpm"},
        "version": {"const": "1.0.0"},
    }:
        raise ValueError("APDS selector must identify agentpm 1.0.0")
    if write:
        (root / COMPATIBILITY).write_bytes(source)
        (root / CHECKSUMS).write_text(
            json.dumps(inventory(root), indent=2) + "\n", encoding="utf-8"
        )
    else:
        if (root / COMPATIBILITY).read_bytes() != source:
            raise ValueError(
                "Generated manifest schema differs from canonical APDS source"
            )
        if json.loads((root / CHECKSUMS).read_text(encoding="utf-8")) != inventory(
            root
        ):
            raise ValueError("APDS bundle checksum drift (including docs and fixtures)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="Synchronize a reviewed, unpublished candidate",
    )
    args = parser.parse_args()
    try:
        synchronize(ROOT, args.write)
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"APDS bundle check failed: {error}\n")
    print("APDS schema copy and bundle checksums agree.")


if __name__ == "__main__":
    main()
