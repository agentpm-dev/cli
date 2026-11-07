"""M14C local/dev preparation and explicit public trust inspection.

Only seed writes database rows, through the reviewed backfill primitive.
No command generates a root, publishes, uploads, changes policy or starts Flask.
"""

import argparse
import base64
import copy
import getpass
import json
import os
import shutil
import uuid
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import HTTPRedirectHandler, Request, build_opener

BASE = "http://127.0.0.1:5000"
FIXTURES = Path(__file__).resolve().parent
REPO = FIXTURES.parents[4]
ROOT_FILES = (
    "crates/agentpm-sdk/contracts/registry-trust-roots-v1.json",
    "app/tools/domain/tools/contracts/registry-trust-roots-v1.json",
)


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    with Path(path).open("x") as output:
        output.write(json.dumps(value, indent=2) + "\n")


def prepare(root):
    root.mkdir(parents=True, exist_ok=True)
    write(root / "session.json", {"name": "m14c-check-" + uuid.uuid4().hex[:12]})
    for directory in ("author", "consumer", "evidence", "runner/src"):
        (root / directory).mkdir(parents=True)
    source = FIXTURES.parent / "m14a"
    manifest = read(source / "agent.fixture.json")
    manifest["name"] = read(root / "session.json")["name"]
    write(root / "author/agent.json", manifest)
    for name in ("tool.py", "README.md"):
        shutil.copyfile(source / name, root / "author" / name)
    sdk = json.dumps(str(REPO / "crates/agentpm-sdk"))
    (root / "runner/Cargo.toml").write_text(
        '[package]\nname = "m14c-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
        f"[dependencies]\nagentpm-sdk = {{ path = {sdk} }}\n"
        'anyhow = "1"\nserde_json = "1"\n'
        'tokio = { version = "1", features = ["macros", "rt-multi-thread"] }\n'
        "[workspace]\n"
    )
    shutil.copyfile(FIXTURES / "consumer.rs", root / "runner/src/main.rs")
    print("Prepared disposable source/evidence and a consumer of the real Rust SDK.")
    print("No root, network request, database write or publication occurred.")


def pin(public_file, api):
    value = read(public_file)
    if (
        set(value) != {"version", "roots"}
        or value["version"] != 1
        or len(value["roots"]) != 1
    ):
        raise ValueError("expected one public root from generate-root")
    root = value["roots"][0]
    if (
        set(root) != {"registry", "rootKeyId", "publicKeyB64"}
        or root["registry"] != BASE
    ):
        raise ValueError(
            "only the exact loopback public pin is accepted; no secret fields"
        )
    public = base64.b64decode(root["publicKeyB64"], validate=True)
    if len(public) != 32 or base64.b64encode(public).decode() != root["publicKeyB64"]:
        raise ValueError("invalid full Ed25519 public key")
    if not isinstance(root["rootKeyId"], str) or not root["rootKeyId"].strip():
        raise ValueError("missing root key ID")
    paths = (REPO / ROOT_FILES[0], api.resolve() / ROOT_FILES[1])
    originals = [read(path) for path in paths]
    if originals[0] != originals[1]:
        raise ValueError(
            "existing audited root copies differ; reconcile before pinning"
        )
    if any(item["registry"] == BASE and item != root for item in originals[0]["roots"]):
        raise ValueError("an established dev root exists; never silently rotate it")
    merged = copy.deepcopy(originals[0])
    if root not in merged["roots"]:
        merged["roots"].append(root)
    for path in paths:
        path.write_text(json.dumps(merged, indent=2) + "\n")
    print(
        "Pinned dev PUBLIC root in both audited files; existing other origins retained."
    )
    print("Review/commit these public diffs later; no push or deployment occurred.")


