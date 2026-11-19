"""Builder checks only: isolated fixtures/PG schemas, mocked transport; not operator acceptance."""

import base64
import importlib.util
import io
import os
import subprocess
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

import psycopg
import pytest
import rfc8785
from app.tools.application import publish_apds, release_publish
from app.tools.domain.tools.entities import NamespaceSigner
from app.tools.domain.tools.legacy_signing import legacy_statement_bytes
from app.tools.domain.tools.release_signing import AuthorSignatureV2
from app.tools.domain.tools.releases import ReleaseManifest
from flask import Flask
from nacl.signing import VerifyKey
from psycopg.rows import dict_row
from sqlalchemy.orm import Session
from test_apds_publish_integration import (
    LegacyStorage,
    apds_database,
    finalize_legacy,
    reserve_legacy,
)
from test_release_migration import disposable_database
from test_release_publish import NAMESPACE, release_database

SPEC = importlib.util.spec_from_file_location(
    "manual_m4a", Path(__file__).with_name("fixture.py")
)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
CLI = Path(os.getenv("M4A_TEST_CLI", str(fixture.REPO / "target/debug/agentpm")))
database_fixture = disposable_database
release_db_fixture = release_database
apds_fixture = apds_database


@pytest.fixture(name="prepared")
def prepare_fixture(tmp_path):
    root = tmp_path / "run"
    fixture.prepare(root, "fixture")
    for label, kind in [
        ("knowledge", "knowledge"),
        ("knowledge-missing-file", "knowledge"),
        ("memory", "memory"),
        ("cli-knowledge", "knowledge"),
        ("cli-memory", "memory"),
    ]:
        result = subprocess.run(
            [
                str(CLI),
                "--base-url",
                "http://127.0.0.1:9",
                kind,
                "build",
                "--manifest",
                str(root / "cases" / label / "source/agent.json"),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 0, result.stderr
    for folder in (root / "cases").iterdir():
        if fixture.read(folder / "case.json")["mode"] != "cli":
            fixture.pack(root, folder.name)
    return root


def test_visible_inventory_local_paths_and_stable_signature_bytes(
    prepared, monkeypatch, tmp_path
):
    assert len(list((prepared / "cases").iterdir())) == 36
    assert (prepared / "author.seed").stat().st_mode & 0o777 == 0o600
    with pytest.raises(ValueError, match="Existing session"):
        fixture.prepare(prepared, "fixture")
    with pytest.raises(ValueError, match="Workspace must"):
        fixture.local_root(fixture.REPO / "target")
    for label in ("agent", "matrix"):
        folder = prepared / "cases" / label
        envelope = fixture.read(folder / "finalize-good.json")["author_signatures"][0]
        statement = envelope["statement_json"]
        if label == "matrix":
            release = ReleaseManifest.model_validate(
                fixture.read(folder / "request.json")["release"]
            )
            assert release.digest() == statement["releaseDigest"]
            AuthorSignatureV2.model_validate(statement)
            message = rfc8785.dumps(statement)
        else:
            message = legacy_statement_bytes(statement)
        VerifyKey(base64.b64decode(envelope["public_key_b64"])).verify(
            message, base64.b64decode(envelope["signature_b64"])
        )
    monkeypatch.setenv(
        "M4A_DATABASE_URL", "postgresql://example:example@remote.invalid/database"
    )
    with pytest.raises(ValueError, match="local DEVELOPMENT"):
        fixture.database()


def test_ordinary_cli_sources_pass_real_publish_preflight_without_network(prepared):
    for kind in fixture.KINDS:
        result = subprocess.run(
            [
                str(CLI),
                "--base-url",
                "http://127.0.0.1:9",
                "publish",
                "--dry-run",
                "--namespace",
                "fixture",
            ],
            cwd=prepared / "cases" / ("cli-" + kind) / "source",
            env={
                **os.environ,
                "AGENTPM_TOKEN": "apm_manual.fixture",
                "AGENTPM_TELEMETRY": "0",
            },
            capture_output=True,
            text=True,
            check=False,
            timeout=60,
        )
        assert result.returncode == 0, (kind, result.stderr)


@pytest.mark.parametrize("invalid_phase", [None, "off", "on"])
def test_phase_evidence_supports_normal_intellij_server_without_launcher_files(
    prepared, invalid_phase
):
    fixture.write(
        prepared / "evidence/off-declared-inspect.json",
        {
            "versions": [
                {
                    "standard_id": "agentpm",
                    "standard_version": "1.0.0",
                    "standard_origin": "declared",
                    "apds_validation": {"status": "conformant"}
                    if invalid_phase == "off"
                    else None,
                }
            ]
        },
    )
    fixture.write(
        prepared / "evidence/missing-standard-finalize-good/001.json",
        {
            "http_status": 500 if invalid_phase == "on" else 422,
            "response": {
                "code": "apds_nonconformant",
                "details": {
                    "apds": {
                        "issues": [
                            {"rule_id": "APDS-ID-001", "instance_path": "/standard"},
                        ]
                    }
                },
            },
        },
    )
    with patch.object(fixture, "registry") as network:
        if invalid_phase:
            with pytest.raises(ValueError, match=invalid_phase + "-phase"):
                fixture.phase_evidence(prepared)
        else:
            assert fixture.phase_evidence(prepared)["off_case"] == "off-declared"
    network.assert_not_called()
    assert not (prepared / "evidence/server-off.json").exists()
    assert not (prepared / "evidence/server-on.json").exists()


def test_signing_timestamp_is_canonical_even_when_clock_has_fractional_trailing_zeros(
    prepared,
):
    with patch.object(fixture, "datetime") as clock:
        clock.now.return_value = datetime(
            2026, 11, 18, 1, 2, 3, 123000, tzinfo=timezone.utc
        )
        fixture.pack(prepared, "matrix")
    statement = fixture.read(prepared / "cases/matrix/finalize-good.json")[
        "author_signatures"
    ][0]["statement_json"]
    assert statement["createdAt"] == "2026-11-18T01:02:03Z"
    AuthorSignatureV2.model_validate(statement)


@pytest.mark.parametrize(
    "label",
    [
        "agent",
        "tool",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
        "template",
        "loop-invalid-entry",
        "knowledge-missing-file",
        "missing-standard",
        "unsupported-standard",
        "malformed-standard",
        "blank-description",
        "wrong-name",
        "wrong-kind",
        "wrong-version",
        "standard-mismatch",
        "matrix",
        "matrix-missing-file",
        "graph-valid",
        "graph-invalid",
        "graph-legacy",
    ],
)
def test_real_archive_expectations_match_the_actual_publisher(prepared, label):
    folder = prepared / "cases" / label
    meta, request = fixture.read(folder / "case.json"), fixture.read(
        folder / "request.json"
    )
    manifest = request["manifest"]

    def selected(_kind, identity, _selector):
        dependency = "off-loop" if identity.endswith("-off-loop") else "loop"
        value = fixture.read(prepared / "cases" / dependency / "source/agent.json")
        return value, value["version"]

    targets = fixture.TARGETS if meta["mode"] == "release" else ("any",)
    app = Flask(__name__)
    with app.app_context():
        for target in targets:
            negative = meta["expected_http"] == 422 and (
                label != "matrix-missing-file" or target == fixture.TARGETS[1]
            )
            stream = io.BytesIO((folder / (target + ".tar.gz")).read_bytes())
            try:
                result = publish_apds.validate_snapshot(
                    stream,
                    manifest,
                    identity=(manifest["kind"], manifest["name"], manifest["version"]),
                    resolve_manifest=selected,
                )
                assert not negative, "Intended defective archive passed"
                assert (
                    result["status"] == meta["expected_status"]
                    or label == "matrix-missing-file"
                )
            except publish_apds.PublishValidationError as error:
                assert negative and error.code == meta["expected_code"]
                if meta["rule"]:
                    assert any(
                        issue["rule_id"] == meta["rule"]
                        and issue["instance_path"] == meta["path"]
                        for issue in error.evidence["issues"]
                    ), error.evidence
                else:
                    assert error.path == meta["path"]


def test_transport_refuses_production_and_checks_exact_negative_rule(
    prepared, monkeypatch
):
    monkeypatch.setenv("M4A_BASE", "https://api.agentpackagemanager.com")
    with patch.object(fixture, "open_request") as network, pytest.raises(
        ValueError, match="exactly"
    ):
        fixture.registry(prepared, "refuse", "/v1/tools/publish/init", {}, 200)
    network.assert_not_called()
    folder = prepared / "cases/missing-standard"
    fixture.write(folder / "init.json", {"upload_id": "fixture"})
    wrong = {
        "code": "apds_nonconformant",
        "details": {
            "apds": {"issues": [{"rule_id": "APDS-ID-001", "instance_path": "/wrong"}]}
        },
    }
    with patch.object(fixture, "registry", return_value=wrong), pytest.raises(
        ValueError, match="Wrong rule/path"
    ):
        fixture.finalize(prepared, "missing-standard", "good", None, None)


@pytest.mark.parametrize(
    "url",
    [
        "https://other-bucket.s3.amazonaws.com/file",
        "https://agentpm-dev-tools.s3.amazonaws.com.evil.invalid/file",
        "http://agentpm-dev-tools.s3.amazonaws.com/file",
    ],
)
def test_storage_destination_is_pinned_to_development(prepared, url):
    fixture.write(prepared / "cases/agent/init.json", {"put_url": url, "headers": {}})
    with patch.object(fixture, "open_request") as network, pytest.raises(
        ValueError, match="Refusing upload"
    ):
        fixture.upload(prepared, "agent", "any")
    network.assert_not_called()


def test_registry_evidence_has_status_body_but_no_auth_headers(prepared, monkeypatch):
    monkeypatch.setenv("M4A_BASE", "http://127.0.0.1:5000")
    monkeypatch.setenv("M4A_TOKEN", "apm_do_not_save_me")

    @contextmanager
    def response(request):
        assert request.get_header("Authorization") == "Bearer apm_do_not_save_me"
        yield SimpleNamespace(status=200, read=lambda: b'{"upload_id":"fixture"}')

    with patch.object(fixture, "open_request", side_effect=response):
        fixture.registry(
            prepared, "transport", "/v1/tools/publish/init", {"manifest": {}}
        )
    text = (prepared / "evidence/transport/001.json").read_text()
    assert "apm_do_not_save_me" not in text and '"http_status": 200' in text
    assert (
        fixture.NoRedirect().redirect_request(
            None, None, 302, "", {}, "https://remote.invalid"
        )
        is None
    )


@pytest.mark.parametrize(
    "label", ["agent", "matrix", "matrix-missing-file", "graph-unavailable"]
)
def test_read_only_inspection_queries_real_columns_in_disposable_postgres(
    prepared, apds_db, label, monkeypatch
):
    folder = prepared / "cases" / label
    request = fixture.read(folder / "request.json")
    storage = LegacyStorage()
    with Session(apds_db) as session:
        session.add(
            NamespaceSigner(
                namespace_id=NAMESPACE,
                algo="ed25519",
                label="manual helper fixture",
                public_key_b64=(prepared / "author-public-key.txt").read_text().strip(),
                is_active=True,
            )
        )
        session.commit()
        if label in ("agent", "graph-unavailable"):
            upload, _ = reserve_legacy(session, storage, request["manifest"])
            upload.expected_sha = request["sha256"]
            upload.expected_size_bytes = request["size"]
            session.commit()
            storage.objects[upload.tmp_key] = (folder / "any.tar.gz").read_bytes()
            fixture.write(folder / "init.json", {"upload_id": str(upload.upload_id)})
            result, status = finalize_legacy(
                session, storage, upload, fixture.read(folder / "finalize-good.json")
            )
            assert status == (422 if label == "graph-unavailable" else 200), result
            if label == "graph-unavailable":
                assert result["code"] == "invalid_dependency_access"
        else:
            result = release_publish.init_release(
                session, request, "fixture", storage, "fixture"
            )
            session.commit()
            fixture.write(folder / "init.json", result)
            for item in result["artifacts"]:
                # Actual storage key from the persisted reservation.
                from app.tools.domain.tools.entities import PackagePublishArtifact

                child = session.get(PackagePublishArtifact, item["artifact_id"])
                storage.objects[child.tmp_key] = (
                    folder / (item["target"] + ".tar.gz")
                ).read_bytes()
            session.rollback()
            if label == "matrix-missing-file":
                with pytest.raises(publish_apds.PublishValidationError):
                    release_publish.finalize_release(
                        session,
                        result["release_id"],
                        "fixture",
                        storage,
                        "fixture",
                        fixture.read(folder / "finalize-good.json"),
                    )
                session.rollback()
            else:
                release_publish.finalize_release(
                    session,
                    result["release_id"],
                    "fixture",
                    storage,
                    "fixture",
                    fixture.read(folder / "finalize-good.json"),
                )
        session.commit()
    with apds_db.connect() as connection:
        schema = connection.exec_driver_sql("SELECT current_schema()").scalar_one()

    def readonly():
        return psycopg.connect(
            apds_db.url.render_as_string(hide_password=False).replace(
                "postgresql+psycopg://", "postgresql://"
            ),
            options=f"-c search_path={schema} -c default_transaction_read_only=on",
            row_factory=dict_row,
        )

    monkeypatch.setattr(fixture, "database", readonly)
    # Transport is mocked for builder checks only. Operator commands use urllib.
    monkeypatch.setattr(fixture, "registry", Mock(return_value={}))
    fixture.inspect_case(
        prepared,
        label,
        "rejected"
        if label in ("matrix-missing-file", "graph-unavailable")
        else "conformant",
    )
    assert fixture.read(prepared / "evidence" / (label + "-inspect.json"))[
        "expected"
    ] in ("rejected", "conformant")
