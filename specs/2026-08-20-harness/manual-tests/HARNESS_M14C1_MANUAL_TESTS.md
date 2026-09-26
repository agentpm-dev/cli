# Harness Milestone 14c.1 Manual Regression Tests

Manual coverage for Milestone 14c.1: provider-native action serialization with the detailed Effective Capability Catalog kept as logical/debug rendering rather than duplicated into native provider prompt text.

This is a regression coordinator, not a replacement for the larger milestone manuals. It reuses the existing manual fixture setup from:

- `HARNESS_RELEASE_BAND_3_MANUAL_TESTS.md` for AgentPM Tool, Skill resource, and PhaseCompletion actions.
- `HARNESS_M12_MANUAL_TESTS.md` for KnowledgeRequest actions.
- `HARNESS_M14B_M14C_MANUAL_TESTS.md` for MemoryRead and MemoryWrite actions.

`external_mcp_tool` is intentionally excluded. Imported MCP action execution is not in scope for this quick 14c.1 check yet.

## Coverage target

Run each non-MCP semantic action kind at least once through both OpenAI and Anthropic:

| Semantic action kind | OpenAI | Anthropic | Fixture |
|---|---:|---:|---|
| `agentpm_tool` | yes | yes | Release Band 3 |
| `skill_resource_read` | yes | yes | Release Band 3 |
| `knowledge_request` | yes | yes | M12 |
| `memory_write` | yes | yes | M14b/M14c |
| `memory_read` | yes | yes | M14b/M14c |
| `phase_completion` | yes | yes | every Harness run |

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
export OPENAI_API_KEY="your OpenAI key"
export ANTHROPIC_API_KEY="your Anthropic key"
```

If either provider model name is unavailable in your account, override it before running setup:

```bash
export AGENTPM_MANUAL_OPENAI_MODEL="${AGENTPM_MANUAL_OPENAI_MODEL:-gpt-4o-mini}"
export AGENTPM_MANUAL_ANTHROPIC_MODEL="${AGENTPM_MANUAL_ANTHROPIC_MODEL:-claude-haiku-4-5-20251001}"
```

## Setup

Create the three fixture workspaces by running the setup section from each source manual:

```bash
# 1. Run the setup block from HARNESS_RELEASE_BAND_3_MANUAL_TESTS.md.
#    Expected workspace:
#      harness-m7-m8-test/workspace

# 2. Run the setup block from HARNESS_M12_MANUAL_TESTS.md.
#    Expected workspace:
#      harness-m12-test/workspace

# 3. Run the setup block from HARNESS_M14B_M14C_MANUAL_TESTS.md.
#    Expected workspace:
#      harness-m14bc-test/workspace
```

Return to the `agentpm` repo root before continuing.

The Release Band 3 fixture is OpenAI-only by default. Add an Anthropic config overlay:

```bash
RB3_WORK="$PWD/harness-m7-m8-test/workspace"
"$AGENTPM_MANUAL_PYTHON" - "$RB3_WORK/agentpm.harness.json" "$RB3_WORK/agentpm.anthropic.harness.json" <<'PY'
import json
import os
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
data = json.loads(source.read_text())
data["model"]["provider"] = "anthropic"
data["model"]["model"] = os.environ.get("AGENTPM_MANUAL_ANTHROPIC_MODEL", "claude-haiku-4-5-20251001")
target.write_text(json.dumps(data, indent=2) + "\n")
PY
```

Set shared variables:

```bash
export M14C1_RUNS="$PWD/harness-m14c1-runs"
export RB3_WORK="$PWD/harness-m7-m8-test/workspace"
export M12_WORK="$PWD/harness-m12-test/workspace"
export M14BC_WORK="$PWD/harness-m14bc-test/workspace"
mkdir -p "$M14C1_RUNS"/{openai,anthropic}
```

Install small trace assertion helpers:

```bash
mkdir -p "$M14C1_RUNS/scripts"
cat > "$M14C1_RUNS/scripts/assert_action_kind.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
expected = set(sys.argv[2:])
report = json.loads(report_path.read_text())
trace_path = Path(report["trace_path"])
text = trace_path.read_text()
events = [json.loads(line) for line in text.splitlines() if line.strip()]