def engine(read_only=True):
    from sqlalchemy import create_engine
    from sqlalchemy.engine import make_url

    url = make_url(os.environ["M14C_DATABASE_URL"])
    if url.get_backend_name() != "postgresql" or url.host not in (
        "127.0.0.1",
        "localhost",
        "::1",
    ):
        raise ValueError(
            "M14C_DATABASE_URL must name the loopback DEVELOPMENT database"
        )
    url = url.set(drivername="postgresql+psycopg")
    options = {"options": "-c default_transaction_read_only=on"} if read_only else {}
    return create_engine(url, connect_args=options)


def seed(args):
    from app.tools.domain.tools.registry_trust import export_inventory, seed_current_key
    from flask import Flask
    from sqlalchemy.orm import Session

    if args.out.exists():
        raise ValueError("inventory output exists; select a new output name")
    application = Flask("m14c-explicit-dev-backfill")
    application.config.update(
        REGISTRY_KEY_ID=args.key_id,
        REGISTRY_PRIVATE_KEY_B64=getpass.getpass(
            "Existing LOCAL backend attestation private key: "
        ),
    )
    # Minimal context: do not run create_app, migrations, ingestors or scheduler.
    with application.app_context(), engine(read_only=False).begin() as connection:
        with Session(bind=connection) as session:
            row = seed_current_key(session)
            inventory = export_inventory(session)
            identity = row.key_id
    write(args.out, inventory)
    print("Verified historical attestations and registered public key:", identity)
    print("Exported complete PUBLIC inventory:", args.out)


def catalogue():
    from sqlalchemy import text

    with engine().connect() as connection:
        rows = connection.execute(
            text(
                "SELECT t.tool_id, n.handle, t.name, v.version, v.release_format, "
                "a.statement_json->>'type' AS attestation_type "
                "FROM tool_versions v JOIN tools t USING(tool_id) "
                "JOIN namespaces n USING(namespace_id) "
                "JOIN registry_attestations a USING(tool_version_id) "
                "WHERE t.kind='tool' AND v.status='published' "
                "ORDER BY v.tool_version_id DESC LIMIT 40"
            )
        )
        for row in rows:
            print(
                row.tool_id,
                f"@{row.handle}/{row.name}@{row.version}",
                row.attestation_type,
                row.release_format or "legacy",
            )


def proof(args):
    from app.tools.domain.tools.entities import Namespace, Package, PackageVersion
    from app.tools.domain.tools.provenance import verification_material
    from sqlalchemy import select
    from sqlalchemy.orm import Session

    with engine().connect() as connection, Session(bind=connection) as session:
        selected = session.execute(
            select(PackageVersion, Package, Namespace)
            .join(Package, Package.package_id == PackageVersion.package_id)
            .join(Namespace, Namespace.namespace_id == Package.namespace_id)
            .where(
                Package.package_id == args.tool_id,
                PackageVersion.version == args.version,
            )
        ).one_or_none()
        if selected is None:
            raise ValueError(
                "version not found; match UUID/version to this local database's catalogue"
            )
        version, package, namespace = selected
        if version.status != "published" or package.kind != "tool":
            raise ValueError("select a published development Tool")
        material = verification_material(session, namespace, version)
        if material["registry_attestation"] is None:
            raise ValueError("selected version has no saved Registry attestation")
        value = {
            "kind": package.kind,
            "name": f"@{namespace.handle}/{package.name}",
            "version": version.version,
            "integrity": version.release_sha_256 or version.sha_256,
            "presigned_url": "https://unused.invalid/no-download",
            "size": None,
            "content_type": None,
            "python_resolution": version.python_resolution,
            "verification": material,
        }
        write(args.out, value)
        print("Saved public proof from existing rows:", value["name"], version.version)
        print("Type:", material["registry_attestation"]["statement_json"]["type"])
        print(
            "Authors:", len(material["authors"]), "namespace policy:", material["mode"]
        )
        print("No DB rows, URLs, credentials or signatures were changed.")


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def keyset(args):
    request = Request(BASE + "/v1/registry/signing-keys")
    try:
        response = build_opener(NoRedirect()).open(request, timeout=15)
    except HTTPError as error:
        response = error
    with response:
        status, raw = response.status, response.read(262145)
    if status != args.expect:
        raise ValueError(f"expected HTTP {args.expect}, got {status}: {raw[:300]!r}")
    if args.expect == 503:
        if json.loads(raw).get("code") != "registry_keyset_unavailable":
            raise ValueError("not the expected fail-closed key endpoint")
        print("PASS: key endpoint fails closed with HTTP 503.")
        return
    if len(raw) > 262144:
        raise ValueError("key set exceeds size limit")
    from app.tools.domain.tools.registry_trust import SignedKeySet

    document = SignedKeySet.model_validate_json(raw)
    document.verify(read(REPO / ROOT_FILES[0])["roots"], BASE)
    write(args.out, json.loads(raw))
    print(
        "PASS: root signature, origin and validity; expires:",
        document.statement.expiresAt,
    )
    print("Registry key IDs:", ", ".join(key.keyId for key in document.statement.keys))


