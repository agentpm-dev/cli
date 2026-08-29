# Harness Release Band 3 Manual Tests

Manual coverage for Harness Milestones 7 and 8:

- `agentpm run --machine` stable Tool execution envelope
- Harness `ToolRuntime` spawning public `agentpm run --machine`
- provider-native semantic actions for Tools, Skill resources, and phase completion
- Skill progressive disclosure and phase-local loaded resources
- Skill-inherited Tools
- Tool-disabled phases suppressing direct and inherited Tools while keeping Skill resources available
- runtime-version suppression during preflight
- schema-valid Tool domain failures returning to the phase transcript
- report and `events.jsonl` trace output

These tests do not require publishing anything to the registry. They create a local workspace with installed packages under `.agentpm/` and a v3 `agent.lock`.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export OPENAI_API_KEY="your key"
```

The live Harness tests use OpenAI by default. If you do not have an API key on this machine yet, run the setup and direct `agentpm run --machine` tests, then skip the live `agentpm harness --headless` tests.

## Setup

Run this from the root of the `agentpm` repo:

```bash
cat > /tmp/setup-harness-m7-m8-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M7_M8_TEST_BASE:-$ROOT/harness-m7-m8-test}"
WORK="$BASE/workspace"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
SCHEMA_URL="https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

if [ -e "$BASE" ]; then
  echo "$BASE already exists. Move or delete it before re-running setup." >&2
  exit 1
fi

mkdir -p "$WORK"

pkg_dir() {
  local plural="$1"
  local package="$2"
  local version="$3"
  local without_at="${package#@}"
  local namespace="${without_at%%/*}"
  local name="${without_at#*/}"
  printf '%s/.agentpm/%s/%s/%s/%s' "$WORK" "$plural" "$namespace" "$name" "$version"
}

write_tool() {
  local package="$1"
  local description="$2"
  local input_schema="$3"
  local output_schema="$4"
  local script="$5"
  local runtime_version="${6:-3}"
  local extra_environment="${7:-}"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir tools "$package" "0.1.0")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "tool",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "$description",
  "entrypoint": {
    "command": "$PYTHON_CMD",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {}
  },
  "runtime": {
    "type": "python",
    "version": "$runtime_version"
  },
  "inputs": $input_schema,
  "outputs": $output_schema,
  "files": ["script.py"]$extra_environment
}
JSON
  cat > "$dir/script.py" <<PY
$script
PY
}

write_tool \
  "@zack/manual-search" \
  "Search local manual-test records." \
  '{"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string"}}}' \
  '{"type":"object","additionalProperties":false,"required":["ok","tool","query","result"],"properties":{"ok":{"type":"boolean"},"tool":{"type":"string"},"query":{"type":"string"},"result":{"type":"string"}}}' \
  'import json, sys
payload = json.load(sys.stdin)
print(json.dumps({
    "ok": True,
    "tool": "manual-search",
    "query": payload["query"],
    "result": "Found one manual Release Band 3 record."
}))'

write_tool \
  "@zack/manual-comment" \
  "Draft a short comment body." \
  '{"type":"object","additionalProperties":false,"required":["body"],"properties":{"body":{"type":"string","minLength":1}}}' \
  '{"type":"object","additionalProperties":false,"required":["ok","tool","body"],"properties":{"ok":{"type":"boolean"},"tool":{"type":"string"},"body":{"type":"string"}}}' \
  'import json, sys
payload = json.load(sys.stdin)
print(json.dumps({
    "ok": True,
    "tool": "manual-comment",
    "body": payload["body"]
}))'

write_tool \
  "@zack/manual-domain-fail" \
  "Return a schema-valid domain-level failure." \
  '{"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string"}}}' \
  '{"type":"object","additionalProperties":false,"required":["ok","tool","reason"],"properties":{"ok":{"type":"boolean"},"tool":{"type":"string"},"reason":{"type":"string"}}}' \
  'import json, sys
