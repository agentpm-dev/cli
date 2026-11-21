"""Builder-only local replay; no operator Registry, DB, S3 or production writes."""

import base64
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

from flask import Flask
from nacl.signing import VerifyKey
import pytest

from app.tools.application.publish_apds import validate_snapshot
from app.tools.domain.tools.legacy_signing import legacy_statement_bytes

SPEC = importlib.util.spec_from_file_location("manual_m5b", Path(__file__).with_name("fixture.py"))
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
CLI = fixture.REPO / "target/debug/agentpm"


@pytest.fixture(name="prepared")
def prepared_fixture(monkeypatch):
    monkeypatch.setenv("AGENTPM_TELEMETRY", "0")
    suffix = uuid.uuid4().hex
    old = fixture.REPO / "cli-m5a-test" / ("builder-m5b-reuse-" + suffix)
    root = fixture.REPO / "cli-m5b-test" / ("builder-" + suffix)
    try:
        fixture.previous.prepare(old, "fixture", CLI)
        with pytest.MonkeyPatch.context() as environment:
            environment.setenv("M5B_ROOT", str(root / "wrong-root"))
            environment.setenv("M5B_NAMESPACE", "wrong-namespace")
            fixture.prepare(root, "fixture", CLI, old)
        yield root
    finally:
        shutil.rmtree(root, ignore_errors=True)
        shutil.rmtree(old, ignore_errors=True)