def tamper(root, source):
    original = read(source)
    changes = {}
    bad = copy.deepcopy(original)
    bad["verification"]["registry_attestation"][
        "registry_key_id"
    ] = "unregistered-m14c-key"
    changes["unknown-key"] = bad
    bad = copy.deepcopy(original)
    bad["verification"]["registry_attestation"]["signature_b64"] = base64.b64encode(
        bytes(64)
    ).decode()
    changes["bad-attestation"] = bad
    bad = copy.deepcopy(original)
    bad["verification"]["manifest"]["version"] = "99.0.0"
    changes["manifest"] = bad
    bad = copy.deepcopy(original)
    bad["verification"]["authors"] = []
    bad["verification"]["mode"] = "required"
    changes["required-unsigned"] = bad
    if original["verification"].get("release"):
        bad = copy.deepcopy(original)
        bad["verification"]["release"]["artifacts"][0][
            "target"
        ] = "x86_64-unknown-linux-gnu"
        # Ensure the mutation differs even for a Linux-first inventory.
        if bad == original:
            bad["verification"]["release"]["artifacts"][0][
                "target"
            ] = "aarch64-apple-darwin"
        changes["target"] = bad
    for label, value in changes.items():
        write(root / "evidence" / f"tampered-{label}.json", value)
    keyset_file = root / "evidence/dev-keyset.json"
    if keyset_file.exists():
        bad = read(keyset_file)
        bad["signatureB64"] = base64.b64encode(bytes(64)).decode()
        write(root / "evidence/tampered-keyset.json", bad)
    print("Saved mutated COPIES only:", ", ".join(changes))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=("prepare", "pin", "catalogue", "seed", "proof", "keyset", "tamper"),
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("--api", type=Path)
    parser.add_argument("--public", type=Path)
    parser.add_argument("--key-id")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--tool-id", type=uuid.UUID)
    parser.add_argument("--version")
    parser.add_argument("--source", type=Path)
    parser.add_argument("--expect", type=int, choices=(200, 503), default=200)
    args = parser.parse_args()
    required = {
        "pin": ("public", "api"),
        "seed": ("key_id", "out"),
        "proof": ("tool_id", "version", "out"),
        "tamper": ("source",),
    }.get(args.command, ())
    if args.command == "keyset" and args.expect == 200:
        required = ("out",)
    for name in required:
        if getattr(args, name) is None:
            parser.error(f"{args.command} requires --{name.replace('_', '-')}")
    try:
        if args.command == "prepare":
            prepare(args.root)
        elif args.command == "pin":
            pin(args.public, args.api)
        elif args.command == "catalogue":
            catalogue()
        elif args.command == "seed":
            seed(args)
        elif args.command == "proof":
            proof(args)
        elif args.command == "keyset":
            keyset(args)
        else:
            tamper(args.root, args.source)
    except (ValueError, OSError, KeyError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
