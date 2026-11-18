"""Visible M4A cases and explicit real-local-Registry HTTP steps, never DB writes."""

import argparse
import base64
import gzip
import hashlib
import io
import json
import os
import re
import tarfile
import uuid
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

import rfc8785
from nacl.signing import SigningKey

REPO = Path(__file__).resolve().parents[5]
CORPUS = REPO / "standards/agentpm/1.0.0/conformance"
KINDS = ("agent", "tool", "skill", "knowledge", "memory", "profile", "loop", "template")
TARGETS = ("aarch64-apple-darwin", "x86_64-pc-windows-msvc")
STANDARD = {"id": "agentpm", "version": "1.0.0"}


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, default=str) + "\n", encoding="utf-8")
    path.chmod(0o600)


def local_root(path):
    path = path.resolve()
    if (
        not path.is_relative_to(REPO / "registry-m4a-test")
        or path == REPO / "registry-m4a-test"
    ):
        raise ValueError(
            "Workspace must be a run directory under agentpm/registry-m4a-test/"
        )
    return path


def database():
    import psycopg
    from psycopg.rows import dict_row

    dsn = os.environ.get("M4A_DATABASE_URL", "").replace(
        "postgresql+psycopg://", "postgresql://", 1
    )
    parts = urlsplit(dsn)
    if parts.scheme not in ("postgres", "postgresql") or parts.hostname not in (
        "127.0.0.1",
        "localhost",
    ):
        raise ValueError(
            "M4A_DATABASE_URL must point to your local DEVELOPMENT PostgreSQL"
        )
    connection = psycopg.connect(
        dsn,
        row_factory=dict_row,
        connect_timeout=10,
        options="-c default_transaction_read_only=on",
    )
    connection.read_only = True
    return connection


def namespace_info(root):
    with database() as connection:
        row = connection.execute(
            "SELECT namespace_id,handle,visibility,signing_mode FROM namespaces WHERE handle=%s",
            (read(root / "session.json")["namespace"],),
        ).fetchone()
        if not row or row["visibility"] != "public":
            raise ValueError(
                "Use an existing public DEVELOPMENT namespace you can publish/manage"
            )
        connection.execute(
            "SELECT standard_id,standard_version,standard_origin,apds_validation FROM tool_versions LIMIT 0"
        )
    return row


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def open_request(request):
    return build_opener(NoRedirect()).open(request, timeout=120)


def registry(root, label, route, body, expected=200, code=None):
    base = os.environ.get("M4A_BASE", "")
    if base != "http://127.0.0.1:5000":
        raise ValueError("M4A_BASE must be exactly http://127.0.0.1:5000")
    token = os.environ.get("M4A_TOKEN", "")
    if not token.startswith("apm_"):
        raise ValueError("Set M4A_TOKEN to a real LOCAL development PAT")
    if not route.startswith("/") or route.startswith("//"):
        raise ValueError("Expected a relative API route")
    headers = {"Authorization": "Bearer " + token, "Content-Type": "application/json"}
    data = None if body is None else json.dumps(body).encode()
    request = Request(
        base + route,
        data=data,
        headers=headers,
        method="GET" if body is None else "POST",
    )
    try:
        with open_request(request) as response:
            status, raw = response.status, response.read()
    except HTTPError as error:
        status, raw = error.code, error.read()
    try:
        result = json.loads(raw) if raw else {}
    except ValueError:
        result = {"message": raw.decode("utf-8", errors="replace")[:500]}
    evidence = root / "evidence" / label
    evidence.mkdir(parents=True, exist_ok=True)
    attempt = len(list(evidence.glob("*.json"))) + 1
    write(
        evidence / f"{attempt:03}.json",
        {
            "method": request.method,
            "url": base + route,
            "request": body,
            "http_status": status,
            "expected_http_status": expected,
            "expected_code": code,
            "response": result,
        },
    )
    print(f"{request.method} {base + route}")
    print(
        f"ACTUAL HTTP {status}/{result.get('code', 'OK')} | EXPECTED HTTP {expected}/{code or 'OK'}"
    )
    if "/finalize" in route or status >= 400:
        print(json.dumps(result, indent=2))
    if status != expected or (code is not None and result.get("code") != code):
        raise ValueError(
            "Unexpected response; stop. A 401/403/429/500 is not an APDS rejection pass."
        )
    return result


