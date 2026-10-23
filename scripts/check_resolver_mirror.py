"""Read-only CLI release gate: deployed resolver inventory and bytes must match pins."""

import argparse
import hashlib
import json
import re
import sys
import urllib.error
import urllib.request
from urllib.parse import urlsplit

from check_resolver_targets import validate
from resolver_pins import ROOT, read_targets

MAX_BINARY_BYTES = 200 * 1024 * 1024


def open_storage(url, location):
    parsed = urlsplit(url)
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and parsed.hostname in ("127.0.0.1", "localhost", "::1")
    ):
        raise ValueError("resolver mirror verification requires HTTPS")
    try:
        # The scheme is allowlisted above; no credentials are forwarded from AgentPM.
        return urllib.request.urlopen(url, timeout=30)  # nosec B310
    except urllib.error.HTTPError as error:
        if error.code in (403, 404):
            raise ValueError(
                f"resolver version/target is not mirrored or is inaccessible ({location}, HTTP {error.code}); "
                "check API resolver-route deployment, version inventory, mirrored object and storage permissions"
            ) from None
        raise ValueError(f"resolver {location} HTTP failure: {error.code}") from None
    except (urllib.error.URLError, OSError):
        raise ValueError(f"resolver {location} network/proxy failure") from None


def verify_mirror(pins, base_url):
    validate(pins, read_targets())
    version = pins["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("resolver version must be an exact numeric release")
    for target, pin in sorted(pins["artifacts"].items()):
        endpoint = (
            f"{base_url.rstrip('/')}/v1/tools/install/resolvers/uv/{version}/{target}"
        )
        with open_storage(
            endpoint, f"registry inventory for {version}/{target}"
        ) as response:
            download = json.load(response)
        digest = hashlib.sha256()
        size = 0
        with open_storage(
            download["presigned_url"], f"mirrored object for {version}/{target}"
        ) as response:
            while block := response.read(64 * 1024):
                size += len(block)
                if size > MAX_BINARY_BYTES:
                    raise ValueError(f"resolver binary exceeds size limit: {target}")
                digest.update(block)
        if not size:
            raise ValueError(f"mirrored resolver binary is empty: {target}")
        if digest.hexdigest() != pin["sha256"]:
            raise ValueError(
                f"mirrored resolver digest does not match CLI pin: {version}/{target}"
            )
        print(f"Verified mirrored uv {version} for {target}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base-url",
        required=True,
        help="deployed Registry to verify before CLI release",
    )
    args = parser.parse_args()
    try:
        pins = json.loads((ROOT / "platforms/uv-pins.json").read_text())
        verify_mirror(pins, args.base_url)
    except (ValueError, OSError, KeyError) as error:
        sys.exit(f"Resolver mirror check failed: {error}")


if __name__ == "__main__":
    main()
