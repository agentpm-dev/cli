"""Install a deterministic native-dependency Tool with the real CLI and local Registry.

The real pinned uv replays the existing checked-in pydantic resolution from PyPI.
No publication, database, model call or real Registry credential is involved.
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def prepare(root: Path, cli: str, uv: str, python: str) -> dict:
    vectors = json.loads((Path(__file__).parent / "compatibility.json").read_text())
    resolution = vectors["resolution"]
    digest = hashlib.sha256(
        json.dumps(resolution, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()
    manifest = {
        "kind": "tool",
        "name": "tool",
        "version": "1.0.0",
        "description": "No-model SDK native dependency fixture",
        "files": ["tool.py"],
        "entrypoint": {"command": "python", "args": ["tool.py"]},
        "runtime": {
            "type": "python",
            "version": "3.11",
            "dependencies": resolution["requirements"],
        },
        "inputs": {"type": "object"},
        "outputs": {"type": "object"},
    }
    script = """import json,sys,os
import pydantic,pydantic_core
from pydantic import BaseModel,Field
class Order(BaseModel):
 quantity:int=Field(gt=0)
 unit_price:float=Field(gt=0)
payload=json.load(sys.stdin)
if payload.get("pause"):
 import time;time.sleep(30)
value=Order.model_validate(payload)
print(json.dumps({'total':value.quantity*value.unit_price,'prefix':sys.prefix,'executable':sys.executable,'native_file':pydantic_core._pydantic_core.__file__,'pydantic_file':pydantic.__file__,'pydantic_version':pydantic.__version__,'layer':os.getenv('LAYER'),'pythonpath':os.getenv('PYTHONPATH'),'pythonhome':os.getenv('PYTHONHOME')}))
"""
    files = {
        "agent.json": json.dumps(manifest).encode(),
        "agentpm-python.json": json.dumps(
            {"python_resolution": resolution, "python_resolution_digest": digest}
        ).encode(),
        "tool.py": script.encode(),
    }
    source = root / "payload-source"
    source.mkdir(parents=True, exist_ok=True)
    for name, data in files.items():
        (source / name).write_bytes(data)
    schema = Path(
        os.getenv(
            "AGENTPM_TEST_SCHEMA",
            str(Path(cli).resolve().parents[2] / "schemas/agentpm.manifest.schema.json"),
        )
    )
    lint = subprocess.run(
        [cli, "lint", "--schema", str(schema), str(source / "agent.json")],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if lint.returncode:
        raise RuntimeError("Fixture manifest lint failed: " + lint.stdout + lint.stderr)
    archive = io.BytesIO()
    with tarfile.open(fileobj=archive, mode="w:gz") as tar:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.mode = 0o644
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    content = archive.getvalue()
    integrity = hashlib.sha256(content).hexdigest()
    item = {
        "kind": "tool",
        "name": "@fixture/tool",
        "version": "1.0.0",
        "integrity": integrity,
        "python_resolution": resolution,
    }
    calls: list[str] = []

    class Registry(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            self.rfile.read(int(self.headers.get("Content-Length", "0")))
            calls.append(self.path)
            if self.path == "/v1/tools/install/resolve":
                value = {"items": [item]}
            elif self.path == "/v1/tools/install/init":
                value = {
                    "session_id": "sdk-fixture",
                    "expires_at": "2099-01-01T00:00:00Z",
                    "artifacts": [
                        {
                            **item,
                            "presigned_url": f"http://127.0.0.1:{self.server.server_port}/artifact",
                        }
                    ],
                }
            elif self.path == "/v1/tools/install/finalize":
                value = {}
            else:
                self.send_error(500, "Unexpected request")
                return
            self.send(json.dumps(value).encode())

        def do_GET(self):
            calls.append(self.path)
            if self.path != "/artifact":
                self.send_error(500, "Unexpected request")
                return
            self.send(content)

        def send(self, data):
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    root.mkdir(parents=True, exist_ok=True)
    (root / "agent.json").write_text(
        json.dumps(
            {
                "kind": "agent",
                "name": "consumer",
                "version": "1.0.0",
                "description": "Local SDK consumer fixture",
                "tools": ["@fixture/tool@1.0.0"],
            }
        )
    )
    env = dict(os.environ, AGENTPM_UV_PATH=uv, AGENTPM_PYTHON=python, AGENTPM_TELEMETRY="0")
    # These are test-owned directories, including the Windows config vocabulary.
    for key in ["XDG_CONFIG_HOME", "XDG_CACHE_HOME", "APPDATA", "LOCALAPPDATA"]:
        path = root / key
        path.mkdir()
        env[key] = str(path)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Registry)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        result = subprocess.run(
            [
                cli,
                "--base-url",
                f"http://127.0.0.1:{server.server_port}",
                "install",
                "--token",
                "apm_sdk.fixture",
                "--quiet",
            ],
            cwd=root,
            env=env,
            capture_output=True,
            text=True,
            timeout=180,
        )
        if result.returncode:
            raise RuntimeError("Real CLI fixture install failed:\n" + result.stderr)
    finally:
        server.shutdown()
        server.server_close()
        worker.join()
    assert calls.count("/v1/tools/install/finalize") == 1, calls
    assert json.loads((root / "agent.lock").read_text())["lockfile_version"] == 4
    return {"root": str(root), "spec": "@fixture/tool@1.0.0", "python": python, "calls": calls}


if __name__ == "__main__":
    print(json.dumps(prepare(Path(sys.argv[1]).resolve(), sys.argv[2], sys.argv[3], sys.argv[4])))