def prepare(root, namespace):
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", namespace):
        raise ValueError("Expected a namespace handle without @")
    if (root / "session.json").exists():
        raise ValueError(
            "Existing session: keep it unchanged or choose a fresh run directory"
        )
    root.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
    prefix = "m4a-" + stamp + "-" + uuid.uuid4().hex[:6]
    write(
        root / "session.json",
        {"namespace": namespace, "prefix": prefix, "created_at": stamp},
    )
    seed = SigningKey.generate()
    (root / "author.seed").write_bytes(bytes(seed))
    (root / "author.seed").chmod(0o600)
    (root / "author-public-key.txt").write_text(
        base64.b64encode(bytes(seed.verify_key)).decode() + "\n"
    )

    def case(
        label,
        kind="agent",
        *,
        mode="legacy",
        issue="No intentional defect",
        expect=200,
        code=None,
        rule=None,
        path=None,
        status="conformant",
        suffix=None,
    ):
        directory = root / "cases" / label
        source = directory / "source"
        source.mkdir(parents=True)
        manifest = read(CORPUS / f"fixtures/valid/{kind}.json")
        manifest.update(
            name=prefix + "-" + (suffix or label),
            description="Local M4A publication check: " + label,
        )
        files = {"README.md": "# Local M4A publication fixture\n"}
        if kind == "tool":
            manifest.update(
                runtime={"type": "node", "version": "20"},
                files=["main.js"],
                entrypoint={"command": "node", "args": ["main.js"]},
            )
            files["main.js"] = "console.log(JSON.stringify({ok:true}));\n"
            if mode == "cli":
                manifest.update(
                    runtime={"type": "python", "version": "3.11"},
                    files=["main.py"],
                    entrypoint={"command": "python3", "args": ["main.py"]},
                )
                del files["main.js"]
                files["main.py"] = "import json\nprint(json.dumps({'ok':True}))\n"
        elif kind == "skill":
            files[
                "SKILL.md"
            ] = "# Review a request\nRead the request and state the next step.\n"
        elif kind == "knowledge":
            files[
                "knowledge/docs/context.md"
            ] = "# Context\nThis fixture tests Registry content inspection.\n"
        elif kind == "memory":
            files["schemas/user-preference.schema.json"] = json.dumps(
                {
                    "type": "object",
                    "properties": {"preference": {"type": "string"}},
                    "required": ["preference"],
                    "additionalProperties": False,
                }
            )
        elif kind == "template":
            files[
                "template/README.md"
            ] = "# {{project_name}}\nA generated starter project.\n"
        for name, value in files.items():
            file = source / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(value, encoding="utf-8")
        write(source / "agent.json", manifest)
        write(
            directory / "case.json",
            {
                "label": label,
                "mode": mode,
                "issue": issue,
                "expected_http": expect,
                "expected_code": code,
                "rule": rule,
                "path": path,
                "expected_status": status,
            },
        )
        return manifest, directory

    for kind in KINDS:
        case(kind, kind)
        case("cli-" + kind, kind, mode="cli")
    for label, declared in [("off-declared", True), ("off-undeclared", False)]:
        value, folder = case(
            label, status="declared-only" if declared else "undeclared"
        )
        if not declared:
            value.pop("standard")
            write(folder / "source/agent.json", value)
    value, folder = case("off-loop", "loop", status="undeclared")
    value.pop("standard")
    write(folder / "source/agent.json", value)

    changes = {
        "missing-standard": (
            "standard",
            "remove",
            "APDS-ID-001",
            "/standard",
            "apds_nonconformant",
        ),
        "unsupported-standard": (
            "standard",
            {"id": "agentpm", "version": "9.0.0"},
            "APDS-ID-001",
            "/standard/version",
            "apds_unsupported",
        ),
        "malformed-standard": (
            "standard",
            None,
            "APDS-ID-001",
            "/standard",
            "apds_nonconformant",
        ),
        "blank-description": (
            "description",
            " ",
            "APDS-ID-002",
            "/description",
            "apds_nonconformant",
        ),
        "wrong-name": (
            "name",
            "different-name",
            None,
            "/name",
            "apds_identity_mismatch",
        ),
        "wrong-version": (
            "version",
            "9.0.0",
            None,
            "/version",
            "apds_identity_mismatch",
        ),
        "standard-mismatch": (
            "standard",
            STANDARD,
            None,
            "/standard",
            "apds_manifest_mismatch",
        ),
    }
    for label, (field, changed, rule, path, code) in changes.items():
        value, folder = case(
            label,
            issue=f"Embedded {field}: {changed!r}; see reservation-manifest.json",
            expect=422,
            code=code,
            rule=rule,
            path=path,
            status="rejected",
        )
        reserved = json.loads(json.dumps(value))
        if label == "standard-mismatch":
            reserved["standard"]["version"] = "9.0.0"
        elif changed == "remove":
            value.pop(field)
        else:
            value[field] = changed
        write(folder / "source/agent.json", value)
        write(folder / "reservation-manifest.json", reserved)
    value, folder = case(
        "malformed-json",
        issue="Archive agent.json is truncated JSON",
        expect=422,
        code="invalid_artifact",
        status="rejected",
    )
    write(folder / "reservation-manifest.json", value)
    (folder / "source/agent.json").write_text('{"kind":"agent",\n', encoding="utf-8")
    value, folder = case(
        "wrong-kind",
        "profile",
        issue="Reserved Agent identity but valid Profile in tar",
        expect=422,
        code="apds_identity_mismatch",
        path="/kind",
        status="rejected",
    )
    reserved = read(CORPUS / "fixtures/valid/agent.json")
    reserved.update(name=value["name"], description=value["description"])
    write(folder / "reservation-manifest.json", reserved)
    value, folder = case(
        "loop-invalid-entry",
        "loop",
        issue="entry_phase names no declared phase",
        expect=422,
        code="apds_nonconformant",
        rule="APDS-LOOP-001",
        path="/loop/entry_phase",
        status="rejected",
    )
    value["loop"]["entry_phase"] = "nonexistent"
    write(folder / "source/agent.json", value)
    case(
        "knowledge-missing-file",
        "knowledge",
        issue="Built Knowledge declares a file omitted from its archive",
        expect=422,
        code="apds_nonconformant",
        rule="APDS-KNOW-002",
        path="/knowledge/documents/0/path",
        status="rejected",
    )
    for label, loop_label, phase, status in [
        ("graph-valid", "loop", "review", "conformant"),
        ("graph-invalid", "loop", "nonexistent", "rejected"),
        ("graph-legacy", "off-loop", "review", "incomplete"),
    ]:
        value, folder = case(
            label,
            issue=f"Phase {phase!r} against selected {loop_label}",
            expect=422 if status == "rejected" else 200,
            code="apds_nonconformant" if status == "rejected" else None,
            rule="APDS-BIND-002" if status == "rejected" else None,
            path="/bindings/phases/nonexistent" if status == "rejected" else None,
            status=status,
        )
        value["loop"] = f"@{namespace}/{prefix}-{loop_label}@0.1.0"
        value["bindings"] = {"phases": {phase: {}}}
        write(folder / "source/agent.json", value)
    value, folder = case(
        "graph-unavailable",
        issue="Declared Loop does not exist in this dev namespace",
        expect=422,
        code="invalid_dependency_access",
        status="rejected",
    )
    value["loop"] = f"@{namespace}/{prefix}-absent-loop@0.1.0"
    write(folder / "source/agent.json", value)
    for label, bad in [("matrix", False), ("matrix-missing-file", True)]:
        value, folder = case(
            label,
            "tool",
            mode="release",
            issue="Windows target omits main.py"
            if bad
            else "Two honest target descriptors, no native build claim",
            expect=422 if bad else 200,
            code="apds_nonconformant" if bad else None,
            rule="APDS-ID-004" if bad else None,
            path="/files/0" if bad else None,
            status="rejected" if bad else "conformant",
        )
        value.update(
            runtime={"type": "python", "version": "3.11"},
            files=["main.py"],
            entrypoint={"command": "python3", "args": ["main.py"]},
        )
        (folder / "source/main.js").unlink()
        (folder / "source/main.py").write_text(
            "import json\nprint(json.dumps({'ok':True}))\n"
        )
        write(folder / "source/agent.json", value)
    print("Prepared visible sources at", root / "cases")
    print(
        "No API request, S3 upload or publication has occurred. Build Knowledge/Memory before pack."
    )


