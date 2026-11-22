"""Visible M5B local publications, replay checks and isolated download faults."""

import argparse
import base64
from copy import deepcopy
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import shlex
import subprocess
import threading
import tarfile
import uuid

from nacl.signing import SigningKey

SPEC = importlib.util.spec_from_file_location(
    "m5b_previous", Path(__file__).parents[1] / "m5a/fixture.py"
)
previous = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(previous)
transport = previous.transport
REPO, STANDARD = previous.REPO, previous.STANDARD
read, write, digest = previous.read, previous.write, previous.digest
REUSED = ("echo-v1", "echo-v2", "skill", "legacy")
SEEDS = ("knowledge", "memory", "profile", "loop", "agent", "empty-agent")
GRAPH = ("echo-v1", "echo-v2", "skill", "knowledge", "memory", "profile", "loop", "agent")
LAYOUT = {
    "tool": "tools",
    "agent": "agents",
    "skill": "skills",
    "knowledge": "knowledge",
    "memory": "memory",
    "profile": "profiles",
    "loop": "loops",
}
PROJECTS = {
    "direct-agent": GRAPH,
    "direct-skill": ("skill", "echo-v1"),
    "local-agent": (*GRAPH[:-1], "legacy"),
    "local-skill": ("echo-v1",),
    "workspace": GRAPH,
    "empty-local": (),
    "empty-workspace": (),
    "empty-direct": ("empty-agent",),
    "guards": GRAPH[:-1],
    "faults": GRAPH,
    "old-tool": ("echo-v1",),
    "fresh": (),
}
ERRORS = {
    "missing-lock": "--frozen requires agent.lock",
    "changed-range": "No locked tool",
    "removed-dependency": "Locked dependency edges differ",
    "local-identity": "Local root identity differs",
    "wrong-kind": "Locked package key differs",
    "missing-transitive": "No locked tool",
    "stale-edge": "No locked tool",
    "pin-digest": "differs from the lock/resolve plan",
    "root-origin": "APDS standard/origin differs from the embedded package manifest",
    "init-digest": "differs from the lock/resolve plan",
    "corrupt-download": "sha256 mismatch",
    "registry-offline": "Controlled Registry unavailable",
}
SUCCESS = (
    "version",
    "whoami",
    *["publish-" + name for name in SEEDS],
    "agent-install",
    "agent-replay",
    "agent-replay-again",
    "skill-install",
    "skill-replay",
    "local-install",
    "local-replay",
    "local-regenerate",
    "local-skill-install",
    "local-skill-replay",
    "workspace-install",
    "workspace-replay",
    "empty-local-install",
    "empty-local-again",
    "empty-local-frozen",
    "empty-workspace-install",
    "empty-workspace-frozen",
    "empty-direct-install",
    "empty-direct-again",
    "empty-direct-frozen",
    "guards-install",
    "control-install",
    "control-frozen",
    "control-clean",
    "cache-repair",
    "refresh-repair",
    "old-tool-install",
    "old-v2-replay",
    "old-v3-replay",
    "run-v1",
    "run-v2",
)
PROOFS = (
    "agent-replay",
    "agent-replay-again",
    "skill-replay",
    "local-replay",
    "local-regenerate",
    "local-skill-replay",
    "workspace-replay",
    "empty-local-again",
    "empty-local-frozen",
    "empty-workspace-frozen",
    "empty-direct-again",
    "empty-direct-frozen",
    "control-frozen",
    "control-clean",
    "cache-repair",
    "refresh-repair",
    "old-v2-replay",
    "old-v3-replay",
    *ERRORS,
)


def local_root(path):
    root = path.resolve()
    parent = REPO / "cli-m5b-test"
    if root == parent or not root.is_relative_to(parent):
        raise ValueError("Use a run directory under agentpm/cli-m5b-test/")
    return root


def project(root, name):
    if name not in PROJECTS:
        raise ValueError("Choose a prepared project")
    folder = (root / "projects" / name).resolve()
    if not folder.is_relative_to(root / "projects"):
        raise ValueError("Project escaped the owned run directory")
    return folder


