"""Explicit dev-only manual steps; never enable the API or mutate database rows."""

import argparse
import base64
import copy
import gzip
import hashlib
import io
import json
import os
import re
import tarfile
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

CASES = ("portable", "matrix", "dependencies", "legacy")
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
)
BUCKET = "agentpm-dev-tools"


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def open_request(request):
    # Fail redirects instead of forwarding Registry credentials or signed PUTs.
    return build_opener(NoRedirect()).open(request, timeout=120)


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")
    path.chmod(0o600)


def prepare(root, namespace):
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", namespace):
        raise ValueError("use a lowercase namespace handle, without @")
    root.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
    # Refuse to overwrite a prior session's identity or publication instructions.
    with (root / "session.json").open("x") as output:
        json.dump({"namespace": namespace, "name": "m13b-check-" + stamp}, output)
    session = read(root / "session.json")
    for index, case in enumerate(CASES):
        directory = root / case
        directory.mkdir()
        manifest = {
            "kind": "tool",
            "name": session["name"] + ("-legacy" if case == "legacy" else ""),
            "version": f"0.1.{index}",
            "description": "Disposable development release upload verification.",
            "entrypoint": {"command": "python3", "args": ["tool.py"]},
            "runtime": {"type": "python", "version": "3.11"},
            "files": ["tool.py", "README.md"],
            "inputs": {"type": "object"},
            "outputs": {"type": "object"},
            "readme": "README.md",
            "license": {"spdx": "MIT"},
        }
        if case == "dependencies":
            manifest["runtime"]["dependencies"] = ["pydantic==2.11.7"]
        write(directory / "agent.json", manifest)
        (directory / "tool.py").write_text('import json\nprint(json.dumps({"ok": True}))\n')
        (directory / "README.md").write_text("# Development upload verification\n")
    print("Prepared:", session["name"], "in namespace", namespace)
    print("No CLI command, network request or publication has run.")


def package(root, case):
    # Use the real Registry contract implementation, including dependency binding.
    from app.tools.domain.tools.release_storage import validate_release_inputs
    from app.tools.domain.tools.releases import manifest_digest
    from app.tools.domain.tools.tar_validation import _validate_tar_stream

    directory = root / case
    if (directory / "init.json").exists():
        raise ValueError("do not rebuild an already reserved inventory; prepare a fresh session")
    sources = list((directory / "target/agentpm").glob("*.tar.gz"))
    if len(sources) != 1:
        raise ValueError("run the case's CLI dry run first; expected exactly one archive")
    manifest = read(directory / "agent.json")
    with tarfile.open(sources[0]) as archive:
        members = [(copy.copy(member), archive.extractfile(member).read()) for member in archive]
    embedded = {member.name: data for member, data in members}
    if json.loads(embedded["agent.json"]) != manifest or "agentpm-artifact.json" in embedded:
        raise ValueError(
            "source archive must contain the authored manifest and no release descriptor"
        )
    resolution = None
    if "agentpm-python.json" in embedded:
        resolution = json.loads(embedded["agentpm-python.json"])["python_resolution"]
    targets = TARGETS if case == "matrix" else ("any",)
    artifacts = []
    for target in targets:
        path = directory / f"{target}.tar.gz"
        with path.open("wb") as output, gzip.GzipFile(
            fileobj=output, mode="wb", filename="", mtime=0
        ) as zipped:
            with tarfile.open(fileobj=zipped, mode="w") as archive:
                for member, data in members:
                    archive.addfile(member, io.BytesIO(data))
                descriptor = json.dumps({"target": target, "format": "tar.gz"}).encode()
                member = tarfile.TarInfo("agentpm-artifact.json")
                member.size, member.mode = len(descriptor), 0o644
                archive.addfile(member, io.BytesIO(descriptor))
        artifacts.append(
            {
                "target": target,
                "digest": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest(),
                "size": path.stat().st_size,
                "contentType": "application/gzip",
                "format": "tar.gz",
            }
        )
    session = read(root / "session.json")
    release = {
        "type": "agentpm.package.release.v1",
        "kind": "tool",
        "name": f"@{session['namespace']}/{manifest['name']}",
        "version": manifest["version"],
        "manifestDigest": manifest_digest(manifest),
        "artifacts": artifacts,
    }
    request = {"manifest": manifest, "release": release, "namespace_handle": session["namespace"]}
    if resolution is not None:
        from app.tools.domain.tools.python_resolution import resolution_digest

        request["python_resolution"] = resolution
        release["pythonResolutionDigest"] = "sha256:" + resolution_digest(resolution)
    value = validate_release_inputs(release, manifest, resolution)
    for artifact in value.artifacts:
        with (directory / f"{artifact.target}.tar.gz").open("rb") as stream:
            valid, reason = _validate_tar_stream(
                stream,
                expected_manifest=manifest,
                expected_python_resolution=resolution,
                expected_release=value,
                expected_target=artifact.target,
            )
            if not valid:
                raise ValueError(reason)
    write(directory / "request.json", request)
    (directory / "release.canonical.json").write_bytes(value.canonical_bytes())
    oversized = copy.deepcopy(request)
    oversized["release"]["artifacts"][0]["size"] = 1_000_000_001
    write(directory / "oversized.json", oversized)
    if case == "matrix":
        excessive = copy.deepcopy(request)
        for artifact in excessive["release"]["artifacts"]:
            artifact["size"] = 300_000_000
        write(directory / "excess-total.json", excessive)
        changed = copy.deepcopy(request)
        changed["release"]["artifacts"].pop()
        write(directory / "changed-inventory.json", changed)
    print(value.name, value.version, "targets:", ", ".join(targets))
    print("Release digest:", value.digest())
    print("Python resolution bound:", resolution is not None)
    print("Created and validated actual small archives; no upload occurred.")


