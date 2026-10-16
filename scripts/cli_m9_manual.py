#!/usr/bin/env python3
"""Prepare empty manual-test directories, or serve optional HTTP error fixtures.

This helper never runs a CLI test case, creates a manifest, or modifies a registry.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REPO = Path(__file__).resolve().parents[1]
DEFAULT_ROOT = REPO / "cli-m9-test"
OWNER = "agentpm-cli-m9-manual-v1"
DIRECTORIES = (
    "knowledge",
    "memory",
    "template",
    "tool",
    "registry",
    "private-valid",
    "private-revoked",
    "private-anonymous",
    "public-anonymous",
    "home",
    "runs",
)


def owned(root):
    if root.is_symlink():
        raise RuntimeError("Refusing a symlink fixture root.")
    marker = root / "fixture.json"
    if marker.is_symlink() or not marker.is_file():
        raise RuntimeError(f"Not an M9 fixture directory: {root}")
    metadata = json.loads(marker.read_text(encoding="utf-8"))
    if not isinstance(metadata, dict) or metadata.get("owner") != OWNER:
        raise RuntimeError(f"Not an M9 fixture directory: {root}")


def check_server_stopped(root):
    pid_file = root / "mock.pid"
    if pid_file.exists():
        try:
            os.kill(int(pid_file.read_text(encoding="utf-8")), 0)
        except ProcessLookupError:
            return
        raise RuntimeError("Stop the optional mock server with Ctrl-C first.")


def setup(root, cli, reset=False):
    root = root.absolute()
    cli = cli.resolve(strict=True)
    if root.is_symlink():
        raise RuntimeError("Refusing a symlink fixture root.")
    if root.exists():
        owned(root)
        if not reset:
            raise RuntimeError("Fixtures already exist; use --reset to discard them.")
        check_server_stopped(root)
    version = subprocess.check_output([str(cli), "--version"], text=True).strip()
    digest = hashlib.sha256(cli.read_bytes()).hexdigest()
    if root.exists():
        shutil.rmtree(root)
    for name in DIRECTORIES:
        (root / name).mkdir(parents=True)
    metadata = {
        "owner": OWNER,
        "cli": str(cli),
        "version": version,
        "sha256": digest,
    }
    (root / "fixture.json").write_text(
        json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
    )
    variables = {
        "M9_ROOT": str(root),
        "M9_APM": str(cli),
        "M9_PYTHON": sys.executable,
        "M9_HELPER": str(Path(__file__).resolve()),
        "M9_HOME": str(root / "home"),
        "M9_OUT": str(root / "runs"),
        "M9_REGISTRY": "http://127.0.0.1:5000",
        "M9_OFFLINE": "http://127.0.0.1:1",
    }
    (root / "env.sh").write_text(
        "".join(
            f"export {key}={shlex.quote(value)}\n" for key, value in variables.items()
        ),
        encoding="utf-8",
    )
    print(f"Prepared empty directories in {root}")
    print("No manifests, builds, installs, or test results were created.")
    print(f"CLI: {version} ({cli})")
    print(f"source {shlex.quote(str(root / 'env.sh'))}")


def fixture_response(method, path, token):
    if (method, path) == ("GET", "/v1/whoami"):
        if token == "Bearer apm_m9_ok.fixture":
            return 200, {
                "kind": "pat",
                "sub": "m9-fixture",
                "email": "m9@example.test",
                "scopes": ["packages:read"],
            }
        if token == "Bearer apm_m9_decode.fixture":
            return 200, {"unexpected": "shape"}
        return 401, {"error": "fixture rejected credentials"}
    if (method, path) == ("POST", "/v1/tools/install/resolve"):
        codes = {
            "Bearer apm_m9_rejected.fixture": 401,
            "Bearer apm_m9_forbidden.fixture": 403,
            "Bearer apm_m9_missing.fixture": 404,
            "Bearer apm_m9_server.fixture": 500,
        }
        code = codes.get(token, 400)
        return code, {"error": f"M9 fixture response {code}"}
    return 400, {"error": "Unexpected M9 mock request"}


class Handler(BaseHTTPRequestHandler):
    def respond(self):
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        code, body = fixture_response(
            self.command, self.path, self.headers.get("Authorization", "")
        )
        data = json.dumps(body).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    do_GET = respond
    do_POST = respond

    def log_message(self, fmt, *args):
        # Do not log credentials or request bodies.
        print(f"{self.command} {self.path}", flush=True)


def serve(root):
    owned(root)
    check_server_stopped(root)
    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        server.daemon_threads = True
        url = f"http://127.0.0.1:{server.server_port}"
        (root / "mock.pid").write_text(str(os.getpid()), encoding="utf-8")
        (root / "mock-env.sh").write_text(
            f"export M9_MOCK={shlex.quote(url)}\n", encoding="utf-8"
        )
        print(f"Optional mock registry: {url}", flush=True)
        print(f"source {shlex.quote(str(root / 'mock-env.sh'))}", flush=True)
        print("Ctrl-C stops this mock only, not your real backend.", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
        finally:
            (root / "mock.pid").unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("setup", help="create empty directories only")
    prepare.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    prepare.add_argument("--cli", type=Path, default=REPO / "target/debug/agentpm")
    prepare.add_argument("--reset", action="store_true")
    mock = commands.add_parser("serve", help="serve optional loopback HTTP fixtures")
    mock.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    args = parser.parse_args()
    try:
        if args.command == "setup":
            setup(args.root, args.cli, args.reset)
        else:
            serve(args.root)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"M9 fixtures: {error}\n")


if __name__ == "__main__":
    main()
