"""One-command, isolated Docker APDS capacity experiment. Host uses stdlib only."""
# Keep workload assembly and its explicit configuration together for review.
# pylint: disable=too-many-locals,too-many-statements,too-many-branches,line-too-long

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess  # nosec B404
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[4]


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def command(argv, *, log=None, check=True):
    # Fixed Docker/Git argv or the operator-selected local CLI; never a shell.
    if log is None:
        return subprocess.run(
            argv, check=check, text=True, capture_output=True
        )  # nosec B603
    with log.open("a", encoding="utf-8") as output:
        return subprocess.run(  # nosec B603
            argv, stdout=output, stderr=subprocess.STDOUT, check=check, text=True
        )


def context(root, api):
    destination = root / "context"
    destination.mkdir()
    source = destination / "api"
    source.mkdir()
    # No dotenv, credentials, host caches, private reports or source-control data.
    for name in ["app", "blog", "docs", "legal"]:
        shutil.copytree(
            api / name,
            source / name,
            ignore=shutil.ignore_patterns(
                "__pycache__", "*.pyc", ".env*", ".git", ".DS_Store"
            ),
        )
    for name in [
        "requirements.txt",
        "run.py",
        "run_migrations.py",
        "gunicorn.conf.py",
        ".pyway.conf",
    ]:
        shutil.copy2(api / name, source / name)
    shutil.copytree(
        HERE, destination / "harness", ignore=shutil.ignore_patterns("__pycache__")
    )
    shutil.copy2(HERE / "Dockerfile", destination / "Dockerfile")
    (source / "scripts").mkdir(exist_ok=True)
    shutil.copy2(
        api / "scripts/inspect_publication_capacity.py",
        source / "scripts/inspect_publication_capacity.py",
    )
    save(
        root / "source-checksums.json",
        {
            str(path.relative_to(destination)): hashlib.sha256(
                path.read_bytes()
            ).hexdigest()
            for path in sorted(destination.rglob("*"))
            if path.is_file()
        },
    )