json.load(sys.stdin)
print(json.dumps({
    "ok": False,
    "tool": "manual-domain-fail",
    "reason": "domain-level failure for manual verification"
}))'

write_tool \
  "@zack/manual-env" \
  "Require an environment variable so preflight can warn without suppressing." \
  '{"type":"object","additionalProperties":false,"properties":{}}' \
  '{"type":"object","additionalProperties":false,"required":["ok","env"],"properties":{"ok":{"type":"boolean"},"env":{"type":"string"}}}' \
  'import json, os, sys
json.load(sys.stdin)
print(json.dumps({
    "ok": True,
    "env": os.environ["MANUAL_REQUIRED_TOKEN"]
}))' \
  '3' \
  ',
  "environment": {
    "vars": {
      "MANUAL_REQUIRED_TOKEN": {
        "description": "Manual test required token.",
        "required": true
      }
    }
  }'

write_tool \
  "@zack/manual-future-python" \
  "Declare an impossible Python version so Harness preflight suppresses it." \
  '{"type":"object","additionalProperties":false,"properties":{}}' \
  '{"type":"object","additionalProperties":false,"properties":{}}' \
  'import json, sys
json.load(sys.stdin)
print(json.dumps({"ok": True}))' \
  '999.0.0'

SKILL_DIR="$(pkg_dir skills '@zack/manual-review-skill' '0.1.0')"
mkdir -p "$SKILL_DIR/references"
cat > "$SKILL_DIR/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "skill",
  "name": "manual-review-skill",
  "version": "0.1.0",
  "description": "Manual Harness review procedure.",
  "tools": ["@zack/manual-search@0.1.0"],
  "skill": {
    "entrypoint": "SKILL.md",
    "references": [
      "references/checklist.md",
      "references/escalation.md"
    ]
  }
}
JSON
cat > "$SKILL_DIR/SKILL.md" <<'MD'
# Manual Review Skill

Use this skill to inspect the user's request, gather only necessary supporting evidence, and produce a concise response.

When evidence is missing, ask for it directly instead of guessing.
MD
cat > "$SKILL_DIR/references/checklist.md" <<'MD'
# Checklist

- Identify the requested outcome.
- Use available evidence before drafting.
- Keep the final answer brief.
MD
cat > "$SKILL_DIR/references/escalation.md" <<'MD'
# Escalation

Hand off when the request requires authority or information outside the current workspace.
MD

LOOP_DIR="$(pkg_dir loops '@zack/manual-tool-skill-loop' '0.1.0')"
mkdir -p "$LOOP_DIR"
cat > "$LOOP_DIR/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "manual-tool-skill-loop",
  "version": "0.1.0",
  "description": "Manual Harness loop for ToolRuntime and Skill progressive-disclosure tests.",
  "loop": {
    "archetype": "classify_act_finish",
    "entry_phase": "classify",
    "limits": {
      "max_steps": 8
    },
    "phases": [
      {
        "id": "classify",
        "objective": "Use available Tools and Skill resources when helpful, then choose the next path.",
        "access": {
          "tools": true,
          "knowledge": false,
          "memory": {
            "read": false,
            "write": false
          }
        },
        "outcomes": [
          {
            "id": "draft",
            "description": "Enough information is available to draft the final response."
          },
          {
            "id": "no-tools",
            "description": "Move to a phase where Tool calls are intentionally unavailable."
          },
          {
            "id": "handoff",
            "description": "The request should be handed off."
          }
        ]
      },
      {
        "id": "no-tools",
        "objective": "Show that Skill resources can still be read when Tool calls are disabled.",
        "access": {
          "tools": false,
          "knowledge": false,
          "memory": {
            "read": false,
            "write": false
          }
        },
        "outcomes": [
          {
            "id": "done",
            "description": "Continue to the final response."
          },
          {
            "id": "handoff",
            "description": "Hand off instead of continuing."
          }
        ]
      },
      {
        "id": "final",
        "objective": "Return the final concise user-facing response."
      }
    ],
    "transitions": [
      { "from": "classify", "on": "draft", "to": "final" },
      { "from": "classify", "on": "no-tools", "to": "no-tools" },
      { "from": "classify", "on": "handoff", "to": "\$handoff" },
      { "from": "no-tools", "on": "done", "to": "final" },
      { "from": "no-tools", "on": "handoff", "to": "\$handoff" },
      { "from": "final", "on": "complete", "to": "\$end" }
    ],
    "error_policy": {
      "tool_failure": {
        "action": "retry",
        "max_retries": 1,
        "on_exhausted": "fail_phase"
      },
      "phase_failure": {
        "action": "abort"
      }
    }
  }
}
JSON