def registry(path, body, expected, code=None):
    base = os.environ.get("M13B_BASE", "")
    parts = urlsplit(base)
    if parts.scheme != "http" or parts.hostname != "127.0.0.1" or parts.path not in ("", "/"):
        raise ValueError("M13B_BASE must be your loopback Registry, e.g. http://127.0.0.1:5000")
    token = os.environ.get("M13B_TOKEN", "")
    if not token.startswith("apm_"):
        raise ValueError("set M13B_TOKEN to your development publish PAT")
    data = None if body is None else json.dumps(body).encode()
    request = Request(
        base.rstrip("/") + path,
        data=data,
        method="GET" if body is None else "POST",
        headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"},
    )
    try:
        response = open_request(request)
    except HTTPError as error:
        response = error
    with response:
        status, raw = response.status, response.read()
    try:
        value = json.loads(raw)
    except json.JSONDecodeError:
        # Existing Flask abort-based read gates can return HTML rather than the
        # structured errors used by the new publish routes.
        if code is not None or status < 400:
            raise ValueError("expected a JSON Registry response") from None
        value = {"message": raw.decode("utf-8", errors="replace")[:300]}
    print("Registry HTTP", status, value.get("code", value.get("message", "OK")))
    if status != expected or (code is not None and value.get("code") != code):
        raise ValueError(
            f"expected HTTP {expected}/{code}, received HTTP {status}/{value.get('code')}: "
            f"{value.get('message', '')}"
        )
    if status >= 400:
        print(value.get("message", ""))
    return value


