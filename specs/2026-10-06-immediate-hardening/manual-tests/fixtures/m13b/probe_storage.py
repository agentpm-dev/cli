"""Tiny explicit production S3 probe; no Registry publication or configuration edits."""

import argparse
import base64
import gzip
import hashlib
import io
import json
import os
import tarfile
import uuid
from contextlib import closing
from pathlib import Path

import boto3
import jsonschema
import rfc8785
from botocore.config import Config
from botocore.exceptions import BotoCoreError, ClientError, NoCredentialsError

BUCKET = "agentpm-tools"
CALLER = "arn:aws:iam::510092505185:user/agentpm-reader-writer"
REPO = Path(__file__).resolve().parents[5]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def payload(identity):
    identity = str(uuid.UUID(identity))
    name = "band9-storage-probe-" + uuid.UUID(identity).hex
    manifest = {
        "kind": "tool",
        "name": name,
        "version": "0.0.0",
        "description": "Benign storage-readiness probe; not a Registry publication.",
        "entrypoint": {"command": "python3", "args": ["tool.py"]},
        "runtime": {"type": "python", "version": "3.11"},
        "files": ["tool.py", "README.md"],
        "inputs": {"type": "object"},
        "outputs": {"type": "object"},
        "readme": "README.md",
        "license": {"spdx": "MIT"},
    }
    entries = {
        "agent.json": rfc8785.dumps(manifest),
        "agentpm-artifact.json": b'{"format":"tar.gz","target":"any"}',
        "tool.py": b'import json\nprint(json.dumps({"ok": True}))\n',
        "README.md": b"# Benign storage readiness probe\n",
    }
    raw = io.BytesIO()
    with gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode="w") as archive:
            for name, data in sorted(entries.items()):
                member = tarfile.TarInfo(name)
                member.size, member.mode = len(data), 0o644
                archive.addfile(member, io.BytesIO(data))
    artifact = raw.getvalue()
    release = {
        "type": "agentpm.package.release.v1",
        "kind": "tool",
        "name": "@zack/" + manifest["name"],
        "version": "0.0.0",
        "manifestDigest": "sha256:" + digest(rfc8785.dumps(manifest)),
        "artifacts": [
            {
                "target": "any",
                "digest": "sha256:" + digest(artifact),
                "size": len(artifact),
                "contentType": "application/gzip",
                "format": "tar.gz",
            }
        ],
    }
    schema = REPO / "crates/agentpm-sdk/contracts/package-release-v1.schema.json"
    jsonschema.validate(release, json.loads(schema.read_text()))
    document = rfc8785.dumps(release)
    if len(artifact) + len(document) > 16_384:
        raise ValueError("probe exceeds its 16 KiB payload budget")
    stem = f"tools/zack/{manifest['name']}/0.0.0/"
    keys = {
        "staging": f"uploads/{identity}/{uuid.uuid5(uuid.UUID(identity), 'artifact')}.tar.gz",
        "artifact": stem + f"artifacts/any/{digest(artifact)}.tar.gz",
        "document": stem + "release.json",
    }
    return artifact, document, keys


def save(root, state):
    temporary = root / "state.json.tmp"
    with open(temporary, "w", opener=lambda path, flags: os.open(path, flags, 0o600)) as output:
        json.dump(state, output, indent=2)
        output.write("\n")
    temporary.replace(root / "state.json")


def prepare(root):
    root.mkdir(parents=True, exist_ok=True)
    # A fresh root owns the UUID-derived keys; never adopt arbitrary supplied keys.
    with open(root / "state.json", "x", opener=lambda path, flags: os.open(path, flags, 0o600)):
        pass
    identity = str(uuid.uuid4())
    artifact, document, keys = payload(identity)
    save(
        root,
        {"probe_id": identity, "bucket": BUCKET, "expected_caller": CALLER, "phase": "prepared"},
    )
    (root / "artifact.tar.gz").write_bytes(artifact)
    (root / "release.json").write_bytes(document)
    source = root / "source"
    source.mkdir()
    with tarfile.open(fileobj=io.BytesIO(artifact), mode="r:gz") as archive:
        for name in ("agent.json", "tool.py", "README.md"):
            with closing(archive.extractfile(name)) as member:
                (source / name).write_bytes(member.read())
    print("Prepared locally:", len(artifact), "archive bytes;", len(document), "document bytes.")
    for kind, key in keys.items():
        print(kind + ":", key)
    print("No AWS call or publication occurred. Final objects remain as probe evidence.")


def verify(storage, key, data, checksum=False):
    options = {"ChecksumMode": "ENABLED"} if checksum else {}
    head = storage.head_object(Bucket=BUCKET, Key=key, **options)
    if head.get("ContentLength") != len(data) or head.get("ServerSideEncryption") != "AES256":
        raise ValueError("size/encryption verification failed: " + key)
    if (
        checksum
        and head.get("ChecksumSHA256") != base64.b64encode(hashlib.sha256(data).digest()).decode()
    ):
        raise ValueError("S3 checksum verification failed: " + key)
    with closing(storage.get_object(Bucket=BUCKET, Key=key)["Body"]) as body:
        if body.read(len(data) + 1) != data:
            raise ValueError("stored bytes differ: " + key)