def configure():
    for suffix in ("BASE", "TOKEN", "DATABASE_URL"):
        os.environ["M4A_" + suffix] = os.getenv("M5B_" + suffix, "")


def command(root, cli, label, *args, cwd=None):
    result = subprocess.run(
        [str(cli), *map(str, args)],
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=120,
        env={**os.environ, "AGENTPM_TELEMETRY": "0"},
    )
    write(
        root / "evidence" / (label + ".json"),
        {
            "command": [str(cli), *map(str, args)],
            "exit_code": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        },
    )
    if result.returncode:
        raise ValueError(result.stdout + result.stderr)
    return result


def prepare(root, namespace, cli, reuse):
    if (root / "session.json").exists():
        raise ValueError("Existing run: resume it or choose a fresh run directory")
    previous.local_root(reuse)
    old = read(reuse / "session.json")
    if namespace != old["namespace"] or not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", namespace):
        raise ValueError("Use the same development namespace as the M5A seed run")
    root.mkdir(parents=True, exist_ok=True)
    root.chmod(0o700)
    prefix = "m5b-" + uuid.uuid4().hex[:10]
    signer = SigningKey.generate()
    (root / "author.seed").write_bytes(bytes(signer))
    (root / "author.seed").chmod(0o600)
    (root / "author-public-key.txt").write_text(
        base64.b64encode(bytes(signer.verify_key)).decode() + "\n"
    )
    refs = {name: old["refs"][name] for name in REUSED}
    for name in REUSED:
        source = reuse / "cases" / name
        target = root / "reused" / name
        shutil.copytree(source / "source", target / "source")
        for filename in ("any.tar.gz", "finalize-good.json"):
            shutil.copyfile(source / filename, target / filename)
    values = {}
    for name in SEEDS:
        kind = "agent" if name == "empty-agent" else name
        snapshot = read(REPO / f"crates/agentpm-cli/tests/fixtures/scaffolds/{kind}.snapshot.json")
        folder = root / "cases" / name / "source"
        folder.mkdir(parents=True)
        for filename, content in snapshot.items():
            if isinstance(content, str):
                path = folder / filename
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
        value = snapshot["agent.json"]
        value.update(name=prefix + "-" + name, version="1.0.0")
        values[name] = value
        refs[name] = f"@{namespace}/{value['name']}@1.0.0"
    values["agent"].update(
        tools=[refs["echo-v2"]],
        skills=[refs["skill"]],
        knowledge=[refs["knowledge"]],
        memory=[refs["memory"]],
        profiles=[refs["profile"]],
        loop=refs["loop"],
    )
    write(
        root / "session.json",
        {
            "namespace": namespace,
            "prefix": prefix,
            "refs": refs,
            "reused_run": str(reuse),
            "new_publications": list(SEEDS),
            "local_enforcement": "keep enabled",
        },
    )
    for name, value in values.items():
        folder = root / "cases" / name
        write(folder / "source/agent.json", value)
        if name in ("knowledge", "memory"):
            command(root, cli, name + "-build", name, "build", cwd=folder / "source")
        command(root, cli, name + "-lint", "lint", folder / "source/agent.json")
        write(
            folder / "case.json",
            {
                "mode": "legacy",
                "archive_protocol": "single-archive HTTP publication",
                "issue": "No intentional defect; declared M5B seed",
                "expected_http": 200,
                "expected_code": None,
                "expected_status": "conformant",
                "rule": None,
                "path": None,
            },
        )
        transport.pack(root, name)
    catalog = {}
    for name in (*REUSED, *SEEDS):
        folder = root / ("reused" if name in REUSED else "cases") / name
        value = read(folder / "source/agent.json")
        catalog[name] = {
            "kind": value["kind"],
            "name": refs[name].rsplit("@", 1)[0],
            "version": value["version"],
            "integrity": digest(folder / "any.tar.gz"),
            "standard": dict(STANDARD),
            "standard_origin": "legacy_inferred" if name == "legacy" else "declared",
            "folder": str(folder.relative_to(root)),
        }
    write(root / "catalog.json", catalog)
    for name in PROJECTS:
        project(root, name).mkdir(parents=True)
    for name, source_kind in (
        ("local-agent", "agent"),
        ("guards", "agent"),
        ("local-skill", "skill"),
        ("empty-local", "agent"),
        ("empty-workspace", "agent"),
        ("workspace", "agent"),
        ("old-tool", "agent"),
        ("fresh", "agent"),
    ):
        value = previous.manifest(source_kind, "consumer-" + name)
        if source_kind == "skill":
            value.update(skill={"entrypoint": "SKILL.md"}, tools=[refs["echo-v1"]])
        elif name in ("local-agent", "guards"):
            value.update(
                {
                    k: deepcopy(v)
                    for k, v in values["agent"].items()
                    if k in ("tools", "skills", "knowledge", "memory", "profiles", "loop")
                }
            )
            if name == "local-agent":
                value["tools"].append(refs["legacy"])
        else:
            value.update(
                tools=[refs["echo-v1"]] if name in ("workspace", "old-tool") else [], skills=[]
            )
        previous.source(project(root, name), value)
        (project(root, name) / "README.md").write_text(
            "# Local M5B consumer fixture\n", encoding="utf-8"
        )
        command(root, cli, name + "-lint", "lint", project(root, name) / "agent.json")
    for name in ("workspace", "empty-workspace"):
        second = previous.manifest("agent", "second-" + name)
        second.update(tools=[refs["echo-v2"]] if name == "workspace" else [], skills=[])
        write(project(root, name) / "agents/second.agent.json", second)
        command(
            root,
            cli,
            name + "-second-lint",
            "lint",
            project(root, name) / "agents/second.agent.json",
        )
        write(
            project(root, name) / "agentpm.workspace.json",
            {
                "schema_version": 1,
                "manifests": ["agent.json", "agents/second.agent.json"],
                "package_roots": (
                    {"agents": [{"name": catalog["agent"]["name"], "version": "1.0.0"}]}
                    if name == "workspace"
                    else {}
                ),
            },
        )
    for name in ("good", "tool", "init-digest", "corrupt-download", "registry-offline"):
        folder = root / "controlled" / name
        items = [
            item(catalog[label]) for label in (("echo-v1",) if name == "tool" else tuple(catalog))
        ]
        if name == "init-digest":
            for entry in items:
                entry["integrity"] = "0" * 64
        if name == "corrupt-download":
            for label, entry in catalog.items():
                data = bytearray((root / entry["folder"] / "any.tar.gz").read_bytes())
                data[len(data) // 2] ^= 1
                path = folder / "archives" / (label + ".tar.gz")
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        write(folder / "init-items.json", items)
        write(
            folder / "case.json",
            {
                "surface": "separate loopback, no real Registry writes",
                "defect": name if name not in ("good", "tool") else None,
                "response_status": 503 if name == "registry-offline" else 200,
                "archive_folder": (
                    "archives/" if name == "corrupt-download" else "catalog source folders"
                ),
            },
        )
    write(
        root / "evidence/setup.json",
        {
            "cli": str(cli),
            "cli_sha256": digest(cli),
            "repo_revision": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=REPO, text=True
            ).strip(),
            "note": "Prepared and CLI-built/linted/signed only; no API/DB/S3 writes",
        },
    )
    defaults = {
        "M5B_REPO": str(REPO),
        "M5B_ROOT": str(root),
        "M5B_API": str(REPO.parent / "agentpm-api"),
        "M5B_PYTHON": os.sys.executable,
        "M5B_FIXTURE": str(Path(__file__).resolve()),
        "M5B_APM": str(cli),
        "M5B_BASE": "http://127.0.0.1:5000",
        "M5B_NAMESPACE": namespace,
        "M5B_DATABASE_URL": "postgresql://postgres:postgres@127.0.0.1:5433/postgres",
    }
    (root / "paths.sh").write_text(
        "".join(
            "export "
            + name
            + "="
            + shlex.quote(
                os.getenv(name, value) if name in ("M5B_BASE", "M5B_DATABASE_URL") else value
            )
            + "\n"
            for name, value in defaults.items()
        )
        + 'source "$M5B_ROOT/functions.sh"\n'
    )
    shutil.copyfile(Path(__file__).with_name("shell.sh"), root / "functions.sh")
    print("Terminal B: source " + shlex.quote(str(root / "paths.sh")))
    print("Open workspace:", root)
    print("New declarations to publish (six):", json.dumps({n: refs[n] for n in SEEDS}, indent=2))
    print(
        "Reused M5A identities (not republished):",
        json.dumps({n: refs[n] for n in REUSED}, indent=2),
    )