cat > "$WORK/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "manual-harness-agent",
  "version": "0.1.0",
  "description": "Local manual Harness Release Band 3 test agent.",
  "tools": [
    "@zack/manual-comment@0.1.0",
    "@zack/manual-domain-fail@0.1.0",
    "@zack/manual-env@0.1.0",
    "@zack/manual-future-python@0.1.0"
  ],
  "skills": ["@zack/manual-review-skill@0.1.0"],
  "loop": "@zack/manual-tool-skill-loop@0.1.0",
  "bindings": {
    "phases": {
      "classify": {
        "skills": ["@zack/manual-review-skill"],
        "tools": [
          "@zack/manual-comment",
          "@zack/manual-domain-fail",
          "@zack/manual-env",
          "@zack/manual-future-python"
        ]
      },
      "no-tools": {
        "tools": ["@zack/manual-comment"],
        "skills": ["@zack/manual-review-skill"]
      }
    }
  }
}
JSON

cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-08-27T00:00:00Z",
  "packages": {
    "tool:@zack/manual-search@0.1.0": {
      "kind": "tool",
      "name": "@zack/manual-search",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/manual-comment@0.1.0": {
      "kind": "tool",
      "name": "@zack/manual-comment",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/manual-domain-fail@0.1.0": {
      "kind": "tool",
      "name": "@zack/manual-domain-fail",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/manual-env@0.1.0": {
      "kind": "tool",
      "name": "@zack/manual-env",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/manual-future-python@0.1.0": {
      "kind": "tool",
      "name": "@zack/manual-future-python",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "skill:@zack/manual-review-skill@0.1.0": {
      "kind": "skill",
      "name": "@zack/manual-review-skill",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "loop:@zack/manual-tool-skill-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/manual-tool-skill-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "manual-harness-agent",
      "version": "0.1.0",
      "tools": [
        "tool:@zack/manual-comment@0.1.0",
        "tool:@zack/manual-domain-fail@0.1.0",
        "tool:@zack/manual-env@0.1.0",
        "tool:@zack/manual-future-python@0.1.0"
      ],
      "skills": ["skill:@zack/manual-review-skill@0.1.0"],
      "loop": "loop:@zack/manual-tool-skill-loop@0.1.0"
    }
  }
}
JSON

cat > "$WORK/agentpm.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-4o-mini"
  },
  "runtime": {
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 8,
      "max_tool_calls_per_phase": 8,
      "max_actions_per_phase": 16,
      "max_tool_call_repairs": 1,
      "max_structured_output_repairs": 4
    }
  },
  "trace": {
    "enabled": true,
    "level": "verbose",
    "content": "full"
  }
}
JSON

cat > "$WORK/redacted.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-4o-mini"
  },
  "runtime": {
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 8,
      "max_tool_calls_per_phase": 8,
      "max_actions_per_phase": 16,
      "max_tool_call_repairs": 1,
      "max_structured_output_repairs": 4
    }
  },
  "trace": {
    "enabled": true,
    "level": "verbose",
    "content": "redacted"
  }
}
JSON

cat > "$WORK/input-tool.txt" <<'TXT'
Look up release band 3 in the available manual records, then draft a short comment body that says "Release Band 3 ToolRuntime is wired." Finish with one concise sentence summarizing what happened.
TXT

