"""Visible M5A dev publications and controlled download faults; no production traffic.

Real publication transport/signing/DB guards reuse the M4A helper. The separate
loopback server is deliberately untrusted fixture data, not the real Registry.
"""

import argparse
import base64
from copy import deepcopy
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import threading
import uuid

from nacl.signing import SigningKey

SPEC = importlib.util.spec_from_file_location(
    "m5a_publication_transport", Path(__file__).parents[1] / "m4a/fixture.py"
)
transport = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(transport)
REPO = transport.REPO
STANDARD = {"id": "agentpm", "version": "1.0.0"}
SEEDS = ("legacy", "echo-v1", "echo-v2", "skill", "agent")
PROJECTS = (
    "simple",
    "graph",
    "local-skill",
    "local-tool",
    "direct-agent",
    "repair",
    "older-response",
    "lock-guards",
)
ERRORS = {
    "unsupported-resolve": "Unsupported APDS standard",
    "unsupported-embedded": "Unsupported APDS standard",
    "malformed-embedded": "Malformed APDS standard declaration",
    "wrong-kind": "Embedded package kind/name/version differs",
    "wrong-name": "Embedded package kind/name/version differs",
    "wrong-version": "Embedded package kind/name/version differs",
    "false-origin": "APDS standard/origin differs from the embedded package manifest",
    "init-drift": "APDS standard/origin differs from the lock/resolve plan",
    "wrong-digest": "sha256 mismatch",
    "future-lock": "Unsupported agent.lock version 5",
    "unknown-field": "unknown field",
    "partial-origin": "must be recorded together",
    "older-cli": "unknown field",
    "frozen-origin": "Local APDS declaration changed",
}
CONTROLS = ("good", "older-response", *tuple(ERRORS)[:9])
read, write = transport.read, transport.write


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def local_root(path):
    root = path.resolve()
    parent = REPO / "cli-m5a-test"
    if root == parent or not root.is_relative_to(parent):
        raise ValueError("Use a run directory under agentpm/cli-m5a-test/")
    return root


def configure():
    # These are child-process aliases for the existing transport's strict guards.
    for suffix in ("BASE", "TOKEN", "DATABASE_URL"):
        os.environ["M4A_" + suffix] = os.getenv("M5A_" + suffix, "")


def manifest(kind, name, version="1.0.0", *, declared=True):
    value = {
        "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json",
        "kind": kind,
        "name": name,
        "version": version,
        "description": "Local manual install/provenance fixture.",
        "readme": "README.md",
    }
    if declared:
        value["standard"] = dict(STANDARD)
    if kind == "tool":
        value.update(
            runtime={"type": "node", "version": "20"},
            entrypoint={"command": "node", "args": ["main.js"]},
            files=["main.js"],
            inputs={"type": "object"},
            outputs={"type": "object"},
        )
    return value


def source(folder, value):
    write(folder / "agent.json", value)
    (folder / "README.md").write_text("# Local M5A fixture\n", encoding="utf-8")
    if value["kind"] == "tool":
        (folder / "main.js").write_text(
            "let input='';process.stdin.setEncoding('utf8');"
            "process.stdin.on('data',v=>input+=v);process.stdin.on('end',()=>"
            "console.log(JSON.stringify({version:"
            + json.dumps(value["version"])
            + ",echo:JSON.parse(input)})));\n",
            encoding="utf-8",
        )
    if value["kind"] == "skill":
        (folder / "SKILL.md").write_text(
            "# Echo procedure\nUse the bound Tool.\n", encoding="utf-8"
        )


