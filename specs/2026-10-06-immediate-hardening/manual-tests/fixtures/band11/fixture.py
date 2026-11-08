"""Small explicit production smoke steps; no deployment, DB or root-private access."""

import argparse
import base64
import copy
import gzip
import hashlib
import importlib.util
import io
import json
import os
import re
import tarfile
import uuid
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import Request

from nacl.signing import SigningKey, VerifyKey

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "band11_local_authoring", HERE.parent / "m15/fixture.py"
)
local = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(local)
read, write = local.read, local.write
BASE = "https://api.agentpackagemanager.com"
BUCKET = "agentpm-tools"
CASES = ("portable", "matrix")
MAX_BYTES = 1024 * 1024


def prepare(root, namespace):
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", namespace):
        raise ValueError("choose your production namespace handle, without @")
    root.mkdir(parents=True, exist_ok=True)
    root.chmod(0o700)
    session = {
        "registry": BASE,
        "bucket": BUCKET,
        "namespace": namespace,
        "name": "release-check-" + uuid.uuid4().hex[:12],
        "target": local.host_target(),
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
        manifest = read(local.HERE / "agent.fixture.json")
        manifest.update(
            name=session["name"],
            version=f"0.1.{index}",
            description="Small signed release used to verify package installation.",
        )
        write(directory / "agent.json", manifest)
        (directory / "tool.py").write_bytes((local.HERE / "tool.py").read_bytes())
        (directory / "README.md").write_text(
            "# Release installation check\n\nValidates an order and returns its total using declared Python dependencies.\n"
        )
    (root / "evidence").mkdir()
    print(
        "Prepared locally:",
        session["name"],
        "in",
        namespace,
        "target:",
        session["target"],
    )
    print("Production writes happen only through explicit init/upload/finalize steps.")


def session_for(root):
    session = read(root / "session.json")
    if session.get("registry") != BASE or session.get("bucket") != BUCKET:
        raise ValueError(
            "session must name the official production Registry and bucket"
        )
    if not re.fullmatch(r"release-check-[0-9a-f]{12}", session["name"]):
        raise ValueError("only this helper's unique release-check packages are allowed")
    return session


def targets(root, case):
    session = session_for(root)
    return ["any"] if case == "portable" else sorted(["any", session["target"]])


def package(root, case):
    from app.tools.domain.tools.python_resolution import resolution_digest
    from app.tools.domain.tools.release_storage import validate_release_inputs
    from app.tools.domain.tools.releases import manifest_digest
    from app.tools.domain.tools.tar_validation import _validate_tar_stream

    session = session_for(root)
    directory = root / case
    if (directory / "init.json").exists():
        raise ValueError("do not rebuild a reserved release; retain its original bytes")
    archives = list((directory / "target/agentpm").glob("*.tar.gz"))
    if len(archives) != 1:
        raise ValueError("run this case's CLI dry run first; expected one archive")
    manifest = read(directory / "agent.json")
    with tarfile.open(archives[0]) as source:
        members = []
        for member in source:
            if not member.isfile():
                raise ValueError("smoke source must contain only regular files")
            members.append((copy.copy(member), source.extractfile(member).read()))
    embedded = {member.name: data for member, data in members}
    if (
        json.loads(embedded["agent.json"]) != manifest
        or "agentpm-artifact.json" in embedded
    ):
        raise ValueError("source manifest mismatch or already has a release descriptor")
    resolution = json.loads(embedded["agentpm-python.json"])["python_resolution"]
    artifacts = []
    for target in targets(root, case):
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
    request = {
        "namespace_handle": session["namespace"],
        "manifest": manifest,
        "python_resolution": resolution,
        "release": {
            "type": "agentpm.package.release.v1",
            "kind": "tool",
            "name": f"@{session['namespace']}/{manifest['name']}",
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
        local.signing.signed_body(root, request, "primary"),
    )
    checked_request(root, case)
    print(
        "Prepared signed",
        release.name,
        release.version,
        "targets:",
        targets(root, case),
        "digest:",
        release.digest(),
    )


def checked_request(root, case):
    from app.tools.domain.tools.release_signing import AuthorSignatureV2
    from app.tools.domain.tools.release_storage import validate_release_inputs

    session = session_for(root)
    request = read(root / case / "request.json")
    release = validate_release_inputs(
        request["release"], request["manifest"], request["python_resolution"]
    )
    if (
        release.name != f"@{session['namespace']}/{session['name']}"
        or release.version != f"0.1.{CASES.index(case)}"
        or release.kind != "tool"
        or request["namespace_handle"] != session["namespace"]
        or sorted(a.target for a in release.artifacts) != targets(root, case)
    ):
        raise ValueError("request escaped the prepared smoke identity or inventory")
    for artifact in release.artifacts:
        path = root / case / f"{artifact.target}.tar.gz"
        if (
            artifact.size >= MAX_BYTES
            or path.stat().st_size != artifact.size
            or "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
            != artifact.digest
        ):
            raise ValueError(
                "smoke archives must match their digests and stay below 1 MiB each"
            )
    body = read(root / case / "finalize-primary.json")
    if (
        body.get("require_signature") is not True
        or len(body.get("author_signatures", [])) != 1
    ):
        raise ValueError("one explicit author signature is required")
    signature = body["author_signatures"][0]
    statement = AuthorSignatureV2.model_validate(signature["statement_json"])
    if (
        not statement.matches(release)
        or signature["algo"] != "ed25519"
        or signature["public_key_b64"]
        != (root / "primary-public-key.txt").read_text().strip()
    ):
        raise ValueError("saved author signature does not bind this smoke release")
    VerifyKey(base64.b64decode(signature["public_key_b64"], validate=True)).verify(
        statement.canonical_bytes(),
        base64.b64decode(signature["signature_b64"], validate=True),
    )
    return request


def registry(path, body=None, expected=200):
    publish = r"/v1/tools/publish/releases/(?:init|[0-9a-f-]{36}/finalize|[0-9a-f-]{36}/artifacts/[0-9a-f-]{36}/presign)"
    security = r"/tools/[0-9a-f-]{36}/0\.1\.[01]/security"
    allowed = re.fullmatch(publish if body is not None else security, path)
    if not allowed and not (body is None and path == "/v1/registry/signing-keys"):
        raise ValueError("unexpected production Registry path")
    token = os.environ.get("BAND11_TOKEN", "")
    if not token.startswith("apm_"):
        raise ValueError("enter BAND11_TOKEN as a production publish PAT")
    request = Request(
        BASE + path,
        data=None if body is None else json.dumps(body).encode(),
        headers={
            "Authorization": "Bearer " + token,
            "Content-Type": "application/json",
        },
        method="GET" if body is None else "POST",
    )
    try:
        response = local.transport.open_request(request)
    except HTTPError as error:
        response = error
    with response:
        status, raw = response.status, response.read(2 * 1024 * 1024 + 1)
    if len(raw) > 2 * 1024 * 1024:
        raise ValueError("Registry response exceeds smoke size bound")
    try:
        value = json.loads(raw)
    except ValueError:
        raise ValueError(f"Registry HTTP {status}: response was not JSON") from None
    if status != expected:
        raise ValueError(
            f"Registry HTTP {status}: {value.get('code', '')} {value.get('message', '')}"
        )
    return value


def keyset(root):
    from app.tools.domain.tools.registry_trust import SignedKeySet

    raw = registry("/v1/registry/signing-keys")
    roots = read(
        local.REPO / "crates/agentpm-sdk/contracts/registry-trust-roots-v1.json"
    )["roots"]
    document = SignedKeySet.model_validate(raw).verify(roots, BASE)
    write(root / "evidence/production-keyset.json", raw)
    return document


def init(root, case):
    request = checked_request(root, case)
    value = registry("/v1/tools/publish/releases/init", request)
    uuid.UUID(value["release_id"])
    if sorted(a["target"] for a in value["artifacts"]) != targets(root, case):
        raise ValueError("reservation inventory differs")
    write(root / case / "init.json", value)
    print(
        "Reserved",
        request["release"]["name"],
        request["release"]["version"],
        "release:",
        value["release_id"],
    )


def upload(root, case, renew=False):
    checked_request(root, case)
    reservation = read(root / case / "init.json")
    identity = str(uuid.UUID(reservation["release_id"]))
    for item in reservation["artifacts"]:
        artifact_id = str(uuid.UUID(item["artifact_id"]))
        if renew:
            refreshed = registry(
                f"/v1/tools/publish/releases/{identity}/artifacts/{artifact_id}/presign",
                {},
            )
            if refreshed["artifact_id"] != item["artifact_id"]:
                raise ValueError("renewal changed artifact identity")
            item.update(refreshed)
            continue
        parts = urlsplit(item["put_url"])
        host = parts.hostname or ""
        if (
            parts.scheme != "https"
            or parts.username
            or parts.password
            or parts.port not in (None, 443)
            or not (
                host == BUCKET + ".s3.amazonaws.com"
                or host.startswith(BUCKET + ".s3.")
                and host.endswith(".amazonaws.com")
            )
            or parts.path != f"/uploads/{identity}/{artifact_id}.tar.gz"
        ):
            raise ValueError(
                "refusing PUT outside this reservation's production staging object"
            )
        path = root / case / f"{item['target']}.tar.gz"
        headers = dict(item["headers"])
        if any(key.lower() == "authorization" for key in headers):
            raise ValueError("Registry authorization must never be forwarded to S3")
        headers["Content-Length"] = str(path.stat().st_size)
        with path.open("rb") as stream:
            request = Request(
                item["put_url"], data=stream, method="PUT", headers=headers
            )
            try:
                with local.transport.open_request(request) as response:
                    if response.status != 200:
                        raise ValueError("S3 PUT was not HTTP 200")
            except HTTPError as error:
                raise ValueError(
                    f"S3 PUT HTTP {error.code}; renew-upload then retry upload"
                ) from None
        print("Uploaded small archive:", item["target"])
    if renew:
        write(root / case / "init.json", reservation)
        print("Renewed upload instructions only; no PUT performed.")


def finalize(root, case):
    checked_request(root, case)
    identity = str(uuid.UUID(read(root / case / "init.json")["release_id"]))
    body = read(root / case / "finalize-primary.json")
    receipt = registry(f"/v1/tools/publish/releases/{identity}/finalize", body)
    from app.tools.domain.tools.releases import ReleaseManifest

    release = ReleaseManifest.model_validate(
        read(root / case / "request.json")["release"]
    )
    if (
        receipt["release_digest"] != release.digest()
        or receipt["author_signatures_accepted"] != 1
    ):
        raise ValueError("finalize receipt differs from signed intent")
    uuid.UUID(receipt["tool_id"])
    write(root / case / "receipt.json", receipt)
    print("Published", release.name, release.version, release.digest())
    print(
        "Security:",
        f"https://www.agentpackagemanager.com/tools/{receipt['tool_id']}/v{release.version}/security",
    )


def security(root, case, clean=False):
    from app.tools.domain.tools.release_signing import (
        AuthorSignatureV2,
        RegistryAttestationV3,
    )
    from app.tools.domain.tools.releases import ReleaseManifest

    request = checked_request(root, case)
    release = ReleaseManifest.model_validate(request["release"])
    receipt = read(root / case / "receipt.json")
    document = keyset(root)
    value = registry(
        f"/tools/{uuid.UUID(receipt['tool_id'])}/{release.version}/security"
    )
    if value["release_digest"] != release.digest() or len(value["artifacts"]) != len(
        release.artifacts
    ):
        raise ValueError("Security release identity/inventory differs")
    expected = {a.target: a for a in release.artifacts}
    if {a["target"] for a in value["artifacts"]} != set(expected):
        raise ValueError("Security targets differ")
    for row in value["artifacts"]:
        artifact = expected[row["target"]]
        if row["sha256"] != artifact.digest[7:] or row["size_bytes"] != artifact.size:
            raise ValueError("Security artifact differs")
    attestation = value["registry_attestation"]
    statement = RegistryAttestationV3.model_validate(attestation["statement_json"])
    key = next(k for k in document.statement.keys if k.keyId == statement.registryKeyId)
    if not statement.matches(release) or attestation["algo"] != "ed25519":
        raise ValueError("Registry attestation does not bind this release")
    VerifyKey(base64.b64decode(key.publicKeyB64)).verify(
        statement.canonical_bytes(), base64.b64decode(attestation["signature_b64"])
    )
    authors = [
        a
        for a in value["author_signatures"]
        if a["public_key_b64"] == (root / "primary-public-key.txt").read_text().strip()
    ]
    if len(authors) != 1:
        raise ValueError("expected this smoke's accepted author signature")
    author = AuthorSignatureV2.model_validate(authors[0]["statement_json"])
    if not author.matches(release):
        raise ValueError("author signature does not bind this release")
    VerifyKey(base64.b64decode(authors[0]["public_key_b64"])).verify(
        author.canonical_bytes(), base64.b64decode(authors[0]["signature_b64"])
    )
    write(root / "evidence" / f"security-{case}.json", value)
    if clean and any(
        row["scan_status"] != "complete" or row["scan_malware_status"] != "clean"
        for row in value["artifacts"]
    ):
        raise ValueError(
            "scans not all complete/clean yet; wait for normal GuardDuty polling and retry security --clean"
        )
    print(
        "PASS: production root/key set → Registry v3 + author v2 → full release inventory."
    )
    for row in value["artifacts"]:
        print(row["target"], "scan:", row["scan_status"], row["scan_malware_status"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=(
            "prepare",
            "preflight",
            "package",
            "init",
            "upload",
            "renew-upload",
            "finalize",
            "security",
            "consumer",
            "check",
            "signer-id",
        ),
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("case", nargs="?", choices=CASES)
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--consumer", type=Path)
    parser.add_argument("--clean", action="store_true")
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.root, args.namespace)
        return
    session_for(args.root)
    if args.command in ("preflight", "signer-id"):
        if args.command == "preflight":
            value = keyset(args.root)
            print(
                "PASS: trusted production key set; expires:", value.statement.expiresAt
            )
        else:
            value = read(args.root / "evidence/security-portable.json")
            public = (args.root / "primary-public-key.txt").read_text().strip()
            author = next(
                a for a in value["author_signatures"] if a["public_key_b64"] == public
            )
            print(uuid.UUID(author["signer_id"]))
        return
    if args.case is None:
        parser.error("choose portable or matrix")
    if args.command in ("consumer", "check"):
        if args.consumer is None:
            parser.error("supply --consumer with a disposable directory")
        getattr(local, args.command)(args)
    elif args.command in ("upload", "renew-upload"):
        upload(args.root, args.case, args.command == "renew-upload")
    elif args.command == "security":
        security(args.root, args.case, args.clean)
    else:
        globals()[args.command](args.root, args.case)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, AssertionError, FileNotFoundError) as error:
        raise SystemExit(f"Production smoke: {error}") from None
