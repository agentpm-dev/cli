"""Explicit local/dev signing walkthrough; reuse the proven M13B archive transport."""

import argparse
import base64
import copy
import getpass
import hashlib
import importlib.util
import io
import os
import uuid
from contextlib import redirect_stdout
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlsplit

from nacl.signing import SigningKey, VerifyKey

PREVIOUS = Path(__file__).parents[1] / "m13b/fixture.py"
SPEC = importlib.util.spec_from_file_location("m14b_archive_transport", PREVIOUS)
transport = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(transport)
read, write = transport.read, transport.write
VARIANTS = (
    "primary",
    "secondary",
    "algorithm",
    "type",
    "identity",
    "digest",
    "bad-signature",
    "unregistered",
    "missing",
    "replay",
)


def prepare(root, namespace):
    with redirect_stdout(io.StringIO()):
        transport.prepare(root, namespace)
    session = read(root / "session.json")
    session["name"] = (
        session["name"].replace("m13b-", "m14b-") + "-" + uuid.uuid4().hex[:8]
    )
    write(root / "session.json", session)
    for case in transport.CASES:
        path = root / case / "agent.json"
        manifest = read(path)
        manifest["name"] = session["name"] + ("-legacy" if case == "legacy" else "")
        manifest["description"] = "Disposable development release signing verification."
        write(path, manifest)
    for name in ("primary", "secondary"):
        key = SigningKey.generate()
        path = root / (name + ".seed")
        with path.open("xb") as output:
            output.write(bytes(key))
        path.chmod(0o600)
        (root / (name + "-public-key.txt")).write_text(
            base64.b64encode(bytes(key.verify_key)).decode() + "\n"
        )
    print("M14B identity:", session["name"])
    print(
        "Created two disposable dev keys; no registration, upload or publish occurred."
    )


def signed_body(root, request, key_name):
    from app.tools.domain.tools.release_signing import AuthorSignatureV2
    from app.tools.domain.tools.releases import ReleaseManifest

    key = SigningKey((root / (key_name + ".seed")).read_bytes())
    release = ReleaseManifest.model_validate(request["release"])
    statement = AuthorSignatureV2.for_release(
        release, datetime.now(timezone.utc).isoformat()
    )
    return {
        "require_signature": True,
        "author_signatures": [
            {
                "algo": "ed25519",
                "public_key_b64": base64.b64encode(bytes(key.verify_key)).decode(),
                "signature_b64": base64.b64encode(
                    key.sign(statement.canonical_bytes()).signature
                ).decode(),
                "statement_json": statement.model_dump(),
            }
        ],
    }


def sign(args):
    directory = args.root / args.case
    path = directory / ("finalize-" + args.key + ".json")
    if path.exists():
        print("Using the unchanged saved envelope:", path)
        return
    request_path = directory / "request.json"
    if not request_path.is_file():
        raise SystemExit(
            f"{args.case}/request.json is missing; signing requires this case's "
            "archive/package preparation first. In the same runbook terminal, run:\n"
            f'  cd "$M14B_ROOT/{args.case}"\n'
            '  m14b publish --dry-run --schema "$M14B_SCHEMA"\n'
            f"  m14b_fixture package {args.case}\n"
            f"  m14b_fixture sign {args.case} --key {args.key}"
        )
    request = read(request_path)
    body = signed_body(args.root, request, args.key)
    write(path, body)
    if args.key == "primary":
        changes = {
            "algorithm": ("algo", "rsa"),
            "bad-signature": ("signature_b64", base64.b64encode(bytes(64)).decode()),
        }
        for variant, (field, value) in changes.items():
            changed = copy.deepcopy(body)
            changed["author_signatures"][0][field] = value
            write(directory / ("finalize-" + variant + ".json"), changed)
        for variant, field, value in (
            ("type", "type", "agentpm.package.signature.v1"),
            ("identity", "name", "@other/example"),
            ("digest", "releaseDigest", "sha256:" + "f" * 64),
        ):
            changed = copy.deepcopy(body)
            changed["author_signatures"][0]["statement_json"][field] = value
            write(directory / ("finalize-" + variant + ".json"), changed)
        write(
            directory / "finalize-unregistered.json",
            signed_body(args.root, request, "secondary"),
        )
        write(directory / "finalize-missing.json", {"require_signature": True})
        if len(request["release"]["artifacts"]) > 1:
            other = copy.deepcopy(request)
            other["release"]["artifacts"] = other["release"]["artifacts"][:1]
            write(directory / "replay-source-request.json", other)
            write(
                directory / "finalize-replay.json",
                signed_body(args.root, other, "primary"),
            )
    print(
        "Saved signing requests, with stable timestamps for exact retries:", directory
    )
    print(
        "Release digest:",
        body["author_signatures"][0]["statement_json"]["releaseDigest"],
    )