def api_step(args):
    directory = args.root / args.case
    if args.command == "init":
        value = registry(
            "/v1/tools/publish/releases/init",
            read(directory / args.request),
            args.expect,
            args.code,
        )
        if args.expect == 200:
            previous = read(directory / "init.json") if (directory / "init.json").exists() else None
            if previous is not None:
                assert value["resumed"] and value["release_id"] == previous["release_id"]
                assert {a["artifact_id"] for a in value["artifacts"]} == {
                    a["artifact_id"] for a in previous["artifacts"]
                }
            write(directory / "init.json", value)
            print("Release:", value["release_id"], "resumed:", value["resumed"])
            print(
                "Caps:",
                value["max_bytes"],
                "per artifact;",
                value["max_total_bytes"],
                "total bytes",
            )
            print("Reservation expires:", value["expires_at"])
            for artifact in value["artifacts"]:
                print(
                    " ",
                    artifact["target"],
                    artifact["artifact_id"],
                    "PUT expires:",
                    artifact["expires_at"],
                )
        return
    init = read(directory / "init.json")
    path = "/v1/tools/publish/releases/" + init["release_id"]
    if args.command == "renew":
        artifact = next(a for a in init["artifacts"] if a["target"] == args.target)
        value = registry(
            path + "/artifacts/" + artifact["artifact_id"] + "/presign", {}, args.expect, args.code
        )
        if args.expect == 200:
            assert value["artifact_id"] == artifact["artifact_id"]
            write(directory / ("renew-" + args.target + ".json"), value)
            print("Same artifact; refreshed PUT expires:", value["expires_at"])
    elif args.command == "finalize":
        value = registry(path + "/finalize", {}, args.expect, args.code)
        if args.expect == 200:
            previous = (
                read(directory / "receipt.json") if (directory / "receipt.json").exists() else None
            )
            if previous is not None:
                assert previous == value, "repeated finalize must return the same receipt"
            write(directory / "receipt.json", value)
            print(
                "Version:",
                value["version"],
                "receipt:",
                value["id"],
                "digest:",
                value["release_digest"],
            )
    else:
        receipt = read(directory / "receipt.json")
        registry(f"/tools/{receipt['tool_id']}/{receipt['version']}/security", None, 409)


def upload(args):
    directory = args.root / args.case
    init = read(directory / "init.json")
    artifact = next(a for a in init["artifacts"] if a["target"] == args.target)
    renewed = directory / ("renew-" + args.target + ".json")
    if renewed.exists():
        artifact = read(renewed)
    url = artifact["put_url"]
    parts = urlsplit(url)
    host = parts.hostname or ""
    allowed = host == BUCKET + ".s3.amazonaws.com" or (
        host.startswith(BUCKET + ".s3.") and host.endswith(".amazonaws.com")
    )
    if parts.scheme != "https" or not allowed:
        raise ValueError(
            "refusing PUT outside https://agentpm-dev-tools.s3[.<region>].amazonaws.com"
        )
    path = directory / (args.target + ".tar.gz")
    if args.corrupt:
        data = path.read_bytes()
        path = directory / (args.target + ".corrupt.tar.gz")
        path.write_bytes(bytes([data[0] ^ 1]) + data[1:])
    headers = dict(artifact["headers"])
    if any(key.lower() == "authorization" for key in headers):
        raise ValueError("storage instructions must not include Registry authorization")
    headers["Content-Length"] = str(path.stat().st_size)
    with path.open("rb") as body:
        request = Request(url, data=body, method="PUT", headers=headers)
        try:
            with open_request(request) as response:
                assert response.status == 200
        except HTTPError as error:
            raise ValueError(
                f"S3 PUT returned HTTP {error.code}; renew this artifact's URL and retry"
            ) from None
    print(
        "S3 PUT 200:", args.target, "(deliberately corrupt)" if args.corrupt else "(valid archive)"
    )