def prepare(root, namespace, cli):
    if (root / "session.json").exists():
        raise ValueError("Existing run: resume it, or choose a fresh run directory")
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", namespace):
        raise ValueError("Use a namespace handle without @")
    root.mkdir(parents=True, exist_ok=True)
    root.chmod(0o700)
    prefix = "m5a-" + uuid.uuid4().hex[:10]
    key = SigningKey.generate()
    (root / "author.seed").write_bytes(bytes(key))
    (root / "author.seed").chmod(0o600)
    (root / "author-public-key.txt").write_text(
        base64.b64encode(bytes(key.verify_key)).decode() + "\n"
    )
    values = {
        "legacy": manifest("tool", prefix + "-legacy", declared=False),
        "echo-v1": manifest("tool", prefix + "-echo"),
        "echo-v2": manifest("tool", prefix + "-echo", "2.0.0"),
        "skill": manifest("skill", prefix + "-skill"),
        "agent": manifest("agent", prefix + "-agent"),
    }
    refs = {label: f"@{namespace}/{v['name']}@{v['version']}" for label, v in values.items()}
    values["skill"].update(skill={"entrypoint": "SKILL.md"}, tools=[refs["echo-v1"]])
    values["agent"].update(tools=[refs["echo-v2"]], skills=[refs["skill"]])
    write(root / "session.json", {"namespace": namespace, "prefix": prefix, "refs": refs})
    for label, value in values.items():
        source(root / "cases" / label / "source", value)
        write(
            root / "cases" / label / "case.json",
            {
                "mode": "legacy",
                "archive_protocol": "single-archive HTTP publication",
                "expected_standard_origin": "legacy_inferred" if label == "legacy" else "declared",
                "issue": (
                    "Historical missing selector" if label == "legacy" else "No intentional defect"
                ),
                "expected_http": 200,
                "expected_code": None,
                "expected_status": None if label == "legacy" else "conformant",
                "rule": None,
                "path": None,
            },
        )
        command = [str(cli), "lint"] + (["--legacy"] if label == "legacy" else [])
        command += [str(root / "cases" / label / "source/agent.json")]
        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
        write(
            root / "evidence" / (label + "-source-lint.json"),
            {
                "command": command,
                "exit_code": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            },
        )
        if result.returncode:
            raise ValueError(result.stdout + result.stderr)
        transport.pack(root, label)
    for project, kind, tools, skills in [
        ("simple", "agent", [refs["echo-v2"], refs["legacy"]], []),
        ("graph", "agent", [refs["echo-v2"], refs["legacy"]], [refs["skill"]]),
        ("local-skill", "skill", [refs["echo-v1"]], []),
        ("local-tool", "tool", [], []),
        ("direct-agent", "agent", [], []),
    ]:
        value = manifest(kind, "consumer-" + project)
        if kind in ("agent", "skill"):
            value["tools"] = tools
        if kind == "agent":
            value["skills"] = skills
        if kind == "skill":
            value["skill"] = {"entrypoint": "SKILL.md"}
        source(root / "projects" / project, value)
    controlled(root)
    write(
        root / "evidence/setup.json",
        {
            "cli": str(cli),
            "cli_sha256": digest(cli),
            "repo": str(REPO),
            "commit": subprocess.check_output(
                ["git", "-C", str(REPO), "rev-parse", "HEAD"], text=True
            ).strip(),
            "worktree": subprocess.check_output(
                ["git", "-C", str(REPO), "status", "--short"], text=True
            ),
            "note": "Preparation/lint/archive creation only; no publication or install has run.",
        },
    )
    print("Open:", root)
    print("Real local publication refs:", json.dumps(refs, indent=2))