def put_once(storage, key, data, content_type, checksum):
    extra = {}
    if checksum:
        extra = {
            "ChecksumAlgorithm": "SHA256",
            "ChecksumSHA256": base64.b64encode(hashlib.sha256(data).digest()).decode(),
        }
    try:
        result = storage.put_object(
            Bucket=BUCKET,
            Key=key,
            Body=data,
            ContentLength=len(data),
            ContentType=content_type,
            IfNoneMatch="*",
            **extra,
        )
    except ClientError as error:
        if error.response.get("Error", {}).get("Code") not in ("412", "PreconditionFailed"):
            raise
        print("Exact existing probe object; verifying:", key)
    else:
        if checksum and result.get("ChecksumSHA256") != extra["ChecksumSHA256"]:
            raise ValueError("PUT response did not confirm SHA-256: " + key)
    verify(storage, key, data, checksum)


def run(command, root):
    state = json.loads((root / "state.json").read_text())
    if state.get("bucket") != BUCKET or state.get("expected_caller") != CALLER:
        raise ValueError("unexpected probe identity; use this helper's own prepared state")
    artifact, document, keys = payload(state["probe_id"])
    if (root / "artifact.tar.gz").read_bytes() != artifact or (
        root / "release.json"
    ).read_bytes() != document:
        raise ValueError("prepared probe bytes changed; stop before any AWS operation")
    config = Config(connect_timeout=10, read_timeout=30, retries={"max_attempts": 1})
    session = boto3.Session(region_name="us-east-1")
    caller = session.client("sts", config=config).get_caller_identity()["Arn"]
    if caller != CALLER:
        raise ValueError("unexpected AWS caller: " + caller)
    print("Caller:", caller)
    storage = session.client("s3", config=config)
    if command == "upload":
        if state["phase"] != "prepared":
            raise ValueError("upload already completed; use check or cleanup")
        put_once(storage, keys["staging"], artifact, "application/gzip", True)
        put_once(storage, keys["artifact"], artifact, "application/gzip", True)
        put_once(storage, keys["document"], document, "application/json", False)
        # Prove create-only semantics without changing the existing bytes.
        try:
            storage.put_object(
                Bucket=BUCKET,
                Key=keys["artifact"],
                Body=artifact,
                ContentLength=len(artifact),
                ContentType="application/gzip",
                IfNoneMatch="*",
            )
        except ClientError as error:
            if error.response.get("Error", {}).get("Code") not in ("412", "PreconditionFailed"):
                raise
            print("PASS: repeat conditional PUT rejected with 412.")
        else:
            raise ValueError("conditional PUT unexpectedly replaced an existing object")
        state["phase"] = "uploaded"
        print("PASS: staging/final writes, final readback, SHA-256 and SSE-S3.")
    elif command == "check":
        if state["phase"] == "prepared":
            raise ValueError("run upload first")
        verify(storage, keys["artifact"], artifact, True)
        verify(storage, keys["document"], document)
        statuses = {}
        for kind in ("artifact", "document"):
            tags = storage.get_object_tagging(Bucket=BUCKET, Key=keys[kind])["TagSet"]
            value = next(
                (tag["Value"] for tag in tags if tag["Key"] == "GuardDutyMalwareScanStatus"),
                "PENDING",
            )
            statuses[kind] = value
            print(kind, value, keys[kind])
            if value not in ("NO_THREATS_FOUND", "PENDING", "SCAN_IN_PROGRESS"):
                raise ValueError("unexpected scan result: " + value)
        state["scan_results"] = statuses
        state["scans_clean"] = all(value == "NO_THREATS_FOUND" for value in statuses.values())
        print(
            "PASS: both real GuardDuty tags read clean."
            if state["scans_clean"]
            else "PENDING: rerun check in a few minutes; scan verification is not complete."
        )
    else:
        if not state.get("scans_clean"):
            raise ValueError("run check until both final objects report NO_THREATS_FOUND first")
        # This UUID-derived staging key is the only deletion permitted by the helper.
        if state["phase"] != "complete":
            storage.delete_object(Bucket=BUCKET, Key=keys["staging"])
        try:
            storage.head_object(Bucket=BUCKET, Key=keys["staging"])
        except ClientError as error:
            if error.response.get("Error", {}).get("Code") not in ("404", "NotFound", "NoSuchKey"):
                raise
        else:
            raise ValueError("staging is still accessible after deletion")
        verify(storage, keys["artifact"], artifact, True)
        verify(storage, keys["document"], document)
        state["phase"] = "complete"
        print("PASS: own staging object deleted; both final objects remain verified.")
    save(root, state)
    print("Saved probe evidence:", root / "state.json")
    print("No Registry database rows, feature flags or policies changed.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "upload", "check", "cleanup"))
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.root)
    else:
        run(args.command, args.root)


if __name__ == "__main__":
    try:
        main()
    except NoCredentialsError:
        raise SystemExit("Load production AWS credentials in this terminal first.") from None
    except (BotoCoreError, ClientError, ValueError, OSError) as error:
        raise SystemExit(str(error)) from None