def finalize(args):
    directory = args.root / args.case
    reservation = read(directory / "init.json")
    body = read(directory / ("finalize-" + args.variant + ".json"))
    value = transport.registry(
        "/v1/tools/publish/releases/" + reservation["release_id"] + "/finalize",
        body,
        args.expect,
        args.code,
    )
    if args.expect != 200:
        return
    assert (
        value["author_signatures_accepted"] == 1
    ), "expected one accepted logical signature"
    receipt = directory / "receipt.json"
    if receipt.exists():
        assert read(receipt) == value, "completed retry changed the receipt"
    write(receipt, value)
    print("Accepted author signatures:", value["author_signatures_accepted"])
    print("Version:", value["version"], "release digest:", value["release_digest"])


def registry_public_key(root):
    # No private key is retained, logged or transmitted. This is an out-of-band
    # dev diagnostic, not a replacement for the later consumer trust anchor.
    private = getpass.getpass("Local DEV Registry REGISTRY_PRIVATE_KEY_B64 (hidden): ")
    key = SigningKey(base64.b64decode(private.strip(), validate=True))
    public = base64.b64encode(bytes(key.verify_key)).decode()
    (root / "registry-public-key.txt").write_text(public + "\n")
    print("Saved only the public verification key; backend configuration is unchanged.")


def database():
    import psycopg
    from psycopg.rows import dict_row

    dsn = os.environ.get("M14B_DATABASE_URL", "").replace(
        "postgresql+psycopg://", "postgresql://", 1
    )
    parts = urlsplit(dsn)
    if parts.scheme not in ("postgres", "postgresql") or parts.hostname not in (
        "localhost",
        "127.0.0.1",
    ):
        raise ValueError(
            "M14B_DATABASE_URL must name your loopback DEVELOPMENT PostgreSQL database"
        )
    connection = psycopg.connect(
        dsn,
        connect_timeout=10,
        row_factory=dict_row,
        options="-c default_transaction_read_only=on",
    )
    location = connection.execute(
        "SELECT current_database() AS database_name, "
        "inet_server_port() AS server_port, current_schema() AS schema_name, "
        "current_setting('search_path') AS search_path, "
        "to_regclass('namespace_signers') IS NOT NULL AS has_signers"
    ).fetchone()
    if not location["has_signers"]:
        connection.close()
        raise SystemExit(
            f"Inspection connected to database {location['database_name']!r}, "
            f"port {location['server_port']}, schema {location['schema_name']!r} "
            f"(search_path={location['search_path']!r}), but namespace_signers "
            "is not visible. Match M14B_DATABASE_URL to the running backend's "
            "SQLALCHEMY_DATABASE_URI, including port/database/schema. "
            "The default local profile uses port 5433, database postgres. "
            "Do not create tables or repeat signer registration in a different database."
        )
    return connection