def inspect_storage(args):
    import boto3
    from botocore.exceptions import ClientError

    directory = args.root / args.case
    request = read(directory / "request.json")
    release = request["release"]
    # This walkthrough deliberately exercises the real protected tools/ layout.
    stem = f"tools/{release['name'][1:]}/{release['version']}/"
    storage = boto3.client("s3", region_name=os.environ.get("AWS_REGION", "us-east-1"))
    if args.command == "staging-cleaned":
        init = read(directory / "init.json")
        for artifact in init["artifacts"]:
            key = f"uploads/{init['release_id']}/{artifact['artifact_id']}.tar.gz"
            try:
                storage.head_object(Bucket=BUCKET, Key=key)
            except ClientError as error:
                if error.response["Error"]["Code"] not in ("404", "NoSuchKey", "NotFound"):
                    raise
                print("Staging removed:", artifact["target"])
            else:
                raise ValueError(
                    "staging still exists; wait for the five-minute cleanup job and retry"
                )
        # Also prove cleanup did not remove final objects.
        args.command = "storage"
    keys = [stem + "release.json"]
    for artifact in release["artifacts"]:
        key = f"{stem}artifacts/{artifact['target']}/{artifact['digest'][7:]}.tar.gz"
        keys.append(key)
        if args.command == "storage":
            with storage.get_object(Bucket=BUCKET, Key=key)["Body"] as body:
                data = body.read()
            assert len(data) == artifact["size"]
            assert "sha256:" + hashlib.sha256(data).hexdigest() == artifact["digest"]
            head = storage.head_object(Bucket=BUCKET, Key=key, ChecksumMode="ENABLED")
            assert (
                head["ChecksumSHA256"]
                == base64.b64encode(bytes.fromhex(artifact["digest"][7:])).decode()
            )
            print("Verified final bytes and S3 SHA-256:", artifact["target"])
    if args.command == "storage":
        with storage.get_object(Bucket=BUCKET, Key=keys[0])["Body"] as body:
            assert body.read() == (directory / "release.canonical.json").read_bytes()
        print("Verified canonical release.json:", keys[0])
    else:
        for key in keys:
            tags = storage.get_object_tagging(Bucket=BUCKET, Key=key)["TagSet"]
            value = next(
                (t["Value"] for t in tags if t["Key"] == "GuardDutyMalwareScanStatus"),
                "not tagged yet",
            )
            print(value, key)


def inspect_sql(args):
    init = read(args.root / args.case / "init.json")
    identity = init["release_id"]
    # The identity is server-issued, but still validate before printing SQL.
    import uuid

    uuid.UUID(identity)
    print(f"""-- Read-only; run against the local development Registry database.
SELECT release_id,status,version,created_at,expires_at FROM package_publish_releases
WHERE release_id='{identity}';
SELECT a.target,a.status,a.tmp_key,a.final_key FROM package_publish_artifacts a
WHERE a.release_id='{identity}' ORDER BY a.target;
SELECT v.tool_version_id,v.version,v.status,v.release_format,v.release_sha_256,
       (v.python_resolution IS NOT NULL) AS has_python_resolution,
       v.sha_256,v.size_bytes,v.s3_key FROM tool_versions v
JOIN package_publish_releases r ON r.package_id=v.tool_id AND r.version=v.version
WHERE r.release_id='{identity}';
SELECT a.target,s.scan_id,s.tool_version_id,a.package_version_id,s.artifact_id,
       s.status,s.malware_status,s.notes,s.sbom_cyclonedx,s.grype_summary,
       s.grype_findings,s.pip_audit_findings,s.osv_findings FROM package_publish_releases r
JOIN tool_versions v ON v.tool_id=r.package_id AND v.version=r.version
JOIN package_version_artifacts a ON a.package_version_id=v.tool_version_id
LEFT JOIN tool_version_scans s ON s.artifact_id=a.artifact_id
WHERE r.release_id='{identity}' ORDER BY a.target;""")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=(
            "prepare",
            "package",
            "init",
            "renew",
            "upload",
            "finalize",
            "security-gate",
            "storage",
            "tags",
            "staging-cleaned",
            "sql",
        ),
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("case", nargs="?", choices=CASES)
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--target", choices=("any", *TARGETS), default="any")
    parser.add_argument(
        "--request",
        choices=("request.json", "oversized.json", "excess-total.json", "changed-inventory.json"),
        default="request.json",
    )
    parser.add_argument("--expect", type=int, default=200)
    parser.add_argument("--code")
    parser.add_argument("--corrupt", action="store_true")
    args = parser.parse_args()
    if args.command != "prepare" and args.case is None:
        parser.error("choose a case")
    if args.command == "prepare":
        prepare(args.root, args.namespace)
    elif args.command == "package":
        package(args.root, args.case)
    elif args.command in ("init", "renew", "finalize", "security-gate"):
        api_step(args)
    elif args.command == "upload":
        upload(args)
    elif args.command in ("storage", "tags", "staging-cleaned"):
        inspect_storage(args)
    else:
        inspect_sql(args)


if __name__ == "__main__":
    main()
