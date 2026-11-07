"""Explicit M15 dev steps: prepare, sign, publish, inspect; never enable a gate."""

import argparse
import base64
import copy
import gzip
import hashlib
import importlib.util
import io
import json
import os
import platform
import re
import shlex
import subprocess
import sys
import tarfile
import uuid
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[4]
BASE = "http://127.0.0.1:5000"
CASES = ("portable", "matrix", "incompatible", "legacy")


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


signing = load("m15_signing", HERE.parent / "m14b/fixture.py")
transport = signing.transport
read, write = transport.read, transport.write


def configure():
    if os.environ.get("M15_BASE") != BASE:
        raise ValueError("M15_BASE must be exactly http://127.0.0.1:5000")
    for destination, source in (
        ("M13B_BASE", "M15_BASE"),
        ("M13B_TOKEN", "M15_TOKEN"),
        ("M14B_DATABASE_URL", "M15_DATABASE_URL"),
    ):
        os.environ[destination] = os.environ.get(source, "")


def host_target():
    target = {
        ("Darwin", "arm64"): "aarch64-apple-darwin",
        ("Darwin", "x86_64"): "x86_64-apple-darwin",
        ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
        ("Windows", "AMD64"): "x86_64-pc-windows-msvc",
    }.get((platform.system(), platform.machine()))
    if target is None:
        raise ValueError("use a supported native host; do not emulate another target")
    return target


def prepare(root, namespace):
    from nacl.signing import SigningKey

    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", namespace):
        raise ValueError("choose a lowercase development namespace without @")
    root.mkdir(parents=True, exist_ok=True)
    root.chmod(0o700)
    session = {
        "namespace": namespace,
        "name": "m15-order-" + uuid.uuid4().hex[:12],
        "target": host_target(),
        "created_at": datetime.now(timezone.utc).isoformat(),
    }
    with (root / "session.json").open("x") as output:
        json.dump(session, output)
    key = SigningKey.generate()
    (root / "primary.seed").write_bytes(bytes(key))
    (root / "primary.seed").chmod(0o600)
    (root / "primary-public-key.txt").write_text(
        base64.b64encode(bytes(key.verify_key)).decode() + "\n"
    )
    for index, case in enumerate(CASES):
        directory = root / case
        directory.mkdir()
        manifest = read(HERE / "agent.fixture.json")
        manifest["name"] = session["name"] + ("-legacy" if case == "legacy" else "")
        manifest["version"] = "0.1.0" if case == "legacy" else f"0.1.{index}"
        if case == "legacy":
            del manifest["runtime"]["dependencies"]
            manifest["inputs"] = {"type": "object"}
            manifest["files"].append("_vendor/helper.py")
            (directory / "_vendor").mkdir()
            (directory / "_vendor/helper.py").write_text(
                "def total():\n    return 9.0\n"
            )
            code = "import json\nfrom _vendor.helper import total\nprint(json.dumps({'total':total()}))\n"
        else:
            code = (HERE / "tool.py").read_text()
        write(directory / "agent.json", manifest)
        (directory / "tool.py").write_text(code)
        (directory / "README.md").write_text("# Disposable development install check\n")
    (root / "evidence").mkdir()
    print("Prepared local source and one disposable dev author key:", session["name"])
    print("Consumer target:", session["target"])
    print("No registration, CLI invocation, DB write, upload or publication occurred.")


def targets_for(root, case):
    if case == "portable":
        return ["any"]
    if case == "matrix":
        return ["any", *transport.TARGETS]
    current = read(root / "session.json")["target"]
    return [next(target for target in transport.TARGETS if target != current)]


