"""Prevent shipping strict Registry verification without an audited official root."""

import argparse
import base64
import json
from pathlib import Path

CONTRACTS = Path(__file__).resolve().parents[1] / "crates/agentpm-sdk/contracts"
ORIGIN = "https://api.agentpackagemanager.com"


def check(roots, fixture, release=False):
    if roots.get("version") != 1 or set(roots) != {"version", "roots"}:
        raise ValueError("unsupported Registry root manifest")
    seen = set()
    for root in roots["roots"]:
        if set(root) != {"registry", "rootKeyId", "publicKeyB64"}:
            raise ValueError("invalid root pin fields")
        raw = base64.b64decode(root["publicKeyB64"], validate=True)
        if len(raw) != 32 or base64.b64encode(raw).decode() != root["publicKeyB64"]:
            raise ValueError(
                "root pin must contain a canonical base64 32-byte public key"
            )
        identity = (root["registry"], root["rootKeyId"])
        if not root["rootKeyId"] or identity in seen:
            raise ValueError("empty/duplicate Registry root ID")
        seen.add(identity)
        if root["registry"] == ORIGIN and root["publicKeyB64"] in {
            row["publicKeyB64"] for row in fixture["roots"]
        }:
            raise ValueError(
                "public fixture root cannot be used for the official Registry"
            )
    if release and not any(root["registry"] == ORIGIN for root in roots["roots"]):
        raise ValueError(
            "official Registry root bootstrap/pinning is incomplete; do not release this CLI yet"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    try:
        check(
            json.loads((CONTRACTS / "registry-trust-roots-v1.json").read_text()),
            json.loads(
                (CONTRACTS / "registry-provenance-v1.fixtures.json").read_text()
            ),
            args.release,
        )
    except (ValueError, OSError) as error:
        raise SystemExit(str(error)) from error
    print(
        "Registry root manifest valid"
        + (
            " for release"
            if args.release
            else "; operator bootstrap may still be pending"
        )
    )


if __name__ == "__main__":
    main()
