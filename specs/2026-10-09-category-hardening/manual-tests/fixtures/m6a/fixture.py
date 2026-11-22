"""Visible, owned M6A preflight fixtures and result checks. No Registry access."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import shlex
import shutil
import subprocess
import sys
import threading

REPO = Path(__file__).resolve().parents[5]
STANDARD = {"id": "agentpm", "version": "1.0.0"}
OUTPUT = {"text": "PASS M6A fixture"}
LOOP = "loop:@manual/workflow@1.0.0"
LOOP_FILE = ".agentpm/loops/manual/workflow/1.0.0/agent.json"
SOURCE = Path(__file__).resolve()
TUI = {
    "ready": "ready",
    "bootstrap": "missing-model",
    "legacy": "legacy-description",
    "no-loop": "no-loop",
}


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def owned(root):
    root = root.resolve()
    if root.parent != (REPO / "cli-m6a-test").resolve():
        raise ValueError("Use one direct child of agentpm/cli-m6a-test/ as the run root")
    return root


def environment(root):
    value = dict(os.environ)
    for key in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY"]:
        value.pop(key, None)
    value.update(AGENTPM_TELEMETRY="0", AGENTPM_TOKEN="apm_m6a.fixture", NO_COLOR="1")
    for key, name in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("APPDATA", "config"),
        ("LOCALAPPDATA", "cache"),
    ]:
        value[key] = str(root / name)
    return value


def sources(root):
    result = {}
    for name in ["cases", "templates", "runtime", "protocol"]:
        for path in (root / name).rglob("*"):
            if (
                path.is_file()
                and ".agentpm-state" not in path.parts
                and path.name != "runtime-requests.jsonl"
            ):
                result[path.relative_to(root).as_posix()] = sha(path)
    for name in ["expectations.json", "paths.sh", "functions.sh"]:
        result[name] = sha(root / name)
    return result


def load(root):
    root = owned(root)
    session = read(root / "session.json")
    assert (
        sha(Path(session["cli"])) == session["cli_sha256"]
    ), "CLI changed: build once, then prepare a new run"
    assert sha(SOURCE) == session["helper_sha256"], "Fixture helper changed: prepare a new run"
    assert (
        sources(root) == session["source_hashes"]
    ), "Fixture inputs changed; inspect or prepare a new run"
    return session, read(root / "expectations.json")


def guard(root, cwd):
    load(root)
    folder = Path(cwd).resolve()
    assert (
        folder.parent == root / "cases" and (folder / "case.json").is_file()
    ), "cd into this run's stated case directory"


def package(kind, name):
    return {
        "kind": kind,
        "name": "@manual/" + name,
        "version": "1.0.0",
        "integrity": "manual-fixture-not-a-release-digest",
        "standard": STANDARD,
        "standard_origin": "declared",
    }


def model_config(root, folder, selected=True):
    value = {
        "version": 1,
        "trace": {"content": "full"},
        "providers": {
            "models": {
                "manual-model": {
                    "implementation": {
                        "type": "process",
                        "command": sys.executable,
                        "args": [
                            str(root / "runtime/model_service.py"),
                            str(folder / "runtime-requests.jsonl"),
                        ],
                        "startup_timeout_ms": 5000,
                        "request_timeout_ms": 5000,
                    }
                }
            }
        },
    }
    if selected:
        value["model"] = {"provider": "manual-model", "model": "fixture"}
    return value


def prepare(root, cli):
    assert sys.version_info >= (3, 11), "Python 3.11+ required"
    root = owned(root)
    cli = cli.resolve()
    assert cli.is_file(), "Build the reviewed CLI first"
    root.parent.mkdir(exist_ok=True)
    root.mkdir(mode=0o700)
    (root / "evidence").mkdir()
    (root / "runtime").mkdir()
    (root / "protocol").mkdir()
    shutil.copyfile(SOURCE.with_name("model_service.py"), root / "runtime/model_service.py")
    shutil.copyfile(SOURCE.with_name("shell.sh"), root / "functions.sh")
    setup = []

    def command(folder, args, expected=0):
        result = subprocess.run(
            [str(cli), "--base-url", "http://127.0.0.1:9", *args],
            cwd=folder,
            env=environment(root),
            capture_output=True,
            text=True,
            timeout=60,
        )
        setup.append(
            {
                "cwd": str(folder),
                "argv": args,
                "expected_exit": expected,
                "exit": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }
        )
        write(root / "evidence/preparation.json", setup)
        assert result.returncode == expected, setup[-1]

    for kind in ["agent", "loop", "skill", "knowledge", "memory"]:
        folder = root / "templates" / kind
        folder.mkdir(parents=True)
        command(folder, ["init", "--kind", kind])
        value = read(folder / "agent.json")
        value.update(
            name={"agent": "consumer", "loop": "workflow"}.get(kind, kind), version="1.0.0"
        )
        if kind == "loop":
            value["loop"] = {
                "entry_phase": "assess",
                "phases": [
                    {"id": "assess", "objective": "Assess the local fixture."},
                    {"id": "execute", "objective": "Complete the local fixture."},
                ],
                "transitions": [
                    {"from": "assess", "on": "complete", "to": "execute"},
                    {"from": "execute", "on": "complete", "to": "$end"},
                ],
            }
        if kind == "memory":
            space = next(iter(value["memory"]["spaces"].values()))
            space.update(model="collection", retrieval={"modes": ["semantic"]})
        write(folder / "agent.json", value)
        if kind in ["memory", "knowledge"]:
            command(folder, [kind, "build"])
        command(folder, ["lint", "--format", "json"])

    cases = {}

    def case(
        name,
        issue,
        definition="conformant",
        support="supported",
        readiness="ready",
        diagnostic=None,
        lint=0,
    ):
        folder = root / "cases" / name
        shutil.copytree(root / "templates/agent", folder)
        shutil.copytree(root / "templates/loop", folder / Path(LOOP_FILE).parent)
        agent = read(folder / "agent.json")
        agent["loop"] = "@manual/workflow@1.0.0"
        lock = {
            "lockfile_version": 4,
            "generated": "2026-11-21T00:00:00Z",
            "packages": {LOOP: package("loop", "workflow")},
            "roots": {
                "local:agent": {
                    "name": "consumer",
                    "version": "1.0.0",
                    "standard": STANDARD,
                    "standard_origin": "declared",
                    "loop": LOOP,
                }
            },
        }
        value = {
            "case": name,
            "issue": issue,
            "apds": definition,
            "harness_support": support,
            "local_readiness": readiness,
            "status": "ready",
            "exit": 0,
            "lint_exit": lint,
            "diagnostic": diagnostic,
        }
        if readiness in ["not_ready", "not_evaluated"]:
            value.update(status="failed", exit=1)
        if definition == "legacy_inferred" and diagnostic:
            value["status"] = "ready_with_warnings"
        cases[name] = value
        return folder, agent, lock, model_config(root, folder)

    def save(values):
        folder, agent, lock, config = values
        write(folder / "agent.json", agent)
        write(folder / "agent.lock", lock)
        write(folder / "agentpm.harness.json", config)
        write(folder / "case.json", cases[folder.name])

    def diag(code, path=None, rule=None, severity="fatal"):
        return {"code": code, "path": path, "rule": rule, "severity": severity}

    save(case("ready", "No defect: supported Loop and a configured local process model."))
    x = case(
        "no-loop",
        "No Loop: valid APDS, not runnable by this Harness.",
        support="not_runnable",
        readiness="not_evaluated",
        diagnostic=diag("agent_missing_loop"),
    )
    x[1].pop("loop")
    x[2]["roots"]["local:agent"].pop("loop")
    x[2]["packages"] = {}
    shutil.rmtree(x[0] / ".agentpm")
    save(x)
    x = case(
        "missing-model",
        "Provider implementation exists, but no model is selected.",
        readiness="not_ready",
        diagnostic=diag("model_not_configured"),
    )
    x[3].pop("model")
    save(x)
    x = case(
        "missing-key",
        "OpenAI is selected but this wrapper removes OPENAI_API_KEY.",
        readiness="not_ready",
        diagnostic=diag("model_provider_not_ready"),
    )
    x[3]["model"] = {"provider": "openai", "model": "fixture"}
    save(x)
    for name, component in [("unsupported-agent", False), ("unsupported-loop", True)]:
        x = case(
            name,
            "Explicit unsupported agentpm / 9.0.0 declaration; no fallback.",
            "unsupported",
            "unsupported",
            "not_evaluated",
            diag("unsupported_standard", LOOP_FILE if component else "agent.json"),
            lint=0 if component else 1,
        )
        if component:
            value = read(x[0] / LOOP_FILE)
            value["standard"]["version"] = "9.0.0"
            write(x[0] / LOOP_FILE, value)
        else:
            x[1]["standard"]["version"] = "9.0.0"
        save(x)
    x = case(
        "unsupported-tool",
        "A selected Skill inherits a future-standard Tool.",
        "unsupported",
        "unsupported",
        "not_evaluated",
        diag("unsupported_standard", ".agentpm/tools/manual/future/1.0.0/agent.json"),
    )
    skill_key = "skill:@manual/skill@1.0.0"
    x[1]["skills"] = ["@manual/skill@1.0.0"]
    x[2]["roots"]["local:agent"]["skills"] = [skill_key]
    x[2]["packages"][skill_key] = package("skill", "skill")
    future = package("tool", "future")
    future.pop("standard")
    future.pop("standard_origin")
    x[2]["packages"]["tool:@manual/future@1.0.0"] = future
    skill_dir = x[0] / ".agentpm/skills/manual/skill/1.0.0"
    shutil.copytree(root / "templates/skill", skill_dir)
    value = read(skill_dir / "agent.json")
    value["tools"] = ["@manual/future@1.0.0"]
    write(skill_dir / "agent.json", value)
    write(
        x[0] / ".agentpm/tools/manual/future/1.0.0/agent.json",
        {"standard": {"id": "agentpm", "version": "9.0.0"}, "future_execution_contract": {}},
    )
    save(x)
    for name, code, path, rule in [
        ("malformed-standard", "apds_invalid_standard", "agent.json", None),
        ("invalid-description", "apds_nonconformant", "agent.json#/description", "APDS-ID-002"),
        (
            "invalid-phase",
            "apds_nonconformant",
            "agent.json#/bindings/phases/ghost",
            "APDS-BIND-002",
        ),
        ("missing-loop", "apds_context_unavailable", LOOP_FILE, None),
        ("provenance", "apds_provenance_mismatch", "agent.json", None),
    ]:
        definition = "incomplete" if name == "missing-loop" else "nonconformant"
        x = case(
            name,
            {
                "malformed-standard": "standard is null, not absent historical metadata.",
                "invalid-description": "Declared Agent has a whitespace-only description.",
                "invalid-phase": "Binding phase ghost is absent from the selected Loop.",
                "missing-loop": "Locked Loop's installed manifest is missing.",
                "provenance": "Lock says legacy_inferred but the source explicitly declares standard.",
            }[name],
            definition,
            "not_evaluated",
            "not_evaluated",
            diag(code, path, rule),
            lint=1 if name in ["malformed-standard", "invalid-description"] else 0,
        )
        if name == "malformed-standard":
            x[1]["standard"] = None
        if name == "invalid-description":
            x[1]["description"] = "   "
        if name == "invalid-phase":
            x[1]["bindings"] = {"phases": {"ghost": {}}}
        if name == "missing-loop":
            (x[0] / LOOP_FILE).unlink()
        if name == "provenance":
            x[2]["roots"]["local:agent"]["standard_origin"] = "legacy_inferred"
        save(x)
    for name in ["legacy", "legacy-description", "legacy-loop-description", "legacy-phase"]:
        component = name == "legacy-loop-description"
        path = LOOP_FILE + "#/description" if component else "agent.json#/description"
        rule = "APDS-BIND-002" if name == "legacy-phase" else "APDS-ID-002"
        if name == "legacy-phase":
            path = "agent.json#/bindings/phases/ghost"
        issue = (
            "Missing declaration only; qualified compatibility, not certification."
            if name == "legacy"
            else "Historical definition has a real defect: "
            + (
                "nonexistent phase binding"
                if name == "legacy-phase"
                else "whitespace-only description"
            )
            + "; warn without certifying or rewriting."
        )
        x = case(
            name,
            issue,
            definition="legacy_inferred",
            diagnostic=(
                None if name == "legacy" else diag("legacy_apds_issue", path, rule, "warning")
            ),
            lint=0 if component else 1,
        )
        if component:
            value = read(x[0] / LOOP_FILE)
            value.pop("standard")
            value["description"] = "   "
            write(x[0] / LOOP_FILE, value)
            x[2]["packages"][LOOP]["standard_origin"] = "legacy_inferred"
        else:
            x[1].pop("standard")
            x[2]["roots"]["local:agent"]["standard_origin"] = "legacy_inferred"
            if name == "legacy-description":
                x[1]["description"] = "   "
            if name == "legacy-phase":
                x[1]["bindings"] = {"phases": {"ghost": {}}}
        save(x)
    for kind in ["memory", "knowledge"]:
        x = case(
            kind + "-limited",
            "Valid "
            + kind
            + " contract; selected local execution surface cannot realize its backend.",
            support="limited",
            readiness="limited",
            diagnostic=diag(
                (
                    "unrealizable_memory_space"
                    if kind == "memory"
                    else "host_implementation_unavailable"
                ),
                severity="suppressed",
            ),
        )
        key = kind + ":@manual/" + kind + "@1.0.0"
        x[1][kind] = ["@manual/" + kind + "@1.0.0"]
        x[2]["roots"]["local:agent"][kind] = [key]
        x[2]["packages"][key] = package(kind, kind)
        folder = x[0] / (".agentpm/" + kind + "/manual/" + kind + "/1.0.0")
        shutil.copytree(root / "templates" / kind, folder)
        if kind == "memory":
            space = next(iter(read(folder / "agent.json")["memory"]["spaces"]))
            x[1]["bindings"] = {
                "global": {"memory": [{"package": "@manual/memory", "spaces": [space]}]}
            }
            x[3]["scopes"] = {"user": "manual-user"}
        else:
            x[1]["bindings"] = {"global": {"knowledge": ["@manual/knowledge"]}}
            x[3]["knowledge"] = {
                "runtimes": {"manual-host": {"implementation": {"type": "host"}}},
                "packages": {"@manual/knowledge": {"runtime": "manual-host"}},
            }
        save(x)
    for name, target in [("handoff", "$handoff"), ("abort", "$abort"), ("approval", "$end")]:
        x = case(
            name,
            "Runtime regression: "
            + (
                "approval is required before the first phase"
                if name == "approval"
                else "implicit complete ends at " + target
            ),
        )
        value = read(x[0] / LOOP_FILE)
        value["loop"]["transitions"][-1]["to"] = target
        if name == "approval":
            value["loop"]["checkpoints"] = [
                {
                    "id": "approve",
                    "type": "approval",
                    "before_phase": "assess",
                    "on_reject": "$handoff",
                }
            ]
        write(x[0] / LOOP_FILE, value)
        save(x)

    expectations = {}
    for name, value in cases.items():
        label = name + "-json"
        expectations[label] = {
            "case": name,
            "mode": "preflight",
            "exit": value["exit"],
            "args": ["harness", "--json"],
        }
        command(root / "cases" / name, ["lint", "--format", "json"], value["lint_exit"])
    # Runtime-only cases don't need repeated JSON preflight in the operator sequence.
    for name in ["handoff", "abort", "approval"]:
        expectations.pop(name + "-json")
    for name, terminal, exit_code in [
        ("ready", "ended", 0),
        ("handoff", "handed_off", 0),
        ("abort", "aborted", 1),
        ("approval", "approval_required", 1),
    ]:
        label = name + "-headless"
        expectations[label] = {
            "case": name,
            "mode": "headless",
            "exit": exit_code,
            "terminal": terminal,
            "args": [
                "harness",
                "--headless",
                "--input",
                "M6A local smoke",
                "--report",
                str(root / "evidence" / (label + ".report.json")),
            ],
        }
    expectations["missing-model-headless"] = {
        "case": "missing-model",
        "mode": "headless-blocked",
        "exit": 1,
        "args": ["harness", "--headless", "--input", "M6A local smoke"],
    }
    for name in ["ready", "missing-model"]:
        expectations[name + "-machine"] = {
            "case": name,
            "mode": "machine",
            "exit": 0,
            "args": ["harness", "--machine"],
        }
    expectations["no-terminal"] = {
        "case": "missing-model",
        "mode": "no-terminal",
        "exit": 1,
        "args": ["harness"],
    }
    write(root / "expectations.json", expectations)
    frames = [
        {
            "protocol": "agentpm-harness-machine",
            "version": 1,
            "kind": "request",
            "id": method,
            "method": method,
            "payload": {"input": "M6A local smoke"} if method == "start_run" else {},
        }
        for method in ["initialize", "preflight", "start_run", "shutdown"]
    ]
    (root / "protocol/requests.jsonl").write_text(
        "".join(json.dumps(frame) + "\n" for frame in frames), encoding="utf-8"
    )
    paths = {
        "M6A_ROOT": str(root),
        "M6A_REPO": str(REPO),
        "M6A_APM": str(cli),
        "M6A_PYTHON": sys.executable,
        "M6A_FIXTURE": str(SOURCE),
    }
    (root / "paths.sh").write_text(
        "\n".join("export " + key + "=" + shlex.quote(value) for key, value in paths.items())
        + "\nsource "
        + shlex.quote(str(root / "functions.sh"))
        + "\n",
        encoding="utf-8",
    )
    version = subprocess.run(
        [str(cli), "--version"], env=environment(root), capture_output=True, text=True, check=True
    ).stdout.strip()
    write(
        root / "session.json",
        {
            "root": str(root),
            "cli": str(cli),
            "cli_version": version,
            "cli_sha256": sha(cli),
            "helper_sha256": sha(SOURCE),
            "source_hashes": sources(root),
            "scope": "synthetic local installed-state; no Registry, DB, S3, signatures or paid models",
        },
    )
    print(
        f"Prepared {len(cases)} visible cases and {len(expectations)} command expectations: {root}"
    )
    print("Source this exact run: source " + shlex.quote(str(root / "paths.sh")))


def capture_size(folder):
    path = folder / "runtime-requests.jsonl"
    return path.stat().st_size if path.exists() else 0


def expected_argv(spec):
    return ["m6a_machine"] if spec["mode"] == "machine" else ["m6a", *spec["args"]]


def machine(root, cwd):
    """Sequence visible requests; never send shutdown during an active Run."""
    guard(root, cwd)
    session, _ = load(root)
    child = subprocess.Popen(
        [session["cli"], "--base-url", "http://127.0.0.1:9", "harness", "--machine"],
        cwd=cwd,
        env=environment(root),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    lines = queue.Queue()

    def read_lines():
        for line in child.stdout:
            lines.put(line)
        lines.put(None)

    threading.Thread(target=read_lines, daemon=True).start()
    try:
        for request in (root / "protocol/requests.jsonl").read_text().splitlines():
            identifier = json.loads(request)["id"]
            child.stdin.write(request + "\n")
            child.stdin.flush()
            while True:
                line = lines.get(timeout=30)
                assert line is not None, "CLI closed stdout before response to " + identifier
                print(line, end="", flush=True)
                frame = json.loads(line)
                if frame.get("id") == identifier and frame["kind"] in ["response", "error"]:
                    break
        child.stdin.close()
        assert child.wait(timeout=10) == 0, "Machine CLI failed"
    finally:
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()


def begin(root, label, expected, cwd, argv):
    _, expectations = load(root)
    spec = expectations[label]
    guard(root, cwd)
    assert Path(cwd).resolve() == root / "cases" / spec["case"], "Wrong case directory"
    assert int(expected) == spec["exit"] and argv == expected_argv(
        spec
    ), "Command differs from the runbook expectation"
    write(
        root / "evidence" / (label + ".begin.json"),
        {
            "case": spec["case"],
            "cwd": cwd,
            "argv": argv,
            "expected_exit": int(expected),
            "sources": sources(root),
            "request_start": capture_size(Path(cwd)),
        },
    )


def check_report(report, case):
    assert report["supported_standards"] == [STANDARD], report
    assert report["apds"]["runner_evaluated"] is False, report
    for key in ["status", "harness_support", "local_readiness"]:
        assert report[key] == case[key], (key, report[key], case[key])
    assert report["apds"]["status"] == case["apds"], report
    required = case["diagnostic"]
    if required:
        matches = [
            d
            for d in report["diagnostics"]
            if d["code"] == required["code"]
            and d["severity"] == required["severity"]
            and (
                required["path"] is None or d.get("path", "").replace("\\", "/") == required["path"]
            )
            and (required["rule"] is None or required["rule"] in d["message"])
        ]
        assert matches, ("Expected specific diagnostic", required, report)
        if required["code"] == "unsupported_standard":
            assert matches[0]["message"].startswith(matches[0]["path"] + ": ")
    if case["apds"] == "legacy_inferred":
        assert all(d["severity"] != "fatal" for d in report["diagnostics"]), report
        legacy = [
            a for a in report["apds"]["artifacts"] if a["standard_origin"] == "legacy_inferred"
        ]
        assert legacy and all(
            a["status"] == "legacy_inferred" and "APDS-ID-001" in a["unevaluated"] for a in legacy
        )


def requests_between(folder, start, end):
    path = folder / "runtime-requests.jsonl"
    data = path.read_bytes() if path.exists() else b""
    assert 0 <= start <= end <= len(data)
    return [json.loads(line) for line in data[start:end].splitlines()]


def check_terminal(report, terminal):
    assert report["terminal_status"] == terminal, report
    if terminal != "approval_required":
        assert report["terminal_output"] == OUTPUT, report
        assert [p["outcome"] for p in report["phase_summaries"]] == ["complete", "complete"], report


def check(root, label, result):
    session, expectations = load(root)
    spec = expectations[label]
    folder = root / "cases" / spec["case"]
    case = read(folder / "case.json")
    before = read(root / "evidence" / (label + ".begin.json"))
    assert before["argv"] == expected_argv(spec) and before["sources"] == session["source_hashes"]
    assert result["exit"] == spec["exit"] and result["source_hashes"] == session["source_hashes"]
    stdout = (root / "evidence" / (label + ".stdout.txt")).read_text(encoding="utf-8")
    stderr = (root / "evidence" / (label + ".stderr.txt")).read_text(encoding="utf-8")
    requests = requests_between(folder, before["request_start"], result["request_end"])
    calls = [r for r in requests if r["method"] == "generate"]
    expected_calls = 0
    if spec["mode"] == "preflight":
        report = json.loads(stdout)
        check_report(report, case)
        print(
            f"APDS {report['apds']['status']} | Harness {report['harness_support']} | local {report['local_readiness']}"
        )
    elif spec["mode"] == "headless-blocked":
        assert not stdout.strip() and "model.provider and model.model" in stderr
    elif spec["mode"] == "no-terminal":
        assert not stdout.strip() and "requires an interactive terminal" in stderr
    elif spec["mode"] == "headless":
        report = read(root / "evidence" / (label + ".report.json"))
        check_terminal(report, spec["terminal"])
        if spec["exit"] == 0:
            assert json.loads(stdout) == OUTPUT
        else:
            assert not stdout.strip() and "Error:" in stderr
        expected_calls = 0 if spec["terminal"] == "approval_required" else 2
    elif spec["mode"] == "machine":
        frames = [json.loads(line) for line in stdout.splitlines()]
        assert frames and all(
            f["protocol"] == "agentpm-harness-machine" and f["version"] == 1 for f in frames
        )

        def response(identifier):
            rows = [
                f
                for f in frames
                if f.get("id") == identifier and f["kind"] in ["response", "error"]
            ]
            assert len(rows) == 1, rows
            return rows[0]

        initial = response("initialize")["payload"]
        assert initial["session"] == {"protocol": "agentpm-harness-machine", "version": 1}
        check_report(initial["preflight"], case)
        report = response("preflight")["payload"]["report"]
        assert report == initial["preflight"]
        start = response("start_run")
        if spec["case"] == "ready":
            assert start["payload"]["status"] == "ended" and start["payload"]["output"] == OUTPUT
            check_terminal(start["payload"]["report"], "ended")
            expected_calls = 2
        else:
            assert start["kind"] == "error" and start["error"]["code"] == "preflight_not_ready"
        assert response("shutdown")["payload"]["shutdown"] is True
    assert len(calls) == expected_calls, ("Unexpected fixture model calls", calls)
    if expected_calls == 0 and spec["mode"] != "headless":
        assert not requests, "Preflight unexpectedly started the model"


def finish(root, label, actual):
    result = {"exit": int(actual), "source_hashes": sources(root)}
    _, specs = load(root)
    result["request_end"] = capture_size(root / "cases" / specs[label]["case"])
    write(root / "evidence" / (label + ".result.json"), result)
    check(root, label, result)
    print("PASS: intended result, unchanged inputs and exact local model traffic: " + label)


def begin_tui(root, name, cwd):
    guard(root, cwd)
    assert name in TUI and Path(cwd).resolve() == root / "cases" / TUI[name]
    reports = [str(p) for p in (Path(cwd) / ".agentpm-state/runs").glob("*/report.json")]
    write(
        root / "evidence" / ("tui-" + name + ".begin.json"),
        {"reports": reports, "request_start": capture_size(Path(cwd))},
    )


def check_tui(root, name, result):
    folder = root / "cases" / TUI[name]
    before = read(root / "evidence" / ("tui-" + name + ".begin.json"))
    requests = requests_between(folder, before["request_start"], result["request_end"])
    if name in ["ready", "bootstrap"]:
        assert result["new_reports"], "Submit one message and wait for the TUI run to finish first"
        for path in result["new_reports"]:
            assert path not in before["reports"] and Path(path).is_relative_to(
                folder / ".agentpm-state/runs"
            )
            check_terminal(read(Path(path)), "ended")
        assert len([r for r in requests if r["method"] == "generate"]) == 2
    else:
        assert (
            not result["new_reports"] and not requests
        ), "View-only TUI check unexpectedly ran the model"


def confirm_tui(root, name):
    load(root)
    assert name in TUI
    folder = root / "cases" / TUI[name]
    before = read(root / "evidence" / ("tui-" + name + ".begin.json"))
    result = {
        "observed": True,
        "case": TUI[name],
        "request_end": capture_size(folder),
        "new_reports": [
            str(p)
            for p in (folder / ".agentpm-state/runs").glob("*/report.json")
            if str(p) not in before["reports"]
        ],
        "note": "Operator confirmed the guide's visible TUI expectations; not an automatic screenshot check.",
    }
    check_tui(root, name, result)
    write(
        root / "evidence" / ("tui-" + name + ".json"),
        result,
    )
    print("Recorded operator observation: " + name)


def audit(root, require_tui=True):
    session, expectations = load(root)
    for label in expectations:
        check(root, label, read(root / "evidence" / (label + ".result.json")))
    if require_tui:
        for name, case in TUI.items():
            observed = read(root / "evidence" / ("tui-" + name + ".json"))
            assert observed["observed"] is True and observed["case"] == case
            check_tui(root, name, observed)
    summary = {
        "status": "pass",
        "checked_command_results": len(expectations),
        "tui_observations_recorded": require_tui,
        "operator_signoff": False,
        "hosted_ci_complete": False,
        "cli_sha256": session["cli_sha256"],
        "scope": "local preflight and deterministic process model; TUI observations "
        + ("recorded" if require_tui else "not checked")
        + "; no Registry/paid service",
    }
    write(root / "evidence/summary.json", summary)
    print(json.dumps(summary, indent=2))
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=[
            "prepare",
            "guard",
            "describe",
            "begin",
            "finish",
            "machine",
            "begin-tui",
            "confirm-tui",
            "audit",
        ],
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("values", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    root = owned(args.root)
    if args.action == "prepare":
        options = argparse.ArgumentParser()
        options.add_argument("--cli", type=Path, default=REPO / "target/debug/agentpm")
        prepare(root, options.parse_args(args.values).cli)
    elif args.action == "guard":
        guard(root, args.values[0])
    elif args.action == "describe":
        load(root)
        print(json.dumps(read(root / "cases" / args.values[0] / "case.json"), indent=2))
    elif args.action == "begin":
        begin(root, args.values[0], args.values[1], args.values[2], args.values[3:])
    elif args.action == "finish":
        finish(root, *args.values)
    elif args.action == "machine":
        machine(root, args.values[0])
    elif args.action == "begin-tui":
        begin_tui(root, *args.values)
    elif args.action == "confirm-tui":
        confirm_tui(root, args.values[0])
    elif args.action == "audit":
        audit(root)


if __name__ == "__main__":
    main()