def package(root, case):
    from app.tools.domain.tools.release_storage import validate_release_inputs
    from app.tools.domain.tools.releases import manifest_digest
    from app.tools.domain.tools.tar_validation import _validate_tar_stream

    directory = root / case
    if case == "legacy" or (directory / "init.json").exists():
        raise ValueError("package only an unreserved new-format case")
    archives = list((directory / "target/agentpm").glob("*.tar.gz"))
    if len(archives) != 1:
        raise ValueError(
            f"run {case}'s CLI dry run first; expected exactly one archive"
        )
    manifest = read(directory / "agent.json")
    with tarfile.open(archives[0]) as archive:
        members = []
        for member in archive:
            if not member.isfile():
                raise ValueError("fixture source archive should contain only files")
            members.append((copy.copy(member), archive.extractfile(member).read()))
    embedded = {member.name: data for member, data in members}
    if (
        json.loads(embedded["agent.json"]) != manifest
        or "agentpm-artifact.json" in embedded
    ):
        raise ValueError(
            "source manifest mismatch or already contains a release descriptor"
        )
    resolution = json.loads(embedded["agentpm-python.json"])["python_resolution"]
    artifacts = []
    for target in targets_for(root, case):
        path = directory / f"{target}.tar.gz"
        with path.open("wb") as output, gzip.GzipFile(
            fileobj=output, mode="wb", filename="", mtime=0
        ) as zipped, tarfile.open(fileobj=zipped, mode="w") as archive:
            for member, data in members:
                archive.addfile(member, io.BytesIO(data))
            data = json.dumps({"target": target, "format": "tar.gz"}).encode()
            member = tarfile.TarInfo("agentpm-artifact.json")
            member.size, member.mode = len(data), 0o644
            archive.addfile(member, io.BytesIO(data))
        artifacts.append(
            {
                "target": target,
                "digest": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest(),
                "size": path.stat().st_size,
                "contentType": "application/gzip",
                "format": "tar.gz",
            }
        )
    from app.tools.domain.tools.python_resolution import resolution_digest

    namespace = read(root / "session.json")["namespace"]
    request = {
        "namespace_handle": namespace,
        "manifest": manifest,
        "python_resolution": resolution,
        "release": {
            "type": "agentpm.package.release.v1",
            "kind": "tool",
            "name": f"@{namespace}/{manifest['name']}",
            "version": manifest["version"],
            "manifestDigest": manifest_digest(manifest),
            "pythonResolutionDigest": "sha256:" + resolution_digest(resolution),
            "artifacts": artifacts,
        },
    }
    release = validate_release_inputs(request["release"], manifest, resolution)
    for artifact in release.artifacts:
        with (directory / f"{artifact.target}.tar.gz").open("rb") as stream:
            valid, reason = _validate_tar_stream(
                stream,
                expected_manifest=manifest,
                expected_python_resolution=resolution,
                expected_release=release,
                expected_target=artifact.target,
            )
            if not valid:
                raise ValueError(reason)
    write(directory / "request.json", request)
    write(
        directory / "finalize-primary.json",
        signing.signed_body(root, request, "primary"),
    )
    print(
        "Prepared and signed",
        release.name,
        release.version,
        "targets:",
        targets_for(root, case),
    )
    print("Whole-release digest:", release.digest())
    print("Fixture descriptors test selection, not native builds on other hosts.")


def preflight(root):
    from app.tools.domain.tools.registry_trust import SignedKeySet

    raw = transport.registry("/v1/registry/signing-keys", None, 200)
    roots = read(REPO / "crates/agentpm-sdk/contracts/registry-trust-roots-v1.json")[
        "roots"
    ]
    document = SignedKeySet.model_validate(raw)
    document.verify(roots, BASE)
    write(root / "evidence/dev-keyset.json", raw)
    print(
        "PASS: served dev key set verifies against the checked-in root; expires:",
        document.statement.expiresAt,
    )


def probe(args):
    request = read(args.root / args.case / "request.json")
    target = args.target or read(args.root / "session.json")["target"]
    init = transport.registry(
        "/v1/tools/install/init",
        {
            "release_format": "agentpm.package.release.v1",
            "consumer": {
                "target": target,
                "python": {
                    "version": platform.python_version(),
                    "implementation": sys.implementation.name,
                },
            },
            "items": [
                {
                    "kind": "tool",
                    "name": request["release"]["name"],
                    "version": request["release"]["version"],
                }
            ],
        },
        200,
    )
    assert len(init["artifacts"]) == 1, "expected just one selected URL"
    artifact = init["artifacts"][0]
    release = request["release"]
    from app.tools.domain.tools.releases import ReleaseManifest

    assert artifact["integrity"] == ReleaseManifest.model_validate(release).digest_hex()
    assert artifact["verification"]["release"] == ReleaseManifest.model_validate(
        release
    ).model_dump(exclude_none=True)
    chosen = next((a for a in release["artifacts"] if a["target"] == target), None)
    chosen = chosen or next(a for a in release["artifacts"] if a["target"] == "any")
    assert artifact["selected_artifact"] == chosen
    assert artifact["size"] == chosen["size"]
    url = artifact["presigned_url"]
    query = parse_qs(urlsplit(url).query)
    assert query.get("X-Amz-Expires") == ["600"], "expected ten-minute selected GET"
    write(args.root / args.case / f"install-{target}.json", init)
    print(
        "PASS: one selected URL for",
        target,
        "→",
        chosen["target"],
        "; full inventory:",
        len(release["artifacts"]),
    )
    print(
        "No artifact downloaded or install finalized; this inspection session stays pending."
    )


