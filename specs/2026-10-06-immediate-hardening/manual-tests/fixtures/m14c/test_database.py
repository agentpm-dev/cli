"""Verify the runbook backfill/proof commands against disposable PostgreSQL."""

import base64
import importlib.util
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

from app.tools.domain.tools.entities import (
    PackageVersion,
    RegistryAttestation,
    RegistrySigningKey,
)
from app.tools.domain.tools.legacy_signing import legacy_statement_bytes
from nacl.signing import SigningKey
from sqlalchemy.orm import Session
from test_release_migration import disposable_database
from test_release_publish import release_database

SPEC = importlib.util.spec_from_file_location(
    "manual_m14c_database", Path(__file__).with_name("fixture.py")
)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
database_fixture = disposable_database
release_db_fixture = release_database


def test_explicit_seed_commits_and_old_proof_export_preserves_saved_statement(
    release_db, tmp_path, monkeypatch, capsys
):
    api = fixture.REPO.parent / "agentpm-api"
    with release_db.begin() as connection:
        connection.exec_driver_sql(
            (
                api / "app/migrations/V1.105__Persist_Registry_Signing_Key_History.sql"
            ).read_text()
        )
    key = SigningKey(bytes([7]) * 32)
    statement = {
        "type": "agentpm.registry.attestation.v2",
        "kind": "tool",
        "name": "@fixture/existing",
        "version": "0.1.0",
        "artifactDigest": "sha256:" + "a" * 64,
        "registryKeyId": "manual-dev-key",
        "storedAt": "2026-11-06T01:00:00Z",
    }
    signature = base64.b64encode(
        key.sign(legacy_statement_bytes(statement)).signature
    ).decode()
    with Session(release_db) as session:
        version = session.get(PackageVersion, 1)
        version.manifest = {"kind": "tool", "name": "existing", "version": "0.1.0"}
        tool_id = version.package_id
        session.add(
            RegistryAttestation(
                package_version_id=1,
                registry_key_id="manual-dev-key",
                algo="ed25519",
                statement_json=statement,
                signature_b64=signature,
            )
        )
        session.commit()
    monkeypatch.setattr(fixture, "engine", lambda read_only=True: release_db)
    secret = base64.b64encode(bytes(key)).decode()
    with patch.object(fixture.getpass, "getpass", return_value=secret):
        fixture.seed(
            Namespace(key_id="manual-dev-key", out=tmp_path / "inventory.json")
        )
        fixture.seed(
            Namespace(key_id="manual-dev-key", out=tmp_path / "inventory-again.json")
        )
    with Session(release_db) as session:
        assert session.get(RegistrySigningKey, "manual-dev-key") is not None
    fixture.proof(
        Namespace(tool_id=tool_id, version="0.1.0", out=tmp_path / "proof.json")
    )
    exported = fixture.read(tmp_path / "proof.json")
    assert exported["integrity"] == "a" * 64
    proof = exported["verification"]["registry_attestation"]
    assert proof["statement_json"] == statement
    assert base64.b64decode(proof["statement_bytes_b64"]) == legacy_statement_bytes(
        statement
    )
    assert proof["signature_b64"] == signature
    fixture.catalogue()
    assert "@fixture/existing@0.1.0" in capsys.readouterr().out
    assert all(secret not in path.read_text() for path in tmp_path.glob("*.json"))