def example_graphs(root):
    """Mirror the heaviest complete local examples without touching their sources."""
    examples = REPO.parent / "agentpm-examples"
    tracked = command(["git", "-C", str(examples), "ls-files", "-z"]).stdout.split("\0")
    categories = {
        "agent-packages",
        "template-packages",
        "tools-node",
        "tools-python",
        "skill-packages",
        "knowledge-packages",
        "memory-packages",
        "profile-packages",
        "loop-packages",
    }
    definitions = {}
    for name in tracked:
        parts = Path(name).parts
        if len(parts) == 3 and parts[0] in categories and parts[-1] == "agent.json":
            value = json.loads((examples / name).read_text())
            definitions[(value["kind"], value["name"])] = (name, value)

    def dependencies(value):
        if value["kind"] not in {"agent", "template", "skill"}:
            return
        owner = (
            value.get("template", {}).get("dependencies", {})
            if value["kind"] == "template"
            else value
        )
        for field, kind in [
            ("tools", "tool"),
            ("skills", "skill"),
            ("knowledge", "knowledge"),
            ("memory", "memory"),
            ("profiles", "profile"),
            ("agents", "agent"),
            ("loop", "loop"),
        ]:
            items = owner.get(field, [])
            if isinstance(items, str):
                items = [items]
            for item in items:
                name = item if isinstance(item, str) else item["name"]
                match = re.fullmatch(r"@[^/]+/([^@]+)(?:@.+)?", name)
                if not match:
                    raise ValueError("Unrecognized example dependency: " + name)
                yield kind, match[1]

    def closure(key, ordered, visiting):
        if key in ordered:
            return
        if key in visiting:
            raise ValueError("Example dependency cycle")
        visiting.add(key)
        for dependency in dependencies(definitions[key][1]):
            closure(dependency, ordered, visiting)
        visiting.remove(key)
        ordered.append(key)

    candidates = []
    chosen = {}
    merged = []
    for kind in ["agent", "template"]:
        complete = []
        for key in definitions:
            if key[0] != kind:
                continue
            ordered = []
            try:
                closure(key, ordered, set())
                complete.append((len(ordered), key, ordered))
                candidates.append(
                    {
                        "kind": kind,
                        "name": key[1],
                        "nodes": len(ordered),
                        "complete": True,
                    }
                )
            except (KeyError, ValueError) as error:
                candidates.append(
                    {
                        "kind": kind,
                        "name": key[1],
                        "complete": False,
                        "unavailable": str(error),
                    }
                )
        if not complete:
            raise ValueError("No complete local example graph for " + kind)
        _, key, ordered = max(complete)
        chosen[kind] = list(key)
        for item in ordered:
            if item not in merged:
                merged.append(item)
    nodes = []
    for index, key in enumerate(merged):
        name, value = definitions[key]
        prefix = str(Path(name).parent) + "/"
        destination = root / "example-inputs" / str(index)
        destination.mkdir(parents=True)
        for filename in tracked:
            if filename.startswith(prefix):
                relative = Path(filename.removeprefix(prefix))
                if any(
                    part.startswith(".env")
                    or part in {".agentpm", "node_modules", ".venv"}
                    for part in relative.parts
                ):
                    continue
                source = examples / filename
                if source.is_symlink():
                    raise ValueError("Refusing example symlink: " + filename)
                target = destination / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
        nodes.append(
            {"key": list(key), "index": index, "manifest": value, "original": name}
        )
    save(
        root / "example-inventory.json",
        {
            "candidates": candidates,
            "selected_roots": chosen,
            "nodes": nodes,
            "revision": command(
                ["git", "-C", str(examples), "rev-parse", "HEAD"]
            ).stdout.strip(),
            "adaptations": [
                "New test names, namespace, standard and 0.1.0 versions; exact local source dependency topology",
                "Tool runtimes replaced with inert Node fixture; no resolver or model execution",
                "Knowledge/Memory rebuilt by CLI; real authored content copied from tracked files only",
                "Most nodes among locally complete graphs; unavailable candidates explicitly reported",
            ],
        },
    )