def item(entry):
    return {key: value for key, value in entry.items() if key != "folder"}


def preflight(root):
    print(json.dumps(transport.namespace_info(root), indent=2, default=str))
    catalog = read(root / "catalog.json")
    response = transport.registry(
        root,
        "reused-resolve",
        "/v1/tools/install/resolve",
        {
            "items": [
                {"name": catalog[label]["name"], "range": catalog[label]["version"]}
                for label in REUSED
            ],
            "release_format": "agentpm.package.release.v1",
        },
    )
    actual = {key(entry): entry for entry in response["items"]}
    assert set(actual) == {key(catalog[label]) for label in REUSED}, response
    for label in REUSED:
        expected = catalog[label]
        observed = actual[key(expected)]
        assert all(
            observed[field] == expected[field]
            for field in ("integrity", "standard", "standard_origin")
        ), observed
    print(
        "PASS: all four reused identities still resolve with their original digest and provenance"
    )


def publish(root, name):
    if name not in SEEDS:
        raise ValueError("Publish only the six new declared cases")
    if (root / "cases" / name / "receipt.json").exists():
        print("Already published; preserving receipt and immutable source.")
        return
    transport.init_case(root, name)
    transport.upload(root, name, "any")
    transport.finalize(root, name, "good", 200, None)


