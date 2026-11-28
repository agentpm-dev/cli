import importlib.util
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

workspace = Path.cwd().parent
cli = workspace / "agentpm/target/debug/agentpm"
spec = importlib.util.spec_from_file_location(
    "helper",
    workspace / "agentpm/crates/agentpm-sdk/contracts/sdk-v1/harness_fixture.py",
)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
actions = json.loads(Path("/private/tmp/m9b-ui/actions.json").read_text())
root = Path(tempfile.mkdtemp(prefix="m9b-contract-"))
env = dict(os.environ, AGENTPM_TELEMETRY="0", DO_NOT_TRACK="1")
for name in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "AGENTPM_HARNESS_CONFIG"]:
    env.pop(name, None)
for name in ["XDG_CONFIG_HOME", "XDG_CACHE_HOME", "APPDATA", "LOCALAPPDATA"]:
    (root / name).mkdir()
    env[name] = str(root / name)
rows = []


def invoke(command, cwd, expected):
    result = subprocess.run(
        command, cwd=cwd, env=env, capture_output=True, text=True, timeout=45
    )
    assert result.returncode == expected, result.stdout + result.stderr
    return result


for mode in ["ready", "no-loop", "missing-model", "legacy", "unsupported"]:
    case = root / mode
    helper.prepare(case, cli, mode)
    source = json.loads((case / "agent.json").read_text())
    source["name"] = "consumer"
    helper.write(case / ".agentpm/agents/fixture/consumer/1.0.0/agent.json", source)
    lock = json.loads((case / "agent.lock").read_text())
    key = "agent:@fixture/consumer@1.0.0"
    lock["packages"][key] = {
        "kind": "agent",
        "name": "@fixture/consumer",
        "version": "1.0.0",
        "integrity": "fixture",
        "standard": source.get("standard", helper.STANDARD),
        "standard_origin": "legacy_inferred" if mode == "legacy" else "declared",
    }
    lock["roots"][key] = lock["roots"].pop("local:agent")
    lock["roots"][key].pop("name", None)
    lock["roots"][key].pop("version", None)
    helper.write(case / "agent.lock", lock)
    # Only the installed root is selected by the exact package command.
    (case / "agent.json").unlink()
    before = hashlib.sha256((case / "agent.lock").read_bytes()).hexdigest()
    output = invoke(
        [str(cli), *actions["preflight"].split()[1:]],
        case,
        0 if mode in ["ready", "legacy"] else 1,
    )
    report = json.loads(output.stdout)
    rows.append(
        {
            "mode": mode,
            "command": actions["preflight"],
            "exit": output.returncode,
            "apds": report["apds"]["status"],
            "harness_support": report["harness_support"],
            "local_readiness": report["local_readiness"],
        }
    )
    assert hashlib.sha256((case / "agent.lock").read_bytes()).hexdigest() == before
    if mode == "missing-model":
        (case / "prompt-1.txt").write_text(
            "Review the incident.\n'quoted' $(not-a-command)"
        )
        result = invoke([str(cli), *actions["runExample"].split()[1:]], case, 1)
        assert (
            "model_not_configured" in result.stderr or "model" in result.stderr
        ), result.stderr
        assert "unexpected argument" not in result.stderr
        rows.append(
            {
                "mode": "headless-missing-model",
                "command": actions["runExample"],
                "exit": 1,
                "expected_block": "missing model; no execution",
            }
        )
    if mode in ["ready", "legacy", "no-loop"]:
        package = case / "node_modules/@agentpm"
        package.mkdir(parents=True)
        (package / "sdk").symlink_to(
            workspace / "agentpm-sdk-node", target_is_directory=True
        )
        (case / "snippet.mjs").write_text(
            actions["node"]
            + "\nconsole.log(JSON.stringify({version: agent.manifest.version, kind:agent.manifest.kind}));\n"
        )
        result = invoke(["node", str(case / "snippet.mjs")], case, 0)
        assert json.loads(result.stdout) == {"version": "1.0.0", "kind": "agent"}
        rows.append({"mode": mode, "sdk": "node", "exit": 0, "version": "1.0.0"})
        python_env = dict(env, PYTHONPATH=str(workspace / "agentpm-sdk-python/src"))
        result = subprocess.run(
            [
                str(workspace / "agentpm-sdk-python/.venv/bin/python"),
                "-c",
                actions["python"]
                + "\nimport json; print(json.dumps({'version':agent['manifest']['version'], 'kind':agent['manifest']['kind']}))",
            ],
            cwd=case,
            env=python_env,
            capture_output=True,
            text=True,
            timeout=20,
        )
        assert result.returncode == 0, result.stderr
        assert json.loads(result.stdout) == {"version": "1.0.0", "kind": "agent"}
        rows.append({"mode": mode, "sdk": "python", "exit": 0, "version": "1.0.0"})
# A configured remote provider with no credential is a different readiness failure.
case = root / "missing-credentials"
helper.prepare(case, cli, "ready")
config = json.loads((case / "agentpm.harness.json").read_text())
config["model"] = {"provider": "openai", "model": "fixture"}
helper.write(case / "agentpm.harness.json", config)
result = invoke([str(cli), "harness", "--json"], case, 1)
report = json.loads(result.stdout)
assert report["local_readiness"] == "not_ready", report
assert "OPENAI_API_KEY" in result.stdout, result.stdout
rows.append(
    {
        "mode": "missing-provider-credential",
        "exit": 1,
        "local_readiness": report["local_readiness"],
        "expected_block": "OPENAI_API_KEY absent; no provider contact",
    }
)
output = workspace / "agentpm/specs/2026-10-09-category-hardening/assets/m9b"
(output / "action-contracts.json").write_text(
    json.dumps(
        {
            "status": "pass",
            "source": "actual agentActions output, real CLI and SDKs; synthetic installed fixtures",
            "model_requests": 0,
            "registry_requests": 0,
            "rows": rows,
        },
        indent=2,
    )
    + "\n"
)
(output / "displayed-actions.json").write_text(json.dumps(actions, indent=2) + "\n")
print(
    json.dumps(
        {"status": "pass", "checked": len(rows), "models": 0, "registry_requests": 0}
    )
)