def controlled(root):
    for label in CONTROLS:
        value = manifest("tool", "controlled")
        if label == "unsupported-embedded":
            value["standard"]["version"] = "9.0.0"
        if label == "malformed-embedded":
            value["standard"] = None
        if label == "wrong-kind":
            value = {
                **manifest("profile", "controlled"),
                "profile": {"identity": {"role": "Fixture reviewer"}},
            }
        if label == "wrong-name":
            value["name"] = "different-name"
        if label == "wrong-version":
            value["version"] = "2.0.0"
        folder = root / "controlled" / label
        source(folder / "source", value)
        data = transport.archive_bytes(folder / "source")
        (folder / "artifact.tar.gz").write_bytes(data)
        item = {
            "kind": "tool",
            "name": "@fixture/controlled",
            "version": "1.0.0",
            "integrity": hashlib.sha256(data).hexdigest(),
            "standard": dict(STANDARD),
            "standard_origin": "declared",
        }
        if label == "unsupported-resolve":
            item["standard"]["version"] = "9.0.0"
        if label == "false-origin":
            item["standard_origin"] = "legacy_inferred"
        if label == "wrong-digest":
            item["integrity"] = "0" * 64
        if label == "older-response":
            item.pop("standard")
            item.pop("standard_origin")
        write(folder / "resolve.json", {"items": [item]})
        artifact = deepcopy(item)
        if label == "init-drift":
            artifact["standard_origin"] = "legacy_inferred"
        write(folder / "init-item.json", artifact)
        write(
            folder / "case.json",
            {
                "defect": None if label in ("good", "older-response") else label,
                "transport": "controlled loopback, not the real API",
                "honest_checksum": label != "wrong-digest",
            },
        )
    for name in ("repair", "older-response", "lock-guards"):
        source(
            root / "projects" / name,
            {**manifest("agent", "consumer-" + name), "tools": ["@fixture/controlled@1.0.0"]},
        )


def publish(root, label):
    if label not in SEEDS:
        raise ValueError("Choose one real seed case")
    if (root / "cases" / label / "receipt.json").exists():
        print("Already published; keeping its immutable source and receipt.")
        return
    transport.init_case(root, label)
    transport.upload(root, label, "any")
    transport.finalize(root, label, "good", 200, None)


def resolve(root, label):
    refs = read(root / "session.json")["refs"]
    selected = [refs[label]] if label in SEEDS else [refs["echo-v2"], refs["legacy"], refs["skill"]]
    items = []
    for ref in selected:
        name, version = ref.rsplit("@", 1)
        items.append({"name": name, "range": version})
    result = transport.registry(
        root,
        "resolve-" + label,
        "/v1/tools/install/resolve",
        {"items": items, "release_format": "agentpm.package.release.v1"},
    )
    catalog = expected_packages(root)
    for item in result["items"]:
        key = f"{item['kind']}:{item['name']}@{item['version']}"
        expected = catalog[key]
        assert (
            item["standard"] == STANDARD and item["standard_origin"] == expected["standard_origin"]
        ), item
        assert item["integrity"] == expected["integrity"], item
        assert "apds_validation" not in item
    if label == "graph":
        assert len(result["items"]) == 4, result
    print(json.dumps(result, indent=2))
    return result


def expected_packages(root):
    result = {}
    namespace = read(root / "session.json")["namespace"]
    for label in SEEDS:
        folder = root / "cases" / label
        value = read(folder / "source/agent.json")
        key = f"{value['kind']}:@{namespace}/{value['name']}@{value['version']}"
        result[key] = {
            "integrity": digest(folder / "any.tar.gz"),
            "standard_origin": "legacy_inferred" if label == "legacy" else "declared",
        }
    result["tool:@fixture/controlled@1.0.0"] = {
        "integrity": digest(root / "controlled/good/artifact.tar.gz"),
        "standard_origin": "declared",
    }
    return result


