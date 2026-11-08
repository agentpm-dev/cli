"""Offline production-scaffold checks; every HTTP operation is mocked."""

import base64
import copy
import hashlib
import importlib.util
import json
import os
import subprocess
import uuid
from datetime import datetime, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

import pytest
from nacl.signing import SigningKey

SPEC = importlib.util.spec_from_file_location(
    "production_smoke", Path(__file__).with_name("fixture.py")
)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


@pytest.fixture(name="prepared")
def prepared_fixture(tmp_path):
    fixture.prepare(tmp_path, "zack")
    return tmp_path


def build(root, case):
    """Use synthetic locked hashes to test real CLI packaging without any index."""
    from app.tools.domain.tools.canonical import canonical_bytes

    executable = os.environ.get("BAND11_TEST_APM")
    if not executable:
        pytest.skip("set BAND11_TEST_APM for real CLI archive preparation")
    directory = root / case
    manifest = fixture.read(directory / "agent.json")
    inputs = {
        "requirements": ["pydantic==2.11.7"],
        "python_requires": ">=3.11",
        "targets": sorted(fixture.local.transport.TARGETS),
        "uv_version": fixture.read(fixture.local.REPO / "platforms/uv-pins.json")[
            "version"
        ],
    }
    resolution = {
        "type": "agentpm.python-resolution.v1",
        "python": {"requires": ">=3.11"},
        "requirements": inputs["requirements"],
        "packages": [
            {"name": "pydantic", "version": "2.11.7", "hashes": ["sha256:" + "a" * 64]}
        ],
    }
    fixture.write(
        directory / "agent.lock",
        {
            "lockfile_version": 4,
            "generated": "2026-11-07T00:00:00Z",
            "packages": {},
            "roots": {
                "local:tool": {
                    "name": manifest["name"],
                    "version": manifest["version"],
                    "tools": [],
                    "python_input_fingerprint": hashlib.sha256(
                        canonical_bytes(inputs)
                    ).hexdigest(),
                    "python_resolution": resolution,
                }
            },
        },
    )
    home = root / "home"
    home.mkdir(exist_ok=True)
    env = dict(
        os.environ,
        HOME=str(home),
        USERPROFILE=str(home),
        APPDATA=str(home),
        LOCALAPPDATA=str(home),
        AGENTPM_TELEMETRY="0",
        AGENTPM_UV_PATH=str(root / "must-not-execute"),
    )
    result = subprocess.run(
        [
            executable,
            "--base-url",
            "http://127.0.0.1:9",
            "publish",
            "--dry-run",
            "--token",
            "apm_fixture",
            "--schema",
            str(fixture.local.REPO / "schemas/agentpm.manifest.schema.json"),
        ],
        cwd=directory,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    fixture.package(root, case)
    return fixture.checked_request(root, case)


def reservation(root, case):
    build(root, case)
    identity = str(uuid.uuid4())
    artifacts = []
    for target in fixture.targets(root, case):
        artifact_id = str(uuid.uuid4())
        artifacts.append(
            {
                "target": target,
                "artifact_id": artifact_id,
                "put_url": f"https://agentpm-tools.s3.amazonaws.com/uploads/{identity}/{artifact_id}.tar.gz?signature=test",
                "headers": {"Content-Type": "application/gzip"},
            }
        )
    value = {"release_id": identity, "artifacts": artifacts}
    fixture.write(root / case / "init.json", value)
    return value


def test_prepare_only_unique_smoke_identity_and_private_seed(prepared):
    session = fixture.session_for(prepared)
    assert session["registry"] == fixture.BASE
    assert session["bucket"] == "agentpm-tools"
    assert session["name"].startswith("release-check-")
    assert (prepared / "primary.seed").stat().st_mode & 0o777 == 0o600
    assert fixture.targets(prepared, "portable") == ["any"]
    assert set(fixture.targets(prepared, "matrix")) == {"any", session["target"]}
    for case in fixture.CASES:
        manifest = fixture.read(prepared / case / "agent.json")
        assert manifest["name"] == session["name"]
        assert "production" not in (prepared / case / "README.md").read_text().lower()
    with pytest.raises(FileExistsError):
        fixture.prepare(prepared, "zack")
    session["bucket"] = "agentpm-dev-tools"
    fixture.write(prepared / "session.json", session)
    with pytest.raises(ValueError, match="production Registry and bucket"):
        fixture.session_for(prepared)


@pytest.mark.parametrize("case", fixture.CASES)
def test_packaging_small_signed_inventory_refuses_rebuild_after_reservation(
    prepared, case
):
    request = build(prepared, case)
    assert len(request["release"]["artifacts"]) == (1 if case == "portable" else 2)
    assert all(a["size"] < fixture.MAX_BYTES for a in request["release"]["artifacts"])
    saved = (prepared / case / "finalize-primary.json").read_bytes()
    fixture.write(prepared / case / "init.json", {"release_id": str(uuid.uuid4())})
    with pytest.raises(ValueError, match="do not rebuild"):
        fixture.package(prepared, case)
    assert (prepared / case / "finalize-primary.json").read_bytes() == saved


@pytest.mark.parametrize("change", ["identity", "bytes", "signature"])
def test_guard_rejects_changed_release_or_archive_before_network(prepared, change):
    request = build(prepared, "portable")
    if change == "identity":
        session = fixture.read(prepared / "session.json")
        session["name"] = "summarize-text"
        fixture.write(prepared / "session.json", session)
    elif change == "bytes":
        (prepared / "portable/any.tar.gz").write_bytes(b"changed")
    else:
        body = fixture.read(prepared / "portable/finalize-primary.json")
        body["require_signature"] = False
        fixture.write(prepared / "portable/finalize-primary.json", body)
    with patch.object(fixture, "registry") as network, pytest.raises(ValueError):
        fixture.init(prepared, "portable")
    network.assert_not_called()
    assert request["release"]["name"].startswith("@zack/release-check-")


@pytest.mark.parametrize(
    "path",
    [
        "https://evil.invalid/v1/tools/publish/releases/init",
        "/v1/tools/publish/init",
        "/v1/registry/signing-keys?redirect=other",
    ],
)
def test_registry_does_not_accept_arbitrary_production_paths(path, monkeypatch):
    monkeypatch.setenv("BAND11_TOKEN", "apm_test.fixture")
    with patch.object(fixture.local.transport, "open_request") as send, pytest.raises(
        ValueError, match="unexpected"
    ):
        fixture.registry(path, {})
    send.assert_not_called()


def test_upload_sends_only_object_headers_and_renew_does_not_put(prepared):
    data = reservation(prepared, "matrix")
    response = MagicMock()
    response.__enter__.return_value.status = 200
    with patch.object(
        fixture.local.transport, "open_request", return_value=response
    ) as put:
        fixture.upload(prepared, "matrix")
    assert put.call_count == 2
    for call in put.call_args_list:
        request = call.args[0]
        assert request.get_method() == "PUT"
        assert "Authorization" not in request.headers
        assert request.full_url.startswith(
            "https://agentpm-tools.s3.amazonaws.com/uploads/"
        )

    def renewal(path, _body):
        selected = next(a for a in data["artifacts"] if a["artifact_id"] in path)
        return {**selected, "put_url": selected["put_url"] + "-renewed"}

    with patch.object(fixture, "registry", side_effect=renewal) as renew, patch.object(
        fixture.local.transport, "open_request"
    ) as put:
        fixture.upload(prepared, "matrix", renew=True)
    assert renew.call_count == 2
    put.assert_not_called()


@pytest.mark.parametrize(
    "url",
    [
        "https://agentpm-dev-tools.s3.amazonaws.com/uploads/{release}/{artifact}.tar.gz",
        "http://agentpm-tools.s3.amazonaws.com/uploads/{release}/{artifact}.tar.gz",
        "https://agentpm-tools.s3.amazonaws.com/tools/other.tar.gz",
    ],
)
def test_upload_refuses_wrong_bucket_scheme_and_object(prepared, url):
    data = reservation(prepared, "portable")
    item = data["artifacts"][0]
    item["put_url"] = url.format(
        release=data["release_id"], artifact=item["artifact_id"]
    )
    fixture.write(prepared / "portable/init.json", data)
    with patch.object(fixture.local.transport, "open_request") as put, pytest.raises(
        ValueError, match="refusing PUT"
    ):
        fixture.upload(prepared, "portable")
    put.assert_not_called()


def test_security_audit_verifies_signatures_inventory_and_complete_scans(prepared):
    from app.tools.domain.tools.release_signing import RegistryAttestationV3
    from app.tools.domain.tools.releases import ReleaseManifest

    request = build(prepared, "matrix")
    release = ReleaseManifest.model_validate(request["release"])
    registry_key = SigningKey(bytes([9]) * 32)
    statement = RegistryAttestationV3(
        type="agentpm.registry.attestation.v3",
        kind="tool",
        name=release.name,
        version=release.version,
        releaseDigest=release.digest(),
        storedAt=datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        publishedByAccountId="fixture",
        registryKeyId="fixture-registry",
    )
    author = fixture.read(prepared / "matrix/finalize-primary.json")[
        "author_signatures"
    ][0]
    value = {
        "release_digest": release.digest(),
        "artifacts": [
            {
                "target": a.target,
                "sha256": a.digest[7:],
                "size_bytes": a.size,
                "scan_status": "complete",
                "scan_malware_status": "clean",
            }
            for a in release.artifacts
        ],
        "author_signatures": [{**author, "signer_id": str(uuid.uuid4())}],
        "registry_attestation": {
            "algo": "ed25519",
            "statement_json": statement.model_dump(),
            "signature_b64": base64.b64encode(
                registry_key.sign(statement.canonical_bytes()).signature
            ).decode(),
        },
    }
    fixture.write(prepared / "matrix/receipt.json", {"tool_id": str(uuid.uuid4())})
    document = SimpleNamespace(
        statement=SimpleNamespace(
            keys=[
                SimpleNamespace(
                    keyId="fixture-registry",
                    publicKeyB64=base64.b64encode(
                        bytes(registry_key.verify_key)
                    ).decode(),
                )
            ]
        )
    )
    with patch.object(fixture, "keyset", return_value=document), patch.object(
        fixture, "registry", return_value=value
    ):
        fixture.security(prepared, "matrix", clean=True)
    for field in ("digest", "inventory", "scan"):
        changed = copy.deepcopy(value)
        if field == "digest":
            changed["release_digest"] = "sha256:" + "f" * 64
        elif field == "inventory":
            changed["artifacts"].pop()
        else:
            changed["artifacts"][0]["scan_malware_status"] = "unknown"
        with patch.object(fixture, "keyset", return_value=document), patch.object(
            fixture, "registry", return_value=changed
        ), pytest.raises(ValueError):
            fixture.security(prepared, "matrix", clean=True)