def archive_bytes(source, *, target=None, omit=None):
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode="w") as archive:
            files = {
                str(p.relative_to(source)): p.read_bytes()
                for p in sorted(source.rglob("*"))
                if p.is_file()
            }
            if any(p.is_symlink() for p in source.rglob("*")):
                raise ValueError("Manual sources must not contain symlinks")
            if target:
                files["agentpm-artifact.json"] = json.dumps(
                    {"target": target, "format": "tar.gz"}
                ).encode()
            for name, data in files.items():
                if name == omit:
                    continue
                member = tarfile.TarInfo(name)
                member.mode, member.size = 0o644, len(data)
                archive.addfile(member, io.BytesIO(data))
    return output.getvalue()


def pack(root, label):
    folder = root / "cases" / label
    meta = read(folder / "case.json")
    if (folder / "init.json").exists():
        raise ValueError("Do not repackage a reserved case; start a fresh run")
    if meta["mode"] == "cli":
        raise ValueError(
            "CLI cases are packaged by agentpm publish, not this raw-client helper"
        )
    reserved = folder / "reservation-manifest.json"
    value = read(reserved if reserved.exists() else folder / "source/agent.json")
    if value["kind"] == "memory" and not (folder / "source/memory/build.json").exists():
        raise ValueError("Run agentpm memory build for this source before packaging")
    if value["kind"] == "knowledge" and "context" not in value["knowledge"]:
        raise ValueError("Run agentpm knowledge build for this source before packaging")
    session = read(root / "session.json")
    request = {"manifest": value, "namespace_handle": session["namespace"]}
    targets = TARGETS if meta["mode"] == "release" else ("any",)
    artifacts = []
    for target in targets:
        omit = (
            "main.py"
            if label == "matrix-missing-file" and target == TARGETS[1]
            else None
        )
        if label == "knowledge-missing-file":
            omit = value["knowledge"]["documents"][0]["path"]
        data = archive_bytes(
            folder / "source",
            target=target if meta["mode"] == "release" else None,
            omit=omit,
        )
        path = folder / (target + ".tar.gz")
        path.write_bytes(data)
        artifacts.append(
            {
                "target": target,
                "digest": "sha256:" + hashlib.sha256(data).hexdigest(),
                "size": len(data),
                "contentType": "application/gzip",
                "format": "tar.gz",
            }
        )
    if meta["mode"] == "release":
        release = {
            "type": "agentpm.package.release.v1",
            "kind": "tool",
            "name": f"@{session['namespace']}/{value['name']}",
            "version": value["version"],
            "manifestDigest": "sha256:"
            + hashlib.sha256(rfc8785.dumps(value)).hexdigest(),
            "artifacts": artifacts,
        }
        request["release"] = release
        statement = {
            "type": "agentpm.package.signature.v2",
            "kind": value["kind"],
            "name": release["name"],
            "version": value["version"],
            "releaseDigest": "sha256:"
            + hashlib.sha256(rfc8785.dumps(release)).hexdigest(),
        }
    else:
        request.update(sha256=artifacts[0]["digest"][7:], size=artifacts[0]["size"])
        statement = {
            "type": "agentpm.package.signature.v1",
            "kind": value["kind"],
            "name": value["name"],
            "version": value["version"],
            "artifactDigest": artifacts[0]["digest"],
        }
    statement["createdAt"] = (
        datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
    )
    key = SigningKey((root / "author.seed").read_bytes())
    message = (
        rfc8785.dumps(statement)
        if meta["mode"] == "release"
        else json.dumps(statement, sort_keys=True, separators=(",", ":")).encode()
    )
    signature = {
        "algo": "ed25519",
        "public_key_b64": base64.b64encode(bytes(key.verify_key)).decode(),
        "signature_b64": base64.b64encode(key.sign(message).signature).decode(),
        "statement_json": statement,
    }
    write(folder / "request.json", request)
    write(
        folder / "finalize-good.json",
        {"require_signature": True, "author_signatures": [signature]},
    )
    signature = {**signature, "signature_b64": base64.b64encode(bytes(64)).decode()}
    write(
        folder / "finalize-bad-signature.json",
        {"require_signature": True, "author_signatures": [signature]},
    )
    write(folder / "finalize-unsigned.json", {"require_signature": True})
    print(label, "packaged locally:", ", ".join(targets))
    print("Issue:", meta["issue"])
    print(
        "Expected finalize:",
        meta["expected_http"],
        meta["expected_code"],
        meta["rule"],
        meta["path"],
    )


