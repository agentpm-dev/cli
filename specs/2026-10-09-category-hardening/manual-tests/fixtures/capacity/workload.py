"""Container-side real HTTP workloads and evidence; no production configuration."""
# Explicit workload/SQL/report assembly is test code, not a service abstraction.
# pylint: disable=too-many-locals,too-many-statements,line-too-long

import argparse
import base64
import concurrent.futures
import gzip
import hashlib
import http.client
import io
import json
import os
import re
import shutil
import tarfile
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

import boto3
import psycopg
import rfc8785
from nacl.signing import SigningKey

ROOT = Path("/evidence")
BASE = "http://registry:8000"
STANDARD = {"id": "agentpm", "version": "1.0.0"}
GATE = os.environ.get("APDS_PUBLISH_ENFORCED") == "true"
LANE = "on" if GATE else "off"
TARGETS = ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"]


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def database():
    if (
        os.environ.get("DATABASE_HOST") != "database"
        or os.environ.get("S3_BUCKET") != "agentpm-capacity"
    ):
        raise ValueError("Refusing non-capacity infrastructure")
    return psycopg.connect(
        "host=database dbname=capacity user=capacity password=capacity-local-only"
    )


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def wait_ready():
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        try:
            with build_opener(NoRedirect()).open(
                BASE + "/health", timeout=3
            ) as response:
                if response.status == 200:
                    return
        except (OSError, URLError, http.client.HTTPException):
            pass
        time.sleep(2)
    raise RuntimeError("Registry did not become healthy; inspect services.log")


def setup_identity():
    # Avoid loading application configuration during an offline report.
    # pylint: disable=import-outside-toplevel
    from app.generate_pat import generate_pat

    path = ROOT / "credentials.json"
    if path.exists():
        return
    token = generate_pat()
    key = SigningKey.generate()
    namespace = uuid.uuid4()
    with database() as connection:
        connection.execute(
            "INSERT INTO users(sub,email,email_verified_at) VALUES ('capacity','capacity@example.invalid',now())"
        )
        connection.execute(
            "INSERT INTO namespaces(namespace_id,kind,handle,owner_user_sub,signing_mode) "
            "VALUES (%s,'user','capacity','capacity','required')",
            (namespace,),
        )
        connection.execute(
            "INSERT INTO namespace_signers(namespace_id,algo,public_key_b64,label) VALUES (%s,'ed25519',%s,'capacity')",
            (namespace, base64.b64encode(bytes(key.verify_key)).decode()),
        )
        connection.execute(
            "INSERT INTO personal_access_tokens(user_sub,name,token_id,secret_hash,secret_preview,scopes) "
            "VALUES ('capacity','capacity',%s,%s,%s,%s)",
            (
                token.token_id,
                token.secret_hash,
                token.secret_preview,
                ["packages:publish", "namespaces:write"],
            ),
        )
    save(
        path, {"token": token.plaintext, "seed": base64.b64encode(bytes(key)).decode()}
    )
    boto3.client("s3").create_bucket(Bucket="agentpm-capacity")


def payload(path, size, random):
    if path.exists():
        return
    with path.open("wb") as output:
        while size:
            count = min(size, 1024 * 1024)
            output.write(os.urandom(count) if random else b"x" * count)
            size -= count


def definition(kind, name):
    value = {
        "kind": kind,
        "name": name,
        "version": "0.1.0",
        "description": "Isolated capacity experiment",
        "standard": STANDARD,
    }
    if kind == "tool":
        value.update(
            runtime={"type": "node", "version": "20"},
            files=["main.js", "payload.bin"],
            entrypoint={"command": "node", "args": ["main.js"]},
            inputs={"type": "object"},
            outputs={"type": "object"},
        )
    elif kind == "skill":
        value["skill"] = {"entrypoint": "SKILL.md"}
    elif kind == "template":
        value["template"] = {
            "display_name": "Capacity starter",
            "use_case": "starter",
            "files_root": "template",
            "execution_surfaces": ["agentpm-harness"],
            "dependencies": {"tools": [], "agents": []},
            "entrypoints": [{"label": "Read starter", "command": "cat README.md"}],
        }
    return value