def check_lock(root, project):
    folder = root / "projects" / project
    lock = read(folder / "agent.lock")
    assert lock["lockfile_version"] == 4
    catalog = expected_packages(root)
    for key, entry in lock["packages"].items():
        assert key == f"{entry['kind']}:{entry['name']}@{entry['version']}"
        assert entry["standard"] == STANDARD
        assert entry["standard_origin"] == catalog[key]["standard_origin"]
        assert entry["integrity"] == catalog[key]["integrity"]
        assert set(entry) == {
            "kind",
            "name",
            "version",
            "integrity",
            "standard",
            "standard_origin",
        }, entry
        namespace, name = entry["name"].removeprefix("@").split("/")
        installed = read(
            folder
            / ".agentpm"
            / (entry["kind"] + "s")
            / namespace
            / name
            / entry["version"]
            / "agent.json"
        )
        if entry["name"] == "@fixture/controlled":
            expected = read(root / "controlled/good/source/agent.json")
        else:
            matches = [read(root / "cases" / label / "source/agent.json") for label in SEEDS]
            expected = next(
                v
                for v in matches
                if v["kind"] == entry["kind"]
                and v["name"] == name
                and v["version"] == entry["version"]
            )
        assert installed == expected, (key, "installed manifest differs from immutable source")
    value = read(folder / "agent.json")
    local = lock["roots"].get("local:" + value["kind"])
    if local:
        assert local["name"] == value["name"] and local["version"] == value["version"]
        assert local["standard"] == STANDARD
        assert local["standard_origin"] == (
            "declared" if "standard" in value else "legacy_inferred"
        )
    if project == "graph":
        assert len(lock["packages"]) == 4
    if project == "simple":
        assert len(lock["packages"]) == 2
    if project == "direct-agent":
        assert len(lock["packages"]) == 4
    if project == "local-tool":
        assert lock["packages"] == {} and local
    if project == "local-skill":
        assert local and len(lock["packages"]) == 1
    write(root / "evidence" / ("lock-" + project + ".json"), lock)
    print(json.dumps(lock, indent=2))
    print("PASS: actual lock identity/origin/digests, local-root source and portable fields")


def state(folder):
    result = {
        "manifest": digest(folder / "agent.json"),
        "lock": digest(folder / "agent.lock") if (folder / "agent.lock").exists() else None,
        "installed": {},
    }
    for kind in ("tools", "skills", "agents", "knowledge", "memory", "profiles", "loops"):
        for path in sorted((folder / ".agentpm" / kind).rglob("*")):
            if path.is_file():
                result["installed"][str(path.relative_to(folder))] = digest(path)
    return result


def snapshot(root, project, label, compare=False):
    path = root / "evidence" / (label + ".snapshot.json")
    actual = state(root / "projects" / project)
    if compare:
        assert (
            read(path) == actual
        ), "Manifest/lock/installed bytes changed; inspect before continuing"
        write(root / "evidence" / (label + "-unchanged.json"), {"passed": True, "snapshot": actual})
        print("PASS: manifest, lock and installed bytes unchanged:", label)
    else:
        write(path, actual)
        print("Saved visible byte-hash snapshot:", path)


def result(root, label, marker=None):
    marker = marker or ERRORS[label]
    saved = read(root / "evidence" / (label + ".result.json"))
    output = (root / "evidence" / (label + ".stdout.txt")).read_text()
    value = json.loads(output)
    assert saved == {"exit_code": 1, "expected_exit_code": 1}, saved
    assert value["schema_version"] == 1 and value["status"] == "error"
    assert marker.lower() in value["error"]["message"].lower(), value
    print("PASS: actual CLI machine failure contains", repr(marker))