def describe(root, label):
    folder = root / "cases" / label
    print("CASE:", folder)
    print(json.dumps(read(folder / "case.json"), indent=2))
    print("EMBEDDED SOURCE:", folder / "source/agent.json")
    print((folder / "source/agent.json").read_text())
    if (folder / "request.json").exists():
        print("RESERVED MANIFEST:")
        print(json.dumps(read(folder / "request.json")["manifest"], indent=2))
        for path in sorted(folder.glob("*.tar.gz")):
            with tarfile.open(path) as archive:
                print(path.name, "members:", archive.getnames())


def init_case(root, label):
    folder = root / "cases" / label
    mode = read(folder / "case.json")["mode"]
    route = "/v1/tools/publish/" + ("releases/init" if mode == "release" else "init")
    value = registry(root, label + "-init", route, read(folder / "request.json"))
    old = read(folder / "init.json") if (folder / "init.json").exists() else None
    if old:
        field = "release_id" if mode == "release" else "upload_id"
        if old[field] != value[field]:
            raise ValueError("Init retry changed reservation identity")
    write(folder / "init.json", value)
    print("Saved reservation:", folder / "init.json")


def upload(root, label, target):
    folder = root / "cases" / label
    init = read(folder / "init.json")
    if "release_id" in init:
        choices = [item for item in init["artifacts"] if item["target"] == target]
        if len(choices) != 1:
            raise ValueError("Choose one target from init.json")
        instruction = choices[0]
    else:
        if target != "any":
            raise ValueError("Single-archive cases use target any")
        instruction = init
    parts = urlsplit(instruction["put_url"])
    if parts.scheme != "https" or not re.fullmatch(
        r"agentpm-dev-tools\.s3(?:[.-][a-z0-9-]+)?\.amazonaws\.com",
        parts.hostname or "",
    ):
        raise ValueError("Refusing upload outside HTTPS agentpm-dev-tools S3 bucket")
    headers = dict(instruction["headers"])
    if any(key.lower() == "authorization" for key in headers):
        raise ValueError("Refusing Registry authorization in storage headers")
    path = folder / (target + ".tar.gz")
    headers["Content-Length"] = str(path.stat().st_size)
    with path.open("rb") as body:
        try:
            with open_request(
                Request(
                    instruction["put_url"], data=body, headers=headers, method="PUT"
                )
            ) as response:
                if response.status != 200:
                    raise ValueError("Unexpected S3 PUT status")
        except HTTPError as error:
            raise ValueError(
                f"S3 PUT HTTP {error.code}; repeat init to renew URLs, then retry upload"
            ) from None
    write(
        root / "evidence" / f"{label}-upload-{target}.json",
        {"http_status": 200, "target": target, "bytes": path.stat().st_size},
    )
    print("S3 PUT 200:", label, target, "to agentpm-dev-tools")


