"""Local checks for manual preparation; no real DB, root or Registry mutations."""

import base64
import importlib.util
import json
from pathlib import Path
from unittest.mock import patch

import pytest

SPEC = importlib.util.spec_from_file_location(
    "manual_m14c", Path(__file__).with_name("fixture.py")
)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


def public_pin():
    return {
        "version": 1,
        "roots": [
            {
                "registry": fixture.BASE,
                "rootKeyId": "test-root",
                "publicKeyB64": base64.b64encode(bytes([9]) * 32).decode(),
            }
        ],
    }


def test_prepare_explicit_sources_and_real_sdk_runner(tmp_path):
    fixture.prepare(tmp_path)
    manifest = fixture.read(tmp_path / "author/agent.json")
    assert manifest["name"].startswith("m14c-check-")
    assert manifest["files"] == ["tool.py", "README.md"]
    assert (tmp_path / "runner/src/main.rs").read_bytes() == (
        fixture.FIXTURES / "consumer.rs"
    ).read_bytes()
    assert (
        str(fixture.REPO / "crates/agentpm-sdk")
        in (tmp_path / "runner/Cargo.toml").read_text()
    )
    assert not any(tmp_path.rglob("*encrypted*"))
    with pytest.raises(FileExistsError):
        fixture.prepare(tmp_path)


def test_pin_preserves_other_origins_and_refuses_rotation_or_private_material(
    tmp_path, monkeypatch
):
    repo, api = tmp_path / "cli", tmp_path / "api"
    monkeypatch.setattr(fixture, "REPO", repo)
    paths = (repo / fixture.ROOT_FILES[0], api / fixture.ROOT_FILES[1])
    existing = public_pin()
    existing["roots"][0]["registry"] = "https://official.example.invalid"
    for path in paths:
        path.parent.mkdir(parents=True)
        fixture.write(path, existing)
    source = tmp_path / "public.json"
    fixture.write(source, public_pin())
    fixture.pin(source, api)
    fixture.pin(source, api)
    assert paths[0].read_bytes() == paths[1].read_bytes()
    assert fixture.read(paths[0])["roots"] == existing["roots"] + public_pin()["roots"]
    previous = paths[0].read_bytes()
    changed = public_pin()
    changed["roots"][0]["rootKeyId"] = "unapproved-rotation"
    source.write_text(json.dumps(changed))
    with pytest.raises(ValueError, match="never silently rotate"):
        fixture.pin(source, api)
    changed = public_pin()
    changed["ciphertextB64"] = "private root"
    source.write_text(json.dumps(changed))
    with pytest.raises(ValueError, match="expected one public"):
        fixture.pin(source, api)
    changed = public_pin()
    changed["roots"][0]["registry"] = "https://api.agentpackagemanager.com"
    source.write_text(json.dumps(changed))
    with pytest.raises(ValueError, match="exact loopback"):
        fixture.pin(source, api)
    assert paths[0].read_bytes() == previous


@pytest.mark.parametrize(
    "url", ["postgresql://u:p@production.invalid/db", "sqlite:///local.db"]
)
def test_database_refuses_nonlocal_connections(monkeypatch, url):
    monkeypatch.setenv("M14C_DATABASE_URL", url)
    with patch("sqlalchemy.create_engine") as create, pytest.raises(
        ValueError, match="loopback"
    ):
        fixture.engine()
    create.assert_not_called()


def test_database_reads_are_read_only_and_seed_must_opt_into_writes(monkeypatch):
    monkeypatch.setenv("M14C_DATABASE_URL", "postgresql://u:p@127.0.0.1:5433/dev")
    with patch("sqlalchemy.create_engine") as create:
        fixture.engine()
        assert (
            create.call_args.kwargs["connect_args"]["options"]
            == "-c default_transaction_read_only=on"
        )
        fixture.engine(read_only=False)
        assert create.call_args.kwargs["connect_args"] == {}


def test_tamper_only_changes_copies_and_required_policy(tmp_path):
    fixture.prepare(tmp_path)
    source = tmp_path / "evidence/release.json"
    value = {
        "verification": {
            "manifest": {"version": "1.0.0"},
            "authors": [{"is_active": False}],
            "mode": "optional",
            "registry_attestation": {
                "registry_key_id": "actual",
                "signature_b64": "original",
            },
            "release": {"artifacts": [{"target": "x86_64-unknown-linux-gnu"}]},
        }
    }
    fixture.write(source, value)
    fixture.write(tmp_path / "evidence/dev-keyset.json", {"signatureB64": "valid"})
    fixture.tamper(tmp_path, source)
    assert fixture.read(source) == value
    assert (
        fixture.read(tmp_path / "evidence/tampered-target.json")["verification"][
            "release"
        ]["artifacts"][0]["target"]
        != "x86_64-unknown-linux-gnu"
    )
    required = fixture.read(tmp_path / "evidence/tampered-required-unsigned.json")
    assert (
        required["verification"]["authors"] == []
        and required["verification"]["mode"] == "required"
    )
    assert (
        fixture.read(tmp_path / "evidence/dev-keyset.json")["signatureB64"] == "valid"
    )