cat > "$WORK/input-skill.txt" <<'TXT'
Use the available review procedure and checklist before answering. Look up "skill inherited search" in the available manual records, then finish with one concise sentence.
TXT

cat > "$WORK/input-domain-fail.txt" <<'TXT'
Check the domain-failure fixture for "domain failure" and explain the result in one sentence.
TXT

cat > "$WORK/input-no-tools.txt" <<'TXT'
Answer using only the available review guidance. Do not look up external records or call executable tools.
TXT

echo "Linting generated manifests..."
"$APM" lint "$WORK/agent.json" --strict
"$APM" lint "$(pkg_dir loops '@zack/manual-tool-skill-loop' '0.1.0')/agent.json" --strict
"$APM" lint "$(pkg_dir skills '@zack/manual-review-skill' '0.1.0')/agent.json" --strict
for tool in manual-search manual-comment manual-domain-fail manual-env manual-future-python; do
  "$APM" lint "$WORK/.agentpm/tools/zack/$tool/0.1.0/agent.json" --strict
done

echo
echo "Created $WORK"
echo "Next:"
echo "  cd $WORK"
echo "  export APM=\"$APM\""
SH

bash /tmp/setup-harness-m7-m8-manual.sh
```

## Test 1: Direct `agentpm run --machine` success over stdin

```bash
cd harness-m7-m8-test/workspace
printf '{"query":"release band 3"}' | "$APM" run @zack/manual-search@0.1.0 --machine \
  >run-search.stdout.json 2>run-search.stderr.txt
cat run-search.stdout.json
cat run-search.stderr.txt
```

Expected:

- Exit code is `0`.
- `run-search.stdout.json` is a single JSON object.
- It contains `"schema_version":1`, `"status":"success"`, and output from `manual-search`.
- Stderr does not contain preflight or human status output.

Optional structural check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

value = json.loads(Path("run-search.stdout.json").read_text())
assert value["schema_version"] == 1
assert value["status"] == "success"
assert value["output"]["tool"] == "manual-search"
assert value["output"]["query"] == "release band 3"
print("direct machine success verified")
PY
```

## Test 2: Direct machine schema and runtime failures are categorized

Input schema failure:

```bash
set +e
printf '{"query":42}' | "$APM" run @zack/manual-search@0.1.0 --machine \
  >run-schema.stdout.json 2>run-schema.stderr.txt
STATUS=$?
set -e
echo "$STATUS"
cat run-schema.stdout.json
```

Expected:

- Exit code is non-zero.
- The machine envelope has `"status":"error"` and `"error":{"category":"schema", ...}`.

Runtime minimum-version failure:

```bash
set +e
printf '{}' | "$APM" run @zack/manual-future-python@0.1.0 --machine \
  >run-runtime.stdout.json 2>run-runtime.stderr.txt
STATUS=$?
set -e
echo "$STATUS"
cat run-runtime.stdout.json
```

Expected:

- Exit code is non-zero.
- The machine envelope has `"status":"error"` and `"error":{"category":"runtime", ...}`.
- The message includes `runtime minimum version not satisfied`.

## Test 3: Harness preflight suppresses future runtime but only warns for missing env

```bash
"$APM" harness --json >preflight.json
cat preflight.json
```

Expected:

- Status is `ready_with_warnings`.
- Diagnostics include `tool_runtime_version_unsatisfied` for `@zack/manual-future-python`.
- Diagnostics include `tool_required_environment_missing` for `@zack/manual-env`.
- The future-version Tool is not considered available.
- The missing-env Tool is not suppressed solely by preflight.

Structural check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