def finalize(root, label, variant, expected, code):
    folder = root / "cases" / label
    meta, init = read(folder / "case.json"), read(folder / "init.json")
    body = read(folder / ("finalize-" + variant + ".json"))
    route = "/v1/tools/publish/"
    if meta["mode"] == "release":
        route += "releases/" + init["release_id"] + "/finalize"
    else:
        route += "finalize"
        body["upload_id"] = init["upload_id"]
    expected = meta["expected_http"] if expected is None else expected
    code = (
        code
        if code is not None
        else (meta["expected_code"] if variant == "good" else None)
    )
    value = registry(root, label + "-finalize-" + variant, route, body, expected, code)
    if expected == 200:
        previous = (
            read(folder / "receipt.json")
            if (folder / "receipt.json").exists()
            else None
        )
        if previous and any(
            previous[key] != value[key]
            for key in ("id", "package_id", "version", "url")
        ):
            raise ValueError(
                "Completed retry changed identity or successful package link"
            )
        write(folder / "receipt.json", value)
    elif variant == "good" and meta["path"]:
        details = value.get("details", {})
        if meta["rule"]:
            issues = details.get("apds", {}).get("issues", [])
            if not any(
                i["rule_id"] == meta["rule"] and i["instance_path"] == meta["path"]
                for i in issues
            ):
                raise ValueError(
                    "Wrong rule/path: this is not the intended negative-test pass"
                )
        elif details.get("instance_path") != meta["path"]:
            raise ValueError("Wrong metadata mismatch field")
    print(
        "PASS: exact HTTP/classification"
        + (" and rule/path" if meta["path"] and variant == "good" else "")
    )