def compose(root, project, gate, *, instrument=True):
    environment = {
        "FLASK_ENV": "prod",
        "DATABASE_HOST": "database",
        "DATABASE_PORT": "5432",
        "DATABASE_NAME": "capacity",
        "DATABASE_USERNAME": "capacity",
        "DATABASE_PASSWORD": "capacity-local-only",
        "DATABASE_URI": "postgresql://capacity:capacity-local-only@database:5432/capacity",
        "CACHE_REDIS_HOST": "cache",
        "AWS_ACCESS_KEY_ID": "capacity-local-only",
        "AWS_SECRET_ACCESS_KEY": "capacity-local-only",
        "AWS_DEFAULT_REGION": "us-east-1",
        "AWS_REGION": "us-east-1",
        "AWS_ENDPOINT_URL_S3": "http://storage:5000",
        "AWS_ENDPOINT_URL_GUARDDUTY": "http://storage:5000",
        "AWS_EC2_METADATA_DISABLED": "true",
        "S3_BUCKET": "agentpm-capacity",
        "REGISTRY_KEY_ID": "capacity-local-only",
        "REGISTRY_PRIVATE_KEY_B64": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "REGISTRY_CANONICAL_ORIGIN": "http://registry:8000",
        "RELEASE_PUBLISH_ENABLED": "true",
        "APDS_PUBLISH_ENFORCED": str(gate).lower(),
        "POSTHOG_API_KEY": "",
        "CLERK_ISSUER": "http://storage:5000",
        "PYTHON_DOTENV_DISABLED": "1",
        "GUNICORN_CMD_ARGS": "--bind 0.0.0.0:8000 --config "
        + ("/harness/observe.py" if instrument else "/srv/gunicorn.conf.py"),
    }
    common = {
        "image": f"{project}-registry",
        "environment": environment,
        "volumes": [f"{root}:/evidence"],
        "networks": ["isolated"],
    }
    document = {
        "services": {
            "database": {
                "image": "pgvector/pgvector:pg16",
                "environment": {
                    "POSTGRES_USER": "capacity",
                    "POSTGRES_PASSWORD": "capacity-local-only",
                    "POSTGRES_DB": "capacity",
                },
                "networks": ["isolated"],
                "healthcheck": {
                    "test": ["CMD-SHELL", "pg_isready -U capacity"],
                    "interval": "2s",
                    "timeout": "2s",
                    "retries": 60,
                },
            },
            "cache": {"image": "redis:7.4-alpine", "networks": ["isolated"]},
            "storage": {
                "image": "ghcr.io/getmoto/motoserver:5.2.4",
                "networks": ["isolated"],
            },
            "registry": {
                **common,
                "build": str(root / "context"),
                "command": ["gunicorn", "run:app"],
                "cpus": 0.5,
                "mem_limit": "512m",
                "memswap_limit": "512m",
                "healthcheck": {
                    "test": [
                        "CMD",
                        "python",
                        "-c",
                        "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/health',timeout=2)",
                    ],
                    "interval": "3s",
                    "timeout": "3s",
                    "retries": 180,
                },
                "depends_on": {
                    "database": {"condition": "service_healthy"},
                    "cache": {"condition": "service_started"},
                    "storage": {"condition": "service_started"},
                },
            },
            "driver": {
                **common,
                "profiles": ["tools"],
                "entrypoint": ["python", "/harness/workload.py"],
            },
        },
        "networks": {"isolated": {"internal": True}},
    }
    path = root / "compose.json"
    save(path, document)
    return [
        "docker",
        "compose",
        "--profile",
        "tools",
        "--project-name",
        project,
        "--file",
        str(path),
    ]