def prepare(profile):
    wait_ready()
    setup_identity()
    small = 64 * 1024 if profile == "quick" else 1024 * 1024
    packed = 256 * 1024 if profile == "quick" else 32 * 1024 * 1024
    expanded = 1024 * 1024 if profile == "quick" else 128 * 1024 * 1024
    count = 4 if profile == "quick" else 20
    blobs = ROOT / "blobs"
    blobs.mkdir(exist_ok=True)
    for name, size, random in [
        ("leaf", small, True),
        ("packed", packed, True),
        ("expanded", expanded, False),
    ]:
        payload(blobs / name, size, random)
    sources = ROOT / "sources"
    sources.mkdir(exist_ok=True)
    plans = []

    def case(label, kind, blob="leaf", version="0.1.0", **extra):
        name = LANE + "-" + label
        folder = sources / (name + "-" + version)
        folder.mkdir(exist_ok=True)
        manifest = definition(kind, name)
        manifest["version"] = version
        if kind == "tool":
            (folder / "main.js").write_text("console.log('{}')\n")
        elif kind == "skill":
            (folder / "SKILL.md").write_text(
                "# Review\nReview the request with the available tools.\n"
            )
        elif kind == "template":
            (folder / "template").mkdir(exist_ok=True)
            (folder / "template/README.md").write_text("# Capacity starter\n")
        spec = {
            "label": label,
            "manifest": manifest,
            "source": str(folder),
            "blob": blob,
            **extra,
        }
        plans.append(spec)
        return spec

    leaves = [case(f"leaf-{number}", "tool") for number in range(count)]
    refs = [f"@capacity/{item['manifest']['name']}@0.1.0" for item in leaves]
    skills = [case(f"skill-{number}", "skill") for number in range(3)]
    for number, skill in enumerate(skills):
        skill["manifest"]["tools"] = refs[number::3]  # Shared paths form a diamond.
    skill_refs = [f"@capacity/{item['manifest']['name']}@0.1.0" for item in skills]
    for patch in range(3):
        agent = case(
            "agent-graph", "agent", version=f"0.1.{patch}", workload="graph-repeat"
        )
        agent["manifest"].update(tools=refs, skills=skill_refs)
    template = case("template-graph", "template", workload="graph")
    template["manifest"]["template"]["dependencies"] = {
        "tools": refs,
        "agents": [f"@capacity/{LANE}-agent-graph@0.1.0"],
    }
    case("compressed-large", "tool", "packed", workload="large-compressed")
    case("expanded-large", "tool", "expanded", workload="large-expanded")
    matrix = case(
        "target-release", "tool", "packed", protocol="release", workload="multi-target"
    )
    matrix["manifest"].update(
        runtime={"type": "python", "version": "3.11"},
        entrypoint={"command": "python", "args": ["main.py"]},
        files=["main.py", "payload.bin"],
    )
    (Path(matrix["source"]) / "main.py").write_text(
        "import json\nprint(json.dumps({}))\n"
    )
    for patch in range(2):
        agent = case(
            "concurrent-agent", "agent", version=f"0.1.{patch}", workload="concurrent"
        )
        agent["manifest"].update(tools=refs, skills=skill_refs)
    inventory = read(ROOT / "example-inventory.json")
    names = {
        node["key"][1]: f"{LANE}-real-{node['index']}" for node in inventory["nodes"]
    }

    def rewrite(value):
        if isinstance(value, str):
            match = re.fullmatch(r"@[^/]+/([^@]+)(@.+)?", value)
            if match and match[1] in names:
                return "@capacity/" + names[match[1]] + ("@0.1.0" if match[2] else "")
        if isinstance(value, list):
            return [rewrite(item) for item in value]
        if isinstance(value, dict):
            result = {key: rewrite(item) for key, item in value.items()}
            if (
                isinstance(value.get("name"), str)
                and value["name"].startswith("@")
                and "version" in value
            ):
                result["name"] = result["name"].split("@0.1.0")[0]
                result["version"] = "0.1.0"
            return result
        return value

    for node in inventory["nodes"]:
        kind, name = node["key"]
        folder = sources / names[name]
        shutil.copytree(
            ROOT / "example-inputs" / str(node["index"]), folder, dirs_exist_ok=True
        )
        value = rewrite(node["manifest"])
        value.update(name=names[name], version="0.1.0", standard=STANDARD)
        if kind == "tool":
            inert = definition("tool", names[name])
            for field in ["runtime", "entrypoint", "files"]:
                value[field] = inert[field]
            (folder / "main.js").write_text("console.log('{}')\n")
        roots = inventory["selected_roots"].values()
        plans.append(
            {
                "label": "real-" + str(node["index"]),
                "manifest": value,
                "source": str(folder),
                "blob": "leaf",
                "workload": "real-" + kind
                if node["key"] in roots
                else "real-dependency",
                "display": kind + ": " + name,
            }
        )
    if GATE:
        case(
            "missing-required",
            "skill",
            "expanded",
            workload="rejection",
            expected=422,
            omit="SKILL.md",
        )
        case("timeout", "tool", "packed", workload="timeout", timeout=True)
        case(
            "timeout-extraction",
            "tool",
            "expanded",
            workload="timeout",
            timeout=True,
            extraction_delay="root",
        )
        paused = case(
            "timeout-dependency",
            "agent",
            workload="timeout",
            timeout=True,
            extraction_delay="dependency",
        )
        paused["manifest"].update(tools=refs, skills=skill_refs)
    for item in plans:
        folder = Path(item["source"])
        if not (folder / "payload.bin").exists():
            os.link(blobs / item["blob"], folder / "payload.bin")
        (folder / "zz-remaining.txt").write_text(
            "Extraction must still process this member after payload.bin.\n"
        )
        # Payload declared only for Tools, but physically present in every archive.
        save(folder / "agent.json", item["manifest"])
    save(
        ROOT / f"plan-{LANE}.json",
        {
            "profile": profile,
            "leaf_count": count,
            "cases": plans,
            "payload_sizes": {p.name: p.stat().st_size for p in blobs.iterdir()},
        },
    )
    print(
        "Prepared",
        len(plans),
        "definitions for actual CLI lint before publication",
        flush=True,
    )


