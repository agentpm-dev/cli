"""Generate resolver pins from attested upstream bytes, never upstream checksums."""

import argparse
import hashlib
import json
import re

# The maintainer-selected attestation verifier is trusted; no shell is invoked.
import subprocess  # nosec B404
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ROOT / "crates/agentpm-sdk/platforms/targets.json"


def read_targets(path=TARGETS):
    targets = json.loads(path.read_text())["targets"]
    ids = [target["id"] for target in targets]
    if not ids or len(ids) != len(set(ids)) or "any" in ids:
        raise ValueError("supported resolver targets must be unique and concrete")
    return targets


def verify_archive(path, gh):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    url = f"https://api.github.com/repos/astral-sh/uv/attestations/sha256:{digest}"
    # Fixed HTTPS GitHub host; the only interpolated path value is a SHA-256 digest.
    with urllib.request.urlopen(url, timeout=60) as response:  # nosec B310
        attestations = json.load(response)["attestations"]
    if not attestations:
        raise ValueError("upstream artifact has no attestation")
    bundle = path.with_name(path.name + ".attestations.jsonl")
    bundle.write_text(
        "".join(json.dumps(item["bundle"]) + "\n" for item in attestations)
    )
    # gh verifies the signature, transparency evidence, subject digest and source repo.
    subprocess.run(  # nosec B603
        [
            gh,
            "attestation",
            "verify",
            str(path),
            "--repo",
            "astral-sh/uv",
            "--bundle",
            str(bundle),
        ],
        check=True,
    )


def binary_from_archive(archive, member):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            data = source.read(member)
    else:
        with tarfile.open(archive, "r:gz") as source:
            entry = source.getmember(member)
            if not entry.isfile() or entry.size > 200 * 1024 * 1024:
                raise ValueError("uv binary must be a bounded regular archive member")
            data = source.extractfile(entry).read()
    if not data or len(data) > 200 * 1024 * 1024:
        raise ValueError("invalid uv binary size")
    return data


def mirror_binary(s3, bucket, key, data):
    digest = hashlib.sha256(data).hexdigest()
    # A conditional PUT makes keys immutable even when maintainers run concurrently.
    try:
        s3.put_object(
            Bucket=bucket,
            Key=key,
            Body=data,
            IfNoneMatch="*",
            ContentType="application/octet-stream",
            Metadata={"sha256": digest},
        )
    except s3.exceptions.ClientError as error:
        if error.response["Error"]["Code"] not in ("PreconditionFailed", "412"):
            raise
        existing = s3.get_object(Bucket=bucket, Key=key)["Body"].read()
        if hashlib.sha256(existing).hexdigest() != digest:
            raise ValueError(
                f"immutable resolver key already contains different bytes: {key}"
            ) from error


def stage_binary(version, staging, target, gh):
    archive = staging / target["uv_archive"]
    url = f"https://github.com/astral-sh/uv/releases/download/{version}/{archive.name}"
    # HTTPS GitHub URL, numeric validated version, checked-in platform asset name.
    with urllib.request.urlopen(url, timeout=120) as response:  # nosec B310
        archive.write_bytes(response.read())
    verify_archive(archive, gh)
    data = binary_from_archive(archive, target["uv_binary"])
    binary = (
        staging / target["id"] / ("uv.exe" if target["id"].endswith("msvc") else "uv")
    )
    binary.parent.mkdir(parents=True, exist_ok=True)
    binary.write_bytes(data)
    binary.chmod(0o755)
    return binary, data