def main():
    started = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile", choices=["quick", "representative"], default="representative"
    )
    parser.add_argument("--cli", type=Path, default=REPO / "target/debug/agentpm")
    args = parser.parse_args()
    os.umask(0o077)
    api = REPO.parent / "agentpm-api"
    if not (api / "run.py").is_file():
        parser.error("Expected sibling agentpm-api checkout")
    engine = command(["docker", "info", "--format", "{{json .}}"])
    command(["docker", "compose", "version"])
    if not args.cli.is_file():
        parser.error("Build the local CLI first: cargo build --locked -p agentpm-cli")
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    root = (
        REPO
        / "release-band1-evidence"
        / "capacity"
        / (stamp + "-" + uuid.uuid4().hex[:6])
    )
    root.mkdir(parents=True, mode=0o700)
    project = "agentpm-capacity-" + uuid.uuid4().hex[:10]
    print("Evidence:", root, flush=True)
    setup = {
        "project": project,
        "profile": args.profile,
        "started": stamp,
        "cli": str(args.cli.resolve()),
        "cli_sha256": hashlib.sha256(args.cli.read_bytes()).hexdigest(),
        "production_changed": False,
    }
    for name, path in [("cli", REPO), ("api", api)]:
        setup[name + "_revision"] = command(
            ["git", "-C", str(path), "rev-parse", "HEAD"]
        ).stdout.strip()
        setup[name + "_status"] = command(
            ["git", "-C", str(path), "status", "--short"]
        ).stdout
    save(root / "setup.json", setup)
    save(root / "docker-engine.json", json.loads(engine.stdout))
    context(root, api)
    example_graphs(root)
    base = compose(root, project, False)
    log = root / "orchestration.log"
    success = False
    try:
        print(
            "Building isolated Registry image (first run downloads dependencies)…",
            flush=True,
        )
        command(base + ["build", "registry"], log=log)
        for gate in [False, True]:
            base = compose(root, project, gate)
            print("Starting Registry; enforcement", "on" if gate else "off", flush=True)
            command(
                base
                + ["up", "--detach", "--wait", "--wait-timeout", "600", "registry"],
                log=log,
            )
            command(
                base + ["run", "--rm", "driver", "prepare", "--profile", args.profile],
                log=log,
            )
            # Lint all newly authored definitions with the actual host CLI.
            for manifest in sorted((root / "sources").glob("*/agent.json")):
                kind = json.loads(manifest.read_text())["kind"]
                if "-real-" in str(manifest) and kind in {"knowledge", "memory"}:
                    command(
                        [
                            str(args.cli.resolve()),
                            kind,
                            "build",
                            "--manifest",
                            str(manifest),
                        ],
                        log=root / "lint.log",
                    )
                command(
                    [str(args.cli.resolve()), "lint", str(manifest)],
                    log=root / "lint.log",
                )
            command(
                base + ["run", "--rm", "driver", "measure", "--profile", args.profile],
                log=log,
            )
            identity = command(base + ["ps", "-q", "registry"]).stdout.strip()
            details = json.loads(command(["docker", "inspect", identity]).stdout)[0]
            save(
                root / f"container-{'on' if gate else 'off'}.json",
                {key: details[key] for key in ["Id", "Image", "State", "HostConfig"]},
            )
            resources = details["HostConfig"]
            if (
                resources["Memory"],
                resources["MemorySwap"],
                resources["NanoCpus"],
            ) != (512 * 1024**2, 512 * 1024**2, 500000000):
                raise RuntimeError(
                    "Docker did not apply the requested CPU/RAM/swap limits"
                )
            if details["State"].get("OOMKilled") or details["RestartCount"]:
                raise RuntimeError("Registry OOM/restart; inspect retained evidence")
            command(
                base + ["logs", "--no-color", "registry"], log=root / "registry.log"
            )
        # Validate the operational probe against the normal production hooks
        # with the real Redis limiter enabled, outside the measured matrix.
        base = compose(root, project, True, instrument=False)
        command(
            base + ["up", "--detach", "--wait", "--wait-timeout", "600", "registry"],
            log=log,
        )
        diagnostic = command(
            base
            + [
                "exec",
                "-T",
                "registry",
                "python",
                "scripts/inspect_publication_capacity.py",
            ]
        )
        probe = json.loads(diagnostic.stdout)
        save(root / "local-operational-probe.json", probe)
        if not probe["rate_limiter"]["redis_request_confirmed"]:
            raise RuntimeError(
                "Live Redis limiter was not confirmed with production hooks"
            )
        success = True
    finally:
        command(base + ["logs", "--no-color"], log=root / "services.log", check=False)
        cleanup = command(
            base + ["down", "--volumes", "--remove-orphans"], log=log, check=False
        )
        save(
            root / "execution.json",
            {
                "completed": False,
                "workloads_completed": success,
                "elapsed_seconds": time.monotonic() - started,
                "cleanup_succeeded": cleanup.returncode == 0,
                "activation_approved": False,
            },
        )
        print("Test teardown attempted. Evidence:", root, flush=True)
    command(
        [
            "docker",
            "run",
            "--rm",
            "--network",
            "none",
            "--volume",
            f"{root}:/evidence",
            f"{project}-registry",
            "python",
            "/harness/workload.py",
            "report",
        ],
        log=log,
    )
    execution = json.loads((root / "execution.json").read_text())
    if not execution["cleanup_succeeded"]:
        raise RuntimeError("Docker cleanup failed; inspect orchestration.log")
    execution["completed"] = True
    save(root / "execution.json", execution)
    print((root / "REPORT.md").read_text(), flush=True)


if __name__ == "__main__":
    try:
        main()
    except (subprocess.CalledProcessError, RuntimeError) as fatal_error:
        print(
            "Capacity run failed; retain logs for diagnosis:",
            fatal_error,
            file=sys.stderr,
        )
        sys.exit(1)