def inspect_case(root, label, expected):
    folder = root / "cases" / label
    meta = read(folder / "case.json")
    manifest = (
        read(folder / "request.json")["manifest"]
        if (folder / "request.json").exists()
        else read(folder / "source/agent.json")
    )
    namespace = read(root / "session.json")["namespace"]
    with database() as connection:
        package = connection.execute(
            "SELECT t.tool_id FROM tools t JOIN namespaces n USING(namespace_id) WHERE n.handle=%s AND t.name=%s",
            (namespace, manifest["name"]),
        ).fetchone()
        if not package:
            raise ValueError(
                "No reserved package found; check API/DB alignment before accepting a rejection"
            )
        rows = (
            []
            if not package
            else connection.execute(
                "SELECT tool_version_id,standard_id,standard_version,standard_origin,apds_validation,sha_256,s3_key,release_sha_256,release_format "
                "FROM tool_versions WHERE tool_id=%s AND version=%s",
                (package["tool_id"], manifest["version"]),
            ).fetchall()
        )
        if len(rows) != (0 if expected == "rejected" else 1):
            raise ValueError(
                "Unexpected version count; check that HTTP and DB point to the same local Registry"
            )
        result = {
            "case": label,
            "expected": expected,
            "versions": rows,
            "linked_rows": {},
        }
        for table in (
            "tool_signatures",
            "registry_attestations",
            "tool_version_scans",
            "package_version_artifacts",
        ):
            # Table names come only from the fixed internal tuple above.
            column = (
                "package_version_id"
                if table == "package_version_artifacts"
                else "tool_version_id"
            )
            count = (
                0
                if not rows
                else connection.execute(
                    "SELECT count(*) AS total FROM "
                    + table
                    + " WHERE "
                    + column
                    + "=%s",
                    (rows[0]["tool_version_id"],),
                ).fetchone()["total"]
            )
            result["linked_rows"][table] = count
        if expected == "rejected":
            init = read(folder / "init.json")
            if meta["mode"] == "release":
                reservation = connection.execute(
                    "SELECT status,package_id AS tool_id,version FROM package_publish_releases WHERE release_id=%s",
                    (init["release_id"],),
                ).fetchone()
            else:
                reservation = connection.execute(
                    "SELECT status,tool_id,version FROM uploads WHERE upload_id=%s",
                    (init["upload_id"],),
                ).fetchone()
            assert reservation and reservation["status"] == "pending"
            assert (
                reservation["tool_id"] == package["tool_id"]
                and reservation["version"] == manifest["version"]
            )
            result["reservation"] = reservation
        if expected != "rejected":
            row = rows[0]
            if expected == "undeclared":
                assert all(
                    row[key] is None
                    for key in (
                        "standard_id",
                        "standard_version",
                        "standard_origin",
                        "apds_validation",
                    )
                )
            else:
                assert (
                    row["standard_id"],
                    row["standard_version"],
                    row["standard_origin"],
                ) == ("agentpm", "1.0.0", "declared")
                if expected == "declared-only":
                    assert row["apds_validation"] is None
                else:
                    evidence = row["apds_validation"]
                    assert (
                        evidence["status"] == expected
                        and evidence["runner_evaluated"] is False
                    )
                    assert set(evidence["levels"]) == {
                        "standalone",
                        "packaged",
                        "resolved",
                    }
                    for level, scoped in evidence["levels"].items():
                        assert scoped["runner_evaluated"] is False
                        assert scoped["status"] == (
                            expected if level == "resolved" else "conformant"
                        )
                    if expected == "incomplete":
                        assert (
                            "APDS-ID-001"
                            in evidence["levels"]["resolved"]["unevaluated"]
                        )
            assert result["linked_rows"]["tool_signatures"] >= 1
            assert result["linked_rows"]["registry_attestations"] == 1
            artifact_count = (
                (2 if meta["mode"] == "release" else 1) if row["release_format"] else 0
            )
            assert result["linked_rows"]["tool_version_scans"] == max(1, artifact_count)
            assert result["linked_rows"]["package_version_artifacts"] == (
                artifact_count
            )
            if (folder / "request.json").exists():
                request = read(folder / "request.json")
                if meta["mode"] == "release":
                    assert (
                        row["release_sha_256"]
                        == hashlib.sha256(rfc8785.dumps(request["release"])).hexdigest()
                    )
                    artifacts = connection.execute(
                        "SELECT target,sha256,size_bytes FROM package_version_artifacts WHERE package_version_id=%s ORDER BY target",
                        (row["tool_version_id"],),
                    ).fetchall()
                    assert artifacts == [
                        {
                            "target": item["target"],
                            "sha256": item["digest"][7:],
                            "size_bytes": item["size"],
                        }
                        for item in request["release"]["artifacts"]
                    ]
                    result["artifacts"] = artifacts
                else:
                    assert row["sha_256"] == request["sha256"]
    write(root / "evidence" / (label + "-inspect.json"), result)
    print(json.dumps(result, indent=2, default=str))
    if package:
        # No install session or package download: direct public detail read.
        registry(
            root,
            label + "-discoverability",
            f"/tools/{package['tool_id']}/{manifest['version']}",
            None,
            404 if expected == "rejected" else 200,
        )
    print("PASS: actual LOCAL database and API publication state match", expected)