def renew(args):
    target = args.target or read(args.root / "session.json")["target"]
    before = read(args.root / args.case / f"install-{target}.json")
    after = transport.registry(
        "/v1/tools/install/renew", {"session_id": before["session_id"]}, 200
    )
    assert before["session_id"] == after["session_id"]
    assert before["expires_at"] == after["expires_at"]
    for value in (before, after):
        for artifact in value["artifacts"]:
            artifact.pop("presigned_url")
    assert before == after, "renewal changed package metadata"
    print("PASS: renewed only URLs; same session, inventory and original deadline.")


def consumer(args):
    manifest = read(args.root / args.case / "agent.json")
    namespace = read(args.root / "session.json")["namespace"]
    args.consumer.mkdir(parents=True, exist_ok=False)
    write(
        args.consumer / "agent.json",
        {
            "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/main/schemas/agentpm.manifest.schema.json",
            "kind": "agent",
            "name": "m15-consumer",
            "version": "0.1.0",
            "description": "Disposable target-aware consumer.",
            "tools": [
                {
                    "name": f"@{namespace}/{manifest['name']}",
                    "version": manifest["version"],
                }
            ],
        },
    )
    (args.consumer / ".gitignore").write_text(".agentpm/\n.agentpm-state/\n")
    print("Created local consumer only:", args.consumer)


def check(args):
    from app.tools.domain.tools.releases import ReleaseManifest

    request = read(args.root / args.case / "request.json")
    release = ReleaseManifest.model_validate(request["release"])
    name, version = release.name, release.version
    current = read(args.root / "session.json")["target"]
    parent = args.consumer / ".agentpm/tools" / name[1:]
    state = read(parent / f".{version}.selection.json")
    expected = next(
        (a for a in request["release"]["artifacts"] if a["target"] == current), None
    )
    expected = expected or next(
        a for a in request["release"]["artifacts"] if a["target"] == "any"
    )
    assert state["target"] == current and state["artifact"] == expected
    assert state["release_integrity"] == release.digest_hex()
    assert read(parent / version / "agentpm-artifact.json") == {
        "target": expected["target"],
        "format": "tar.gz",
    }
    lock = read(args.consumer / "agent.lock")
    locked = lock["packages"][f"tool:{name}@{version}"]
    assert locked["integrity"] == release.digest_hex()
    assert locked["python_resolution"] == request["python_resolution"]
    assert (
        not {"target", "selected_artifact", "artifact_digest", "runtime_fingerprint"}
        & locked.keys()
    )
    identity = ["tool", name, version, expected["target"], expected["digest"][7:]]
    cache_name = (
        hashlib.sha256(
            json.dumps(identity, separators=(",", ":"), ensure_ascii=False).encode()
        ).hexdigest()
        + ".tgz"
    )
    cache = args.consumer / ".agentpm/cache" / cache_name
    assert cache.stat().st_size == expected["size"]
    assert (
        "sha256:" + hashlib.sha256(cache.read_bytes()).hexdigest() == expected["digest"]
    )
    assert list((args.consumer / ".agentpm/cache").glob("*.tgz")) == [
        cache
    ], "use a fresh one-Tool consumer"
    states = list((args.consumer / ".agentpm/runtimes/python").rglob("ready.json"))
    assert len(states) == 1, states
    ready = read(states[0])
    assert ready["fingerprint"]["target"] == current
    assert (
        ready["fingerprint"]["resolution_digest"] == release.pythonResolutionDigest[7:]
    )
    python = (
        states[0].parent
        / "venv"
        / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    )
    script = (
        "import json,sys,pydantic,pydantic_core._pydantic_core as native;"
        "print(json.dumps({'prefix':sys.prefix,'native':native.__file__,"
        "'pydantic':pydantic.__version__}))"
    )
    runtime = json.loads(
        subprocess.check_output([str(python), "-I", "-c", script], text=True)
    )
    assert Path(runtime["prefix"]).resolve() == (states[0].parent / "venv").resolve()
    assert (states[0].parent / "venv").resolve() in Path(
        runtime["native"]
    ).resolve().parents
    assert runtime["pydantic"] == "2.11.7"
    evidence = {
        "target": current,
        "selected": expected,
        "release_integrity": release.digest_hex(),
        "cache": str(cache.resolve()),
        "environment": str(states[0].parent.resolve()),
        "runtime": runtime,
    }
    write(args.consumer / "m15-evidence.json", evidence)
    print(
        "PASS: selected",
        expected["target"],
        "; one verified cache; portable lock; real managed native dependency.",
    )
    print(json.dumps(evidence, indent=2))


