#!/usr/bin/env python3
"""Prepare and exercise disposable CLI authoring fixtures, one runbook section at a time."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tarfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REPO = Path(__file__).resolve().parents[1]
BASE = REPO / "cli-m8-test"
KINDS = ("tool", "agent", "template", "skill", "knowledge", "memory", "profile", "loop")
SECTIONS = (
    "scaffolds",
    "tool",
    "packaging",
    "diagnostics",
    "machine",
    "standard",
    "knowledge",
    "install",
)
FIXTURE_ID = "agentpm-cli-m8-manual-v1"
STANDARD = {"name": "future-standard", "opaque": {"version": 7}}
FUTURE = {"opaque": [3, 2, 1]}
ECHO_MARKER = "M8_LARGE_ECHO_MARKER"
DIAGNOSTICS = {
    "missing-tools": ("/tools", 'Missing required field "tools"'),
    "package-ref": ("/tools/0", "@ns/name@1.2.3"),
    "memory-operation": (
        "/memory/operations/refresh/type",
        "consolidate, transform, delete",
    ),
    "memory-trigger": ("/memory/operations/refresh/trigger/type", "record_count"),
    "loop-transition": ("/loop/transitions/0/to", "supported terminal"),
    "loop-policy": ("/loop/error_policy/tool_failure/action", "retry"),
    "knowledge-mode": ("/knowledge/mode", "context"),
    "memory-binding": ("/bindings/global/memory/0", "spaces or operations"),
    "entrypoint": ("/entrypoint/command", "absolute interpreter path"),
    "large-value": ("/tools/0", "package reference"),
}
EXPECTED_COMMANDS = {
    "scaffolds": {
        f"{prefix}-{kind}" for kind in KINDS for prefix in ("init", "lint", "lint-json")
    },
    "tool": {"tool-echo", "keep-source", "tool-package"},
    "packaging": {
        f"missing-{label}-{suffix}"
        for label in ("file", "directory", "glob")
        for suffix in ("lint", "publish")
    },
    "diagnostics": {f"diagnostic-{label}" for label in DIAGNOSTICS}
    | {"publish-context"},
    "machine": {
        f"machine-{label}-{fmt}"
        for label in (
            "loop-transition",
            "memory-operation",
            "missing-tools",
            "large-value",
        )
        for fmt in ("json", "ndjson")
    },
    "standard": {f"standard-{kind}-{index}" for kind in KINDS for index in range(6)}
    | {f"unknown-{kind}-{typo}" for kind in KINDS for typo in ("standrad", "tool")},
    "knowledge": {
        "knowledge-lint",
        "knowledge-build",
        "knowledge-unknown-bundled",
        "knowledge-forward-lint",
        "knowledge-forward-build",
    },
    "install": {"install-writeback"},
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def binary_digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Suite:
    def __init__(self, metadata):
        self.metadata = metadata
        self.apm = Path(metadata["apm"])
        self.records = []

    def run(self, label, cwd, args, expected=0, source=False, stdin=None):
        require(
            cwd.resolve().is_relative_to(BASE.resolve()), "Command outside fixture root"
        )
        command = [sys.executable if source else str(self.apm), *map(str, args)]
        print(
            f"\n[{label}] (cd {shlex.quote(str(cwd))}) {shlex.join(command)}",
            flush=True,
        )
        env = dict(os.environ)
        env.update(
            NO_COLOR="1",
            RUST_LOG="",
            AGENTPM_BASE_URL="http://127.0.0.1:9",
            AGENTPM_TOKEN="m8-fixture-not-a-credential",
        )
        result = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            input=stdin,
            capture_output=True,
            text=True,
            timeout=60,
        )
        prefix = BASE / "runs" / label
        prefix.with_suffix(".stdout.txt").write_text(result.stdout, encoding="utf-8")
        prefix.with_suffix(".stderr.txt").write_text(result.stderr, encoding="utf-8")
        write_json(
            prefix.with_suffix(".command.json"),
            {
                "command": command,
                "cwd": str(cwd),
                "stdin": stdin,
                "exit_code": result.returncode,
                "expected_exit_code": expected,
                "binary_sha256": self.metadata["sha256"],
            },
        )
        self.records.append(label)
        for stream in (result.stdout, result.stderr):
            print(stream[:5000], end="" if stream.endswith("\n") else "\n")
            if len(stream) > 5000:
                print(f"[Display truncated; complete output: {prefix}.*.txt]")
        require(
            result.returncode == expected,
            f"{label}: expected exit {expected}, got {result.returncode}; see {prefix}.*.txt",
        )
        print(f"Expected exit {expected}: OK")
        return result.stdout, result.stderr

    def lint_json(self, label, cwd, expected=0, extra=()):
        stdout, _ = self.run(
            label, cwd, ["lint", "--strict", "--format", "json", *extra], expected
        )
        reports = json.loads(stdout)
        require(len(reports) == 1, f"{label}: expected one manifest report")
        require(
            reports[0]["ok"] == (expected == 0), f"{label}: incorrect report status"
        )
        if expected == 0:
            require(reports[0]["issues"] == [], f"{label}: unexpected lint issues")
        return reports[0]


def setup(apm, reset):
    require(not BASE.is_symlink(), "Refusing to replace a symlink fixture root")
    apm = apm.resolve(strict=True)
    require(
        not apm.is_relative_to(BASE.resolve()),
        "CLI binary must live outside the fixture root",
    )
    if BASE.exists():
        require(
            reset, "Fixtures already exist. Use setup --reset to discard their results."
        )
        marker = BASE / "fixture.json"
        require(
            marker.is_file() and read_json(marker).get("fixture_id") == FIXTURE_ID,
            "Refusing to delete a directory not owned by this fixture helper",
        )
        shutil.rmtree(BASE)
    BASE.mkdir()
    (BASE / "runs").mkdir()
    metadata = {
        "fixture_id": FIXTURE_ID,
        "apm": str(apm),
        "sha256": binary_digest(apm),
        "version": subprocess.check_output([str(apm), "--version"], text=True).strip(),
        "setup_complete": False,
    }
    write_json(BASE / "fixture.json", metadata)
    suite = Suite(metadata)
    for kind in KINDS:
        seed = BASE / "baseline" / kind
        seed.mkdir(parents=True)
        (BASE / "scaffolds" / kind).mkdir(parents=True)
        suite.run(
            f"setup-init-{kind}", seed, ["init", "--kind", kind, "--name", f"m8-{kind}"]
        )
        suite.lint_json(f"setup-lint-{kind}", seed)
        shutil.copytree(seed, BASE / "standard" / kind)

    def clone(kind, relative):
        target = BASE / relative
        shutil.copytree(BASE / "baseline" / kind, target)
        return target

    clone("tool", "tool")
    existing = clone("tool", "existing-source")
    (existing / "main.py").write_text(
        '# M8 existing source: keep me\nprint("existing")\n', encoding="utf-8"
    )
    for label, missing in (
        ("file", "missing.txt"),
        ("directory", "missing-dir"),
        ("glob", "*.py"),
    ):
        target = clone("tool", f"missing-{label}")
        manifest = read_json(target / "agent.json")
        manifest["files"].append(missing)
        write_json(target / "agent.json", manifest)

    for label in DIAGNOSTICS:
        kind = "agent"
        if label.startswith("memory-") and label != "memory-binding":
            kind = "memory"
        elif label.startswith("loop-"):
            kind = "loop"
        elif label == "knowledge-mode":
            kind = "knowledge"
        elif label == "entrypoint":
            kind = "tool"
        target = clone(kind, f"invalid/{label}")
        manifest = read_json(target / "agent.json")
        if label == "missing-tools":
            del manifest["tools"]
        elif label == "package-ref":
            manifest["tools"] = [{}]
        elif label == "memory-operation":
            manifest["memory"]["operations"] = {"refresh": {"type": "archive"}}
        elif label == "memory-trigger":
            manifest["memory"]["operations"] = {
                "refresh": {
                    "type": "transform",
                    "description": "Refresh preferences.",
                    "trigger": {"type": "unknown"},
                    "inputs": [{"space": "profile", "record_type": "user_preference"}],
                    "output": {"space": "profile", "record_type": "user_preference"},
                    "source_handling": "retain",
                    "preserve_provenance": True,
                }
            }
        elif label == "loop-transition":
            manifest = read_json(
                REPO / "crates/agentpm-cli/tests/fixtures/lint/invalid-loop.json"
            )
        elif label == "loop-policy":
            manifest["loop"]["error_policy"]["tool_failure"] = {"action": "unknown"}
        elif label == "knowledge-mode":
            manifest["knowledge"]["mode"] = "unknown"
        elif label == "memory-binding":
            manifest["memory"] = ["@manual/m8-memory"]
            manifest["bindings"] = {
                "global": {"memory": [{"package": "@manual/m8-memory"}]}
            }
        elif label == "entrypoint":
            manifest["entrypoint"]["command"] = ""
        elif label == "large-value":
            manifest["tools"] = [{"not-a-reference": ECHO_MARKER * 1000}]
            manifest["description"] = "Long fixture description. " * 1000
        write_json(target / "agent.json", manifest)

    for name in ("knowledge", "knowledge-forward", "install"):
        target = clone("agent" if name == "install" else "knowledge", name)
        manifest = read_json(target / "agent.json")
        manifest["standard"] = STANDARD
        if name != "knowledge":
            manifest["future_extension"] = FUTURE
        write_json(target / "agent.json", manifest)
        write_json(target / "before.json", manifest)
    schema = read_json(REPO / "schemas/agentpm.manifest.schema.json")
    schema["properties"]["future_extension"] = {}
    write_json(BASE / "knowledge-forward/schemas/agentpm.manifest.schema.json", schema)
    metadata["setup_complete"] = True
    write_json(BASE / "fixture.json", metadata)
    exports = {
        "APM": str(apm),
        "M8_ROOT": str(BASE),
        "M8_OUT": str(BASE / "runs"),
        "M8_HELPER": str(Path(__file__).resolve()),
        "M8_PYTHON": sys.executable,
    }
    (BASE / "env.sh").write_text(
        "\n".join(
            f"export {key}={shlex.quote(value)}" for key, value in exports.items()
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"\nFixtures ready. Source {BASE / 'env.sh'}")


def scaffolds(suite):
    for kind in KINDS:
        target = BASE / "scaffolds" / kind
        suite.run(
            f"init-{kind}", target, ["init", "--kind", kind, "--name", f"m8-{kind}"]
        )
        suite.run(f"lint-{kind}", target, ["lint", "--strict"])
        suite.lint_json(f"lint-json-{kind}", target)
        require(
            "standard" not in read_json(target / "agent.json"),
            "init must not emit standard",
        )


def tool(suite):
    target = BASE / "tool"
    manifest = read_json(target / "agent.json")
    require(
        manifest["runtime"] == {"type": "python", "version": "3"}, "Incorrect runtime"
    )
    require(
        manifest["entrypoint"] == {"command": "python3", "args": ["main.py"]},
        "Incorrect entrypoint",
    )
    require(manifest["files"] == ["main.py"], "Incorrect source files")
    request = {"message": "hello", "nested": {"count": 2}}
    stdout, _ = suite.run(
        "tool-echo", target, ["main.py"], source=True, stdin=json.dumps(request)
    )
    require(json.loads(stdout) == request, "Tool did not echo its JSON input")
    existing = BASE / "existing-source"
    before = (existing / "main.py").read_bytes()
    suite.run("keep-source", existing, ["init", "--kind", "tool", "--name", "m8-tool"])
    require(
        (existing / "main.py").read_bytes() == before, "init overwrote existing source"
    )
    suite.run(
        "tool-package",
        target,
        ["publish", "--dry-run", "--token", "m8-fixture-not-a-credential", "--quiet"],
    )
    artifact = target / "target/agentpm/m8-tool-0.1.0-python3.tar.gz"
    with tarfile.open(artifact) as archive:
        require(
            {"agent.json", "main.py"}.issubset(archive.getnames()),
            "Archive missing source",
        )
        require(
            json.load(archive.extractfile("agent.json")) == manifest,
            "Packaged manifest changed",
        )
        require(
            archive.extractfile("main.py").read() == (target / "main.py").read_bytes(),
            "Packaged source changed",
        )
    print(f"Archive verified: {artifact}")


def packaging(suite):
    for label, missing in (
        ("file", "missing.txt"),
        ("directory", "missing-dir"),
        ("glob", "*.py"),
    ):
        target = BASE / f"missing-{label}"
        suite.lint_json(f"missing-{label}-lint", target)
        _, stderr = suite.run(
            f"missing-{label}-publish",
            target,
            [
                "publish",
                "--dry-run",
                "--token",
                "m8-fixture-not-a-credential",
                "--quiet",
            ],
            1,
        )
        require(
            "manifest.files entry" in stderr and missing in stderr,
            "Missing-path diagnostic absent",
        )
        output = target / "target/agentpm"
        require(
            not output.exists() or not list(output.iterdir()),
            "Failed packaging left an artifact",
        )


def diagnostics(suite):
    for label, (pointer, fragment) in DIAGNOSTICS.items():
        stdout, _ = suite.run(
            f"diagnostic-{label}", BASE / "invalid" / label, ["lint"], 1
        )
        require(stdout.startswith("\u2717 agent.json\n"), "Missing manifest context")
        require(
            f"at instance {pointer}\n" in stdout and fragment in stdout,
            f"{label}: missing path or explanation",
        )
        require(
            ECHO_MARKER not in stdout and len(stdout) < 8000,
            f"{label}: unbounded human output",
        )
        errors = [line for line in stdout.splitlines() if "[ERROR]" in line]
        require(
            errors and len(errors) == len(set(errors)),
            f"{label}: missing or duplicate errors",
        )
        if label == "loop-transition":
            require(
                "transition destination" in errors[0],
                "Semantic transition error must come first",
            )
            require('"phases":' not in stdout, "Containing Loop object leaked")
            _, publish = suite.run(
                "publish-context",
                BASE / "invalid" / label,
                [
                    "publish",
                    "--dry-run",
                    "--token",
                    "m8-fixture-not-a-credential",
                    "--quiet",
                ],
                1,
            )
            require(
                stdout in publish, "Publish did not reuse lint's contextual diagnostics"
            )
        if label == "memory-operation":
            require(stdout.count(fragment) == 1, "Duplicated Memory union error")
        if label == "missing-tools":
            require(
                "vs schema  /oneOf/0/required" in stdout,
                "Missing Agent branch diagnostic",
            )


def machine(suite):
    for label in (
        "loop-transition",
        "memory-operation",
        "missing-tools",
        "large-value",
    ):
        for fmt in ("json", "ndjson"):
            stdout, _ = suite.run(
                f"machine-{label}-{fmt}",
                BASE / "invalid" / label,
                ["lint", "--format", fmt],
                1,
            )
            reports = (
                json.loads(stdout)
                if fmt == "json"
                else [json.loads(line) for line in stdout.splitlines()]
            )
            require(len(reports) == 1, "Expected exactly one machine report")
            report = reports[0]
            require(
                set(report) == {"file", "ok", "issues"}, "Machine report fields changed"
            )
            require(
                report["file"] == "agent.json"
                and report["ok"] is False
                and report["issues"],
                "Empty or successful error report",
            )
            for issue in report["issues"]:
                require(
                    set(issue)
                    == {"file", "level", "message", "instance_path", "schema_path"},
                    "Machine issue fields changed",
                )
                require(
                    all(isinstance(value, str) for value in issue.values()),
                    "Machine issue type changed",
                )
            if label == "missing-tools":
                require(
                    any(
                        issue["instance_path"] == "/tools"
                        and issue["schema_path"] == "/oneOf/0/required"
                        for issue in report["issues"]
                    ),
                    "Machine output lost root branch resolution",
                )
            if label == "large-value":
                require(
                    ECHO_MARKER in stdout,
                    "Human truncation incorrectly discarded machine details",
                )


def standard(suite):
    for kind in KINDS:
        target = BASE / "standard" / kind
        original = read_json(BASE / "baseline" / kind / "agent.json")
        for index, value in enumerate((STANDARD, "opaque", None, True, [1, 2], 42)):
            manifest = {**original, "standard": value}
            write_json(target / "agent.json", manifest)
            suite.lint_json(f"standard-{kind}-{index}", target)
            require(
                read_json(target / "agent.json") == manifest,
                "Lint rewrote ignored metadata",
            )
        for typo in ("standrad", "tool"):
            write_json(target / "agent.json", {**original, typo: {}})
            report = suite.lint_json(f"unknown-{kind}-{typo}", target, 1)
            require(
                any(
                    issue["schema_path"] == "/additionalProperties"
                    for issue in report["issues"]
                ),
                "Bundled root no longer rejects unknown fields",
            )
        write_json(target / "agent.json", {**original, "standard": STANDARD})


def knowledge(suite):
    for name in ("knowledge", "knowledge-forward"):
        target = BASE / name
        before = read_json(target / "agent.json")
        if name == "knowledge-forward":
            suite.lint_json(
                "knowledge-unknown-bundled",
                target,
                1,
                ["--schema", str(REPO / "schemas/agentpm.manifest.schema.json")],
            )
        suite.lint_json(f"{name}-lint", target)
        suite.run(f"{name}-build", target, ["knowledge", "build"])
        after = read_json(target / "agent.json")
        require(
            after["standard"] == before["standard"], "Knowledge build lost standard"
        )
        if name == "knowledge-forward":
            require(
                after["future_extension"] == before["future_extension"],
                "Knowledge build lost unknown metadata",
            )
        require(
            after["knowledge"]["context"]["document_count"] == 1,
            "Knowledge build did not populate document metadata",
        )
        write_json(target / "after.json", after)


def install(suite):
    skill = BASE / "baseline/skill"
    content = io.BytesIO()
    with tarfile.open(fileobj=content, mode="w:gz") as archive:
        for name in ("agent.json", "SKILL.md"):
            data = (skill / name).read_bytes()
            member = tarfile.TarInfo(name)
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
    artifact = content.getvalue()
    digest = hashlib.sha256(artifact).hexdigest()
    requests = []

    class Registry(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_GET(self):
            requests.append(f"GET {self.path}")
            if self.path != "/artifact/skill":
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/gzip")
            self.send_header("Content-Length", str(len(artifact)))
            self.end_headers()
            self.wfile.write(artifact)

        def do_POST(self):
            requests.append(f"POST {self.path}")
            request = json.loads(
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
            )
            if self.path == "/v1/tools/install/finalize":
                self.send_response(204)
                self.end_headers()
                return
            items = request.get("items", [])
            if len(items) != 1 or items[0].get("name") != "@manual/m8-skill":
                self.send_error(400, "Unexpected fixture package")
                return
            item = {
                "kind": "skill",
                "name": "@manual/m8-skill",
                "version": "0.1.0",
                "integrity": digest,
            }
            if self.path == "/v1/tools/install/resolve":
                response = {"items": [item]}
            elif self.path == "/v1/tools/install/init":
                response = {
                    "session_id": "m8-local-session",
                    "expires_at": "2099-01-01T00:00:00Z",
                    "artifacts": [
                        {
                            **item,
                            "presigned_url": f"http://127.0.0.1:{self.server.server_port}/artifact/skill",
                            "size": len(artifact),
                            "content_type": "application/gzip",
                        }
                    ],
                }
            else:
                self.send_error(404)
                return
            data = json.dumps(response).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    target = BASE / "install"
    before = read_json(target / "agent.json")
    server = ThreadingHTTPServer(("127.0.0.1", 0), Registry)
    worker = threading.Thread(target=server.serve_forever)
    worker.start()
    try:
        suite.run(
            "install-writeback",
            target,
            [
                "--base-url",
                f"http://127.0.0.1:{server.server_port}",
                "install",
                "@manual/m8-skill@0.1.0",
                "--refresh",
                "--token",
                "m8-fixture-not-a-credential",
                "--quiet",
            ],
        )
    finally:
        server.shutdown()
        server.server_close()
        worker.join()
        write_json(BASE / "runs/install-requests.json", requests)
    after = read_json(target / "agent.json")
    write_json(target / "after.json", after)
    require(
        after["standard"] == before["standard"]
        and after["future_extension"] == before["future_extension"],
        "Install write-back lost opaque metadata",
    )
    require(
        after["tools"] == before["tools"], "Install changed existing Tool dependencies"
    )
    require(
        after["skills"] == [{"name": "@manual/m8-skill", "version": "0.1.0"}],
        "Skill dependency not added",
    )
    installed = list((target / ".agentpm/skills").rglob("SKILL.md"))
    require(
        len(installed) == 1
        and installed[0].read_bytes() == (skill / "SKILL.md").read_bytes(),
        "Skill was not installed",
    )
    require(
        set(requests)
        == {
            "POST /v1/tools/install/resolve",
            "POST /v1/tools/install/init",
            "GET /artifact/skill",
            "POST /v1/tools/install/finalize",
        },
        "Install did not exercise the full local registry flow",
    )


def load_suite():
    metadata = read_json(BASE / "fixture.json")
    require(
        metadata.get("fixture_id") == FIXTURE_ID and metadata.get("setup_complete"),
        "Run setup first",
    )
    require(
        binary_digest(Path(metadata["apm"])) == metadata["sha256"],
        "CLI binary changed. Regenerate fixtures before collecting new evidence.",
    )
    return Suite(metadata)


def summary(suite):
    for section in SECTIONS:
        path = BASE / "runs" / f"section-{section}.json"
        require(path.is_file(), f"Missing section: {section}")
        result = read_json(path)
        require(
            result.get("status") == "passed"
            and set(result.get("commands", [])) == EXPECTED_COMMANDS[section]
            and len(result["commands"]) == len(EXPECTED_COMMANDS[section])
            and result.get("binary_sha256") == suite.metadata["sha256"],
            f"Section is incomplete, failed, or stale: {section}",
        )
        for label in result["commands"]:
            prefix = BASE / "runs" / label
            command = read_json(prefix.with_suffix(".command.json"))
            require(
                command["exit_code"] == command["expected_exit_code"]
                and command["binary_sha256"] == suite.metadata["sha256"],
                f"Command failed: {label}",
            )
            require(
                prefix.with_suffix(".stdout.txt").is_file()
                and prefix.with_suffix(".stderr.txt").is_file(),
                f"Missing command evidence: {label}",
            )
    result = {"status": "passed", "sections": list(SECTIONS), "binary": suite.metadata}
    write_json(BASE / "runs/summary.json", result)
    print(json.dumps(result, indent=2))


def main():
    require(sys.version_info >= (3, 9), "Use Python 3.9 or newer")
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    prepare = sub.add_parser(
        "setup", help="Generate fixtures; never publishes or installs anything"
    )
    prepare.add_argument("--apm", type=Path, default=REPO / "target/debug/agentpm")
    prepare.add_argument(
        "--reset",
        action="store_true",
        help="Discard this helper's previous fixtures and results",
    )
    run = sub.add_parser(
        "run", help="Run one numbered runbook section against the real CLI"
    )
    run.add_argument("section", choices=SECTIONS)
    sub.add_parser("summary", help="Require evidence from all eight sections")
    args = parser.parse_args()
    if args.action == "setup":
        setup(args.apm, args.reset)
        return
    suite = load_suite()
    if args.action == "summary":
        (BASE / "runs/summary.json").unlink(missing_ok=True)
        summary(suite)
        return
    (BASE / "runs/summary.json").unlink(missing_ok=True)
    report = BASE / "runs" / f"section-{args.section}.json"
    write_json(report, {"status": "running", "binary_sha256": suite.metadata["sha256"]})
    try:
        globals()[args.section](suite)
        require(
            set(suite.records) == EXPECTED_COMMANDS[args.section]
            and len(suite.records) == len(EXPECTED_COMMANDS[args.section]),
            f"Incomplete command coverage: {args.section}",
        )
    except Exception:
        write_json(
            report,
            {
                "status": "failed",
                "commands": suite.records,
                "binary_sha256": suite.metadata["sha256"],
            },
        )
        raise
    write_json(
        report,
        {
            "status": "passed",
            "commands": suite.records,
            "binary_sha256": suite.metadata["sha256"],
        },
    )
    print(f"\nPASS: {args.section}. Inspect output and artifacts under {BASE}.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Manual verification failed: {error}", file=sys.stderr)
        sys.exit(1)