def signer_id(args):
    public = (args.root / (args.key + "-public-key.txt")).read_text().strip()
    namespace = read(args.root / "session.json")["namespace"]
    with database() as connection:
        rows = connection.execute(
            "SELECT s.signer_id FROM namespace_signers s JOIN namespaces n "
            "ON n.namespace_id=s.namespace_id WHERE n.handle=%s AND s.public_key_b64=%s",
            (namespace, public),
        ).fetchall()
    if len(rows) != 1:
        raise ValueError(
            "expected one registration for this disposable public key in the dev namespace"
        )
    print(rows[0]["signer_id"])


def inspect(args):
    from app.tools.domain.tools.release_signing import (
        AuthorSignatureV2,
        RegistryAttestationV3,
    )
    from app.tools.domain.tools.releases import ReleaseManifest

    directory = args.root / args.case
    identity = str(uuid.UUID(read(directory / "init.json")["release_id"]))
    with database() as connection:
        reservation = connection.execute(
            "SELECT status FROM package_publish_releases WHERE release_id=%s",
            (identity,),
        ).fetchone()
        assert reservation is not None, "reservation not found in this local database"
        versions = connection.execute(
            "SELECT v.* FROM tool_versions v JOIN package_publish_releases r "
            "ON v.tool_id=r.package_id AND v.version=r.version WHERE r.release_id=%s",
            (identity,),
        ).fetchall()
        if args.pending:
            assert reservation["status"] == "pending" and not versions
            print(
                "PASS: reservation pending; no version or version-linked signing rows published."
            )
            return
        assert reservation["status"] == "finalized" and len(versions) == 1
        version = versions[0]
        version_id = version["tool_version_id"]
        authors = connection.execute(
            "SELECT a.*,s.public_key_b64,s.is_active,s.revoked_ts FROM tool_signatures a "
            "JOIN namespace_signers s ON s.signer_id=a.signer_id WHERE a.tool_version_id=%s",
            (version_id,),
        ).fetchall()
        registries = connection.execute(
            "SELECT * FROM registry_attestations WHERE tool_version_id=%s",
            (version_id,),
        ).fetchall()
        artifacts = connection.execute(
            "SELECT target,sha256,size_bytes FROM package_version_artifacts "
            "WHERE package_version_id=%s ORDER BY target",
            (version_id,),
        ).fetchall()
    release = ReleaseManifest.model_validate(
        read(directory / "request.json")["release"]
    )
    assert version["release_format"] == "agentpm.package.release.v1"
    assert version["status"] == "published" and version[
        "release_manifest"
    ] == release.model_dump(exclude_none=True)
    assert "sha256:" + version["release_sha_256"] == release.digest()
    assert len(authors) == len(registries) == 1
    assert [(a["target"], a["sha256"], a["size_bytes"]) for a in artifacts] == [
        (a.target, a.digest[7:], a.size) for a in release.artifacts
    ]
    for row in authors:
        statement = AuthorSignatureV2.model_validate(row["statement_json"])
        assert statement.matches(release) and row["algo"] == "ed25519"
        assert (
            row["public_key_b64"]
            == (args.root / (args.key + "-public-key.txt")).read_text().strip()
        )
        VerifyKey(base64.b64decode(row["public_key_b64"])).verify(
            statement.canonical_bytes(),
            base64.b64decode(row["signature_b64"]),
        )
        assert row["is_active"] == (not args.revoked)
        assert (row["revoked_ts"] is not None) == args.revoked
    registry = registries[0]
    statement = RegistryAttestationV3.model_validate(registry["statement_json"])
    assert registry["algo"] == "ed25519"
    assert statement.publishedByAccountId == version["published_by_user_sub"]
    assert (
        statement.matches(release)
        and statement.registryKeyId == registry["registry_key_id"]
    )
    public = (args.root / "registry-public-key.txt").read_text().strip()
    VerifyKey(base64.b64decode(public)).verify(
        statement.canonical_bytes(),
        base64.b64decode(registry["signature_b64"]),
    )
    evidence = {
        "release_digest": release.digest(),
        "version_id": version_id,
        "authors": authors,
        "registry_attestations": registries,
        "artifacts": artifacts,
    }
    # Public provenance only, no PAT, database URL or private key.
    import json

    path = directory / ("evidence-revoked.json" if args.revoked else "evidence.json")
    path.write_text(json.dumps(evidence, default=str, indent=2) + "\n")
    print(
        "PASS: one author v2 + one Registry v3 verify for",
        len(artifacts),
        "artifact(s).",
    )
    print("Current author active:", authors[0]["is_active"], "revoked:", args.revoked)
    print("Saved public signing evidence:", path)