def key(entry):
    return f"{entry['kind']}:{entry['name']}@{entry['version']}"


def installed(folder, entry):
    namespace, name = entry["name"].removeprefix("@").split("/")
    return folder / ".agentpm" / LAYOUT[entry["kind"]] / namespace / name / entry["version"]


def check_lock(root, name):
    folder = project(root, name)
    lock, catalog = read(folder / "agent.lock"), read(root / "catalog.json")
    expected = {key(catalog[label]): catalog[label] for label in PROJECTS[name]}
    assert set(lock["packages"]) == set(expected), (name, set(lock["packages"]), set(expected))
    assert (
        lock["lockfile_version"] == 4
    ), "Normal/restored fixture locks must retain v4 APDS metadata"
    for identity, entry in lock["packages"].items():
        wanted = expected[identity]
        assert key(entry) == identity and entry["integrity"] == wanted["integrity"], entry
        assert not (
            set(entry) - {"kind", "name", "version", "integrity", "standard", "standard_origin"}
        ), entry
        assert (
            entry["standard"] == STANDARD and entry["standard_origin"] == wanted["standard_origin"]
        ), entry
        source = root / wanted["folder"] / "source"
        archive = root / wanted["folder"] / "any.tar.gz"
        assert digest(archive) == wanted["integrity"], (identity, "immutable archive changed")
        destination = installed(folder, wanted)
        with tarfile.open(archive, "r:gz") as payload:
            files = {
                member.name: payload.extractfile(member).read()
                for member in payload
                if member.isfile()
            }
        source_paths = {p.relative_to(source).as_posix() for p in source.rglob("*") if p.is_file()}
        assert set(files) == source_paths, (identity, "immutable source file set changed")
        for relative, data in files.items():
            assert (source / relative).read_bytes() == data, (
                identity,
                "immutable source changed",
                relative,
            )
            assert (destination / relative).read_bytes() == data, (
                identity,
                "installed payload differs",
                relative,
            )
    expected_roots = set()
    for path in [folder / "agent.json", folder / "agents/second.agent.json"]:
        if path.exists():
            value = read(path)
            rel = path.relative_to(folder).as_posix()
            root_key = "local:" + value["kind"] + (":" + rel if rel != "agent.json" else "")
            expected_roots.add(root_key)
            local = lock["roots"][root_key]
            check_edges(local, value)
            assert local["name"] == value["name"] and local["version"] == value["version"]
            assert local["standard"] == STANDARD and local["standard_origin"] == "declared"
    root_label = {
        "direct-agent": "agent",
        "faults": "agent",
        "workspace": "agent",
        "direct-skill": "skill",
        "empty-direct": "empty-agent",
    }.get(name)
    if root_label:
        root_key = key(catalog[root_label])
        expected_roots.add(root_key)
        check_edges(
            lock["roots"][root_key],
            read(root / catalog[root_label]["folder"] / "source/agent.json"),
        )
    assert set(lock["roots"]) == expected_roots, (name, "missing/unexpected roots", lock["roots"])
    write(root / "evidence" / ("lock-" + name + ".json"), lock)
    print(json.dumps(lock, indent=2))
    print("PASS: exact expected graph, metadata, digests and every installed payload file:", name)


