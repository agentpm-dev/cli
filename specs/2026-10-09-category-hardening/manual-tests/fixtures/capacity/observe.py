"""Test-only Gunicorn hooks: measure real storage streams and APDS extraction."""
# Imports and instrumentation must happen after Gunicorn loads/forks the app.
# pylint: disable=import-outside-toplevel,protected-access

import json
import os
import runpy
import signal
import threading
import time
from pathlib import Path
from tempfile import gettempdir

import boto3
from flask import has_request_context, request

LOCK = threading.Lock()
TEMP_ROOT = Path(gettempdir())
PRODUCTION_HOOKS = runpy.run_path("/srv/gunicorn.conf.py")


def emit(event, **fields):
    record = {"event": event, "at": time.time(), "pid": os.getpid(), **fields}
    if has_request_context():
        record["request_id"] = request.headers.get("X-Request-Id")
    raw = (json.dumps(record) + "\n").encode()
    # One append write, process-safe for these small local records.
    with LOCK:
        descriptor = os.open(
            "/evidence/events.jsonl", os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600
        )
        try:
            os.write(descriptor, raw)
        finally:
            os.close(descriptor)


class Body:
    def __init__(self, body, key):
        self.body, self.key, self.count = body, key, 0

    def read(self, *args, **kwargs):
        if (
            self.count == 0
            and has_request_context()
            and request.headers.get("X-Capacity-Delay") == "35"
        ):
            emit("controlled_delay", seconds=35)
            time.sleep(35)
        data = self.body.read(*args, **kwargs)
        self.count += len(data)
        emit("storage_read", key=self.key, bytes=len(data))
        return data

    def close(self):
        return self.body.close()

    def __getattr__(self, name):
        return getattr(self.body, name)


def storage_parameters(params, context, **_kwargs):
    context["capacity_key"] = params["Key"]


def storage_response(parsed, context, **_kwargs):
    key = context["capacity_key"]
    if has_request_context():
        request.environ["capacity.key"] = key
    emit("storage_get", key=key, content_length=parsed.get("ContentLength"))
    parsed["Body"] = Body(parsed["Body"], key)


def sample():
    while True:
        try:
            rss = {}
            for entry in Path("/proc").glob("[0-9]*/status"):
                try:
                    values = dict(
                        line.split(":", 1)
                        for line in entry.read_text().splitlines()
                        if ":" in line
                    )
                    if values["Name"].strip() in {"gunicorn", "python"}:
                        rss[entry.parent.name] = (
                            int(values.get("VmRSS", "0 kB").split()[0]) * 1024
                        )
                except (OSError, ValueError, KeyError):
                    continue
            files = {}
            for entry in TEMP_ROOT.rglob("*"):
                try:
                    if entry.is_file():
                        status = entry.stat()
                        files[(status.st_dev, status.st_ino)] = status
                except OSError:
                    continue
            # TemporaryFile snapshots are unnamed; directory walks alone miss
            # their compressed bytes. Include open/deleted /tmp file descriptors.
            for entry in Path("/proc").glob("[0-9]*/fd/*"):
                try:
                    if os.readlink(entry).startswith(str(TEMP_ROOT) + os.sep):
                        status = entry.stat()
                        files[(status.st_dev, status.st_ino)] = status
                except OSError:
                    continue
            cgroup = Path("/sys/fs/cgroup")
            disk = os.statvfs(TEMP_ROOT)
            emit(
                "sample",
                rss=rss,
                memory_current=int((cgroup / "memory.current").read_text()),
                memory_peak=int((cgroup / "memory.peak").read_text()),
                memory_events=(cgroup / "memory.events").read_text(),
                cpu_stat=(cgroup / "cpu.stat").read_text(),
                temp_bytes=sum(item.st_size for item in files.values()),
                temp_allocated_bytes=sum(
                    item.st_blocks * 512 for item in files.values()
                ),
                disk_free=disk.f_bavail * disk.f_frsize,
            )
        except (OSError, ValueError) as error:
            emit("sampling_error", message=str(error))
        time.sleep(0.2)


def post_worker_init(worker):
    from app.rate_limit import limiter
    from app.tools.application import publish_apds

    # Capacity probes deliberately bypass request pacing, not authentication,
    # signing, archive validation or DB transactions. Product code is unedited.
    limiter.enabled = False

    boto3.setup_default_session()
    boto3.DEFAULT_SESSION.events.register(
        "before-parameter-build.s3.GetObject", storage_parameters
    )
    boto3.DEFAULT_SESSION.events.register("after-call.s3.GetObject", storage_response)
    original = publish_apds._extract

    def extraction(archive, root):
        members = archive.getmembers()
        emit(
            "extract_start",
            bytes=sum(max(0, member.size) for member in members),
            root=str(root),
            key=request.environ.get("capacity.key") if has_request_context() else None,
        )
        try:
            # Deliberately pause after writing real bytes while the directory
            # lock/context is live. The watchdog must kill this worker and the
            # production master hook must reclaim its orphan afterward.
            if has_request_context() and request.headers.get(
                "X-Capacity-Extraction-Delay"
            ):
                target = request.headers["X-Capacity-Extraction-Delay"]
                if target == "root" or (
                    target == "dependency"
                    and root.name.startswith("agentpm-dependency-apds-")
                ):
                    extract_member = archive.extract

                    def slow_member(member, *args, **kwargs):
                        extract_member(member, *args, **kwargs)
                        if member.name == "payload.bin":
                            emit(
                                "controlled_extraction_delay",
                                root=str(root),
                                bytes=member.size,
                            )
                            # Force the watchdog's SIGKILL escalation so Python
                            # finally/TemporaryDirectory cleanup cannot help.
                            signal.signal(signal.SIGABRT, signal.SIG_IGN)
                            time.sleep(35)

                    archive.extract = slow_member
            return original(archive, root)
        finally:
            emit("extract_end", root=str(root))

    publish_apds._extract = extraction
    emit(
        "worker",
        timeout=worker.cfg.timeout,
        workers=worker.cfg.workers,
        threads=worker.cfg.threads,
        worker_class=str(worker.cfg.worker_class),
        architecture=os.uname().machine,
    )
    threading.Thread(target=sample, daemon=True).start()


def when_ready(server):
    emit("master", timeout=server.cfg.timeout, workers=server.cfg.workers)


def on_starting(server):
    PRODUCTION_HOOKS["on_starting"](server)


def post_fork(server, worker):
    PRODUCTION_HOOKS["post_fork"](server, worker)


def child_exit(server, worker):
    PRODUCTION_HOOKS["child_exit"](server, worker)
    emit("reaped", worker_pid=worker.pid)


# Extend the actual production logging configuration, without changing defaults.
accesslog = "-"  # pylint: disable=invalid-name
errorlog = "-"  # pylint: disable=invalid-name
loglevel = "info"  # pylint: disable=invalid-name