@pytest.fixture(name="server")
def server_fixture(prepared):
    process = subprocess.Popen(
        [sys.executable, "-B", str(Path(__file__).with_name("fixture.py")), "serve", str(prepared)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        for _ in range(250):
            if (prepared / "control-server.json").exists():
                break
            if process.poll() is not None:
                raise AssertionError(process.communicate())
            time.sleep(0.02)
        origin = fixture.read(prepared / "control-server.json")["base_url"]
        yield prepared, origin
    finally:
        process.terminate()
        process.communicate(timeout=10)


def invoke(root, origin, project, label, *args, case="good", expected=0):
    fixture.begin(root, label)
    output = subprocess.run(
        [str(CLI), "--base-url", origin + "/" + case, *args],
        cwd=fixture.project(root, project),
        env={**os.environ, "AGENTPM_TELEMETRY": "0", "AGENTPM_TOKEN": "apm_m5b.fixture"},
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=120,
    )
    for stream in ("stdout", "stderr"):
        (root / "evidence" / (label + "." + stream + ".txt")).write_text(getattr(output, stream))
    fixture.write(
        root / "evidence" / (label + ".result.json"),
        {"exit_code": output.returncode, "expected_exit_code": expected},
    )
    assert output.returncode == expected, output.stdout + output.stderr
    if expected:
        fixture.result(root, label)
    else:
        assert json.loads(output.stdout)["status"] == "success"
    return output


def test_sources_built_linted_signed_and_independently_publishable(prepared, monkeypatch):
    root = prepared
    assert len(list((root / "cases").iterdir())) == 6
    assert len(list((root / "reused").iterdir())) == 4
    assert (root / "author.seed").stat().st_mode & 0o777 == 0o600
    assert "M5B_TOKEN" not in (root / "paths.sh").read_text()
    assert (root / "cases/memory/source/memory/build.json").exists()
    assert "context" in fixture.read(root / "cases/knowledge/source/agent.json")["knowledge"]
    assert subprocess.run(["zsh", "-n", str(root / "paths.sh")]).returncode == 0
    assert subprocess.run(["zsh", "-n", str(root / "functions.sh")]).returncode == 0
    catalog = fixture.read(root / "catalog.json")

    def selected(kind, name, version):
        entry = next(
            e
            for e in catalog.values()
            if (e["kind"], e["name"], e["version"]) == (kind, name, version)
        )
        return fixture.read(root / entry["folder"] / "source/agent.json"), version

    with Flask(__name__).app_context():
        for name in fixture.SEEDS:
            folder = root / "cases" / name
            manifest = fixture.read(folder / "source/agent.json")
            signature = fixture.read(folder / "finalize-good.json")["author_signatures"][0]
            VerifyKey(base64.b64decode(signature["public_key_b64"])).verify(
                legacy_statement_bytes(signature["statement_json"]),
                base64.b64decode(signature["signature_b64"]),
            )
            result = validate_snapshot(
                io.BytesIO((folder / "any.tar.gz").read_bytes()),
                manifest,
                identity=(manifest["kind"], manifest["name"], manifest["version"]),
                resolve_manifest=selected,
            )
            assert result["status"] == "conformant", result
    responses = [fixture.item(catalog[name]) for name in fixture.REUSED]
    monkeypatch.setattr(fixture.transport, "namespace_info", lambda _root: {"handle": "fixture"})
    monkeypatch.setattr(fixture.transport, "registry", lambda *_args: {"items": responses})
    fixture.preflight(root)
    responses[0]["integrity"] = "0" * 64
    with pytest.raises(AssertionError):
        fixture.preflight(root)
    assert "wrong-root" not in (root / "paths.sh").read_text()
    assert "wrong-namespace" not in (root / "paths.sh").read_text()
    with pytest.raises(ValueError, match="run directory"):
        fixture.local_root(fixture.REPO / "target")
    with pytest.raises(ValueError, match="Existing run"):
        fixture.prepare(root, "fixture", CLI, root)
    # No receipt or invented operator/network approval is emitted by preparation.
    assert not list((root / "cases").glob("*/receipt.json"))
    with pytest.raises(FileNotFoundError):
        fixture.audit(root)


def test_positive_installs_clean_replay_regeneration_empty_and_legacy(server):
    root, origin = server
    refs = fixture.read(root / "session.json")["refs"]
    for name, label, spec in (
        ("direct-agent", "agent", refs["agent"]),
        ("direct-skill", "skill", refs["skill"]),
        ("local-agent", "local", None),
        ("local-skill", "local-skill", None),
        ("workspace", "workspace", None),
    ):
        args = ["install", *([spec] if spec else []), "--require-signature", "--machine"]
        invoke(root, origin, name, label + "-install", *args)
        fixture.check_lock(root, name)
        fixture.snapshot(root, name, label + "-replay")
        fixture.clean(root, name)
        invoke(root, origin, name, label + "-replay", *args, "--frozen")
        fixture.snapshot(root, name, label + "-replay", compare=True)
        fixture.check_lock(root, name)
        fixture.traffic(root, label + "-replay", gets=len(fixture.PROJECTS[name]))
    fixture.snapshot(root, "direct-agent", "agent-replay-again")
    fixture.clean(root, "direct-agent")
    invoke(
        root,
        origin,
        "direct-agent",
        "agent-replay-again",
        "install",
        refs["agent"],
        "--frozen",
        "--machine",
    )
    fixture.snapshot(root, "direct-agent", "agent-replay-again", compare=True)
    fixture.snapshot(root, "local-agent", "local-regenerate")
    invoke(root, origin, "local-agent", "local-regenerate", "install", "--machine")
    fixture.snapshot(root, "local-agent", "local-regenerate", compare=True)
    for project in ("empty-local", "empty-workspace", "empty-direct"):
        spec = [refs["empty-agent"]] if project == "empty-direct" else []
        invoke(root, origin, project, project + "-install", "install", *spec, "--machine")
        if project == "empty-local":
            fixture.snapshot(root, project, "empty-local-again")
            invoke(root, origin, project, "empty-local-again", "install", "--machine")
            fixture.snapshot(root, project, "empty-local-again", compare=True)
        label = project + ("-frozen" if project != "empty-direct" else "-again")
        fixture.snapshot(root, project, label)
        invoke(
            root,
            origin,
            project,
            label,
            "install",
            *spec,
            "--machine",
            *([] if project == "empty-direct" else ["--frozen"])
        )
        fixture.snapshot(root, project, label, compare=True)
        label = project + "-frozen"
        fixture.snapshot(root, project, label)
        if project != "empty-local":
            fixture.clean(root, project)
        invoke(root, origin, project, label, "install", *spec, "--machine", "--frozen")
        fixture.snapshot(root, project, label, compare=True)
        fixture.check_lock(root, project)
    for name, version in (("run-v1", "1.0.0"), ("run-v2", "2.0.0")):
        entry = refs["echo-v1" if version == "1.0.0" else "echo-v2"]
        output = invoke(
            root,
            origin,
            "direct-agent",
            name,
            "run",
            entry,
            "--input",
            '{"message":"manual"}',
            "--machine",
        )
        assert json.loads(output.stdout)["output"]["version"] == version


def test_faults_cache_refresh_old_locks_and_exact_errors_keep_state(server):
    root, origin = server
    refs = fixture.read(root / "session.json")["refs"]
    invoke(root, origin, "faults", "control-install", "install", refs["agent"], "--machine")
    fixture.check_lock(root, "faults")
    for change in ("missing-transitive", "wrong-kind", "pin-digest", "stale-edge", "root-origin"):
        fixture.mutate(root, "faults", change)
        fixture.snapshot(root, "faults", change)
        invoke(
            root,
            origin,
            "faults",
            change,
            "install",
            refs["agent"],
            "--frozen",
            "--machine",
            expected=1,
        )
        fixture.snapshot(root, "faults", change, compare=True)
        fixture.traffic(root, change, failed=True, local=change == "wrong-kind")
        fixture.mutate(root, "faults", "restore")
    for case in ("init-digest", "corrupt-download", "registry-offline"):
        fixture.snapshot(root, "faults", case)
        invoke(
            root,
            origin,
            "faults",
            case,
            "install",
            refs["agent"],
            "--frozen",
            "--refresh",
            "--machine",
            case=case,
            expected=1,
        )
        fixture.snapshot(root, "faults", case, compare=True)
        fixture.traffic(root, case, failed=True, gets=0 if case != "corrupt-download" else 8)
    invoke(root, origin, "guards", "guards-install", "install", "--machine")
    for change in ("changed-range", "removed-dependency", "local-identity"):
        fixture.mutate(root, "guards", change)
        fixture.snapshot(root, "guards", change)
        invoke(root, origin, "guards", change, "install", "--frozen", "--machine", expected=1)
        fixture.snapshot(root, "guards", change, compare=True)
        fixture.traffic(root, change, local=True)
        fixture.mutate(root, "guards", "restore")
    fixture.snapshot(root, "fresh", "missing-lock")
    invoke(root, origin, "fresh", "missing-lock", "install", "--frozen", "--machine", expected=1)
    fixture.snapshot(root, "fresh", "missing-lock", compare=True)
    fixture.traffic(root, "missing-lock", local=True)
    for change, label, gets in (
        (None, "control-frozen", 0),
        ("clean", "control-clean", 8),
        ("cache", "cache-repair", 1),
        ("payload", "refresh-repair", 8),
    ):
        fixture.snapshot(root, "faults", label)
        if change == "clean":
            fixture.clean(root, "faults")
        elif change:
            fixture.mutate(root, "faults", change)
        invoke(
            root,
            origin,
            "faults",
            label,
            "install",
            refs["agent"],
            "--frozen",
            "--machine",
            *(["--refresh"] if change == "payload" else [])
        )
        fixture.snapshot(root, "faults", label, compare=True)
        fixture.traffic(root, label, gets=gets)
    invoke(root, origin, "old-tool", "old-tool-install", "install", "--machine")
    for name, change, label, spec in (
        ("old-tool", "old-v2", "old-v2-replay", []),
        ("faults", "old-v3", "old-v3-replay", [refs["agent"]]),
    ):
        fixture.mutate(root, name, change)
        fixture.snapshot(root, name, label)
        invoke(root, origin, name, label, "install", *spec, "--frozen", "--machine")
        fixture.snapshot(root, name, label, compare=True)
        fixture.traffic(root, label, gets=0)
        fixture.mutate(root, name, "restore")
    fixture.check_lock(root, "faults")
    fixture.check_lock(root, "guards")
    fixture.check_lock(root, "old-tool")


def test_shell_capture_and_exact_error_checks_do_not_self_certify(server):
    root, _origin = server
    script = """source "$1"
cd "$M5B_ROOT/projects/fresh" || exit 1
m5b_fixture snapshot fresh missing-lock || exit 1
m5b_expect missing-lock 1 m5b_fault good install --frozen --machine || exit 1
m5b_fixture check-result missing-lock || exit 1
m5b_fixture unchanged fresh missing-lock || exit 1
m5b_fixture traffic missing-lock --local || exit 1
"""
    result = subprocess.run(
        ["zsh", "-c", script, "m5b-shell", str(root / "paths.sh")],
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "EXPECTED exit 1" in result.stdout
    path = root / "evidence/missing-lock.stdout.txt"
    value = fixture.read(path)
    value["error"]["message"] = "unrelated failure"
    fixture.write(path, value)
    with pytest.raises(AssertionError):
        fixture.result(root, "missing-lock")
    with pytest.raises(FileNotFoundError):
        fixture.audit(root)


def test_actual_runbook_shell_blocks_replay_without_operator_or_publication_claims(server):
    import re

    root, _origin = server
    guide = Path(__file__).parents[2] / "CLI_M5B_MANUAL_TESTS.md"
    text = guide.read_text().split("## 3.", 1)[1].split("## 12.", 1)[0]
    blocks = re.findall(r"```bash\n(.*?)\n```", text, re.S)
    blocks = [block for block in blocks if not re.search(r"^m5b_fixture serve$", block, re.M)]
    # Only the test child redirects the real-API function to the isolated fixture.
    # The committed operator helper keeps its fixed loopback:5000 safety guard.
    script = 'set -e\nsource "$1"\nm5b() { m5b_fault good "$@"; }\n' + "\n".join(blocks)
    result = subprocess.run(
        ["zsh", "-c", script, "m5b-replay", str(root / "paths.sh")],
        capture_output=True,
        text=True,
        timeout=180,
    )
    assert result.returncode == 0, result.stdout[-7000:] + result.stderr[-3000:]
    excluded = {"version", "whoami", *["publish-" + name for name in fixture.SEEDS]}
    for label in (*[name for name in fixture.SUCCESS if name not in excluded], *fixture.ERRORS):
        recorded = fixture.read(root / "evidence" / (label + ".result.json"))
        assert recorded["exit_code"] == (1 if label in fixture.ERRORS else 0), (label, recorded)
    for label in fixture.PROOFS:
        assert fixture.read(root / "evidence" / (label + "-unchanged.json"))["passed"], label
    for project in fixture.PROJECTS:
        if project != "fresh":
            fixture.check_lock(root, project)
    path = root / "projects/workspace/agent.lock"
    saved = path.read_bytes()
    changed = fixture.read(path)
    changed["roots"]["local:agent:agents/second.agent.json"]["tools"] = changed["roots"][
        "local:agent"
    ]["tools"]
    fixture.write(path, changed)
    with pytest.raises(AssertionError, match="wrong exact root edges"):
        fixture.check_lock(root, "workspace")
    path.write_bytes(saved)
    assert not (root / "evidence/summary.json").exists()
    assert not list((root / "cases").glob("*/receipt.json"))
    # The complete operator audit must still refuse absent real publication evidence.
    with pytest.raises(FileNotFoundError):
        fixture.audit(root)