def legacy_check(args):
    from app.tools.domain.tools.legacy_signing import bound_author_statement_bytes
    from app.tools.domain.tools.release_signing import key_id

    manifest = read(args.root / "legacy/agent.json")
    identity = str(uuid.UUID(args.tool_id))
    security = transport.registry(
        f"/tools/{identity}/{manifest['version']}/security", None, 200
    )
    public = Path(args.public_key).read_text().strip()
    archives = list((args.root / "legacy/target/agentpm").glob("*.tar.gz"))
    assert len(archives) == 1
    digest = "sha256:" + hashlib.sha256(archives[0].read_bytes()).hexdigest()
    matched = [
        a for a in security["author_signatures"] if a["public_key_b64"] == public
    ]
    assert matched, "no accepted legacy author signature from the selected key"
    for row in matched:
        assert row["statement_json"]["type"] == "agentpm.package.signature.v1"
        message = bound_author_statement_bytes(
            row["statement_json"],
            name=manifest["name"],
            version=manifest["version"],
            kind="tool",
            artifact_digest=digest,
        )
        assert message is not None and row["key_id"] == key_id(public)
        VerifyKey(base64.b64decode(public)).verify(
            message, base64.b64decode(row["signature_b64"])
        )
    assert security["registry_attested"] is True
    write(args.root / "legacy/security-evidence.json", security)
    print(
        "PASS: real legacy v1 signature, 16-character key ID and attestation presence."
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=(
            "prepare",
            "package",
            "sign",
            "init",
            "upload",
            "renew",
            "finalize",
            "inspect",
            "signer-id",
            "registry-public-key",
            "legacy-check",
        ),
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("case", nargs="?", choices=transport.CASES)
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--key", choices=("primary", "secondary"), default="primary")
    parser.add_argument("--variant", choices=VARIANTS, default="primary")
    parser.add_argument("--target", choices=("any", *transport.TARGETS), default="any")
    parser.add_argument("--expect", type=int, default=200)
    parser.add_argument("--code")
    parser.add_argument("--corrupt", action="store_true")
    parser.add_argument("--pending", action="store_true")
    parser.add_argument("--revoked", action="store_true")
    parser.add_argument("--tool-id")
    parser.add_argument("--public-key")
    args = parser.parse_args()
    if (
        args.command
        not in ("prepare", "signer-id", "registry-public-key", "legacy-check")
        and args.case is None
    ):
        parser.error("choose a case")
    if args.command == "legacy-check" and not (args.tool_id and args.public_key):
        parser.error("legacy-check needs --tool-id and --public-key")
    # Namespaced aliases apply only inside this helper process; no shell/backend
    # environment is changed. The old transport enforces loopback/dev S3 guards.
    os.environ["M13B_BASE"] = os.environ.get("M14B_BASE", "")
    os.environ["M13B_TOKEN"] = os.environ.get("M14B_TOKEN", "")
    args.request = "request.json"
    if args.command == "prepare":
        prepare(args.root, args.namespace)
    elif args.command == "registry-public-key":
        registry_public_key(args.root)
    elif args.command == "package":
        transport.package(args.root, args.case)
    elif args.command == "sign":
        sign(args)
    elif args.command in ("init", "renew"):
        transport.api_step(args)
    elif args.command == "upload":
        transport.upload(args)
    elif args.command == "finalize":
        finalize(args)
    elif args.command == "inspect":
        inspect(args)
    elif args.command == "signer-id":
        signer_id(args)
    else:
        legacy_check(args)


if __name__ == "__main__":
    main()