def signer_id(root):
    public = (root / "author-public-key.txt").read_text().strip()
    with database() as connection:
        rows = connection.execute(
            "SELECT s.signer_id FROM namespace_signers s JOIN namespaces n USING(namespace_id) WHERE n.handle=%s AND s.public_key_b64=%s AND s.is_active",
            (read(root / "session.json")["namespace"], public),
        ).fetchall()
    if len(rows) != 1:
        raise ValueError(
            "Register this run's disposable public key in the LOCAL namespace first"
        )
    print(rows[0]["signer_id"])


def phase_evidence(root):
    # Real publication outcomes establish the two phases even when the API was
    # started normally in IntelliJ and no helper-launcher config files exist.
    off = read(root / "evidence/off-declared-inspect.json")["versions"][0]
    if (
        off["standard_id"],
        off["standard_version"],
        off["standard_origin"],
        off["apds_validation"],
    ) != ("agentpm", "1.0.0", "declared", None):
        raise ValueError(
            "The off-phase declared-but-unvalidated publication was not observed"
        )
    attempts = sorted((root / "evidence/missing-standard-finalize-good").glob("*.json"))
    on = read(attempts[-1])
    issues = on.get("response", {}).get("details", {}).get("apds", {}).get("issues", [])
    if not (
        on["http_status"] == 422
        and on["response"].get("code") == "apds_nonconformant"
        and any(
            issue["rule_id"] == "APDS-ID-001" and issue["instance_path"] == "/standard"
            for issue in issues
        )
    ):
        raise ValueError("The on-phase missing-standard rejection was not observed")
    return {
        "source": "observed API responses and persisted version metadata",
        "off_case": "off-declared",
        "on_case": "missing-standard",
    }


def audit(root):
    count = 0
    for folder in sorted((root / "cases").iterdir()):
        meta = read(folder / "case.json")
        inspected = read(root / "evidence" / (folder.name + "-inspect.json"))
        if inspected["expected"] != meta["expected_status"]:
            raise ValueError("Missing final publication inspection for " + folder.name)
        if meta["mode"] == "cli":
            result = read(root / "evidence" / (folder.name + ".result.json"))
            assert result["exit_code"] == result["expected_exit_code"] == 0
        else:
            attempts = sorted(
                (root / "evidence" / (folder.name + "-finalize-good")).glob("*.json")
            )
            result = read(attempts[-1])
            assert result["http_status"] == meta["expected_http"]
            if meta["expected_code"]:
                assert result["response"]["code"] == meta["expected_code"]
            if meta["rule"]:
                assert any(
                    issue["rule_id"] == meta["rule"]
                    and issue["instance_path"] == meta["path"]
                    for issue in result["response"]["details"]["apds"]["issues"]
                )
        count += 1
    for variant, code in [
        ("unsigned", "signature_required"),
        ("bad-signature", "invalid_signature"),
    ]:
        result = read(
            sorted((root / "evidence" / ("agent-finalize-" + variant)).glob("*.json"))[
                -1
            ]
        )
        assert result["http_status"] == 422 and result["response"]["code"] == code
    phases = phase_evidence(root)
    write(
        root / "evidence/acceptance-summary.json",
        {
            "cases_checked": count,
            "result": "pass",
            "workspace": str(root),
            "server_phases": phases,
        },
    )
    print(
        f"PASS: {count} saved case inspections, exact API/CLI outcomes and both local server phases."
    )
    print(
        "This checks saved evidence only; it does not rerun commands or mark operator acceptance in tasks.md."
    )


