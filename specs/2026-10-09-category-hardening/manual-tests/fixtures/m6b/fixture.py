"""Visible M6B fixtures and evidence checks. Never modify an existing run."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys

SOURCE = Path(__file__).resolve()
REPO = SOURCE.parents[5]
STANDARD = {"id": "agentpm", "version": "1.0.0"}
OUTPUT = {"text": "PASS M6B fixture"}


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def environment(root):
    env = dict(os.environ)
    for key in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "AGENTPM_CONFIG", "PYTHONPATH"]:
        env.pop(key, None)
    for key, folder in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("APPDATA", "config"),
        ("LOCALAPPDATA", "cache"),
    ]:
        env[key] = str(root / "isolation" / folder)
    env.update(
        AGENTPM_TELEMETRY="0", AGENTPM_BASE_URL="http://127.0.0.1:9", PYTHONDONTWRITEBYTECODE="1"
    )
    # Preserve the real terminal palette in the interactive checks.
    env.pop("NO_COLOR", None)
    return env


def source_hashes(root):
    files = list((root / "runtime").glob("*"))
    for case in (root / "cases").iterdir():
        files += list(case.glob("*.json"))
        files += list((case / "agents").glob("*.json"))
        files += [p for p in (case / ".agentpm").rglob("*") if p.is_file()]
    return {str(p.relative_to(root)): sha(p) for p in sorted(files) if p.is_file()}


def pins(session):
    return {name: sha(Path(name)) for name in session["pins"]}


def guard(root):
    session = read(root / "session.json")
    assert session["root"] == str(root), "Wrong run directory"
    assert read(root / "expectations.json") == specs(root), "Command expectations changed"
    assert source_hashes(root) == session["source_hashes"], "Prepared inputs changed"
    assert pins(session) == session["pins"], "CLI, SDK or helper changed; prepare a fresh run"
    assert not list((root / "cases").rglob("skill-script-ran")), "Skill script auto-executed"
    return session


def specs(root):
    result = {}

    def add(label, case, mode, command, exit=0, agent=""):
        result[label] = dict(case=case, mode=mode, command=command, exit=exit, agent=agent)

    for major in [1, 2]:
        agent = f"consumer-{major}"
        add(f"json-{major}", "standalone", "json", ["m6b", "harness", agent, "--json"], agent=agent)
        for case, prefix in [
            ("standalone", "headless"),
            ("scoped", "scoped"),
            ("skill-only", "skill-only"),
        ]:
            if case != "standalone" and major != 1:
                continue
            label = f"{prefix}-{major}"
            add(
                label,
                case,
                "headless",
                [
                    "m6b",
                    "harness",
                    agent,
                    "--headless",
                    "--input",
                    "Check exact versions.",
                    "--report",
                    str(root / "evidence" / f"{label}.report.json"),
                ],
                agent=agent,
            )
        for sdk in ["node", "python"]:
            add(f"{sdk}-{major}", "sdk-host", "sdk-run", [f"m6b_{sdk}", "run", agent], agent=agent)
    add(
        "ambiguous-json",
        "ambiguous",
        "json",
        ["m6b", "harness", "consumer-1", "--json"],
        1,
        "consumer-1",
    )
    add(
        "ambiguous-headless",
        "ambiguous",
        "blocked",
        ["m6b", "harness", "consumer-1", "--headless", "--input", "Must not execute."],
        1,
        "consumer-1",
    )
    for sdk in ["node", "python"]:
        add(
            f"ambiguous-{sdk}",
            "ambiguous",
            "sdk-blocked",
            [f"m6b_{sdk}", "blocked", "consumer-1"],
            agent="consumer-1",
        )
    for case in ["ready", "no-loop", "missing-model", "legacy", "unsupported"]:
        add(
            f"report-{case}",
            case,
            "json",
            ["m6b", "harness", "--json"],
            0 if case in ["ready", "legacy"] else 1,
        )
        for sdk in ["node", "python"]:
            mode = "inspect" if case in ["ready", "legacy"] else "blocked"
            add(f"{sdk}-{case}", case, "sdk-report", [f"m6b_{sdk}", mode, ""])
    add("description-ab", "standalone", "description", ["m6b_description"])
    return result


def prepare(root, cli, node_sdk, python_sdk):
    assert not root.exists(), "Choose a new run path; existing runs are never overwritten"
    assert root.parent == REPO / "cli-m6b-test", "Keep runs under agentpm/cli-m6b-test"
    cli, node_sdk, python_sdk = map(Path.resolve, [cli, node_sdk, python_sdk])
    assert cli.is_file() and (node_sdk / "dist/index.js").is_file()
    assert (python_sdk / ".venv/bin/python").is_file()
    root.mkdir(parents=True, mode=0o700)
    for name in ["runtime", "evidence", "cases", "isolation/config", "isolation/cache"]:
        (root / name).mkdir(parents=True, exist_ok=True)
    for name in ["model_service.py", "sdk_python.py", "sdk_node.mjs"]:
        shutil.copyfile(SOURCE.with_name(name), root / "runtime" / name)
    factory_path = REPO / "crates/agentpm-sdk/contracts/sdk-v1/harness_fixture.py"
    shutil.copyfile(factory_path, root / "runtime/harness_fixture.py")
    module_spec = importlib.util.spec_from_file_location("m6b_factory", factory_path)
    factory = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(factory)
    preparation, original_run = [], subprocess.run

    def recorded_run(argv, **kwargs):
        kwargs["env"] = environment(root)
        completed = original_run(argv, **kwargs)
        preparation.append(
            dict(
                argv=argv,
                cwd=str(kwargs.get("cwd")),
                exit=completed.returncode,
                stdout=(
                    completed.stdout.decode()
                    if isinstance(completed.stdout, bytes)
                    else completed.stdout
                ),
                stderr=(
                    completed.stderr.decode()
                    if isinstance(completed.stderr, bytes)
                    else completed.stderr
                ),
            )
        )
        return completed

    subprocess.run = recorded_run
    try:
        for case in [
            "standalone",
            "sdk-host",
            "scoped",
            "ambiguous",
            "skill-only",
            "ready",
            "no-loop",
            "missing-model",
            "legacy",
            "unsupported",
        ]:
            folder = root / "cases" / case
            factory.prepare(
                folder,
                cli,
                (
                    "versions"
                    if case in ["standalone", "sdk-host", "scoped", "ambiguous", "skill-only"]
                    else case
                ),
            )
            if case in ["standalone", "scoped", "ambiguous", "skill-only"]:
                config = read(folder / "agentpm.harness.json")
                config["providers"]["models"]["sdk-model"]["implementation"] = {
                    "type": "process",
                    "command": sys.executable,
                    "args": [
                        str(root / "runtime/model_service.py"),
                        str(folder / "model-requests.jsonl"),
                    ],
                }
                write(folder / "agentpm.harness.json", config)
            if case in ["ambiguous", "skill-only"]:
                path = folder / ".agentpm/skills/fixture/skill/1.0.0/agent.json"
                value = read(path)
                value["tools"] = ["@fixture/echo@2.0.0"]
                write(path, value)
            if case in ["scoped", "skill-only"]:
                path = folder / "agents/agent-1.agent.json"
                agent = read(path)
                agent["tools"] = []
                agent["bindings"]["global"].pop("tools")
                lock = read(folder / "agent.lock")
                lock["roots"]["local:agent:agents/agent-1.agent.json"]["tools"] = []
                write(folder / "agent.lock", lock)
                if case == "scoped":
                    global_bindings = agent["bindings"]["global"]
                    skills = global_bindings.pop("skills")
                    memory = global_bindings.pop("memory")
                    agent["bindings"]["phases"] = {
                        "work": {"skills": skills},
                        "review": {"memory": memory},
                    }
                    path_loop = folder / ".agentpm/loops/fixture/workflow/1.0.0/agent.json"
                    loop = read(path_loop)
                    loop["loop"]["phases"][1]["access"] = {
                        "memory": {"read": False, "write": False}
                    }
                    write(path_loop, loop)
                write(path, agent)
            # Re-lint the modified authored inputs through the CLI, never bypass it.
            if case in ["scoped", "ambiguous", "skill-only"]:
                for path in [*folder.glob("agents/*.json"), *folder.glob(".agentpm/**/agent.json")]:
                    recorded_run(
                        [str(cli), "lint", str(path), "--format", "json"],
                        cwd=folder,
                        check=True,
                        capture_output=True,
                        text=True,
                    )
    finally:
        subprocess.run = original_run
        write(root / "evidence/preparation.json", preparation)
    selected_pins = [
        SOURCE,
        *SOURCE.parent.glob("sdk_*"),
        SOURCE.with_name("model_service.py"),
        factory_path,
        cli,
        *node_sdk.glob("dist/*"),
        *python_sdk.glob("src/agentpm/**/*.py"),
    ]
    session = dict(
        root=str(root),
        cli=str(cli),
        node_sdk=str(node_sdk),
        python_sdk=str(python_sdk),
        python=sys.executable,
        node=shutil.which("node"),
        pins={str(p): sha(p) for p in selected_pins if p.is_file()},
        source_hashes=source_hashes(root),
        scope="synthetic installed state; local Engine and SDKs; no Registry or paid service",
    )
    write(root / "session.json", session)
    write(root / "expectations.json", specs(root))
    assignments = {
        "M6B_ROOT": root,
        "M6B_REPO": REPO,
        "M6B_FIXTURE": SOURCE,
        "M6B_PYTHON": sys.executable,
        "M6B_APM": cli,
    }
    shell = "\n".join(f"export {k}={shlex.quote(str(v))}" for k, v in assignments.items())
    shell += """