def check_edges(locked, manifest):
    for field, kind in (
        ("tools", "tool"),
        ("skills", "skill"),
        ("knowledge", "knowledge"),
        ("memory", "memory"),
        ("profiles", "profile"),
    ):
        expected = [kind + ":" + ref for ref in manifest.get(field, [])]
        assert sorted(locked.get(field, [])) == sorted(expected), (
            "wrong exact root edges",
            field,
            locked,
            expected,
        )
    expected = "loop:" + manifest["loop"] if isinstance(manifest.get("loop"), str) else None
    assert locked.get("loop") == expected, ("wrong exact Loop edge", locked, expected)


def state(folder):
    value = {
        "sources": {},
        "lock": digest(folder / "agent.lock") if (folder / "agent.lock").exists() else None,
        "installed": {},
    }
    for path in sorted(folder.rglob("*")):
        rel = path.relative_to(folder)
        if path.is_file() and ".agentpm" not in rel.parts and rel.as_posix() != "agent.lock":
            value["sources"][rel.as_posix()] = digest(path)
    for layout in LAYOUT.values():
        for path in sorted((folder / ".agentpm" / layout).rglob("*")):
            if path.is_file():
                value["installed"][path.relative_to(folder).as_posix()] = digest(path)
    return value


def snapshot(root, name, label, compare=False):
    actual, path = state(project(root, name)), root / "evidence" / (label + ".snapshot.json")
    if compare:
        assert actual == read(path), "Source, lock or installed bytes changed: " + label
        write(root / "evidence" / (label + "-unchanged.json"), {"passed": True, "snapshot": actual})
        print("PASS: source/lock/installed bytes unchanged:", label)
    else:
        write(path, actual)
        print("Saved byte hashes:", path)


def clean(root, name):
    target = project(root, name) / ".agentpm"
    if not target.resolve().is_relative_to(project(root, name)):
        raise ValueError("Refusing to clean outside this owned test project")
    if target.exists():
        shutil.rmtree(target)
    print("Removed only installed/cache state:", target)
    print("Kept authored source and agent.lock. Run the displayed frozen install next.")