def sessions(args):
    request = read(args.root / args.case / "request.json")
    release = request["release"]
    with signing.database() as connection:
        rows = connection.execute(
            "SELECT session_id,status,created_ts,expires_at FROM install_sessions "
            "WHERE EXISTS (SELECT 1 FROM jsonb_array_elements(plan_json->'items') AS item "
            "WHERE item->>'name'=%s AND item->>'version'=%s) ORDER BY created_ts",
            (release["name"], release["version"]),
        ).fetchall()
    result = [{key: str(value) for key, value in row.items()} for row in rows]
    for row in rows:
        lifetime = (row["expires_at"] - row["created_ts"]).total_seconds()
        assert 7190 <= lifetime <= 7210, lifetime
    if args.baseline:
        previous = {row["session_id"] for row in read(args.baseline)}
        added = [row for row in result if row["session_id"] not in previous]
        assert added and all(row["status"] == "pending" for row in added), added
        print(
            "PASS: failed provisioning added pending session(s), no completed install."
        )
    if args.out:
        write(args.out, result)
    print(json.dumps(result, indent=2))


def provider_command(root):
    provider = HERE.parent / "m11c/provider.py"
    print(
        shlex.join(
            [
                sys.executable,
                "-B",
                str(provider),
                "--port",
                os.environ.get("M15_MODEL_PORT", "18160"),
                "--events",
                str(root / "evidence/harness-events.jsonl"),
            ]
        )
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=(
            "prepare",
            "package",
            "init",
            "upload",
            "finalize",
            "upload-renew",
            "preflight",
            "probe",
            "renew",
            "consumer",
            "check",
            "sessions",
            "signer-id",
            "provider-command",
        ),
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("case", nargs="?", choices=CASES)
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--target", choices=("any", *transport.TARGETS))
    parser.add_argument("--consumer", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    configure()
    if (
        args.command
        in (
            "package",
            "init",
            "upload",
            "finalize",
            "upload-renew",
            "probe",
            "renew",
            "consumer",
            "check",
            "sessions",
        )
        and args.case is None
    ):
        parser.error("choose a case")
    if args.command in ("consumer", "check") and args.consumer is None:
        parser.error("supply --consumer with the disposable consumer directory")
    if args.command == "prepare":
        prepare(args.root, args.namespace)
    elif args.command == "package":
        package(args.root, args.case)
    elif args.command == "preflight":
        preflight(args.root)
    elif args.command == "init":
        args.request, args.expect, args.code = "request.json", 200, None
        transport.api_step(args)
    elif args.command in ("upload", "upload-renew"):
        renew_upload = args.command == "upload-renew"
        targets = [args.target] if args.target else targets_for(args.root, args.case)
        for target in targets:
            args.target, args.corrupt = target, False
            if renew_upload:
                args.command, args.expect, args.code = "renew", 200, None
                transport.api_step(args)
            else:
                transport.upload(args)
    elif args.command == "finalize":
        args.variant, args.expect, args.code = "primary", 200, None
        signing.finalize(args)
    elif args.command == "signer-id":
        args.key = "primary"
        signing.signer_id(args)
    elif args.command == "provider-command":
        provider_command(args.root)
    else:
        globals()[args.command](args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, AssertionError, FileNotFoundError) as error:
        raise SystemExit(f"M15 fixture: {error}") from None
