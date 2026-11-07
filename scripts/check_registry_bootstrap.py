"""Prevent shipping strict Registry verification without an audited official root."""

import argparse
import base64
import json
from pathlib import Path
from urllib.parse import urlsplit

CONTRACTS = Path(__file__).resolve().parents[1] / "crates/agentpm-sdk/contracts"
ORIGIN = "https://api.agentpackagemanager.com"


def check(roots, fixture, release=False):
    if roots.get("version") != 1 or set(roots) != {"version", "roots"}:
        raise ValueError("unsupported Registry root manifest")
    seen = set()
    fixture_keys = {row["publicKeyB64"] for row in fixture["roots"]}
    for root in roots["roots"]:
        if set(root) != {"registry", "rootKeyId", "publicKeyB64"}:
            raise ValueError("invalid root pin fields")
        origin = urlsplit(root["registry"])
        if (
            not origin.hostname
            or origin.geturl() != root["registry"]
            or origin.path
            or origin.query
            or origin.fragment
            or origin.username is not None
            or origin.password is not None
            or origin.scheme not in ("https", "http")
            or (
                origin.scheme == "http"
                and origin.hostname not in ("localhost", "127.0.0.1", "::1")
            )
        ):
            raise ValueError(
                "root registry must be an exact HTTPS origin or loopback HTTP development origin"
            )
        # Also reject malformed/non-numeric/out-of-range ports.
        _ = origin.port
        if release and root["registry"] != ORIGIN:
            raise ValueError(
                "release root pins must use only the official production HTTPS origin; "
                "remove development/non-production pins from both audited manifests before release"
            )
        raw = base64.b64decode(root["publicKeyB64"], validate=True)
        if len(raw) != 32 or base64.b64encode(raw).decode() != root["publicKeyB64"]:
            raise ValueError(
                "root pin must contain a canonical base64 32-byte public key"
            )
        identity = (root["registry"], root["rootKeyId"])
        if not root["rootKeyId"] or identity in seen:
            raise ValueError("empty/duplicate Registry root ID")
        seen.add(identity)
        if root["publicKeyB64"] in fixture_keys:
            raise ValueError(
                "public fixture root cannot be pinned for any Registry origin"
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
