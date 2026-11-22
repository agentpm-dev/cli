"""Local Harness fixtures shared by SDKs; no Registry or model API requests."""

import json
import subprocess
import sys
from pathlib import Path

STANDARD = {"id": "agentpm", "version": "1.0.0"}


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def manifest(kind, name, version="1.0.0", **fields):
    return dict(
        kind=kind,
        name=name,
        version=version,
        description="Local SDK Harness fixture.",
        standard=STANDARD,
        **fields,
    )


def prepare(root, cli, mode="ready"):
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=True)
    packages, roots = {}, {}
    loop_key = "loop:@fixture/workflow@1.0.0"
    packages[loop_key] = dict(
        kind="loop",
        name="@fixture/workflow",
        version="1.0.0",
        integrity="fixture",
        standard=STANDARD,
        standard_origin="declared",
    )
    loop = manifest(
        "loop",
        "workflow",
        loop={
            "entry_phase": "work",
            "phases": [
                {"id": "work", "objective": "Use the selected version."},
                {"id": "review", "objective": "Review without Tools.", "access": {"tools": False}},
            ],
            "transitions": [
                {"from": "work", "on": "complete", "to": "review"},
                {"from": "review", "on": "complete", "to": "$end"},
            ],
        },
    )
    write(root / ".agentpm/loops/fixture/workflow/1.0.0/agent.json", loop)
    config = {
        "version": 1,
        "trace": {"content": "full"},
        "model": {"provider": "ollama", "model": "fixture"},
    }
    if mode == "versions":
        config["model"] = {"provider": "sdk-model", "model": "fixture"}
        config["scopes"] = {"user": "fixture-user"}
        config["providers"] = {"models": {"sdk-model": {"implementation": {"type": "host"}}}}
    write(root / "agentpm.harness.json", config)
    for major in [1, 2] if mode == "versions" else [1]:
        version = f"{major}.0.0"
        path = f"agents/agent-{major}.agent.json" if mode == "versions" else "agent.json"
        key = f"local:agent:{path}" if mode == "versions" else "local:agent"
        agent = manifest("agent", f"consumer-{major}", loop="@fixture/workflow@1.0.0")
        selected = dict(
            name=agent["name"],
            version="1.0.0",
            standard=STANDARD,
            standard_origin="declared",
            loop=loop_key,
        )
        if mode == "versions":
            tool_key = f"tool:@fixture/echo@{version}"
            skill_key = f"skill:@fixture/skill@{version}"
            packages[tool_key] = dict(
                kind="tool",
                name="@fixture/echo",
                version=version,
                integrity="fixture",
                standard=STANDARD,
                standard_origin="declared",
            )
            packages[skill_key] = dict(
                kind="skill",
                name="@fixture/skill",
                version=version,
                integrity="fixture",
                standard=STANDARD,
                standard_origin="declared",
            )
            tool_root = root / f".agentpm/tools/fixture/echo/{version}"
            write(
                tool_root / "agent.json",
                manifest(
                    "tool",
                    "echo",
                    version,
                    runtime={"type": "node", "version": "20"},
                    entrypoint={"command": "node", "args": ["main.cjs"]},
                    files=["main.cjs"],
                    inputs={"type": "object"},
                    outputs={"type": "object"},
                ),
            )
            (tool_root / "main.cjs").write_text(
                "const fs = require('node:fs');\nprocess.stdin.resume();\nprocess.stdin.on('end', () => { const value = "
                + json.dumps({"version": version})
                + "; fs.appendFileSync("
                + json.dumps(str(root / "tool-runs.jsonl"))
                + ", JSON.stringify(value) + '\\n'); console.log(JSON.stringify(value)); });\n",
                encoding="utf-8",
            )
            skill_root = root / f".agentpm/skills/fixture/skill/{version}"
            write(
                skill_root / "agent.json",
                manifest(
                    "skill",
                    "skill",
                    version,
                    tools=[f"@fixture/echo@{version}"],
                    skill={"entrypoint": "SKILL.md", "scripts": ["scripts/not-automatic.cjs"]},
                ),
            )
            (skill_root / "SKILL.md").write_text(
                f"# Skill {version}\nUse the selected Tool.\n", encoding="utf-8"
            )
            script = skill_root / "scripts/not-automatic.cjs"
            script.parent.mkdir()
            script.write_text(
                "require('node:fs').writeFileSync("
                + json.dumps(str(root / "skill-script-ran"))
                + ", 'unexpected');",
                encoding="utf-8",
            )
            # Deliberately omit a dedicated Skill lock root: select its exact
            # authored Tool edge within the selected Agent's closure.
            agent.update(
                tools=[f"@fixture/echo@{version}"],
                skills=[f"@fixture/skill@{version}"],
                bindings={"global": {"skills": ["@fixture/skill"], "tools": ["@fixture/echo"]}},
            )
            selected.update(tools=[tool_key], skills=[skill_key])
            if major == 2:
                agent["tools"] = []
                selected["tools"] = []
                agent["bindings"]["global"].pop("tools")
            current_loop_key = f"loop:@fixture/workflow@{version}"
            packages[current_loop_key] = dict(packages[loop_key], version=version)
            current_loop = dict(loop, version=version)
            write(root / f".agentpm/loops/fixture/workflow/{version}/agent.json", current_loop)
            selected["loop"] = current_loop_key
            agent["loop"] = f"@fixture/workflow@{version}"
            for kind, field in [
                ("knowledge", "knowledge"),
                ("memory", "memory"),
                ("profile", "profiles"),
            ]:
                component = root / f".agentpm/{field}/fixture/{kind}/{version}"
                component.mkdir(parents=True)
                subprocess.run(
                    [str(cli), "init", "--kind", kind],
                    cwd=component,
                    check=True,
                    capture_output=True,
                )
                value = json.loads((component / "agent.json").read_text(encoding="utf-8"))
                value.update(name=kind, version=version)
                if kind == "memory":
                    value["memory"]["operations"] = {
                        "prune": {
                            "type": "delete",
                            "description": "An external operation on an unbound space.",
                            "trigger": {"type": "external"},
                            "targets": [{"space": "profile"}],
                            "cascade_derived_records": False,
                        }
                    }
                write(component / "agent.json", value)
                if kind in ["knowledge", "memory"]:
                    subprocess.run(
                        [str(cli), kind, "build"], cwd=component, check=True, capture_output=True
                    )
                component_key = f"{kind}:@fixture/{kind}@{version}"
                packages[component_key] = dict(
                    kind=kind,
                    name=f"@fixture/{kind}",
                    version=version,
                    integrity="fixture",
                    standard=STANDARD,
                    standard_origin="declared",
                )
                agent[field] = [f"@fixture/{kind}@{version}"]
                selected[field] = [component_key]
                agent["bindings"]["global"][field] = (
                    [{"package": "@fixture/memory", "operations": ["prune"]}]
                    if kind == "memory"
                    else [f"@fixture/{kind}"]
                )

        if mode == "no-loop":
            agent.pop("loop")
            selected.pop("loop")
        if mode == "legacy":
            agent.pop("standard")
            selected["standard_origin"] = "legacy_inferred"
        if mode == "unsupported":
            agent["standard"] = {"id": "agentpm", "version": "9.0.0"}
            selected["standard"] = agent["standard"]
        write(root / path, agent)
        roots[key] = selected
    if mode == "missing-model":
        config.pop("model")
        write(root / "agentpm.harness.json", config)
    write(
        root / "agent.lock",
        {
            "lockfile_version": 4,
            "generated": "2026-11-21T00:00:00Z",
            "packages": packages,
            "roots": roots,
        },
    )
    lint_inputs = list(root.glob(".agentpm/**/agent.json"))
    lint_inputs.extend(
        root.glob("agents/*.agent.json") if mode == "versions" else [root / "agent.json"]
    )
    for source in lint_inputs:
        result = subprocess.run(
            [str(cli), "lint", str(source), "--format", "json"],
            cwd=root,
            text=True,
            encoding="utf-8",
            capture_output=True,
            check=False,
        )
        expected = 1 if mode in ["legacy", "unsupported"] and source == root / "agent.json" else 0
        assert result.returncode == expected, result.stdout + result.stderr
    # Capture the actual standalone surface for SDK machine-result comparison.
    if mode != "versions":
        result = subprocess.run(
            [str(cli), "harness", "--json"],
            cwd=root,
            text=True,
            encoding="utf-8",
            capture_output=True,
            check=False,
        )
        report = json.loads(result.stdout)
        assert result.returncode == (0 if mode in ["ready", "legacy"] else 1), result.stderr
        write(root / "expected-preflight.json", report)
    return root


if __name__ == "__main__":
    prepare(Path(sys.argv[1]), sys.argv[2], sys.argv[3])