def generate(
    version,
    staging,
    gh="gh",
    bucket=None,
    prefix="runtimes/uv",
    s3=None,
    history=None,
    smoke_results=None,
    smoke_target=None,
):
    from resolver_smoke import smoke, verify_smokes

    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("uv version must be an exact numeric release")
    prefix = prefix.strip("/")
    if prefix != "runtimes/uv" and not prefix.endswith("/runtimes/uv"):
        raise ValueError(
            "resolver prefix must end in runtimes/uv, matching Registry storage"
        )
    staging.mkdir(parents=True, exist_ok=True)
    targets = read_targets()
    if smoke_target:
        if bucket:
            raise ValueError("native smoke mode cannot upload or write pin tables")
        target = next((t for t in targets if t["id"] == smoke_target), None)
        if target is None:
            raise ValueError("unknown native smoke target")
        binary, _ = stage_binary(version, staging, target, gh)
        report = smoke(binary, version, target["id"])
        destination = staging / "smoke"
        destination.mkdir(exist_ok=True)
        (destination / f"{target['id']}.json").write_text(
            json.dumps(report, indent=2) + "\n"
        )
        return None
    artifacts = {}
    staged = []
    for target in targets:
        binary, data = stage_binary(version, staging, target, gh)
        artifacts[target["id"]] = {"sha256": hashlib.sha256(data).hexdigest()}
        staged.append((target["id"], binary, data))
    expected = {target["id"] for target in read_targets()}
    if set(artifacts) != expected:
        raise ValueError("missing supported target; refusing partial pin table")
    pins = {
        "_generated": "scripts/resolver_pins.py; DO NOT EDIT",
        "version": version,
        "artifacts": artifacts,
    }
    if history and history.exists() and json.loads(history.read_text()) != pins:
        raise ValueError("refusing to overwrite an existing resolver version inventory")
    # All native execution results must match these exact bytes before the first PUT.
    verify_smokes(
        [(target, binary, artifacts[target]["sha256"]) for target, binary, _ in staged],
        version,
        smoke_results,
    )
    if bucket:
        for target, binary, data in staged:
            mirror_binary(
                s3, bucket, f"{prefix}/{version}/{target}/{binary.name}", data
            )
    return pins


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("--staging", type=Path, required=True)
    parser.add_argument("--gh", default="gh")
    parser.add_argument(
        "--smoke-results",
        type=Path,
        help="native target reports from the Resolver Contract workflow",
    )
    parser.add_argument(
        "--smoke-target",
        help="stage and execute only this native target; emits a report, not pins (requires --stage-only)",
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--bucket",
        help="existing AgentPM registry S3 bucket; writes immutable resolver objects",
    )
    mode.add_argument(
        "--stage-only",
        action="store_true",
        help="verify and generate locally; does NOT satisfy mirror deployment acceptance",
    )
    parser.add_argument("--prefix", default="runtimes/uv")
    parser.add_argument("--output", type=Path, default=ROOT / "platforms/uv-pins.json")
    parser.add_argument(
        "--registry-output",
        type=Path,
        help="write matching registry inventory after successful generation",
    )
    args = parser.parse_args()
    if sys.version_info < (3, 11):
        parser.error(
            "Resolver ingestion requires Python 3.11+; use ../agentpm-api/.venv/bin/python"
        )
    if args.smoke_target and not args.stage_only:
        parser.error(
            "--smoke-target requires --stage-only and never uploads or writes pins"
        )
    if args.bucket is not None and not args.bucket.strip():
        parser.error(
            "--bucket must name the existing Registry S3 bucket; S3_BUCKET is empty"
        )
    s3 = None
    if args.bucket:
        import boto3  # Only maintainer ingestion needs the registry's existing dependency.

        s3 = boto3.client("s3")
    history = (
        args.registry_output.parent / "uv-pins" / f"{args.version}.json"
        if args.registry_output
        else None
    )
    pins = generate(
        args.version,
        args.staging,
        gh=args.gh,
        bucket=args.bucket,
        prefix=args.prefix,
        s3=s3,
        history=history,
        smoke_results=args.smoke_results,
        smoke_target=args.smoke_target,
    )
    if args.smoke_target:
        print(
            f"Native uv contract passed; report: {args.staging / 'smoke' / (args.smoke_target + '.json')}"
        )
        return
    encoded = json.dumps(pins, indent=2, sort_keys=True) + "\n"
    args.output.write_text(encoded)
    if args.registry_output:
        history.parent.mkdir(parents=True, exist_ok=True)
        history.write_text(encoded)
        args.registry_output.write_text(encoded)
    print(
        f"Generated {args.output}; {'mirrored' if args.bucket else 'STAGED ONLY, mirror deployment still required'}"
    )


if __name__ == "__main__":
    main()
