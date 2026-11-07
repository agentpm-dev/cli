"""Local-only checks for the manual fixture; never use a real Registry or S3."""

import base64
import importlib.util
import json
import os
from argparse import Namespace
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch

import pytest
from app.tools.application import release_publish as service
from app.tools.domain.tools.release_signing import AuthorSignatureV2
from app.tools.domain.tools.releases import ReleaseManifest
from nacl.signing import VerifyKey
from sqlalchemy import text
from sqlalchemy.orm import Session
from test_release_migration import disposable_database
from test_release_publish import Storage, release_database, reserve
from test_release_signing import register

SPEC = importlib.util.spec_from_file_location(
    "manual_m14b", Path(__file__).with_name("fixture.py")
)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
database_fixture = disposable_database
release_db_fixture = release_database
CONTRACT = (
    Path(__file__).parents[5]
    / "crates/agentpm-sdk/contracts/release-signing-v1.fixtures.json"
)


@pytest.fixture(name="prepared")
def prepared_fixture(tmp_path):
    fixture.prepare(tmp_path, "zack")
    request = json.loads(CONTRACT.read_text())["vectors"][2]["release"]
    fixture.write(tmp_path / "matrix/request.json", {"release": request})
    return tmp_path


def test_preparation_is_private_unique_and_cannot_overwrite_a_session(prepared):
    session = fixture.read(prepared / "session.json")
    assert session["name"].startswith("m14b-check-")
    primary, secondary = prepared / "primary.seed", prepared / "secondary.seed"
    assert primary.read_bytes() != secondary.read_bytes()
    assert primary.stat().st_mode & 0o777 == 0o600
    assert secondary.stat().st_mode & 0o777 == 0o600
    assert fixture.read(prepared / "legacy/agent.json")["name"].endswith("-legacy")
    with pytest.raises(SystemExit, match="m14b_fixture package portable"):
        fixture.sign(Namespace(root=prepared, case="portable", key="primary"))
    assert not (prepared / "portable/finalize-primary.json").exists()
    with pytest.raises(FileExistsError):
        fixture.prepare(prepared, "zack")


def test_signing_saves_exact_retries_and_an_authentic_subset_replay(prepared):
    args = Namespace(root=prepared, case="matrix", key="primary")
    fixture.sign(args)
    path = prepared / "matrix/finalize-primary.json"
    first = path.read_bytes()
    fixture.sign(args)
    assert path.read_bytes() == first
    original = fixture.read(path)["author_signatures"][0]
    replay = fixture.read(prepared / "matrix/finalize-replay.json")[
        "author_signatures"
    ][0]
    public = VerifyKey(base64.b64decode(original["public_key_b64"]))
    release = ReleaseManifest.model_validate(
        fixture.read(prepared / "matrix/request.json")["release"]
    )
    for envelope, matches in ((original, True), (replay, False)):
        statement = AuthorSignatureV2.model_validate(envelope["statement_json"])
        public.verify(
            statement.canonical_bytes(), base64.b64decode(envelope["signature_b64"])
        )
        assert statement.matches(release) is matches
    fixture.sign(Namespace(root=prepared, case="matrix", key="secondary"))
    secondary = fixture.read(prepared / "matrix/finalize-secondary.json")
    assert (
        secondary["author_signatures"][0]["public_key_b64"]
        != original["public_key_b64"]
    )


def test_registry_key_derivation_retains_only_public_material(prepared, capsys):
    secret = base64.b64encode(bytes([9]) * 32).decode()
    with patch.object(fixture.getpass, "getpass", return_value=secret):
        fixture.registry_public_key(prepared)
    assert secret not in capsys.readouterr().out
    assert (
        len(base64.b64decode((prepared / "registry-public-key.txt").read_text())) == 32
    )
    assert not any(
        secret in path.read_text(errors="replace") for path in prepared.rglob("*.json")
    )


@pytest.mark.parametrize(
    "dsn",
    ["", "postgresql://user:password@production.invalid/db", "https://localhost/db"],
)
def test_database_inspection_rejects_nonlocal_urls(monkeypatch, dsn):
    monkeypatch.setenv("M14B_DATABASE_URL", dsn)
    with patch("psycopg.connect") as connect, pytest.raises(
        ValueError, match="loopback"
    ):
        fixture.database()
    connect.assert_not_called()