def archive(case, target):
    folder = Path(case["source"])
    path = ROOT / "archives" / (folder.name + "-" + target + ".tar.gz")
    path.parent.mkdir(exist_ok=True)
    with path.open("wb") as output, gzip.GzipFile(
        fileobj=output, mode="wb", mtime=0, filename=""
    ) as zipped:
        with tarfile.open(fileobj=zipped, mode="w|") as tar:
            for member in sorted(folder.rglob("*")):
                if member.is_file() and str(member.relative_to(folder)) != case.get(
                    "omit"
                ):
                    tar.add(
                        member, arcname=str(member.relative_to(folder)), recursive=False
                    )
            if target != "any":
                data = json.dumps({"target": target, "format": "tar.gz"}).encode()
                info = tarfile.TarInfo("agentpm-artifact.json")
                info.size, info.mode = len(data), 0o644
                tar.addfile(info, io.BytesIO(data))
    with path.open("rb") as content:
        digest = hashlib.file_digest(content, "sha256").hexdigest()
    with tarfile.open(path) as tar:
        expanded = sum(member.size for member in tar.getmembers())
    return (
        path,
        {
            "target": target,
            "digest": "sha256:" + digest,
            "size": path.stat().st_size,
            "format": "tar.gz",
            "contentType": "application/gzip",
        },
        expanded,
    )


