"""Validate manual scaffolding offline; never use the operator's Registry or S3."""

import base64
import copy
import hashlib
import importlib.util
import json
import os
import subprocess
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

import pytest

SPEC = importlib.util.spec_from_file_location(
    "manual_m15", Path(__file__).with_name("fixture.py")
)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


@pytest.fixture(name="prepared")
def prepared_fixture(tmp_path):
    fixture.prepare(tmp_path, "zack")
    return tmp_path


def test_prepare_explicit_sources_private_key_and_unique_identity(prepared):
    from nacl.signing import SigningKey

    session = fixture.read(prepared / "session.json")
    assert session["target"] in fixture.transport.TARGETS
    assert session["name"].startswith("m15-order-")
    assert prepared.stat().st_mode & 0o777 == 0o700
    assert (prepared / "primary.seed").stat().st_mode & 0o777 == 0o600
    key = SigningKey((prepared / "primary.seed").read_bytes())
    assert base64.b64decode((prepared / "primary-public-key.txt").read_text()) == bytes(
        key.verify_key
    )
    for case in fixture.CASES:
        manifest = fixture.read(prepared / case / "agent.json")
        assert "target" not in manifest
        assert manifest["files"][:2] == ["tool.py", "README.md"]
        assert not (prepared / case / "request.json").exists()
    assert "dependencies" not in fixture.read(prepared / "legacy/agent.json")["runtime"]
    assert len(fixture.targets_for(prepared, "matrix")) == 5
    assert fixture.targets_for(prepared, "portable") == ["any"]
    assert session["target"] not in fixture.targets_for(prepared, "incompatible")
    with pytest.raises(FileExistsError):
        fixture.prepare(prepared, "zack")


@pytest.mark.parametrize(
    "base", ["", "http://localhost:5000", "https://api.agentpackagemanager.com"]
)
def test_operations_require_exact_dev_origin(monkeypatch, base):
    monkeypatch.setenv("M15_BASE", base)
    with pytest.raises(ValueError, match="exactly"):
        fixture.configure()