def test_database_inspection_forces_read_only_connections(monkeypatch):
    monkeypatch.setenv(
        "M14B_DATABASE_URL", "postgresql+psycopg://fixture:password@127.0.0.1/dev"
    )
    with patch("psycopg.connect") as connect:
        fixture.database()
    assert connect.call_args.args == ("postgresql://fixture:password@127.0.0.1/dev",)
    assert connect.call_args.kwargs["options"] == "-c default_transaction_read_only=on"
    connection = connect.return_value
    connection.execute.return_value.fetchone.return_value = {
        "database_name": "other_database",
        "server_port": 5432,
        "schema_name": "public",
        "search_path": '"$user", public',
        "has_signers": False,
    }
    with patch("psycopg.connect", return_value=connection):
        with pytest.raises(SystemExit, match="port 5433, database postgres"):
            fixture.database()
    connection.close.assert_called_once()


def test_finalize_sends_saved_signing_intent_and_requires_acceptance(prepared):
    args = Namespace(
        root=prepared,
        case="matrix",
        key="primary",
        variant="primary",
        expect=200,
        code=None,
    )
    fixture.sign(args)
    fixture.write(prepared / "matrix/init.json", {"release_id": "fixture-reservation"})
    body = fixture.read(prepared / "matrix/finalize-primary.json")
    receipt = {
        "author_signatures_accepted": 1,
        "version": "1.2.0",
        "release_digest": "fixture",
    }
    with patch.object(fixture.transport, "registry", return_value=receipt) as registry:
        fixture.finalize(args)
        fixture.finalize(args)
    assert registry.call_args.args[1] == body
    assert fixture.read(prepared / "matrix/receipt.json") == receipt
    with patch.object(
        fixture.transport,
        "registry",
        return_value={**receipt, "author_signatures_accepted": 0},
    ):
        with pytest.raises(AssertionError, match="one accepted"):
            fixture.finalize(args)


def test_read_only_inspection_verifies_actual_postgres_signing_rows(
    prepared, release_db, monkeypatch
):
    import psycopg
    from app.tools.domain.tools.release_signing import registry_signing_key
    from psycopg.rows import dict_row

    with release_db.connect() as connection:
        schema = connection.scalar(text("SELECT current_schema()"))

    def connect():
        return psycopg.connect(
            os.environ["AGENTPM_M13A_TEST_DATABASE_URL"].replace(
                "postgresql+psycopg://", "postgresql://", 1
            ),
            row_factory=dict_row,
            options=f"-c search_path={schema} -c default_transaction_read_only=on",
        )

    monkeypatch.setattr(fixture, "database", connect)
    storage = Storage()
    with Session(release_db) as session:
        key, signer = register(session)
        (prepared / "primary.seed").write_bytes(bytes(key))
        (prepared / "primary-public-key.txt").write_text(
            base64.b64encode(bytes(key.verify_key)).decode()
        )
        data, reservation = reserve(session, storage, fixture.transport.TARGETS)
        fixture.write(prepared / "matrix/request.json", data)
        fixture.write(prepared / "matrix/init.json", reservation)
        args = Namespace(
            root=prepared, case="matrix", key="primary", pending=True, revoked=False
        )
        fixture.inspect(args)
        _, registry_key = registry_signing_key()
        (prepared / "registry-public-key.txt").write_text(
            base64.b64encode(bytes(registry_key.verify_key)).decode()
        )
        service.finalize_release(
            session,
            reservation["release_id"],
            "fixture",
            storage,
            "fixture",
            fixture.signed_body(prepared, data, "primary"),
        )
        session.commit()
        args.pending = False
        fixture.inspect(args)
        signer.is_active = False
        signer.revoked_ts = datetime.now(timezone.utc)
        session.commit()
        args.revoked = True
        fixture.inspect(args)
    evidence = fixture.read(prepared / "matrix/evidence.json")
    assert len(evidence["authors"]) == len(evidence["registry_attestations"]) == 1
    assert len(evidence["artifacts"]) == 4
    historical = fixture.read(prepared / "matrix/evidence-revoked.json")
    assert not historical["authors"][0]["is_active"]
