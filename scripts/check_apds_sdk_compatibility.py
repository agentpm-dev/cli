#!/usr/bin/env python3
"""Probe supplied SDK installations and an older CLI using disposable v4 locks.

No Registry requests, installation or publication. Install SDK 0.1.17 separately
into temporary environments; see the M5A verification record for exact commands.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def check(command, root, env, *, success=True):
    result = subprocess.run(
        command, cwd=root, env=env, capture_output=True, text=True, timeout=60
    )
    if (result.returncode == 0) != success:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout + result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--older-cli", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--node-module", type=Path, required=True)
    args = parser.parse_args()
    node_version = json.loads(
        (args.node_module.absolute().parent.parent / "package.json").read_text()
    )["version"]
    python_version = subprocess.check_output(
        [
            str(args.python.absolute()),
            "-c",
            "import importlib.metadata; print(importlib.metadata.version('agentpm'))",
        ],
        text=True,
    ).strip()
    assert node_version == python_version == "0.1.17", (node_version, python_version)
    standard = {"id": "agentpm", "version": "1.0.0"}
    with tempfile.TemporaryDirectory(prefix="agentpm-apds-sdk-") as temporary:
        root = Path(temporary)
        packages = {}
        for kind in ("tool", "skill", "agent"):
            folder = root / ".agentpm" / (kind + "s") / "fixture" / kind / "1.0.0"
            folder.mkdir(parents=True)
            manifest = {
                "kind": kind,
                "name": kind,
                "version": "1.0.0",
                "description": "Compatibility probe",
            }
            if kind == "tool":
                manifest.update(
                    entrypoint={"command": "python", "args": ["tool.py"]},
                    inputs={"type": "object"},
                    outputs={"type": "object"},
                )
                (folder / "tool.py").write_text(
                    "import json,sys\nprint(json.dumps(json.load(sys.stdin)))\n"
                )
            else:
                manifest.update(standard=standard, tools=["@fixture/tool@1.0.0"])
                if kind == "agent":
                    manifest["skills"] = ["@fixture/skill@1.0.0"]
                else:
                    manifest["skill"] = {"entrypoint": "SKILL.md"}
                    (folder / "SKILL.md").write_text("# Procedure\n")
            (folder / "agent.json").write_text(json.dumps(manifest))
            packages[f"{kind}:@fixture/{kind}@1.0.0"] = {
                "kind": kind,
                "name": f"@fixture/{kind}",
                "version": "1.0.0",
                "integrity": "a" * 64,
                "standard": standard,
                "standard_origin": "legacy_inferred" if kind == "tool" else "declared",
            }
        local = {
            "kind": "agent",
            "name": "consumer",
            "version": "1.0.0",
            "standard": standard,
            "tools": ["@fixture/tool@1.0.0"],
        }
        (root / "agent.json").write_text(json.dumps(local))
        tool = "tool:@fixture/tool@1.0.0"
        lock = {
            "lockfile_version": 4,
            "generated": "2026-11-20T00:00:00Z",
            "packages": packages,
            "roots": {
                "local:agent": {
                    "name": "consumer",
                    "version": "1.0.0",
                    "tools": [tool],
                    "standard": standard,
                    "standard_origin": "declared",
                },
                "agent:@fixture/agent@1.0.0": {
                    "tools": [tool],
                    "skills": ["skill:@fixture/skill@1.0.0"],
                },
                "skill:@fixture/skill@1.0.0": {"tools": [tool]},
            },
        }
        path = root / "agent.lock"
        path.write_text(json.dumps(lock))
        before = path.read_bytes()
        env = dict(
            os.environ,
            AGENTPM=str(args.cli.resolve()),
            AGENTPM_PYTHON=str(args.python.absolute()),
            AGENTPM_TELEMETRY="0",
            XDG_CONFIG_HOME=str(root / "config"),
            APPDATA=str(root / "config"),
        )
        node = """import assert from 'node:assert/strict';
const sdk = await import(process.argv[1]);
assert.equal((await sdk.loadAgent('@fixture/agent@1.0.0')).resolvedTools[0].version, '1.0.0');
assert.equal((await sdk.loadSkill('@fixture/skill@1.0.0')).resolvedTools[0].version, '1.0.0');
const loaded = await sdk.load('@fixture/tool@1.0.0', {withMeta:true});
assert.deepEqual(await loaded.func({message:'unicode 😀', ok:true}), {message:'unicode 😀',ok:true});
"""
        python = """from agentpm import load,load_agent,load_skill
assert load_agent('@fixture/agent@1.0.0')['resolvedTools'][0]['version'] == '1.0.0'
assert load_skill('@fixture/skill@1.0.0')['resolvedTools'][0]['version'] == '1.0.0'
assert load('@fixture/tool@1.0.0',with_meta=True)['func']({'message':'unicode 😀','ok':True}) == {'message':'unicode 😀','ok':True}
"""
        check(
            [
                "node",
                "--input-type=module",
                "-e",
                node,
                args.node_module.resolve().as_uri(),
            ],
            root,
            env,
        )
        check([str(args.python.absolute()), "-c", python], root, env)
        assert path.read_bytes() == before
        older = check(
            [
                str(args.older_cli.resolve()),
                "run",
                "@fixture/tool",
                "--machine",
                "--input",
                "{}",
            ],
            root,
            env,
            success=False,
        )
        assert "unknown field" in older, older
        assert path.read_bytes() == before
        lock["lockfile_version"] = 5
        path.write_text(json.dumps(lock))
        future = path.read_bytes()
        check(
            [
                str(args.python.absolute()),
                "-c",
                "from agentpm import load_agent; load_agent('@fixture/agent@1.0.0')",
            ],
            root,
            env,
            success=False,
        )
        check(
            [
                "node",
                "--input-type=module",
                "-e",
                "const sdk=await import(process.argv[1]); await sdk.loadAgent('@fixture/agent@1.0.0');",
                args.node_module.resolve().as_uri(),
            ],
            root,
            env,
            success=False,
        )
        assert path.read_bytes() == future
        print(
            json.dumps(
                {
                    "node_sdk_version": node_version,
                    "python_sdk_version": python_version,
                    "extended_v4_public_agent_skill_loading_and_cli_invocation": "pass",
                    "sdk_future_version_rejection": "pass",
                    "older_cli_safe_unknown_field_rejection": "pass",
                    "lock_unchanged_sha256": hashlib.sha256(before).hexdigest(),
                },
                indent=2,
            )
        )


if __name__ == "__main__":
    main()