def serve(root):
    mutex = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            self.respond()

        def do_POST(self):
            self.respond()

        def respond(self):
            parts = self.path.split("/", 2)
            if len(parts) != 3 or parts[1] not in CONTROLS:
                self.send_error(404)
                return
            case, route = parts[1:]
            folder = root / "controlled" / case
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length)
            with mutex:
                with (root / "evidence/control-requests.jsonl").open("a", encoding="utf-8") as log:
                    log.write(
                        json.dumps(
                            {
                                "case": case,
                                "route": route,
                                "method": self.command,
                                "request": json.loads(body) if body else None,
                            }
                        )
                        + "\n"
                    )
            if route == "v1/tools/install/resolve":
                value = read(folder / "resolve.json")
            elif route == "v1/tools/install/init":
                item = read(folder / "init-item.json")
                item["presigned_url"] = (
                    f"http://127.0.0.1:{self.server.server_port}/{case}/artifact"
                )
                value = {
                    "session_id": "m5a-" + case,
                    "expires_at": "2099-01-01T00:00:00Z",
                    "artifacts": [item],
                }
            elif route == "v1/tools/install/finalize":
                value = {}
            elif route == "artifact":
                value = None
            else:
                self.send_error(404)
                return
            payload = (
                json.dumps(value).encode()
                if value is not None
                else (folder / "artifact.tar.gz").read_bytes()
            )
            self.send_response(200)
            self.send_header(
                "Content-Type", "application/json" if value is not None else "application/gzip"
            )
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    write(
        root / "control-server.json",
        {
            "base_url": f"http://127.0.0.1:{server.server_port}",
            "pid": os.getpid(),
            "kind": "controlled fixture, not real Registry",
        },
    )
    print("Controlled faults only:", read(root / "control-server.json"), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def mutate(root, project, change):
    folder = root / "projects" / project
    if change == "rename-local":
        path = folder / "agent.json"
        value = read(path)
        value["name"] = "renamed-consumer"
        value["version"] = "2.0.0"
        write(path, value)
    elif change == "declared-local":
        path = folder / "agent.json"
        value = read(path)
        value["standard"] = dict(STANDARD)
        write(path, value)
    elif change == "legacy-local":
        path = folder / "agent.json"
        value = read(path)
        value.pop("standard")
        write(path, value)
    elif change == "installed-selector":
        path = folder / ".agentpm/tools/fixture/controlled/1.0.0/agent.json"
        value = read(path)
        value.pop("standard")
        write(path, value)
    elif change == "cache":
        # Earlier rejected archives may remain hash-correct in the cache. Target
        # this project's original good archive, not an arbitrary cached fault.
        expected = digest(root / "controlled/good/artifact.tar.gz")
        paths = [
            p
            for p in (folder / ".agentpm/cache").iterdir()
            if p.is_file() and digest(p) == expected
        ]
        assert len(paths) == 1, paths
        path = paths[0]
        path.write_bytes(b"deliberately corrupted manual-test cache")
    else:
        path = folder / "agent.lock"
        backup = folder / "original-lock.json"
        if not backup.exists():
            shutil.copyfile(path, backup)
        value = read(backup)
        if change == "future-lock":
            value["lockfile_version"] = 5
        elif change == "unknown-field":
            value["future_required_state"] = True
        elif change == "partial-origin":
            next(iter(value["packages"].values())).pop("standard_origin")
        elif change != "restore-lock":
            raise ValueError("Unknown change")
        write(path, value)
    print("Intentional fixture mutation:", change, "in", path)


def audit(root):
    required = {
        "version": 0,
        "whoami": 0,
        **{"publish-" + label: 0 for label in SEEDS},
        **{
            label: 0
            for label in (
                "install-simple",
                "regenerate-simple",
                "frozen-simple",
                "run-v1",
                "run-v2",
                "install-graph",
                "install-skill",
                "install-local-tool",
                "install-agent",
                "refresh-local",
                "infer-local",
                "restore-local",
                "control-good",
                "older-response",
                "repair-cache",
                "repair-selector",
                "guards-good",
            )
        },
        **{label: 1 for label in ERRORS},
    }
    for label, expected in required.items():
        actual = read(root / "evidence" / (label + ".result.json"))
        assert actual == {"exit_code": expected, "expected_exit_code": expected}, (label, actual)
        if expected:
            result(root, label)
    for label, expected_version in (("run-v1", "1.0.0"), ("run-v2", "2.0.0")):
        actual = read(root / "evidence" / (label + ".stdout.txt"))
        assert actual["status"] == "success" and actual["output"]["version"] == expected_version
        assert actual["tool"]["version"] == expected_version
    requests = [
        json.loads(line)
        for line in (root / "evidence/control-requests.jsonl").read_text().splitlines()
    ]
    for case in CONTROLS[2:]:
        routes = [event["route"] for event in requests if event["case"] == case]
        assert routes and "v1/tools/install/finalize" not in routes, (case, routes)
        if case == "unsupported-resolve":
            assert set(routes) == {"v1/tools/install/resolve"}
        if case == "init-drift":
            assert "artifact" not in routes
    for project in PROJECTS:
        check_lock(root, project)
    # Later intentional local-root edits change simple's manifest/lock. Its
    # frozen byte comparison is retained as a separate observed check record.
    for label in (
        "regenerate-simple",
        "frozen-simple",
        "frozen-origin",
        "selector-repair",
        "older-cli",
        "future-lock",
        "unknown-field",
        "partial-origin",
    ):
        proof = read(root / "evidence" / (label + "-unchanged.json"))
        assert proof["passed"] and proof["snapshot"] == read(
            root / "evidence" / (label + ".snapshot.json")
        ), label
    snapshot(root, "repair", "failures", compare=True)
    for label in SEEDS:
        transport.inspect_case(root, label, "undeclared" if label == "legacy" else "conformant")
    assert (
        digest(Path(read(root / "evidence/setup.json")["cli"]))
        == read(root / "evidence/setup.json")["cli_sha256"]
    ), "CLI changed during the run"
    write(
        root / "evidence/summary.json",
        {
            "status": "pass",
            "checked_command_results": len(required),
            "operator_signoff": False,
            "hosted_ci_complete": False,
            "scope": "local API publication/resolve/install plus separate controlled download faults",
        },
    )
    print(
        "PASS: saved outcomes and actual state. Operator acceptance and later hosted CI remain separate."
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=[
            "prepare",
            "preflight",
            "publish",
            "inspect-db",
            "resolve",
            "check-lock",
            "snapshot",
            "unchanged",
            "check-result",
            "serve",
            "mutate",
            "describe",
            "audit",
        ],
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("name", nargs="?")
    parser.add_argument("value", nargs="?")
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--cli", type=Path, default=REPO / "target/debug/agentpm")
    args = parser.parse_args()
    root = local_root(args.root)
    configure()
    if (
        args.action in ("check-lock", "snapshot", "unchanged", "mutate")
        and args.name not in PROJECTS
    ):
        parser.error("Choose one prepared project name")
    if args.action in ("publish", "inspect-db", "resolve") and args.name not in (*SEEDS, "graph"):
        parser.error("Choose a prepared publication/graph name")
    if args.action == "describe" and args.name not in (*SEEDS, *CONTROLS):
        parser.error("Choose a prepared case")
    if args.action in ("snapshot", "unchanged") and not re.fullmatch(
        r"[a-z0-9-]+", args.value or ""
    ):
        parser.error("Use a simple snapshot label")
    if args.action == "prepare":
        prepare(root, args.namespace, args.cli.resolve())
    elif args.action == "preflight":
        print(json.dumps(transport.namespace_info(root), indent=2, default=str))
    elif args.action == "publish":
        publish(root, args.name)
    elif args.action == "inspect-db":
        transport.inspect_case(
            root, args.name, "undeclared" if args.name == "legacy" else "conformant"
        )
    elif args.action == "resolve":
        resolve(root, args.name)
    elif args.action == "check-lock":
        check_lock(root, args.name)
    elif args.action in ("snapshot", "unchanged"):
        snapshot(root, args.name, args.value, args.action == "unchanged")
    elif args.action == "check-result":
        result(root, args.name, args.value)
    elif args.action == "serve":
        serve(root)
    elif args.action == "audit":
        audit(root)
    elif args.action == "mutate":
        mutate(root, args.name, args.value)
    elif args.action == "describe":
        folder = root / ("cases" if args.name in SEEDS else "controlled") / args.name
        for name in ("case.json", "source/agent.json", "resolve.json", "request.json"):
            if (folder / name).exists():
                print(name, json.dumps(read(folder / name), indent=2))


if __name__ == "__main__":
    main()
