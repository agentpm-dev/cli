"""Native execution gate for adopting an AgentPM-managed uv binary (Python 3.11+)."""

import hashlib
import json
import os
import platform
import re

# Only an attested, maintainer-selected executable is passed to this module.
import subprocess  # nosec B404
import tempfile
import tomllib
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
COMMANDS = ROOT / "platforms/uv-resolution-commands.json"
TARGETS = ROOT / "crates/agentpm-sdk/platforms/targets.json"
REQUIREMENTS = ["click==8.1.8", "pydantic==2.11.7"]


def native_target():
    machine = platform.machine().lower()
    system = platform.system()
    if system == "Darwin":
        return {
            "arm64": "aarch64-apple-darwin",
            "x86_64": "x86_64-apple-darwin",
        }.get(machine)
    if machine in ("amd64", "x86_64"):
        if system == "Windows":
            return "x86_64-pc-windows-msvc"
        if system == "Linux":
            if platform.libc_ver()[0] == "glibc":
                return "x86_64-unknown-linux-gnu"
            # CPython's executable scan may miss libc; query the running libc directly.
            try:
                libc = os.confstr("CS_GNU_LIBC_VERSION")
            except (AttributeError, OSError, ValueError):
                libc = None
            if libc and libc.startswith("glibc "):
                return "x86_64-unknown-linux-gnu"
    return None


def contract_digest():
    # Changes to the fixture, commands or supported environments invalidate old evidence.
    return hashlib.sha256(
        b"".join(
            path.read_text(encoding="utf-8").encode("utf-8")
            for path in (Path(__file__), COMMANDS, TARGETS)
        )
    ).hexdigest()


def validate_pylock(raw):
    lock = tomllib.loads(raw)
    if lock.get("lock-version") != "1.0":
        raise ValueError("uv smoke: unsupported pylock lock-version (expected 1.0)")
    packages = lock.get("packages", [])
    by_name = {}
    for package in packages:
        if not isinstance(package.get("name"), str) or not isinstance(
            package.get("version"), str
        ):
            raise ValueError("uv smoke: missing package name/version")
        if "marker" in package and not isinstance(package["marker"], str):
            raise ValueError("uv smoke: invalid package marker")
        artifacts = list(package.get("wheels", []))
        if package.get("sdist"):
            artifacts.append(package["sdist"])
        if not artifacts or any(
            not re.fullmatch("[0-9a-f]{64}", item.get("hashes", {}).get("sha256", ""))
            for item in artifacts
        ):
            raise ValueError("uv smoke: missing distribution SHA-256")
        by_name[package["name"]] = package
    for name, version in (
        ("click", "8.1.8"),
        ("pydantic", "2.11.7"),
        ("pydantic-core", "2.33.2"),
    ):
        if by_name.get(name, {}).get("version") != version:
            raise ValueError(f"uv smoke: missing fixture package {name}=={version}")
    if "win32" not in by_name.get("colorama", {}).get("marker", ""):
        raise ValueError("uv smoke: missing Windows-only transitive dependency marker")
    core = by_name["pydantic-core"]
    wheels = core.get("wheels", [])
    if len({wheel["hashes"]["sha256"] for wheel in wheels}) < 2:
        raise ValueError(
            "uv smoke: native dependency must retain multiple wheel digests"
        )
    filenames = [unquote(urlsplit(wheel.get("url", "")).path) for wheel in wheels]
    expected = {
        "aarch64-apple-darwin": r"-cp311-.*macosx_.*(?:arm64|universal2)\.whl$",
        "x86_64-apple-darwin": r"-cp311-.*macosx_.*(?:x86_64|universal2)\.whl$",
        "x86_64-unknown-linux-gnu": r"-cp311-.*manylinux.*x86_64\.whl$",
        "x86_64-pc-windows-msvc": r"-cp311-.*win_amd64\.whl$",
    }
    targets = {target["id"] for target in json.loads(TARGETS.read_text())["targets"]}
    if targets != set(expected):
        raise ValueError(
            "uv smoke: add wheel coverage assertions for the new target set"
        )
    for target, pattern in expected.items():
        if not any(re.search(pattern, filename) for filename in filenames):
            raise ValueError(f"uv smoke: missing native wheel for {target}")
    if not core.get("sdist"):
        raise ValueError(
            "uv smoke: native dependency lost its source-distribution digest"
        )


def smoke(binary, version, target):
    if target != native_target():
        raise ValueError(
            f"uv smoke: {target} requires a native runner, not {native_target()}"
        )
    binary = binary.resolve(strict=True)
    before = hashlib.sha256(binary.read_bytes()).hexdigest()
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("UV_", "AGENTPM_"))
    }
    with tempfile.TemporaryDirectory(prefix="agentpm-uv-smoke-") as root:
        directory = Path(root)
        environment.update(
            {key: root for key in ("HOME", "USERPROFILE", "APPDATA", "XDG_CONFIG_HOME")}
        )
        environment.update(
            UV_NO_SYSTEM_CONFIG="1", UV_CACHE_DIR=str(directory / "cache")
        )
        environments = [t["marker"] for t in json.loads(TARGETS.read_text())["targets"]]
        (directory / "pyproject.toml").write_text(
            "[project]\nname = 'agentpm-resolution-input'\nversion = '0.0.0'\n"
            "requires-python = '>=3.11'\ndependencies = "
            + json.dumps(REQUIREMENTS)
            + "\n[tool.uv]\nenvironments = "
            + json.dumps(environments)
            + "\n"
        )
        for arguments in [["--version"], *json.loads(COMMANDS.read_text())]:
            # No shell; the binary has already passed upstream attestation verification.
            try:
                result = subprocess.run(  # nosec B603
                    [str(binary), *arguments],
                    cwd=directory,
                    env=environment,
                    capture_output=True,
                    text=True,
                    check=True,
                    timeout=180,
                )
            except subprocess.CalledProcessError as error:
                raise ValueError(
                    f"uv smoke: {arguments[0]} failed: {error.stderr}"
                ) from error
            if arguments == ["--version"] and result.stdout.split()[:2] != [
                "uv",
                version,
            ]:
                raise ValueError(
                    "uv smoke: binary version does not match ingestion version"
                )
        raw = (directory / "pylock.toml").read_text()
        validate_pylock(raw)
    if hashlib.sha256(binary.read_bytes()).hexdigest() != before:
        raise ValueError("uv smoke: binary changed during execution")
    return {
        "version": version,
        "target": target,
        "sha256": before,
        "contract_sha256": contract_digest(),
        "pylock": raw,
    }


def verify_smokes(staged, version, reports):
    for target, binary, digest in staged:
        if target == native_target():
            report = smoke(binary, version, target)
        else:
            paths = list(reports.rglob(f"{target}.json")) if reports else []
            if not paths:
                raise ValueError(
                    f"uv smoke: missing native result for {target}; run the Resolver Contract "
                    "workflow and pass its downloaded reports with --smoke-results"
                )
            if len(paths) != 1:
                raise ValueError(f"uv smoke: ambiguous native results for {target}")
            report = json.loads(paths[0].read_text())
        expected = {
            "version": version,
            "target": target,
            "sha256": digest,
            "contract_sha256": contract_digest(),
        }
        if any(report.get(key) != value for key, value in expected.items()):
            raise ValueError(
                f"uv smoke: stale or mismatched native result for {target}"
            )
        validate_pylock(report.get("pylock", ""))
