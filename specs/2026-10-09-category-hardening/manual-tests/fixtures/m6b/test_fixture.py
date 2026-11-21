"""Builder replay of the guide and real TTY; never grants operator acceptance."""

import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import time
import uuid

import pytest

SPEC = importlib.util.spec_from_file_location("manual_m6b", Path(__file__).with_name("fixture.py"))
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
GUIDE = Path(__file__).resolve().parents[2] / "CLI_M6B_MANUAL_TESTS.md"


@pytest.fixture(scope="module")
def prepared():
    name = "builder-" + uuid.uuid4().hex
    root = fixture.REPO / "cli-m6b-test" / name
    assert not root.exists()
    try:
        fixture.prepare(
            root,
            fixture.REPO / "target/debug/agentpm",
            fixture.REPO.parent / "agentpm-sdk-node",
            fixture.REPO.parent / "agentpm-sdk-python",
        )
        yield root
    finally:
        # Only this test's exact UUID path is disposable. Never sweep timestamp runs.
        assert root.name == name and root.parent == fixture.REPO / "cli-m6b-test"
        if root.exists():
            shutil.rmtree(root)


def test_preparation_and_all_shell_blocks(prepared):
    assert len(list((prepared / "cases").iterdir())) == 10
    assert len(fixture.specs(prepared)) == 30
    commands = fixture.read(prepared / "evidence/preparation.json")
    assert {c["argv"][1] for c in commands} == {"init", "lint", "knowledge", "memory", "harness"}
    assert any(c["exit"] == 1 and c["argv"][1] == "lint" for c in commands)
    assert fixture.environment(prepared)["AGENTPM_BASE_URL"] == "http://127.0.0.1:9"
    assert "OPENAI_API_KEY" not in fixture.environment(prepared)
    for block in re.findall(r"```bash\n(.*?)\n```", GUIDE.read_text(), re.S):
        subprocess.run(["zsh", "-n"], input=block, text=True, check=True)
    subprocess.run(["zsh", "-n", str(prepared / "paths.sh")], check=True)


def replay_tui(root, agent):
    import fcntl
    import pty
    import select
    import struct
    import termios

    folder = root / "cases/standalone"
    previous_cwd = Path.cwd()
    os.chdir(folder)
    try:
        fixture.tui(root, "begin-tui", agent)
        with pytest.raises(AssertionError, match="exactly one new TUI Run"):
            fixture.tui(root, "confirm-tui", agent)
    finally:
        os.chdir(previous_cwd)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 44, 150, 0, 0))
    script = "source " + shlex.quote(str(root / "paths.sh")) + "; m6b harness " + agent
    child = subprocess.Popen(
        ["zsh", "-f", "-c", script],
        cwd=folder,
        env={**os.environ, "TERM": "xterm-256color"},
        stdin=slave,
        stdout=slave,
        stderr=slave,
        start_new_session=True,
    )
    os.close(slave)
    output = bytearray()

    def pump():
        if select.select([master], [], [], 0.05)[0]:
            try:
                output.extend(os.read(master, 65536))
            except OSError:
                pass

    def screen_has(value):
        return value in re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", output.decode(errors="replace"))

    def wait_for(predicate):
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            pump()
            if predicate():
                return
            if child.poll() is not None:
                assert predicate(), output.decode(errors="replace")[-3000:]
                return
        raise AssertionError(output.decode(errors="replace")[-6000:])

    def key(value):
        os.write(master, value)
        until = time.monotonic() + 0.2
        while time.monotonic() < until:
            pump()

    try:
        wait_for(
            lambda: all(
                screen_has(value) for value in ["APDS definition", "Conformant", "Supported", agent]
            )
        )
        key(b"Check exact versions.\r")
        wait_for(lambda: screen_has("ENDED"))
        key(b"\x1b")
        key(b"O")
        key(b"\x1b")
        key(b"q")
        wait_for(lambda: child.poll() is not None)
        assert child.returncode == 0
        os.chdir(folder)
        fixture.tui(root, "confirm-tui", agent)
    finally:
        os.chdir(previous_cwd)
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
        os.close(master)
        (root / "evidence" / f"builder-tui-{agent}.ansi").write_bytes(output)
        (fixture.REPO / "target" / f"m6b-manual-builder-tui-{agent}.ansi").write_bytes(output)


def test_actual_shell_replay_both_tuis_and_final_audit(prepared):
    text = GUIDE.read_text().split("## 10.", 1)[0]
    blocks = re.findall(r"```bash\n(.*?)\n```", text, re.S)[1:]
    script = "set -e\nsource " + shlex.quote(str(prepared / "paths.sh")) + "\n" + "\n".join(blocks)
    completed = subprocess.run(
        ["zsh", "-f"], cwd=prepared, input=script, text=True, capture_output=True, timeout=180
    )
    log = fixture.REPO / "target/m6b-manual-builder.log"
    log.write_text(completed.stdout + completed.stderr)
    assert completed.returncode == 0, completed.stdout[-6000:] + completed.stderr[-6000:]
    assert len(list((prepared / "evidence").glob("*.result.json"))) == 30
    with pytest.raises(FileNotFoundError):
        fixture.audit(prepared)
    for agent in ["consumer-1", "consumer-2"]:
        replay_tui(prepared, agent)
    fixture.audit(prepared)
    summary = fixture.read(prepared / "evidence/summary.json")
    assert summary["checked_command_results"] == 30 and summary["tui_observations_recorded"]
    assert summary["operator_signoff"] is False and summary["hosted_ci_complete"] is False


def test_saved_wrong_version_and_missing_diagnostic_are_rejected(prepared):
    for label in ["node-2", "ambiguous-json"]:
        path = prepared / "evidence" / f"{label}.stdout.txt"
        original = path.read_bytes()
        try:
            value = json.loads(original)
            if label == "node-2":
                value["requests"][0]["runtime"]["tools"][0]["version"] = "1.0.0"
            else:
                value["diagnostics"] = []
            fixture.write(path, value)
            with pytest.raises(AssertionError):
                fixture.check(prepared, label)
        finally:
            path.write_bytes(original)


def test_prepared_inputs_expectations_and_existing_runs_are_protected(prepared):
    path = prepared / "cases/standalone/agents/agent-1.agent.json"
    original = path.read_bytes()
    try:
        path.write_bytes(original + b"\n")
        with pytest.raises(AssertionError, match="inputs changed"):
            fixture.guard(prepared)
    finally:
        path.write_bytes(original)
    path = prepared / "expectations.json"
    original = path.read_bytes()
    try:
        fixture.write(path, {})
        with pytest.raises(AssertionError, match="expectations changed"):
            fixture.guard(prepared)
    finally:
        path.write_bytes(original)
    with pytest.raises(AssertionError, match="never overwritten"):
        fixture.prepare(
            prepared,
            fixture.REPO / "target/debug/agentpm",
            fixture.REPO.parent / "agentpm-sdk-node",
            fixture.REPO.parent / "agentpm-sdk-python",
        )
    fixture.guard(prepared)
