"""Builder replay of real commands and TTY flows; no operator acceptance granted."""

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

SPEC = importlib.util.spec_from_file_location("manual_m6a", Path(__file__).with_name("fixture.py"))
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
GUIDE = Path(__file__).resolve().parents[2] / "CLI_M6A_MANUAL_TESTS.md"


@pytest.fixture(scope="module")
def prepared():
    root = fixture.REPO / "cli-m6a-test" / ("builder-" + uuid.uuid4().hex)
    try:
        fixture.prepare(root, fixture.REPO / "target/debug/agentpm")
        yield root
    finally:
        if root.exists():
            shutil.rmtree(root)


def test_prepare_is_visible_linted_and_shell_syntax_is_valid(prepared):
    root = prepared
    session, expected = fixture.load(root)
    assert len(list((root / "cases").iterdir())) == 21
    assert len(expected) == 26
    setup = fixture.read(root / "evidence/preparation.json")
    assert len([r for r in setup if r["argv"][0] == "lint"]) == 26
    assert all(r["exit"] == r["expected_exit"] for r in setup)
    assert {r["argv"][0] for r in setup} == {"init", "lint", "knowledge", "memory"}
    assert session["root"] == str(root)
    assert fixture.environment(root).get("OPENAI_API_KEY") is None
    assert fixture.environment(root).get("ANTHROPIC_API_KEY") is None
    for path in [root / "paths.sh", root / "functions.sh"]:
        subprocess.run(["zsh", "-n", str(path)], check=True)
    for block in re.findall(r"```bash\n(.*?)\n```", GUIDE.read_text(), flags=re.S):
        result = subprocess.run(["zsh", "-n"], input=block, text=True, capture_output=True)
        assert result.returncode == 0, result.stderr


def test_actual_runbook_blocks_replay_and_audit_rejects_missing_diagnostic(prepared):
    root = prepared
    text = GUIDE.read_text().split("## 11.", 1)[0]
    blocks = re.findall(r"```bash\n(.*?)\n```", text, flags=re.S)[1:]
    script = "set -e\nsource " + shlex.quote(str(root / "paths.sh")) + "\n"
    script += "\n".join(blocks) + "\n"
    (root / "evidence/builder-replay.zsh").write_text(script)
    env = {
        **os.environ,
        "OPENAI_API_KEY": "builder-sentinel-not-a-real-key",
        "ANTHROPIC_API_KEY": "builder-sentinel-not-a-real-key",
    }
    result = subprocess.run(
        ["zsh", "-f"], input=script, cwd=root, env=env, text=True, capture_output=True, timeout=180
    )
    (root / "evidence/builder-replay.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, result.stdout[-8000:] + result.stderr[-8000:]
    assert "builder-sentinel" not in result.stdout + result.stderr
    summary = fixture.audit(root, require_tui=False)
    assert summary["checked_command_results"] == 26
    assert summary["operator_signoff"] is False and summary["hosted_ci_complete"] is False
    with pytest.raises(FileNotFoundError):
        fixture.audit(root)
    path = root / "evidence/legacy-description-json.stdout.txt"
    original = path.read_text()
    try:
        report = json.loads(original)
        report["diagnostics"] = [
            d for d in report["diagnostics"] if d["code"] != "legacy_apds_issue"
        ]
        fixture.write(path, report)
        with pytest.raises(AssertionError):
            fixture.audit(root, require_tui=False)
    finally:
        path.write_text(original)


def test_guard_rejects_changed_inputs_and_stale_tui_run_proof(prepared):
    root = prepared
    path = root / "cases/ready/agent.json"
    original = path.read_bytes()
    try:
        path.write_bytes(original + b"\n")
        with pytest.raises(AssertionError, match="inputs changed"):
            fixture.guard(root, root / "cases/ready")
    finally:
        path.write_bytes(original)
    with pytest.raises(AssertionError, match="cd into"):
        fixture.guard(root, fixture.REPO)
    fixture.begin_tui(root, "ready", root / "cases/ready")
    with pytest.raises(AssertionError, match="Submit one message"):
        fixture.confirm_tui(root, "ready")


def test_real_tty_views_model_selection_and_fresh_run_reports(prepared):
    if os.name != "posix":
        pytest.skip("operator runbook targets macOS/zsh TTY")
    import fcntl
    import pty
    import select
    import struct
    import termios

    root = prepared
    cli = fixture.read(root / "session.json")["cli"]
    for name, case in fixture.TUI.items():
        folder = root / "cases" / case
        fixture.begin_tui(root, name, folder)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 44, 150, 0, 0))
        child = subprocess.Popen(
            [cli, "--base-url", "http://127.0.0.1:9", "harness"],
            cwd=folder,
            env={**fixture.environment(root), "TERM": "xterm-256color"},
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

        def wait_for(predicate, timeout=20):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                pump()
                if predicate():
                    return
                if child.poll() is not None:
                    assert predicate(), output.decode(errors="replace")[-1500:]
                    return
            raise AssertionError(output.decode(errors="replace")[-6000:])

        def screen_contains(value, since=0):
            text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", output[since:].decode(errors="replace"))
            return value in text

        def key(value):
            os.write(master, value)
            until = time.monotonic() + 0.2
            while time.monotonic() < until:
                pump()

        try:
            wait_for(lambda: screen_contains("APDS definition"))
            if name != "no-loop":
                key(b"\x1b")
            key(b"1")
            if name == "bootstrap":
                assert screen_contains("Needs model")
                # Resolution input is at the end of the paginated rail.
                key(b"\x1b[5~")
                key(b"P")
                wait_for(lambda: screen_contains("> openai/"))
                submitted_at = len(output)
                os.write(master, b"\x7f" * 7 + b"manual-model/fixture\r")
                # Wait for the rebuilt ready view, not the echoed prompt text.
                wait_for(lambda: screen_contains("APDS definition", submitted_at))
                key(b"\x1b")
            if name in ["ready", "bootstrap"]:
                key(b"2")
                key(b"\r")
                key(b"M6A interactive smoke\r")
                before = fixture.read(root / "evidence" / ("tui-" + name + ".begin.json"))
                wait_for(
                    lambda: any(
                        str(p) not in before["reports"]
                        for p in (folder / ".agentpm-state/runs").glob("*/report.json")
                    )
                )
                wait_for(lambda: screen_contains("ENDED"))
                # The differential ANSI stream is not a screen buffer. Verify
                # terminal output from the fresh report, not a substring that
                # may be split across cursor updates or output-panel pages.
                key(b"\x1b")
                key(b"O")
                key(b"\x1b")
            elif name == "legacy":
                assert screen_contains("Legacy")
            elif name == "no-loop":
                assert screen_contains("Not runnable")
            os.write(master, b"\x03")
            wait_for(lambda: child.poll() is not None)
            assert child.returncode == 0
            # Simulated operator acknowledgment is checked only in this discarded builder run.
            fixture.confirm_tui(root, name)
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=5)
            os.close(master)
            (root / "evidence" / ("builder-tui-" + name + ".ansi")).write_bytes(output)
    summary = fixture.audit(root)
    assert summary["status"] == "pass" and summary["tui_observations_recorded"] is True
    assert summary["operator_signoff"] is False