data = json.loads(Path("preflight.json").read_text())
codes = {item["code"] for item in data["diagnostics"]}
assert data["status"] == "ready_with_warnings"
assert "tool_runtime_version_unsatisfied" in codes
assert "tool_required_environment_missing" in codes
text = json.dumps(data)
assert "@zack/manual-future-python" in text
assert "@zack/manual-env" in text
print("preflight warning/suppression diagnostics verified")
PY
```

## Test 4: Harness ToolRuntime invokes two real Tools through provider-native actions

This requires `OPENAI_API_KEY`.

```bash
"$APM" harness --headless --input-file input-tool.txt --report tool-report.json \
  >tool-stdout.txt 2>tool-stderr.txt
cat tool-stdout.txt
cat tool-stderr.txt
```

Expected:

- Exit code is `0`.
- Stdout contains only the final user-facing answer.
- Stderr contains preflight/status text.
- `tool-report.json` exists.
- `tool-report.json.trace_path` points to an `events.jsonl` file.

Trace check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("tool-report.json").read_text())
trace = Path(report["trace_path"])
events = [json.loads(line) for line in trace.read_text().splitlines()]
types = [event["event_type"] for event in events]
assert "tool_candidates_computed" in types
assert "tool_invoked" in types
assert "tool_completed" in types
assert "model_request_completed" in types
trace_text = trace.read_text()
assert "@zack/manual-search" in trace_text
assert "@zack/manual-comment" in trace_text
assert "finish_reason" in trace_text
assert "tool_calls" in trace_text
print("provider-native ToolRuntime trace verified")
PY
```

This is a live-provider smoke test, so the exact model behavior can vary. If the model does not call both Tools on the first run, rerun the same command or inspect the trace to confirm whether Harness requested repair after an invalid combined action.

## Test 5: Skill resources are loaded on demand and grouped into later prompts

This requires `OPENAI_API_KEY`.

```bash
"$APM" harness --headless --input-file input-skill.txt --report skill-report.json \
  >skill-stdout.txt 2>skill-stderr.txt
cat skill-stdout.txt
```

Expected:

- Skill resource events appear in the trace.
- The inherited `@zack/manual-search` Tool can be invoked even though it is inherited from the Skill rather than directly listed in the Agent's `tools` array.
- Later `prompt_prepared` events include any loaded Skill resource content grouped under one `Skill: @zack/manual-review-skill` section.

Trace check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("skill-report.json").read_text())
trace = Path(report["trace_path"])
events = [json.loads(line) for line in trace.read_text().splitlines()]
types = [event["event_type"] for event in events]
assert "skill_activated" in types
assert "skill_resource_requested" in types
assert "skill_resource_loaded" in types
assert "tool_invoked" in types
text = trace.read_text()
loaded_resources = [
    event["payload"]["fields"]["resource"]
    for event in events
    if event["event_type"] == "skill_resource_loaded"
]
assert loaded_resources, "expected at least one loaded Skill resource"
for resource in loaded_resources:
    assert f"Loaded resource: {resource}" in text
assert text.count("Skill: @zack/manual-review-skill") >= 1
assert "@zack/manual-search" in text
final_effective = [
    event for event in events
    if event["event_type"] == "effective_phase_computed"
    and event["payload"]["fields"].get("phase_id") == "final"
]
if final_effective:
    final_descriptors = final_effective[0]["payload"]["fields"]["capability_descriptors"]
    assert all(
        descriptor["action_kind"] == "phase_completion"
        for descriptor in final_descriptors
    ), final_descriptors
print("Skill progressive disclosure and inherited Tool path verified")
PY
```

Live providers may choose `entrypoint`, `references/checklist.md`, or both. The required behavior is that Skill content is not present until a `skill_resource_read` succeeds, any loaded resources are grouped under the active Skill in the next prompt, and the `final` phase is answer-only if reached.

## Test 6: Schema-valid domain `ok:false` Tool result stays phase-local

This requires `OPENAI_API_KEY`.

```bash
"$APM" harness --headless --input-file input-domain-fail.txt --report domain-fail-report.json \
  >domain-fail-stdout.txt 2>domain-fail-stderr.txt
