"""Guards for the manual helper; live operator checks are a separate activity."""

import importlib.util
import json
from pathlib import Path
from urllib.request import Request
import uuid

import pytest

SPEC = importlib.util.spec_from_file_location(
    "m8a_manual", Path(__file__).with_name("fixture.py")
)
helper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(helper)


@pytest.fixture
def prepared(tmp_path, monkeypatch):
    repo = tmp_path / "agentpm"
    repo.mkdir()
    monkeypatch.setattr(helper, "REPO", repo)
    implementation = tmp_path / "implementation.py"
    implementation.write_text("unchanged")
    monkeypatch.setattr(
        helper,
        "implementation_pins",
        lambda: {str(implementation): helper.digest(implementation)},
    )
    m4a, m5a = tmp_path / "m4a", tmp_path / "m5a"
    canonical = (
        helper.SOURCE.parents[5] / "standards/agentpm/1.0.0/conformance/fixtures/valid"
    )
    for number, kind in enumerate(helper.ROUTES, 1):
        folder = m4a / "cases" / kind
        manifest = helper.read(canonical / (kind + ".json"))
        helper.write(folder / "source/agent.json", manifest)
        helper.write(
            folder / "receipt.json",
            dict(package_id=str(uuid.UUID(int=number)), version=manifest["version"]),
        )
    for label in ("echo-v1", "echo-v2", "legacy"):
        manifest = helper.read(canonical / "tool.json")
        manifest["version"] = "2.0.0" if label == "echo-v2" else "1.0.0"
        if label == "legacy":
            del manifest["standard"]
        helper.write(m5a / "cases" / label / "source/agent.json", manifest)
        output = m5a / "evidence" / f"publish-{label}.stdout.txt"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(str(uuid.UUID(int=11 if label == "legacy" else 10)))
    root = repo / "web-m8a-test" / "owned-run"
    helper.prepare(root, m4a, m5a, "http://localhost:3000", "http://127.0.0.1:5000")
    return root, implementation, m4a, m5a


@pytest.mark.parametrize(
    "origin",
    [
        "https://api.agentpackagemanager.com",
        "http://evil.invalid",
        "http://localhost@evil.invalid",
        "http://localhost:3000/?next=remote",
        "http://localhost:3000/path",
        "http://localhost:3000#fragment",
    ],
)
def test_network_origins_cannot_escape_loopback(origin):
    with pytest.raises(ValueError, match="loopback"):
        helper.local_origin(origin)


def test_redirects_are_not_followed():
    with pytest.raises(ValueError, match="no redirect was followed"):
        helper.NoRedirect().redirect_request(
            Request("http://localhost:3000/a"),
            None,
            302,
            "",
            {},
            "https://example.com/",
        )


def test_prepare_reuses_all_kinds_and_version_pair_without_touching_old_runs(prepared):
    root, _, m4a, m5a = prepared
    _, catalog = helper.load(root)
    assert len(catalog) == 11
    assert {entry["kind"] for entry in catalog.values()} == set(helper.ROUTES)
    assert catalog["legacy"]["origin"] == "legacy_inferred"
    assert catalog["echo-v1"]["id"] == catalog["echo-v2"]["id"]
    assert catalog["echo-v1"]["version"] != catalog["echo-v2"]["version"]
    assert "http://localhost:3000" in (root / "index.html").read_text()
    assert "M8A_ROOT=" in (root / "paths.sh").read_text()
    old = {
        str(p): helper.digest(p)
        for folder in (m4a, m5a)
        for p in folder.rglob("*")
        if p.is_file()
    }
    with pytest.raises(AssertionError, match="overwrite"):
        helper.prepare(root, m4a, m5a, "http://localhost:3000", "http://127.0.0.1:5000")
    assert old == {
        str(p): helper.digest(p)
        for folder in (m4a, m5a)
        for p in folder.rglob("*")
        if p.is_file()
    }


@pytest.mark.parametrize("target", ["catalog", "implementation", "input"])
def test_changed_inputs_are_rejected(prepared, target):
    root, implementation, _, _ = prepared
    path = (
        root / "catalog.json"
        if target == "catalog"
        else implementation
        if target == "implementation"
        else root / "fixtures/tool/source-manifest.json"
    )
    path.write_text(path.read_text() + "\n")
    with pytest.raises(AssertionError, match="changed"):
        helper.load(root)