def call(route, body, identity, *, delay=False, extraction_delay=None):
    headers = {
        "Authorization": "Bearer " + read(ROOT / "credentials.json")["token"],
        "Content-Type": "application/json",
        "X-Request-Id": identity,
    }
    if delay:
        headers["X-Capacity-Delay"] = "35"
    if extraction_delay:
        headers["X-Capacity-Extraction-Delay"] = extraction_delay
    request = Request(
        BASE + route, data=json.dumps(body).encode(), headers=headers, method="POST"
    )
    started = time.time()
    try:
        with build_opener(NoRedirect()).open(request, timeout=90) as response:
            status, raw = response.status, response.read()
    except HTTPError as error:
        status, raw = error.code, error.read()
    except (URLError, OSError, http.client.HTTPException) as error:
        return {"status": 0, "error": str(error)}, {
            "started": started,
            "ended": time.time(),
            "request_id": identity,
        }
    try:
        value = json.loads(raw)
    except ValueError:
        value = {"error": raw.decode(errors="replace")[:500]}
    return {"status": status, "body": value}, {
        "started": started,
        "ended": time.time(),
        "request_id": identity,
    }


def reserve(case):
    manifest = read(Path(case["source"]) / "agent.json")
    case["manifest"] = manifest
    release = case.get("protocol") == "release"
    artifacts = [archive(case, target) for target in (TARGETS if release else ["any"])]
    body = {"manifest": manifest, "namespace_handle": "capacity"}
    key = SigningKey(base64.b64decode(read(ROOT / "credentials.json")["seed"]))
    if release:
        document = {
            "type": "agentpm.package.release.v1",
            "kind": manifest["kind"],
            "name": "@capacity/" + manifest["name"],
            "version": manifest["version"],
            "manifestDigest": "sha256:"
            + hashlib.sha256(rfc8785.dumps(manifest)).hexdigest(),
            "artifacts": [item[1] for item in artifacts],
        }
        body["release"] = document
        statement = {
            "type": "agentpm.package.signature.v2",
            "kind": manifest["kind"],
            "name": document["name"],
            "version": manifest["version"],
            "releaseDigest": "sha256:"
            + hashlib.sha256(rfc8785.dumps(document)).hexdigest(),
        }
    else:
        body.update(sha256=artifacts[0][1]["digest"][7:], size=artifacts[0][1]["size"])
        statement = {
            "type": "agentpm.package.signature.v1",
            "kind": manifest["kind"],
            "name": manifest["name"],
            "version": manifest["version"],
            "artifactDigest": artifacts[0][1]["digest"],
        }
    statement["createdAt"] = (
        datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
    )
    message = (
        rfc8785.dumps(statement)
        if release
        else json.dumps(statement, sort_keys=True, separators=(",", ":")).encode()
    )
    signature = {
        "algo": "ed25519",
        "public_key_b64": base64.b64encode(bytes(key.verify_key)).decode(),
        "signature_b64": base64.b64encode(key.sign(message).signature).decode(),
        "statement_json": statement,
    }
    # Rate pacing is disabled only inside the isolated test worker.
    result, _ = call(
        "/v1/tools/publish/" + ("releases/init" if release else "init"),
        body,
        uuid.uuid4().hex,
    )
    if result["status"] != 200:
        raise RuntimeError("Reservation failed: " + json.dumps(result))
    receipt = result["body"]
    for path, descriptor, _expanded in artifacts:
        instruction = (
            next(
                item
                for item in receipt["artifacts"]
                if item["target"] == descriptor["target"]
            )
            if release
            else receipt
        )
        url = urlsplit(instruction["put_url"])
        if (url.scheme, url.hostname, url.port) != ("http", "storage", 5000):
            raise ValueError("Refusing upload outside isolated storage")
        headers = {**instruction["headers"], "Content-Length": str(path.stat().st_size)}
        with path.open("rb") as data:
            with build_opener(NoRedirect()).open(
                Request(
                    instruction["put_url"], data=data, headers=headers, method="PUT"
                ),
                timeout=120,
            ) as response:
                if response.status != 200:
                    raise RuntimeError("Upload failed")
    finalize = {"require_signature": True, "author_signatures": [signature]}
    route = "/v1/tools/publish/"
    if release:
        route += "releases/" + receipt["release_id"] + "/finalize"
    else:
        route += "finalize"
        finalize["upload_id"] = receipt["upload_id"]
    return {
        "case": case,
        "route": route,
        "body": finalize,
        "root_keys": [
            receipt[field] for field in ["tmp_key", "final_key"] if field in receipt
        ],
        "artifacts": [
            {**descriptor, "expanded": expanded}
            for _, descriptor, expanded in artifacts
        ],
    }