cat domain-fail-stdout.txt
```

Expected:

- The Harness Run still completes unless the model chooses a terminal handoff/abort.
- The Tool result is treated as successful ToolRuntime execution because it is schema-valid.
- The next prompt includes an `ActionResult [agentpm_tool @zack/manual-domain-fail]` transcript entry with `"ok":false`.

Trace check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("domain-fail-report.json").read_text())
trace = Path(report["trace_path"])
text = trace.read_text()
assert "tool_completed" in text
assert "ActionResult [agentpm_tool @zack/manual-domain-fail]" in text
assert '"ok":false' in text.replace(" ", "")
assert "domain-level failure for manual verification" in text
print("schema-valid domain failure transcript handling verified")
PY
```

## Test 7: Tool-disabled phase suppresses direct and Skill-inherited Tools

This requires `OPENAI_API_KEY`.

```bash
"$APM" harness --headless --input-file input-no-tools.txt --report no-tools-report.json \
  >no-tools-stdout.txt 2>no-tools-stderr.txt
cat no-tools-stdout.txt
```

Expected:

- The `no-tools` phase has suppressed Tool capabilities because `access.tools=false`.
- Skill resource reads are still available in the `no-tools` phase.
- No Tool invocation occurs while `phase_id` is `no-tools`.

Trace check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("no-tools-report.json").read_text())
trace = Path(report["trace_path"])
events = [json.loads(line) for line in trace.read_text().splitlines()]
no_tools_effective = [
    event for event in events
    if event["event_type"] == "effective_phase_computed"
    and event.get("phase_execution_id")
    and event["payload"]["fields"].get("phase_id") == "no-tools"
]
assert no_tools_effective, "missing no-tools EffectivePhase event"
text = json.dumps(no_tools_effective)
assert "Loop access.tools=false for this phase" in text
assert "@zack/manual-comment" in text
assert "@zack/manual-search" in text
assert "skill_resource_read" in text
for event in events:
    if event["event_type"] == "tool_invoked":
        assert event["payload"]["fields"].get("phase_id") != "no-tools", event
print("Tool-disabled phase suppression verified")
PY
```

## Test 8: Redacted trace hides prompt/input/output content

This requires `OPENAI_API_KEY`.

```bash
"$APM" harness --headless --config redacted.harness.json --input-file input-tool.txt --report redacted-report.json \
  >redacted-stdout.txt 2>redacted-stderr.txt
```

Expected:

- The run writes a report and trace.
- The trace includes redaction markers.
- Prompt text, run input, and final Tool/domain content are not visible in the trace.

Trace check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("redacted-report.json").read_text())
trace = Path(report["trace_path"])
text = trace.read_text()
assert "[redacted]" in text
assert "In the classify phase" not in text
assert "Release Band 3 ToolRuntime is wired" not in text
assert "Found one manual Release Band 3 record" not in text
assert "OPENAI_API_KEY" not in text
print("redacted trace policy verified")
PY
```

## Test 9: Missing required Tool env is authoritative at invocation time

Preflight should warn but keep `@zack/manual-env` available. Direct Tool invocation should fail until the required env var exists.

Without env:

```bash
set +e
printf '{}' | env -u MANUAL_REQUIRED_TOKEN "$APM" run @zack/manual-env@0.1.0 --machine \
  >env-missing.stdout.json 2>env-missing.stderr.txt
STATUS=$?
set -e
echo "$STATUS"
cat env-missing.stdout.json
```

Expected:

- Exit code is non-zero.
- Machine category is `runtime`.
- Message mentions missing required environment variables.

With env:

```bash
printf '{}' | MANUAL_REQUIRED_TOKEN=manual-ok "$APM" run @zack/manual-env@0.1.0 --machine \
  >env-present.stdout.json 2>env-present.stderr.txt
cat env-present.stdout.json
```

Expected:

- Exit code is `0`.
- Output contains `"env":"manual-ok"`.

## Test 10: Clean up

```bash
cd ../..
rm -rf harness-m7-m8-test
rm -f /tmp/setup-harness-m7-m8-manual.sh
```