@pytest.mark.parametrize("case", ["portable", "matrix", "incompatible"])
def test_real_cli_dry_run_then_packaging_produces_valid_signed_inventory(
    prepared, case
):
    """Synthetic locked hashes test packaging, not actual resolver/wheel execution."""
    from app.tools.domain.tools.canonical import canonical_bytes
    from app.tools.domain.tools.release_signing import AuthorSignatureV2
    from app.tools.domain.tools.releases import ReleaseManifest
    from nacl.signing import VerifyKey

    executable = os.environ.get("M15_TEST_APM")
    if not executable:
        pytest.skip("set M15_TEST_APM to test real CLI packaging without networking")
    directory = prepared / case
    manifest = fixture.read(directory / "agent.json")
    pins = fixture.read(fixture.REPO / "platforms/uv-pins.json")
    inputs = {
        "requirements": ["pydantic==2.11.7"],
        "python_requires": ">=3.11",
        "targets": sorted(fixture.transport.TARGETS),
        "uv_version": pins["version"],
    }
    resolution = {
        "type": "agentpm.python-resolution.v1",
        "python": {"requires": ">=3.11"},
        "requirements": inputs["requirements"],
        "packages": [
            {"name": "pydantic", "version": "2.11.7", "hashes": ["sha256:" + "a" * 64]}
        ],
    }
    fingerprint = hashlib.sha256(canonical_bytes(inputs)).hexdigest()
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
                    "python_input_fingerprint": fingerprint,
                    "python_resolution": resolution,
                }
            },
        },
    )
    home = prepared / "home"
    home.mkdir(exist_ok=True)
    environment = dict(
        os.environ,
        HOME=str(home),
        USERPROFILE=str(home),
        APPDATA=str(home),
        LOCALAPPDATA=str(home),
        AGENTPM_TELEMETRY="0",
        AGENTPM_UV_PATH=str(prepared / "must-not-execute"),
    )
    command = [
        executable,
        "--base-url",
        "http://127.0.0.1:9",
        "publish",
        "--dry-run",
        "--token",
        "apm_m15.fixture",
        "--schema",
        str(fixture.REPO / "schemas/agentpm.manifest.schema.json"),
    ]
    result = subprocess.run(
        command,
        cwd=directory,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    fixture.package(prepared, case)
    request = fixture.read(directory / "request.json")
    release = ReleaseManifest.model_validate(request["release"])
    assert [a.target for a in release.artifacts] == sorted(
        fixture.targets_for(prepared, case)
    )
    signature = fixture.read(directory / "finalize-primary.json")["author_signatures"][
        0
    ]
    statement = AuthorSignatureV2.model_validate(signature["statement_json"])
    VerifyKey(base64.b64decode(signature["public_key_b64"])).verify(
        statement.canonical_bytes(), base64.b64decode(signature["signature_b64"])
    )
    assert statement.matches(release)
    if case != "incompatible":
        audit_selection(prepared, case, request, release)
    original = (directory / "finalize-primary.json").read_bytes()
    fixture.write(directory / "init.json", {"release_id": "reserved"})
    with pytest.raises(ValueError, match="unreserved"):
        fixture.package(prepared, case)
    assert (directory / "finalize-primary.json").read_bytes() == original


def audit_selection(root, case, request, release):
    """Check the runbook's audits using synthetic installed state, not fake trust."""
    target = fixture.read(root / "session.json")["target"]
    selected = next(
        (a for a in request["release"]["artifacts"] if a["target"] == target), None
    )
    selected = selected or next(
        a for a in request["release"]["artifacts"] if a["target"] == "any"
    )
    response = {
        "session_id": "probe",
        "expires_at": "deadline",
        "artifacts": [
            {
                "integrity": release.digest_hex(),
                "verification": {"release": release.model_dump(exclude_none=True)},
                "selected_artifact": selected,
                "size": selected["size"],
                "presigned_url": "https://storage.invalid/one?X-Amz-Expires=600",
            }
        ],
    }
    args = Namespace(root=root, case=case, target=None)
    with patch.object(fixture.transport, "registry", return_value=response):
        fixture.probe(args)
    saved = root / case / f"install-{target}.json"
    assert fixture.read(saved) == response
    invalid = copy.deepcopy(response)
    invalid["artifacts"][0]["selected_artifact"]["digest"] = "sha256:" + "f" * 64
    with patch.object(
        fixture.transport, "registry", return_value=invalid
    ), pytest.raises(AssertionError):
        fixture.probe(args)
    assert fixture.read(saved) == response

    consumer = root / "audit-consumer"
    args.consumer = consumer
    fixture.consumer(args)
    parent = consumer / ".agentpm/tools" / release.name[1:]
    payload = parent / release.version
    payload.mkdir(parents=True)
    fixture.write(
        payload / "agentpm-artifact.json",
        {"target": selected["target"], "format": "tar.gz"},
    )
    state = {
        "target": target,
        "artifact": selected,
        "release_integrity": release.digest_hex(),
    }
    fixture.write(parent / f".{release.version}.selection.json", state)
    fixture.write(
        consumer / "agent.lock",
        {
            "packages": {
                f"tool:{release.name}@{release.version}": {
                    "integrity": release.digest_hex(),
                    "python_resolution": request["python_resolution"],
                }
            }
        },
    )
    identity = [
        "tool",
        release.name,
        release.version,
        selected["target"],
        selected["digest"][7:],
    ]
    cache_name = (
        hashlib.sha256(json.dumps(identity, separators=(",", ":")).encode()).hexdigest()
        + ".tgz"
    )
    cache = consumer / ".agentpm/cache" / cache_name
    cache.parent.mkdir(parents=True)
    cache.write_bytes((root / case / f"{selected['target']}.tar.gz").read_bytes())
    environment = consumer / ".agentpm/runtimes/python/test-environment"
    environment.mkdir(parents=True)
    fixture.write(
        environment / "ready.json",
        {
            "fingerprint": {
                "target": target,
                "resolution_digest": release.pythonResolutionDigest[7:],
            }
        },
    )
    runtime = {
        "prefix": str(environment / "venv"),
        "native": str(environment / "venv/lib/native.so"),
        "pydantic": "2.11.7",
    }
    with patch.object(
        fixture.subprocess, "check_output", return_value=json.dumps(runtime)
    ):
        fixture.check(args)
    assert fixture.read(consumer / "m15-evidence.json")["selected"] == selected
    state["release_integrity"] = "f" * 64
    fixture.write(parent / f".{release.version}.selection.json", state)
    with pytest.raises(AssertionError):
        fixture.check(args)


def test_renewal_only_accepts_same_deadline_and_metadata(prepared):
    target = fixture.read(prepared / "session.json")["target"]
    response = {
        "session_id": "same",
        "expires_at": "same-deadline",
        "artifacts": [{"presigned_url": "old", "integrity": "a" * 64}],
    }
    fixture.write(prepared / "matrix" / f"install-{target}.json", response)
    args = Namespace(root=prepared, case="matrix", target=None)
    refreshed = copy.deepcopy(response)
    refreshed["artifacts"][0]["presigned_url"] = "new"
    with patch.object(fixture.transport, "registry", return_value=refreshed):
        fixture.renew(args)
    for changed in (
        {"session_id": "other"},
        {"expires_at": "later"},
        {"artifacts": []},
    ):
        with patch.object(
            fixture.transport, "registry", return_value={**response, **changed}
        ):
            with pytest.raises(AssertionError):
                fixture.renew(args)


def test_upload_renew_does_not_accidentally_upload_later_targets(prepared, monkeypatch):
    monkeypatch.setenv("M15_BASE", fixture.BASE)
    monkeypatch.setattr(
        "sys.argv", ["fixture.py", "upload-renew", str(prepared), "matrix"]
    )
    with patch.object(fixture.transport, "api_step") as renew, patch.object(
        fixture.transport, "upload"
    ) as upload:
        fixture.main()
    assert renew.call_count == 5
    upload.assert_not_called()


def test_consumer_setup_is_exact_and_refuses_overwrite(prepared):
    consumer = prepared / "consumer-test"
    args = Namespace(root=prepared, case="matrix", consumer=consumer)
    fixture.consumer(args)
    value = fixture.read(consumer / "agent.json")
    assert value["tools"][0]["version"] == "0.1.1"
    assert not (consumer / "agent.lock").exists()
    with pytest.raises(FileExistsError):
        fixture.consumer(args)