def finish(prepared):
    case = prepared["case"]
    request_id = uuid.uuid4().hex
    result, timing = call(
        prepared["route"],
        prepared["body"],
        request_id,
        delay=case.get("timeout", False) and not case.get("extraction_delay"),
        extraction_delay=case.get("extraction_delay"),
    )
    record = {
        "lane": LANE,
        "label": case["label"],
        "display": case.get("display", case["label"]),
        "version": case["manifest"]["version"],
        "protocol": case.get("protocol", "single"),
        "workload": case.get("workload", "dependency-seed"),
        "artifacts": prepared["artifacts"],
        "root_keys": prepared["root_keys"],
        "result": result,
        **timing,
    }
    save(ROOT / "requests" / (LANE + "-" + request_id + ".json"), record)
    expected = case.get("expected", 200)
    if case.get("timeout"):
        if result["status"] not in [0, 500, 502]:
            raise RuntimeError("Controlled timeout did not interrupt request")
        wait_ready()
    elif result["status"] != expected:
        raise RuntimeError("Unexpected finalize result: " + json.dumps(record))
    elif expected == 422 and result["body"].get("code") != "apds_nonconformant":
        raise RuntimeError("Unrelated error is not the expected APDS rejection")
    # Actual DB outcome, rather than interpreting an HTTP code as atomicity proof.
    with database() as connection:
        versions = connection.execute(
            "SELECT tv.standard_origin,tv.apds_validation FROM tool_versions tv "
            "JOIN tools t USING(tool_id) WHERE t.name=%s AND tv.version=%s",
            (case["manifest"]["name"], case["manifest"]["version"]),
        ).fetchall()
    if expected == 200 and not case.get("timeout"):
        if (
            len(versions) != 1
            or versions[0][0] != "declared"
            or bool(versions[0][1]) != GATE
        ):
            raise RuntimeError("Publication/evidence did not match enforcement mode")
    elif versions:
        raise RuntimeError("Rejected/interrupted finalize created a version")
    time.sleep(1)
    record["settled"] = time.time()
    save(ROOT / "requests" / (LANE + "-" + request_id + ".json"), record)
    print(
        LANE,
        case["label"],
        result["status"],
        f"{timing['ended'] - timing['started']:.2f}s",
        flush=True,
    )
    return record


def measure():
    plans = read(ROOT / f"plan-{LANE}.json")["cases"]
    queued = []
    for case in plans:
        if case.get("workload") == "concurrent":
            queued.append(reserve(case))
        else:
            finish(reserve(case))
    # Concurrent clients expose queueing even when production uses one sync worker.
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        list(executor.map(finish, queued))


