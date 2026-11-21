"""Builder verification: fixture-only network; no real Registry/DB/S3 writes."""

import base64
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time
from types import SimpleNamespace
import uuid

from flask import Flask
from nacl.signing import VerifyKey
import pytest

from app.tools.application.publish_apds import validate_snapshot
from app.tools.application.cli import _resolve_install_graph
from app.tools.application.install_apds import metadata
from app.tools.domain.tools.legacy_signing import legacy_statement_bytes

SPEC = importlib.util.spec_from_file_location("manual_m5a", Path(__file__).with_name("fixture.py"))
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
CLI = fixture.REPO / "target/debug/agentpm"


@pytest.fixture(name="prepared")
def prepared_fixture():
    root = fixture.REPO / "cli-m5a-test" / ("builder-helper-" + uuid.uuid4().hex)
    try:
        fixture.prepare(root, "fixture", CLI)
        yield root
    finally:
        shutil.rmtree(root, ignore_errors=True)


@pytest.fixture(name="server")
def server_fixture(prepared):
    process = subprocess.Popen(
        [sys.executable, "-B", str(Path(__file__).with_name("fixture.py")), "serve", str(prepared)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        for _ in range(200):
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


def install(root, origin, project, case, *, frozen=False, label=None, expected=0):
    command = [str(CLI), "--base-url", origin + "/" + case, "install", "--machine"]
    if frozen:
        command.append("--frozen")
    result = subprocess.run(
        command,
        cwd=root / "projects" / project,
        env={**os.environ, "AGENTPM_TELEMETRY": "0", "AGENTPM_TOKEN": "apm_m5a.fixture"},
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert result.returncode == expected, result.stdout + result.stderr
    if label:
        for stream in ("stdout", "stderr"):
            (root / "evidence" / (label + "." + stream + ".txt")).write_text(
                getattr(result, stream)
            )
        fixture.write(
            root / "evidence" / (label + ".result.json"),
            {"exit_code": result.returncode, "expected_exit_code": expected},
        )
    return result


def test_preparation_paths_signatures_and_positive_archive_semantics(prepared):
    assert len(list((prepared / "cases").iterdir())) == 5
    assert len(list((prepared / "controlled").iterdir())) == 11
    assert prepared.joinpath("author.seed").stat().st_mode & 0o777 == 0o600
    with pytest.raises(ValueError, match="Existing run"):
        fixture.prepare(prepared, "fixture", CLI)
    with pytest.raises(ValueError, match="run directory"):
        fixture.local_root(fixture.REPO / "target")
    candidates = [
        fixture.read(prepared / "cases" / name / "source/agent.json") for name in fixture.SEEDS
    ]

    def selected(kind, name, version):
        value = next(
            v
            for v in candidates
            if v["kind"] == kind and name == "@fixture/" + v["name"] and version == v["version"]
        )
        return value, value["version"]

    app = Flask(__name__)
    with app.app_context():
        for name in fixture.SEEDS:
            folder = prepared / "cases" / name
            value = fixture.read(folder / "source/agent.json")
            signature = fixture.read(folder / "finalize-good.json")["author_signatures"][0]
            VerifyKey(base64.b64decode(signature["public_key_b64"])).verify(
                legacy_statement_bytes(signature["statement_json"]),
                base64.b64decode(signature["signature_b64"]),
            )
            if name == "legacy":
                continue
            evidence = validate_snapshot(
                io.BytesIO((folder / "any.tar.gz").read_bytes()),
                value,
                identity=(value["kind"], value["name"], value["version"]),
                resolve_manifest=selected,
            )
            assert evidence["status"] == "conformant", evidence


def test_real_cli_rejects_controlled_defects_without_replacement(server):
    root, origin = server
    install(root, origin, "repair", "good")
    fixture.check_lock(root, "repair")
    fixture.snapshot(root, "repair", "failures")
    for case in tuple(fixture.ERRORS)[:9]:
        install(root, origin, "repair", case, label=case, expected=1)
        fixture.result(root, case)
        fixture.snapshot(root, "repair", "failures", compare=True)
    requests = [
        json.loads(line)
        for line in (root / "evidence/control-requests.jsonl").read_text().splitlines()
    ]
    for case in tuple(fixture.ERRORS)[:9]:
        events = [v["route"] for v in requests if v["case"] == case]
        assert "v1/tools/install/finalize" not in events
        if case == "unsupported-resolve":
            assert events == ["v1/tools/install/resolve"]
        if case == "init-drift":
            assert "artifact" not in events
    # Replay the runbook's next section with the rejected archives still cached.
    fixture.mutate(root, "repair", "cache")
    install(root, origin, "repair", "good")
    fixture.snapshot(root, "repair", "failures", compare=True)


def test_cache_and_installed_selector_repairs_and_lock_guards(server):
    root, origin = server
    install(root, origin, "repair", "good")
    fixture.snapshot(root, "repair", "original")
    fixture.mutate(root, "repair", "cache")
    install(root, origin, "repair", "good")
    fixture.snapshot(root, "repair", "original", compare=True)
    install(root, origin, "older-response", "older-response")
    fixture.snapshot(root, "older-response", "selector")
    fixture.mutate(root, "older-response", "installed-selector")
    install(root, origin, "older-response", "older-response")
    fixture.snapshot(root, "older-response", "selector", compare=True)
    fixture.check_lock(root, "older-response")
    install(root, origin, "lock-guards", "good")
    for case in ("future-lock", "unknown-field", "partial-origin"):
        fixture.mutate(root, "lock-guards", case)
        fixture.snapshot(root, "lock-guards", case)
        install(root, origin, "lock-guards", "good", frozen=True, label=case, expected=1)
        fixture.result(root, case)
        fixture.snapshot(root, "lock-guards", case, compare=True)
    fixture.mutate(root, "lock-guards", "restore-lock")
    fixture.check_lock(root, "lock-guards")


def test_runbook_positive_commands_with_actual_resolve_logic_and_signed_fixture_archives(prepared):
    """Replay author-verified CLI commands; in-memory records replace real DB/S3.

    This is builder evidence, not operator acceptance against the local API.
    """
    records = {}
    for label in fixture.SEEDS:
        folder = prepared / "cases" / label
        value = fixture.read(folder / "source/agent.json")
        records[("@fixture/" + value["name"], value["version"])] = (label, value)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            self.respond()

        def do_POST(self):
            self.respond()

        def respond(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            request = json.loads(body) if body else {}
            origin = f"http://127.0.0.1:{self.server.server_port}/real"

            def selected(name, selector):
                label, value = records[(name, selector)]
                return (
                    SimpleNamespace(handle="fixture"),
                    SimpleNamespace(name=value["name"], kind=value["kind"]),
                    SimpleNamespace(
                        version=value["version"],
                        manifest=value,
                        sha_256=fixture.digest(prepared / "cases" / label / "any.tar.gz"),
                    ),
                )

            if self.path == "/real/v1/tools/install/resolve":
                items, error = _resolve_install_graph(request["items"], selected)
                assert error is None
                value = {"items": items}
            elif self.path == "/real/v1/tools/install/init":
                artifacts = []
                for item in request["items"]:
                    label, manifest = records[(item["name"], item["version"])]
                    folder = prepared / "cases" / label
                    author = fixture.read(folder / "finalize-good.json")["author_signatures"][0]
                    author.update(
                        signer_id="fixture-author",
                        is_active=True,
                        statement_bytes_b64=base64.b64encode(
                            legacy_statement_bytes(author["statement_json"])
                        ).decode(),
                    )
                    artifacts.append(
                        {
                            **item,
                            **metadata(SimpleNamespace(manifest=manifest)),
                            "size": (folder / "any.tar.gz").stat().st_size,
                            "presigned_url": origin + "/archive/" + label,
                            "runtime": manifest.get("runtime"),
                            "verification": {
                                "mode": "required",
                                "min_author_signatures": 1,
                                "manifest": manifest,
                                "authors": [author],
                                "registry_attestation": None,
                                "registry_keyset_path": "/v1/registry/signing-keys",
                            },
                        }
                    )
                value = {
                    "session_id": "fixture-real-protocol",
                    "expires_at": "2099-01-01T00:00:00Z",
                    "artifacts": artifacts,
                }
            elif self.path == "/real/v1/tools/install/finalize":
                value = {}
            elif self.path.startswith("/real/archive/"):
                value = (prepared / "cases" / self.path.split("/")[-1] / "any.tar.gz").read_bytes()
            else:
                self.send_error(404)
                return
            payload = value if isinstance(value, bytes) else json.dumps(value).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    origin = f"http://127.0.0.1:{server.server_port}"
    try:

        def command(project, *args, expected=0):
            output = subprocess.run(
                [str(CLI), "--base-url", origin + "/real", *args],
                cwd=prepared / "projects" / project,
                env={**os.environ, "AGENTPM_TELEMETRY": "0", "AGENTPM_TOKEN": "apm_fixture"},
                capture_output=True,
                text=True,
                timeout=60,
            )
            assert output.returncode == expected, output.stdout + output.stderr
            return output

        for project in ("simple", "graph", "local-skill", "local-tool"):
            command(project, "install", "--require-signature", "--machine")
            fixture.check_lock(prepared, project)
        fixture.snapshot(prepared, "simple", "regenerate")
        command("simple", "install", "--require-signature", "--machine")
        fixture.snapshot(prepared, "simple", "regenerate", compare=True)
        fixture.snapshot(prepared, "simple", "frozen")
        shutil.move(
            prepared / "projects/simple/.agentpm",
            prepared / "projects/simple/installed-before-frozen",
        )
        command("simple", "install", "--frozen", "--require-signature", "--machine")
        fixture.snapshot(prepared, "simple", "frozen", compare=True)
        fixture.mutate(prepared, "simple", "rename-local")
        command("simple", "install", "--require-signature", "--machine")
        fixture.check_lock(prepared, "simple")
        fixture.mutate(prepared, "simple", "legacy-local")
        fixture.snapshot(prepared, "simple", "frozen-origin")
        command("simple", "install", "--frozen", "--machine", expected=1)
        fixture.snapshot(prepared, "simple", "frozen-origin", compare=True)
        command("simple", "install", "--require-signature", "--machine")
        fixture.check_lock(prepared, "simple")
        refs = fixture.read(prepared / "session.json")["refs"]
        for name, version in (("echo-v1", "1.0.0"), ("echo-v2", "2.0.0")):
            result = json.loads(
                command(
                    "graph", "run", refs[name], "--machine", "--input", '{"message":"test"}'
                ).stdout
            )
            assert result["output"]["version"] == version and result["output"]["echo"] == {
                "message": "test"
            }
        command("direct-agent", "install", refs["agent"], "--require-signature", "--machine")
        fixture.check_lock(prepared, "direct-agent")
    finally:
        server.shutdown()
        server.server_close()
        worker.join(timeout=10)