def test_html_assertions_do_not_accept_labels_hidden_in_framework_scripts():
    page = helper.Page(
        b'<h1 id="real">Actual content</h1><a href="#real">Visible</a><script>APDS declaration: agentpm / 1.0.0</script><template><a href="/forged">View agent.json</a></template>'
    )
    assert page.contains("Actual content")
    assert not page.contains("APDS declaration:")
    assert page.links == [["#real", "Visible"]]


def test_unavailable_api_projection_cannot_be_mistaken_for_recorded_conformance():
    expected = dict(
        origin="declared",
        api_status="unknown",
        result="unknown",
        levels={},
        evaluated_at=None,
    )
    helper.evidence_page(
        helper.Page(b"APDS conformance evidence unavailable or not evaluated."),
        expected,
    )
    with pytest.raises(AssertionError):
        helper.evidence_page(
            helper.Page(b"Registry-recorded APDS result: conformant."), expected
        )


def test_visible_evidence_matches_across_react_interpolation_boundaries():
    page = helper.Page(
        b"<p>Registry-recorded APDS result: <!-- -->conformant<!-- -->.</p><li>resolved<!-- -->: <!-- -->conformant<!-- -->; <!-- -->0<!-- --> unevaluated rules.</li><p>Evaluation time unavailable.</p>"
    )
    helper.evidence_page(
        page,
        dict(
            origin="declared",
            api_status="passed",
            result="conformant",
            levels={"resolved": {"status": "conformant", "unevaluated": []}},
            evaluated_at=None,
        ),
    )


def test_failed_recheck_cannot_leave_an_old_pass_usable(prepared, monkeypatch):
    root, _, _, _ = prepared
    helper.write(root / "checks.json", dict(status="pass"))
    helper.write(root / "evidence/summary.json", dict(status="pass"))
    monkeypatch.setattr(
        helper, "fetch", lambda *_args: (200, {}, b"<h1>Wrong reference</h1>")
    )
    with pytest.raises(AssertionError, match="Reference/canonical"):
        helper.check(root)
    assert helper.read(root / "checks.json")["status"] != "pass"
    assert helper.read(root / "evidence/summary.json")["status"] != "pass"
    with pytest.raises(AssertionError, match="Run check"):
        helper.audit(root)


def test_private_check_requires_operator_positive_and_actual_anonymous_denial(
    prepared, monkeypatch
):
    root, _, _, _ = prepared
    url = f"http://localhost:3000/packages/{uuid.UUID(int=99)}/1.0.0/agent.json"
    calls = []

    def denied(origin, path):
        calls.append((origin, path))
        return (
            404,
            {
                "cache-control": "private, no-store",
                "x-robots-tag": "noindex, nofollow",
                "x-content-type-options": "nosniff",
                "content-type": "application/json",
            },
            b'{"error":"Package manifest not found"}',
        )

    monkeypatch.setattr(helper, "fetch", denied)
    with pytest.raises(AssertionError, match="observe a real signed-in"):
        helper.private(root, url, 404)
    assert not calls
    helper.private(root, url, 200)
    assert len(calls) == 2
    proof = helper.read(root / "private.json")
    assert (
        proof["status"] == "pass"
        and "operator-reported" in proof["authorized_observation"]
    )
    monkeypatch.setattr(helper, "fetch", lambda *_args: (200, {}, b"{}"))
    with pytest.raises(AssertionError, match="not hidden"):
        helper.private(root, url, 200)
    assert helper.read(root / "private.json")["status"] != "pass"


def test_audit_never_invents_observations_or_operator_signoff(prepared):
    root, _, _, _ = prepared
    helper.write(
        root / "checks.json",
        dict(status="pass", evidence="evidence/check", fixtures=11, requests=60),
    )
    helper.write(root / "evidence/check/result.json", dict(status="pass"))
    helper.write(root / "private.json", dict(status="pass"))
    with pytest.raises(AssertionError, match="observations still missing"):
        helper.audit(root)
    for label in helper.OBSERVATIONS:
        helper.observe(
            root, label, "pass", "Synthetic helper test, not an operator observation"
        )
    helper.audit(root)
    result = helper.read(root / "evidence/summary.json")
    assert result["operator_signoff"] is False
    assert result["browser_observations"] == len(helper.OBSERVATIONS)
    helper.observe(root, "responsive", "fail", "Intentional later failed observation")
    assert helper.read(root / "evidence/summary.json")["status"] != "pass"
    with pytest.raises(AssertionError, match="responsive"):
        helper.audit(root)
    assert helper.read(root / "evidence/summary.json")["status"] != "pass"