seen = set()
for event in events:
    payload = event.get("payload") or {}
    kind = payload.get("action_kind")
    if kind:
        seen.add(kind)
    fields = payload.get("fields") or {}
    kind = fields.get("action_kind")
    if kind:
        seen.add(kind)

for kind in expected:
    marker = f'"action_kind":"{kind}"'
    if kind not in seen and marker not in text.replace(" ", ""):
        raise AssertionError(f"{report_path} missing action kind {kind}; saw {sorted(seen)}")

assert report.get("terminal_status") == "ended", report.get("terminal_status")
print(json.dumps({"ok": True, "report": str(report_path), "expected": sorted(expected)}, indent=2))
PY
chmod +x "$M14C1_RUNS/scripts/assert_action_kind.py"
```

## Serialization boundary checks

These deterministic tests directly check the 14c.1 provider request boundary. They are required because the normal report trace keeps the canonical diagnostic prompt; it does not currently expose the exact provider-wire prompt as a separate trace field.

```bash
cargo test -p agentpm-cli native_provider_prompt_omits_catalog_for_supported_built_in_providers -- --nocapture \
  | tee "$M14C1_RUNS/cargo-native-provider-prompt.txt"

cargo test -p agentpm-cli built_in_runtime_generate_passes_aliases_to_transport -- --nocapture \
  | tee "$M14C1_RUNS/cargo-provider-aliases.txt"

cargo test -p agentpm-cli provider_call_decoding_covers_all_non_phase_action_kinds -- --nocapture \
  | tee "$M14C1_RUNS/cargo-provider-action-decoding.txt"
```

Expected:

- native OpenAI, Anthropic, and supported Ollama provider requests carry structured actions;
- provider prompt text omits the detailed `EFFECTIVE CAPABILITY CATALOG` block when structured actions are sent;
- canonical logical rendering still includes Section 5 for diagnostics/debugging;
- non-phase action decoding still covers Tool, MCP, Skill resource, Knowledge, Memory read, and Memory write. MCP is covered here only at the provider-normalization level.

## Provider-wire prompt inspection

Verbose/full traces include `model_runtime_request_prepared` events. For built-in providers, this event has `request_kind: "provider_wire_request"` and the `prompt` field is the actual provider-bound prompt text AgentPM sent alongside native structured actions.

After any OpenAI or Anthropic run below, inspect one report:

```bash
REPORT="$M14C1_RUNS/openai/01-rb3-skill-tool.report.json"
TRACE="$("$AGENTPM_MANUAL_PYTHON" - "$REPORT" <<'PY'
import json
import sys
from pathlib import Path
print(json.loads(Path(sys.argv[1]).read_text())["trace_path"])
PY
)"

"$AGENTPM_MANUAL_PYTHON" - "$TRACE" <<'PY'
import json
import sys
from pathlib import Path