m6b_fixture() { "$M6B_PYTHON" -B "$M6B_FIXTURE" "$1" "$M6B_ROOT" "${@:2}"; }
m6b_expect() { m6b_fixture execute "$@"; }
m6b() { m6b_fixture launch m6b "$@"; }
m6b_node() { m6b_fixture launch m6b_node "$@"; }
m6b_python() { m6b_fixture launch m6b_python "$@"; }
m6b_description() { m6b_fixture launch m6b_description; }
"""
    (root / "paths.sh").write_text(shell, encoding="utf-8")
    print(f"Prepared 10 visible cases and {len(specs(root))} command expectations: {root}")
    print("Source this exact run: source " + shlex.quote(str(root / "paths.sh")))


def command(root, tokens):
    session = read(root / "session.json")
    kind, *args = tokens
    if kind == "m6b":
        return [session["cli"], *args]
    if kind == "m6b_node":
        return [
            session["node"],
            str(root / "runtime/sdk_node.mjs"),
            str(Path(session["node_sdk"]) / "dist/index.js"),
            session["cli"],
            *args,
        ]
    if kind == "m6b_python":
        return [
            str(Path(session["python_sdk"]) / ".venv/bin/python"),
            "-B",
            str(root / "runtime/sdk_python.py"),
            session["python_sdk"],
            session["cli"],
            *args,
        ]
    assert kind == "m6b_description" and not args, tokens
    return [
        "cargo",
        "test",
        "--manifest-path",
        str(REPO / "Cargo.toml"),
        "-p",
        "agentpm-cli",
        "--bin",
        "agentpm",
        "description_prompt_ab",
        "--",
        "--nocapture",
    ]


def records(folder, name):
    path = folder / name
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def positions(folder):
    return {
        name: len(records(folder, name)) for name in ["model-requests.jsonl", "tool-runs.jsonl"]
    }


def check_report(report, case, agent=""):
    assert report["supported_standards"] == [STANDARD]
    assert report["apds"]["runner_evaluated"] is False
    expected = {
        "ambiguous": ("conformant", "limited", "not_ready"),
        "legacy": ("legacy_inferred", "supported", "ready"),
        "unsupported": ("unsupported", "unsupported", "not_evaluated"),
        "no-loop": ("conformant", "not_runnable", "not_evaluated"),
        "missing-model": ("conformant", "supported", "not_ready"),
    }.get(case, ("conformant", "supported", "ready"))
    actual = (report["apds"]["status"], report["harness_support"], report["local_readiness"])
    assert actual == expected, (actual, expected)
    if agent:
        major = agent.removeprefix("consumer-")
        version = major + ".0.0"
        artifacts = {a["artifact_id"] for a in report["apds"]["artifacts"]}
        expected_artifacts = {f"local:agent:agents/agent-{major}.agent.json"}
        expected_artifacts.update(
            f"{kind}:@fixture/{name}@{version}"
            for kind, name in [
                ("skill", "skill"),
                ("loop", "workflow"),
                ("knowledge", "knowledge"),
                ("memory", "memory"),
                ("profile", "profile"),
            ]
        )
        expected_artifacts.add(f"tool:@fixture/echo@{version}")
        if case == "ambiguous":
            expected_artifacts.add("tool:@fixture/echo@2.0.0")
        assert artifacts == expected_artifacts, (artifacts, expected_artifacts)
    diagnostic = {
        "ambiguous": "ambiguous_locked_component",
        "no-loop": "agent_missing_loop",
        "missing-model": "model_not_configured",
        "unsupported": "unsupported_standard",
        "legacy": "legacy_standard_inferred",
    }.get(case)
    if diagnostic:
        found = [d for d in report["diagnostics"] if d["code"] == diagnostic]
        assert found, (diagnostic, report)
        if case == "ambiguous":
            assert any(
                d["severity"] == "fatal" and "1.0.0" in d["message"] and "2.0.0" in d["message"]
                for d in found
            )


def check_requests(requests, case, agent):
    version = "2.0.0" if agent == "consumer-2" else "1.0.0"
    tool_version = "2.0.0" if case == "skill-only" else version
    assert [r["phase_id"] for r in requests] == ["work", "work", "review"]
    for request in requests:
        runtime, phase = request["runtime"], request["effective_phase"]
        for field in ["skills", "knowledge", "profiles"]:
            assert [p["version"] for p in runtime[field]] == [version], (field, runtime[field])
        assert [p["version"] for p in runtime["tools"]] == [tool_version]
        assert runtime["loop_package"]["version"] == version
        assert [p["package_version"] for p in runtime["memory_operations"]] == [version]
        work = request["phase_id"] == "work"
        assert [p["version"] for p in phase["active_tools"]] == ([tool_version] if work else [])
        assert [p["version"] for p in phase["active_skills"]] == (
            [version] if work or case != "scoped" else []
        )
        assert [p["version"] for p in phase["active_profiles"]] == [version]
        assert (
            phase["active_memory"] == []
        ), "Operation target must not become a direct Memory surface"
        operations = phase["active_memory_operations"]
        assert [p["package_version"] for p in operations] == (
            [] if work and case == "scoped" else [version]
        )


def check_terminal(report):
    assert report["terminal_status"] == "ended", report
    assert report["terminal_output"] == OUTPUT
    assert [p["outcome"] for p in report["phase_summaries"]] == ["complete", "complete"]
    assert report["memory_summaries"] == [], "An external operation must not run automatically"
    assert report["tool_summaries"] == [
        dict(count=1, identity="@fixture/echo", operation_kind="agentpm_tool", status="completed")
    ]


def check(root, label):
    session = guard(root)
    spec = read(root / "expectations.json")[label]
    result = read(root / "evidence" / f"{label}.result.json")
    assert result["spec"] == spec and result["exit"] == spec["exit"]
    assert result["source_hashes"] == session["source_hashes"]
    folder = root / "cases" / spec["case"]
    stdout = (root / "evidence" / f"{label}.stdout.txt").read_text()
    stderr = (root / "evidence" / f"{label}.stderr.txt").read_text()
    traffic = {
        name: records(folder, name)[start : result["after"][name]]
        for name, start in result["before"].items()
    }
    requests = traffic["model-requests.jsonl"]
    mode, case = spec["mode"], spec["case"]
    ran = mode in ["headless", "sdk-run"]
    if mode == "json":
        check_report(json.loads(stdout), case, spec["agent"])
        if (folder / "expected-preflight.json").exists():
            assert json.loads(stdout) == read(folder / "expected-preflight.json")
    elif mode == "blocked":
        assert not stdout.strip() and "ambiguous" in stderr.lower()
    elif mode == "headless":
        assert json.loads(stdout) == OUTPUT
        check_terminal(read(root / "evidence" / f"{label}.report.json"))
    elif mode.startswith("sdk-"):
        value = json.loads(stdout)
        assert value["sdk"] == spec["command"][0].removeprefix("m6b_")
        info, preflight = value["initialized"], value["preflight"]
        assert info["session"] == {"protocol": "agentpm-harness-machine", "version": 1}
        assert info["preflight"] == preflight["report"]
        assert value["supported"] == [STANDARD]
        check_report(preflight["report"], case, spec["agent"])
        if (folder / "expected-preflight.json").exists():
            assert preflight["report"] == read(folder / "expected-preflight.json")
        if ran:
            assert value["result"]["status"] == "ended"
            check_terminal(value["result"]["report"])
        elif mode == "sdk-blocked" or case in ["unsupported", "no-loop", "missing-model"]:
            assert value["blocked"] == "preflight_not_ready"
        assert not requests
        requests = value["requests"]
    elif mode == "description":
        report = read(root / "evidence/description-ab.json")
        assert report["decision"] == "defer_injection_keep_current_behavior"
        assert len(report["rows"]) == 4
        for phases in [1, 3]:
            a, b = [r for r in report["rows"] if r["phases"] == phases]
            assert not a["include_description"] and b["include_description"]
            assert (
                a["output"] == b["output"]
                and a["terminal_status"] == b["terminal_status"] == "ended"
            )
            assert all(
                r["provider_tokens"] is None
                and r["provider_cost"] is None
                and r["paid_requests"] == 0
                for r in [a, b]
            )
            assert all(
                y["utf8_bytes"] > x["utf8_bytes"]
                for x, y in zip(a["prompts"], b["prompts"], strict=True)
            )
    else:
        raise AssertionError(mode)
    if ran:
        check_requests(requests, case, spec["agent"])
    else:
        assert not requests, "Model must not be called for this case"
    version = "2.0.0" if spec["agent"] == "consumer-2" or case == "skill-only" else "1.0.0"
    assert traffic["tool-runs.jsonl"] == ([{"version": version}] if ran else [])
    print(f"PASS {label}: intended result, exact runtime/Tool evidence and unchanged inputs")


def execute(root, label, expected, tokens):
    guard(root)
    spec = read(root / "expectations.json")[label]
    assert (
        Path.cwd().resolve() == root / "cases" / spec["case"]
    ), "Change to the case directory shown in the runbook"
    assert spec["command"] == tokens and spec["exit"] == int(
        expected
    ), "Command differs from expectation"
    result_path = root / "evidence" / f"{label}.result.json"
    assert (
        not result_path.exists()
    ), "Result already captured; prepare a new run instead of overwriting evidence"
    before = positions(Path.cwd())
    env = environment(root)
    env["AGENTPM_DESCRIPTION_AB_RESULTS"] = str(root / "evidence/description-ab.json")
    print("COMMAND " + shlex.join(tokens), flush=True)
    completed = subprocess.run(
        command(root, tokens), env=env, text=True, capture_output=True, timeout=180
    )
    for stream in ["stdout", "stderr"]:
        text = getattr(completed, stream)
        (root / "evidence" / f"{label}.{stream}.txt").write_text(text)
        if stream == "stdout" and spec["mode"].startswith("sdk-") and completed.returncode == 0:
            value = json.loads(text)
            report = value["preflight"]["report"]
            print(
                json.dumps(
                    dict(
                        sdk=value["sdk"],
                        apds=report["apds"]["status"],
                        harness_support=report["harness_support"],
                        local_readiness=report["local_readiness"],
                        supported=value["supported"],
                        blocked=value["blocked"],
                        run_status=value["result"]["status"] if value["result"] else None,
                        model_requests=len(value["requests"]),
                    ),
                    indent=2,
                )
            )
            print(f"Full SDK stdout, envelopes and requests saved: evidence/{label}.stdout.txt")
        else:
            print(text, end="", file=sys.stdout if stream == "stdout" else sys.stderr, flush=True)
    write(
        result_path,
        dict(
            spec=spec,
            exit=completed.returncode,
            before=before,
            after=positions(Path.cwd()),
            source_hashes=source_hashes(root),
        ),
    )
    print(f"ACTUAL exit {completed.returncode} | EXPECTED exit {expected} | {label}", flush=True)
    check(root, label)


def tui(root, operation, agent):
    guard(root)
    assert agent in ["consumer-1", "consumer-2"]
    folder = root / "cases/standalone"
    assert Path.cwd().resolve() == folder
    path = root / "evidence" / f"tui-{agent}.json"
    if operation == "begin-tui":
        assert not path.exists(), "TUI observation already started"
        write(
            path,
            dict(
                before=positions(folder),
                reports=[str(p) for p in folder.glob(".agentpm-state/runs/*/report.json")],
            ),
        )
        print(
            "Now run m6b harness " + agent + "; submit one message and wait for Ended, then quit."
        )
    else:
        value = read(path)
        assert not value.get("confirmed"), "TUI observation already confirmed"
        reports = [
            p
            for p in folder.glob(".agentpm-state/runs/*/report.json")
            if str(p) not in value["reports"]
        ]
        assert len(reports) == 1, "Complete exactly one new TUI Run"
        check_terminal(read(reports[0]))
        check_requests(
            records(folder, "model-requests.jsonl")[value["before"]["model-requests.jsonl"] :],
            "standalone",
            agent,
        )
        version = "1.0.0" if agent == "consumer-1" else "2.0.0"
        assert records(folder, "tool-runs.jsonl")[value["before"]["tool-runs.jsonl"] :] == [
            {"version": version}
        ]
        value.update(confirmed=True, agent=agent, report=str(reports[0]), after=positions(folder))
        write(path, value)
        print(
            "PASS: operator TUI observation recorded with a fresh ended Run and exact Tool/model evidence"
        )


def audit(root):
    guard(root)
    for label in specs(root):
        check(root, label)
    for agent in ["consumer-1", "consumer-2"]:
        value = read(root / "evidence" / f"tui-{agent}.json")
        assert value["confirmed"] and value["agent"] == agent
        check_terminal(read(Path(value["report"])))
        folder = root / "cases/standalone"
        before, after = value["before"], value["after"]
        check_requests(
            records(folder, "model-requests.jsonl")[
                before["model-requests.jsonl"] : after["model-requests.jsonl"]
            ],
            "standalone",
            agent,
        )
        expected_version = "1.0.0" if agent == "consumer-1" else "2.0.0"
        assert records(folder, "tool-runs.jsonl")[
            before["tool-runs.jsonl"] : after["tool-runs.jsonl"]
        ] == [{"version": expected_version}]
    summary = dict(
        status="pass",
        checked_command_results=len(specs(root)),
        tui_observations_recorded=True,
        operator_signoff=False,
        hosted_ci_complete=False,
        cli_sha256=sha(Path(read(root / "session.json")["cli"])),
        scope="local exact-version Engine/SDK replay; synthetic installed fixtures; no Registry or paid model",
    )
    write(root / "evidence/summary.json", summary)
    print(json.dumps(summary, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "operation",
        choices=["prepare", "execute", "launch", "describe", "audit", "begin-tui", "confirm-tui"],
    )
    parser.add_argument("root", type=Path)
    parser.add_argument("values", nargs="*")
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--node-sdk", type=Path, default=REPO.parent / "agentpm-sdk-node")
    parser.add_argument("--python-sdk", type=Path, default=REPO.parent / "agentpm-sdk-python")
    args, remaining = parser.parse_known_args()
    root = args.root.resolve()
    values = args.values + remaining
    if args.operation == "prepare":
        prepare(root, args.cli or REPO / "target/debug/agentpm", args.node_sdk, args.python_sdk)
    elif args.operation == "execute":
        execute(root, values[0], values[1], values[2:])
    elif args.operation == "launch":
        guard(root)
        os.execvpe(command(root, values)[0], command(root, values), environment(root))
    elif args.operation == "describe":
        guard(root)
        print(
            json.dumps({k: v for k, v in specs(root).items() if v["case"] == values[0]}, indent=2)
        )
    elif args.operation == "audit":
        audit(root)
    else:
        tui(root, args.operation, values[0])


if __name__ == "__main__":
    main()