def report():  # pylint: disable=too-many-branches
    events = [
        json.loads(line) for line in (ROOT / "events.jsonl").read_text().splitlines()
    ]
    rows = []
    records = sorted(
        (read(path) for path in (ROOT / "requests").glob("*.json")),
        key=lambda record: record["started"],
    )
    for record in records:
        related = [
            event for event in events if event.get("request_id") == record["request_id"]
        ]
        samples = [
            event
            for event in events
            if event["event"] == "sample"
            and record["started"] - 0.3 <= event["at"] <= record["ended"] + 0.5
        ]
        before = [
            event
            for event in events
            if event["event"] == "sample" and event["at"] < record["started"]
        ]
        after = [
            event
            for event in events
            if event["event"] == "sample"
            and record.get("settled", record["ended"] + 1) - 0.4
            <= event["at"]
            <= record.get("settled", record["ended"] + 1)
        ]
        root_digests = tuple(artifact["digest"][7:] for artifact in record["artifacts"])
        root_keys = tuple(record.get("root_keys", []))

        def is_root(event, digests=root_digests, keys=root_keys):
            key = event.get("key", "")
            return (
                key in keys
                or key.startswith("uploads/")
                or any(digest in key for digest in digests)
            )

        row = {
            "lane": record["lane"],
            "label": record["label"],
            "display": record.get("display", record["label"]),
            "version": record["version"],
            "workload": record["workload"],
            "status": record["result"]["status"],
            "seconds": round(record["ended"] - record["started"], 3),
            "gets": sum(event["event"] == "storage_get" for event in related),
            "root_gets": sum(
                event["event"] == "storage_get" and is_root(event) for event in related
            ),
            "dependency_gets": sum(
                event["event"] == "storage_get" and not is_root(event)
                for event in related
            ),
            "downloaded": sum(
                event.get("bytes", 0)
                for event in related
                if event["event"] == "storage_read"
            ),
            "dependency_downloaded": sum(
                event.get("bytes", 0)
                for event in related
                if event["event"] == "storage_read" and not is_root(event)
            ),
            "expanded_work": sum(
                event.get("bytes", 0)
                for event in related
                if event["event"] == "extract_start"
            ),
            "sampled_memory": max(
                (event["memory_current"] for event in samples), default=0
            ),
            "sampled_worker_rss": max(
                (sum(event["rss"].values()) for event in samples), default=0
            ),
            "sampled_temp": max((event["temp_bytes"] for event in samples), default=0),
            "temp_before": before[-1]["temp_bytes"] if before else None,
            "temp_after": after[0]["temp_bytes"] if after else None,
            "cleanup_shared_with_concurrent_request": record["workload"]
            == "concurrent",
        }
        rows.append(row)
    samples = [event for event in events if event["event"] == "sample"]
    workers = [event for event in events if event["event"] == "worker"]
    summary = {
        "rows": rows,
        "workers": workers,
        "sample_count": len(samples),
        "peak_cgroup_memory": max(event["memory_peak"] for event in samples),
        "sampling_errors": [
            event for event in events if event["event"] == "sampling_error"
        ],
        "activation_approved": False,
        "capacity_gate_complete": False,
    }
    errors = summary["sampling_errors"]
    if errors:
        raise RuntimeError("Sampler failed; capacity evidence is incomplete")
    for sample in samples:
        counters = dict(
            line.split() for line in sample.get("memory_events", "").splitlines()
        )
        if int(counters.get("oom_kill", 0)):
            raise RuntimeError("Registry cgroup recorded an OOM kill")
    timeout_count = sum(record["workload"] == "timeout" for record in records)
    if len(workers) != 2 + timeout_count:
        raise RuntimeError("Unexpected worker restart or missing worker configuration")
    for lane in ["off", "on"]:
        expected = read(ROOT / f"plan-{lane}.json")["cases"]
        actual = [row for row in rows if row["lane"] == lane]
        if len(actual) != len(expected):
            raise RuntimeError("Missing workload results")
        for case in expected:
            row = next(
                row
                for row in actual
                if (row["label"], row["version"])
                == (case["label"], case["manifest"]["version"])
            )
            if not row["sampled_memory"]:
                raise RuntimeError("Missing Registry memory samples")
            if row["gets"] == 0:
                raise RuntimeError("Missing storage instrumentation")
            if (
                lane == "on"
                and case.get("workload") != "timeout"
                and row["expanded_work"] == 0
            ):
                raise RuntimeError("Missing extraction instrumentation")
            if row["temp_before"] is None or row["temp_after"] is None:
                raise RuntimeError("Missing temporary-file cleanup samples")
            if (
                row["workload"] != "concurrent"
                and row["temp_before"] != row["temp_after"]
            ):
                raise RuntimeError("Sequential request left temporary-file residue")
            if case.get("workload") == "timeout":
                raw = next(
                    record
                    for record in records
                    if record["lane"] == lane and record["label"] == row["label"]
                )
                delayed = [
                    event
                    for event in events
                    if event.get("request_id") == raw["request_id"]
                    and event["event"]
                    in {"controlled_delay", "controlled_extraction_delay"}
                ]
                replacements = [
                    worker
                    for worker in workers
                    if delayed and delayed[0]["at"] < worker["at"] <= raw["settled"]
                ]
                if not delayed or len(replacements) != 1:
                    raise RuntimeError(
                        "Timeout did not produce the expected worker replacement"
                    )
            elif row["status"] != case.get("expected", 200):
                raise RuntimeError("Unexpected HTTP outcome in evidence")
        concurrent_records = [
            record
            for record in records
            if record["lane"] == lane and record["workload"] == "concurrent"
        ]
        if concurrent_records and max(
            record["started"] for record in concurrent_records
        ) >= min(record["ended"] for record in concurrent_records):
            raise RuntimeError("Concurrent client requests did not actually overlap")
    save(ROOT / "measurements.json", summary)
    text = [
        "# Local APDS capacity measurements",
        "",
        "Execution completed; activation approval remains pending.",
        "",
        "| Gate | Workload | HTTP | Seconds | GETs | Read MiB | Extract MiB | Sampled RAM MiB | Temp MiB |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in rows:
        if row["workload"] in {"dependency-seed", "real-dependency"}:
            continue
        text.append(
            f"| {row['lane']} | {row['display']} {row['version']} | {row['status']} | {row['seconds']} | "
            f"{row['gets']} | {row['downloaded']/1024**2:.1f} | {row['expanded_work']/1024**2:.1f} | "
            f"{row['sampled_memory']/1024**2:.1f} | {row['sampled_temp']/1024**2:.1f} |"
        )
    text.extend(
        [
            "",
            "Includes synthetic fan-out/diamond graphs and adapted mirrors "
            "of the largest locally complete example graphs.",
            "See example-inventory.json for selection, unavailable references and all adaptations; "
            "existing packages are unchanged.",
            "This is not a claim that every real package or supported maximum fits.",
            "The S3 service is Moto: real HTTP transfers, no real S3/network latency or durability measurement.",
            "Local CPU quota, architecture and disk differ from Render; production proxy timeout is not reproduced.",
            "RSS/temp samples are 200 ms observations; cgroup memory.peak also covers between-sample memory peaks.",
            "Timeout probes cover storage reads and SIGKILL during root/dependency extraction, "
            "delegating orphan recovery to the real production Gunicorn hooks.",
            "Rate limiting is disabled by the test hook to isolate finalize capacity; "
            "real PAT/authentication/signatures remain enabled.",
            "Raw request, worker, sampling, storage and extraction events are retained for independent review.",
            "The quick profile is tooling verification only. "
            "Claude must review the representative results and residual gaps.",
            "",
        ]
    )
    (ROOT / "REPORT.md").write_text("\n".join(text))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["prepare", "measure", "report"])
    parser.add_argument(
        "--profile", choices=["quick", "representative"], default="representative"
    )
    args = parser.parse_args()
    os.umask(0o077)
    if args.action == "prepare":
        prepare(args.profile)
    elif args.action == "measure":
        measure()
    else:
        report()