def serve(root, enforcement):
    # Import only after setting flags. These are the actual app/routes/auth/storage.
    os.environ.update(
        FLASK_ENV="local",
        APDS_PUBLISH_ENFORCED="true" if enforcement == "on" else "false",
        RELEASE_PUBLISH_ENABLED="true",
    )
    from app.config import LocalConfig
    from sqlalchemy.engine import make_url

    db_url = make_url(LocalConfig.SQLALCHEMY_DATABASE_URI)
    if db_url.host not in ("localhost", "127.0.0.1"):
        raise ValueError("Local launcher requires a loopback DEVELOPMENT database")
    if LocalConfig.S3_BUCKET != "agentpm-dev-tools":
        raise ValueError(
            "Local launcher requires S3_BUCKET=agentpm-dev-tools in this process. "
            "IntelliJ run-configuration variables are not inherited by Terminal. "
            "You can run the API normally from IntelliJ instead; see runbook section 2."
        )
    from app.rate_limit import limiter
    from run import app

    # This local walkthrough contains many deliberate rejections; throttling is
    # not its subject. Auth, DB, storage and every application gate remain real.
    limiter.enabled = False
    write(
        root / "evidence" / f"server-{enforcement}.json",
        {
            "api": "http://127.0.0.1:5000",
            "database_host": db_url.host,
            "database_port": db_url.port,
            "database_name": db_url.database,
            "bucket": app.config["S3_BUCKET"],
            "APDS_PUBLISH_ENFORCED": app.config["APDS_PUBLISH_ENFORCED"],
            "RELEASE_PUBLISH_ENABLED": app.config["RELEASE_PUBLISH_ENABLED"],
            "rate_limiter_enabled": limiter.enabled,
        },
    )
    print(
        "REAL LOCAL API; enforcement",
        enforcement,
        "; DB",
        db_url.host,
        db_url.port,
        db_url.database,
    )
    app.run(host="127.0.0.1", port=5000, use_reloader=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=[
            "prepare",
            "preflight",
            "pack",
            "pack-all",
            "describe",
            "init",
            "upload",
            "finalize",
            "inspect",
            "signer-id",
            "serve",
            "audit",
        ],
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("case", nargs="?")
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--target", default="any")
    parser.add_argument(
        "--variant", choices=["good", "unsigned", "bad-signature"], default="good"
    )
    parser.add_argument("--expect", type=int)
    parser.add_argument("--code")
    parser.add_argument(
        "--state",
        choices=["conformant", "incomplete", "declared-only", "undeclared", "rejected"],
    )
    parser.add_argument("--enforcement", choices=["on", "off"], default="off")
    args = parser.parse_args()
    root = local_root(args.root)
    if args.action == "prepare":
        prepare(root, args.namespace)
    elif args.action == "preflight":
        print(json.dumps(namespace_info(root), indent=2, default=str))
        print("PASS: read-only local DB connection, namespace and APDS columns exist")
    elif args.action == "signer-id":
        signer_id(root)
    elif args.action == "serve":
        serve(root, args.enforcement)
    elif args.action == "pack-all":
        for folder in sorted((root / "cases").iterdir()):
            if read(folder / "case.json")["mode"] != "cli":
                pack(root, folder.name)
    elif args.action == "audit":
        audit(root)
    else:
        if not args.case or not re.fullmatch(r"[a-z][a-z0-9-]*", args.case):
            parser.error("Choose one named case")
        if args.action == "pack":
            pack(root, args.case)
        elif args.action == "describe":
            describe(root, args.case)
        elif args.action == "init":
            init_case(root, args.case)
        elif args.action == "upload":
            upload(root, args.case, args.target)
        elif args.action == "finalize":
            finalize(root, args.case, args.variant, args.expect, args.code)
        elif args.action == "inspect":
            inspect_case(
                root,
                args.case,
                args.state
                or read(root / "cases" / args.case / "case.json")["expected_status"],
            )


if __name__ == "__main__":
    main()