def mutate(root, name, change):
    folder = project(root, name)
    backup = root / "backups" / name
    if not backup.exists():
        backup.mkdir(parents=True)
        for filename in ("agent.json", "agent.lock"):
            if (folder / filename).exists():
                shutil.copyfile(folder / filename, backup / filename)
    if change == "restore":
        for filename in ("agent.json", "agent.lock"):
            if (backup / filename).exists():
                shutil.copyfile(backup / filename, folder / filename)
            elif (folder / filename).exists():
                (folder / filename).unlink()
        print("Restored original fixture source and lock:", folder)
        return
    catalog = read(root / "catalog.json")
    if change in ("changed-range", "removed-dependency", "local-identity"):
        value = read(backup / "agent.json")
        if change == "changed-range":
            value["tools"] = [catalog["echo-v2"]["name"] + "@^3"]
        elif change == "removed-dependency":
            value["tools"] = []
        else:
            value.update(name="renamed-consumer", version="2.0.0")
        path = folder / "agent.json"
    elif change in ("cache", "payload"):
        if change == "cache":
            matches = [
                p
                for p in (folder / ".agentpm/cache").iterdir()
                if p.is_file() and digest(p) == catalog["echo-v1"]["integrity"]
            ]
            assert len(matches) == 1, matches
            path = matches[0]
        else:
            path = installed(folder, catalog["echo-v1"]) / "main.js"
        path.write_bytes(b"deliberately modified owned M5B fixture\n")
        print("Intentional", change, "corruption:", path)
        return
    else:
        path = folder / "agent.lock"
        value = read(backup / "agent.lock")
        echo = key(catalog["echo-v1"])
        if change == "missing-transitive":
            del value["packages"][echo]
        elif change == "wrong-kind":
            value["packages"][echo]["kind"] = "skill"
        elif change == "pin-digest":
            value["packages"][echo]["integrity"] = "0" * 64
        elif change == "stale-edge":
            value["roots"][key(catalog["agent"])]["tools"] = [echo]
        elif change == "root-origin":
            value["roots"][key(catalog["agent"])].update(
                standard=dict(STANDARD), standard_origin="legacy_inferred"
            )
        elif change in ("old-v2", "old-v3"):
            value["lockfile_version"] = int(change[-1])
            for collection in ("packages", "roots"):
                for entry in value.get(collection, {}).values():
                    entry.pop("standard", None)
                    entry.pop("standard_origin", None)
        else:
            raise ValueError("Unknown fixture mutation")
    write(path, value)
    print("Intentional fixture change:", change, "at", path)


def begin(root, label):
    path = root / "evidence/control-requests.jsonl"
    offset = len(path.read_text().splitlines()) if path.exists() else 0
    write(root / "evidence" / (label + ".begin.json"), {"request_offset": offset})


def result(root, label):
    value = read(root / "evidence" / (label + ".stdout.txt"))
    saved = read(root / "evidence" / (label + ".result.json"))
    assert saved == {"exit_code": 1, "expected_exit_code": 1}, saved
    assert value["schema_version"] == 1 and value["status"] == "error", value
    assert ERRORS[label].lower() in value["error"]["message"].lower(), value
    print("PASS: actual intended error:", value["error"]["message"])