events = [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]
prepared = [
    event for event in events
    if event.get("event_type") == "model_runtime_request_prepared"
]
assert prepared, "missing model_runtime_request_prepared"
first = prepared[0]["payload"]["fields"]
assert first["request_kind"] == "provider_wire_request", first
assert first["structured_actions"] > 0, first
assert first["capability_catalog_in_prompt"] is False, first
assert "EFFECTIVE CAPABILITY CATALOG" not in first["prompt"], first["prompt"]
print(json.dumps({
    "ok": True,
    "provider": first["provider"],
    "model": first["model"],
    "structured_actions": first["structured_actions"],
    "capability_catalog_in_prompt": first["capability_catalog_in_prompt"],
}, indent=2))
PY
```

With `trace.content: "redacted"`, the same event remains subject to normal content policy and `payload.fields.prompt` is `[redacted]`. With `trace.content: "none"`, the `prompt` field is removed.

## OpenAI live action checks

AgentPM Tool + SkillResourceRead + PhaseCompletion:

```bash
REPORT="$M14C1_RUNS/openai/01-rb3-skill-tool.report.json"
(cd "$RB3_WORK" && "$APM" harness \
  --config agentpm.harness.json \
  --headless \
  --input-file input-skill.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/openai/01-rb3-skill-tool.stdout.txt" \
  2>"$M14C1_RUNS/openai/01-rb3-skill-tool.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" agentpm_tool skill_resource_read phase_completion
```

KnowledgeRequest:

```bash
REPORT="$M14C1_RUNS/openai/02-knowledge-context.report.json"
(cd "$M12_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --input-file input-context.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/openai/02-knowledge-context.stdout.txt" \
  2>"$M14C1_RUNS/openai/02-knowledge-context.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" knowledge_request phase_completion
```

MemoryWrite + MemoryRead:

```bash
REPORT="$M14C1_RUNS/openai/03-memory-write.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14c1-openai-user \
  --scope conversation=m14c1-openai-conversation \
  --input-file inputs/03-write-notes.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/openai/03-memory-write.stdout.txt" \
  2>"$M14C1_RUNS/openai/03-memory-write.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" memory_write phase_completion

REPORT="$M14C1_RUNS/openai/04-memory-read.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14c1-openai-user \
  --scope conversation=m14c1-openai-conversation \
  --input-file inputs/04-read-filters.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/openai/04-memory-read.stdout.txt" \
  2>"$M14C1_RUNS/openai/04-memory-read.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" memory_read phase_completion
```

## Anthropic live action checks

AgentPM Tool + SkillResourceRead + PhaseCompletion:

```bash
REPORT="$M14C1_RUNS/anthropic/01-rb3-skill-tool.report.json"
(cd "$RB3_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --input-file input-skill.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/anthropic/01-rb3-skill-tool.stdout.txt" \
  2>"$M14C1_RUNS/anthropic/01-rb3-skill-tool.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" agentpm_tool skill_resource_read phase_completion
```

KnowledgeRequest:

```bash
REPORT="$M14C1_RUNS/anthropic/02-knowledge-context.report.json"
(cd "$M12_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --input-file input-context.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/anthropic/02-knowledge-context.stdout.txt" \
  2>"$M14C1_RUNS/anthropic/02-knowledge-context.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" knowledge_request phase_completion
```

MemoryWrite + MemoryRead:

```bash
REPORT="$M14C1_RUNS/anthropic/03-memory-write.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14c1-anthropic-user \
  --scope conversation=m14c1-anthropic-conversation \
  --input-file inputs/03-write-notes.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/anthropic/03-memory-write.stdout.txt" \
  2>"$M14C1_RUNS/anthropic/03-memory-write.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" memory_write phase_completion

REPORT="$M14C1_RUNS/anthropic/04-memory-read.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14c1-anthropic-user \
  --scope conversation=m14c1-anthropic-conversation \
  --input-file inputs/04-read-filters.txt \
  --report "$REPORT" \
  >"$M14C1_RUNS/anthropic/04-memory-read.stdout.txt" \
  2>"$M14C1_RUNS/anthropic/04-memory-read.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M14C1_RUNS/scripts/assert_action_kind.py" \
  "$REPORT" memory_read phase_completion
```

## Final inspection

Summarize all reports:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M14C1_RUNS" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
for path in sorted(root.rglob("*.report.json")):
    report = json.loads(path.read_text())
    usage = report.get("usage", {})
    print(json.dumps({
        "report": str(path),
        "terminal_status": report.get("terminal_status"),
        "tool_calls": usage.get("tool_calls", 0),
        "knowledge_requests": usage.get("knowledge_requests", 0),
        "memory_requests": usage.get("memory_requests", 0),
        "accepted_semantic_actions": usage.get("accepted_semantic_actions", 0),
        "trace_path": report.get("trace_path"),
    }, indent=2))
PY
```

Expected:

- every report has `terminal_status: ended`;
- OpenAI and Anthropic each accepted at least one Tool action, Skill resource action, Knowledge action, Memory write, Memory read, and PhaseCompletion across the suite;
- focused provider tests prove the detailed Effective Capability Catalog is not duplicated into native provider prompt text when structured actions are sent;
- verbose trace/report/debug logical prompts may still include Section 5, subject to the existing trace level/content policy.

## Cleanup

```bash
rm -rf "$M14C1_RUNS"
```

If desired, also remove the underlying fixture workspaces after their source manual checks are complete:

```bash
rm -rf harness-m7-m8-test harness-m12-test harness-m14bc-test
```