def traffic(root, label, gets=None, failed=False, local=False):
    path = root / "evidence/control-requests.jsonl"
    entries = [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    offset = read(root / "evidence" / (label + ".begin.json"))["request_offset"]
    events = entries[offset:]
    routes = [e["route"] for e in events]
    assert "v1/tools/install/resolve" not in routes, routes
    if local:
        assert not events, events
    elif failed:
        assert routes.count("v1/tools/install/init") == 1, routes
        assert "v1/tools/install/finalize" not in routes, routes
    else:
        assert routes.count("v1/tools/install/finalize") == 1, routes
    if gets is not None:
        assert sum(route.startswith("archive/") for route in routes) == gets, routes
    write(
        root / "evidence" / (label + ".traffic.json"),
        {
            "passed": True,
            "events": events,
            "start_offset": offset,
            "end_offset": offset + len(events),
        },
    )
    print("PASS: captured requests:", routes)


def serve(root):
    catalog = read(root / "catalog.json")
    by_key = {key(entry): (label, entry) for label, entry in catalog.items()}
    lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            self.respond()

        def do_POST(self):
            self.respond()

        def respond(self):
            parts = self.path.split("/", 2)
            if len(parts) != 3 or parts[1] not in (
                "good",
                "tool",
                "init-digest",
                "corrupt-download",
                "registry-offline",
            ):
                self.send_error(404)
                return
            case, route = parts[1:]
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            request = json.loads(body) if body else {}
            with lock:
                with (root / "evidence/control-requests.jsonl").open("a", encoding="utf-8") as log:
                    log.write(json.dumps({"case": case, "route": route, "request": request}) + "\n")
            status = 200
            if case == "registry-offline":
                status, value = 503, {
                    "code": "fixture_unavailable",
                    "message": "Controlled Registry unavailable",
                }
            elif route == "v1/tools/install/resolve":
                pending, selected = list(request["items"]), {}
                while pending:
                    req = pending.pop()
                    matches = [
                        e
                        for e in catalog.values()
                        if e["name"] == req["name"] and e["version"] == req["range"]
                    ]
                    if len(matches) != 1:
                        self.send_error(400, "Controlled resolver accepts exact fixture refs only")
                        return
                    entry = matches[0]
                    if key(entry) in selected:
                        continue
                    selected[key(entry)] = item(entry)
                    manifest = read(root / entry["folder"] / "source/agent.json")
                    for field in ("tools", "skills", "knowledge", "memory", "profiles", "loop"):
                        refs = (
                            [manifest[field]]
                            if field == "loop" and field in manifest
                            else manifest.get(field, [])
                        )
                        if not isinstance(refs, list):
                            continue
                        for ref in refs:
                            if isinstance(ref, str):
                                name, version = ref.rsplit("@", 1)
                                pending.append({"name": name, "range": version})
                value = {"items": list(selected.values())}
            elif route == "v1/tools/install/init":
                advertised = {
                    key(entry): entry
                    for entry in read(root / "controlled" / case / "init-items.json")
                }
                artifacts = []
                for requested in request["items"]:
                    label, entry = by_key[key(requested)]
                    folder = root / entry["folder"]
                    author = read(folder / "finalize-good.json")["author_signatures"][0]
                    statement = json.dumps(
                        author["statement_json"], sort_keys=True, separators=(",", ":")
                    ).encode()
                    author.update(
                        signer_id="controlled-fixture",
                        is_active=True,
                        statement_bytes_b64=base64.b64encode(statement).decode(),
                    )
                    artifact = {
                        **deepcopy(advertised[key(requested)]),
                        "size": (folder / "any.tar.gz").stat().st_size,
                        "presigned_url": f"http://127.0.0.1:{self.server.server_port}/{case}/archive/{label}",
                        "verification": {
                            "mode": "required",
                            "min_author_signatures": 1,
                            "manifest": read(folder / "source/agent.json"),
                            "authors": [author],
                            "registry_attestation": None,
                            "registry_keyset_path": "/v1/registry/signing-keys",
                        },
                    }
                    artifacts.append(artifact)
                value = {
                    "session_id": "m5b-fixture",
                    "expires_at": "2099-01-01T00:00:00Z",
                    "artifacts": artifacts,
                }
            elif route.startswith("archive/"):
                label = route.split("/", 1)[1]
                entry = catalog[label]
                path = root / "controlled" / case / "archives" / (label + ".tar.gz")
                value = (
                    path if path.exists() else root / entry["folder"] / "any.tar.gz"
                ).read_bytes()
            elif route == "v1/tools/install/finalize":
                value = {}
            else:
                self.send_error(404)
                return
            payload = value if isinstance(value, bytes) else json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    write(
        root / "control-server.json",
        {
            "base_url": f"http://127.0.0.1:{server.server_port}",
            "pid": os.getpid(),
            "surface": "controlled loopback",
        },
    )
    print("Controlled fixture server:", server.server_address, flush=True)
    try:
        server.serve_forever()
    finally:
        server.server_close()


def audit(root):
    required = {**{label: 0 for label in SUCCESS}, **{label: 1 for label in ERRORS}}
    for label, expected in required.items():
        value = read(root / "evidence" / (label + ".result.json"))
        assert value == {"exit_code": expected, "expected_exit_code": expected}, (label, value)
        if expected:
            result(root, label)
        elif label not in ("version", "whoami", *["publish-" + n for n in SEEDS]):
            output = read(root / "evidence" / (label + ".stdout.txt"))
            assert output["status"] == "success", (label, output)
    for label in PROOFS:
        proof = read(root / "evidence" / (label + "-unchanged.json"))
        assert proof["passed"] and proof["snapshot"] == read(
            root / "evidence" / (label + ".snapshot.json")
        ), label
    for name in PROJECTS:
        if name != "fresh":
            check_lock(root, name)
    for label, version in (("run-v1", "1.0.0"), ("run-v2", "2.0.0")):
        output = read(root / "evidence" / (label + ".stdout.txt"))
        assert output["output"]["version"] == version, output
    for label in (
        *ERRORS,
        "control-frozen",
        "control-clean",
        "cache-repair",
        "refresh-repair",
        "old-v2-replay",
        "old-v3-replay",
    ):
        proof = read(root / "evidence" / (label + ".traffic.json"))
        raw = [
            json.loads(line)
            for line in (root / "evidence/control-requests.jsonl").read_text().splitlines()
        ]
        assert (
            proof["passed"] and proof["events"] == raw[proof["start_offset"] : proof["end_offset"]]
        ), label
    for name in SEEDS:
        transport.inspect_case(root, name, "conformant")
    setup = read(root / "evidence/setup.json")
    assert digest(Path(setup["cli"])) == setup["cli_sha256"], "Reviewed CLI changed during this run"
    write(
        root / "evidence/summary.json",
        {
            "status": "pass",
            "checked_command_results": len(required),
            "operator_signoff": False,
            "hosted_ci_complete": False,
            "scope": "real local Registry graph replay plus isolated controlled faults; no production requests",
        },
    )
    print("PASS:", len(required), "saved command expectations and actual byte/graph checks.")
    print("Operator acceptance and hosted CI remain separately confirmed.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=[
            "prepare",
            "preflight",
            "publish",
            "inspect-db",
            "check-lock",
            "snapshot",
            "unchanged",
            "clean",
            "mutate",
            "begin",
            "check-result",
            "traffic",
            "serve",
            "audit",
            "describe",
        ],
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("name", nargs="?")
    parser.add_argument("value", nargs="?")
    parser.add_argument("--namespace", default="zack")
    parser.add_argument("--cli", type=Path, default=REPO / "target/debug/agentpm")
    parser.add_argument("--reuse", type=Path)
    parser.add_argument("--gets", type=int)
    parser.add_argument("--failed", action="store_true")
    parser.add_argument("--local", action="store_true")
    args = parser.parse_args()
    root = local_root(args.root)
    if args.action in ("begin", "check-result", "traffic") and args.name not in (*SUCCESS, *ERRORS):
        parser.error("Use a documented command label")
    configure()
    if args.action == "prepare":
        if not args.reuse:
            parser.error("--reuse must name your retained M5A seed run directory")
        prepare(root, args.namespace, args.cli.resolve(), args.reuse.resolve())
    elif args.action == "preflight":
        preflight(root)
    elif args.action == "publish":
        publish(root, args.name)
    elif args.action == "inspect-db":
        if args.name not in SEEDS:
            parser.error("Choose a new M5B publication case")
        transport.inspect_case(root, args.name, "conformant")
    elif args.action == "check-lock":
        check_lock(root, args.name)
    elif args.action in ("snapshot", "unchanged"):
        if not re.fullmatch(r"[a-z0-9-]+", args.value or ""):
            parser.error("Use a simple evidence label")
        snapshot(root, args.name, args.value, args.action == "unchanged")
    elif args.action == "clean":
        clean(root, args.name)
    elif args.action == "mutate":
        mutate(root, args.name, args.value)
    elif args.action == "begin":
        begin(root, args.name)
    elif args.action == "check-result":
        result(root, args.name)
    elif args.action == "traffic":
        traffic(root, args.name, args.gets, args.failed, args.local)
    elif args.action == "serve":
        serve(root)
    elif args.action == "audit":
        audit(root)
    elif args.action == "describe":
        catalog = read(root / "catalog.json")
        if args.name in catalog:
            entry = catalog[args.name]
            print(json.dumps(entry, indent=2))
            print(json.dumps(read(root / entry["folder"] / "source/agent.json"), indent=2))
        else:
            print(json.dumps(read(root / "controlled" / args.name / "case.json"), indent=2))
            print(json.dumps(read(root / "controlled" / args.name / "init-items.json"), indent=2))


if __name__ == "__main__":
    main()
