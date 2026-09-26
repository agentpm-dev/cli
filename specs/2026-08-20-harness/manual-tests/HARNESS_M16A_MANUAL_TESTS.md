# Harness Milestone 16a Manual Tests

Manual coverage for Milestone 16a: provider-facing semantic action aliases, action descriptions, native-provider prompt shape, and long-name alias edge cases.

M16a changes provider-facing names and descriptions only. Canonical Harness identity, authorization, dispatch, Memory routing, persistence review, and action validation should remain unchanged.

## Coverage Target

| Requirement | Manual coverage |
|---|---|
| Semantic aliases replace positional `action_N` names | Test 1 validates every model request alias is semantic and non-positional |
| Every supported Harness semantic action kind is covered | Test 0 covers external MCP alias normalization; Test 1 executes AgentPM Tool, Skill resource, Knowledge, Memory read/write, PhaseCompletion, and PersistenceReviewComplete |
| Long names stay provider-safe and under 64 chars | Test 1 prints aliases and validates length/ASCII/suffix shape |
| Memory aliases preserve fixed target signal | Test 1 checks long Memory package/space aliases include `memory_write`, space signal, and fixed `note` record type |
| Native structured providers do not get a duplicate prose catalog | Test 2 checks the provider wire request omits `EFFECTIVE CAPABILITY CATALOG` while structured aliases remain present |
| Persistence review uses review-only actions | Test 1 checks review requests expose Memory read/write plus `persistence_review_complete`, not phase/tools/Knowledge/Skill/MCP |

External MCP note: imported MCP execution is not live in this Harness path yet. The semantic action kind is covered by provider normalization and alias generation, so Test 0 runs the focused Rust smoke that includes `external_mcp_tool`. Do not expect a live `mcp_tool_completed` event from this manual fixture.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

Test 2 uses OpenAI only to inspect the native provider request shape. If you do not have a key available, skip Test 2.

```bash
export OPENAI_API_KEY="your key"
```

## Setup

Run this from the root of the `agentpm` repo:

```bash
cat > /tmp/setup-harness-m16a-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M16A_TEST_BASE:-$ROOT/harness-m16a-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
SCHEMA_URL="https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

rm -rf "$BASE"
mkdir -p "$WORK" "$RUNS"

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
  local package="@zack/m16a-search-tool-with-long-alias-truncation-target-name"
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
  "description": "M16a long-name Tool used to verify provider alias readability.",
  "entrypoint": {
    "command": "$PYTHON_CMD",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {}
  },
  "runtime": {
    "type": "python",
    "version": "3"
  },
  "inputs": {
    "type": "object",
    "additionalProperties": false,
    "required": ["query"],
    "properties": {
      "query": { "type": "string" }
    }
  },
  "outputs": {
    "type": "object",
    "additionalProperties": false,
    "required": ["ok", "query", "result"],
    "properties": {
      "ok": { "type": "boolean" },
      "query": { "type": "string" },
      "result": { "type": "string" }
    }
  },
  "files": ["script.py"]
}
JSON
  cat > "$dir/script.py" <<'PY'
import json
import sys

payload = json.load(sys.stdin)
print(json.dumps({
    "ok": True,
    "query": payload["query"],
    "result": "M16a long-name AgentPM Tool executed."
}))
PY
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_skill() {
  local package="@zack/m16a-progressive-skill-with-long-alias-truncation-name"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir skills "$package" "0.1.0")"
  mkdir -p "$dir/references"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "skill",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M16a long-name Skill for alias testing.",
  "skill": {
    "entrypoint": "SKILL.md",
    "references": ["references/checklist.md"]
  }
}
JSON
  cat > "$dir/SKILL.md" <<'MD'
# M16a Manual Skill

This skill exists so the manual Harness run can request a Skill resource and verify the semantic alias remains readable.
MD
  cat > "$dir/references/checklist.md" <<'MD'
# M16a Checklist

- Verify semantic aliases are not positional.
- Verify canonical identities remain available in traces.
MD
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_knowledge() {
  local package="@zack/m16a-contextual-knowledge-with-long-alias-target-name"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir knowledge "$package" "0.1.0")"
  mkdir -p "$dir/knowledge/docs"
  cat > "$dir/knowledge/docs/overview.md" <<'MD'
# M16a Context Knowledge

The key manual-test phrase is: provider aliases should be semantic and stable.
MD
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "knowledge",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M16a context Knowledge package with a long identity.",
  "knowledge": {
    "mode": "context",
    "documents": [
      {
        "path": "knowledge/docs/overview.md",
        "content_type": "text/markdown",
        "role": "context",
        "description": "M16a alias overview."
      }
    ]
  }
}
JSON
  "$APM" knowledge build --manifest "$dir/agent.json" >/dev/null
}

write_memory() {
  local package="@zack/m16a-memory-package-with-long-alias-truncation-name"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir memory "$package" "0.1.0")"
  mkdir -p "$dir/schemas"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "memory",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M16a Memory Blueprint with long names for alias testing.",
  "memory": {
    "scopes": {
      "user": { "description": "Manual user scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Manual note.",
        "schema": "schemas/note.schema.json"
      },
      "summary": {
        "version": "1.0.0",
        "description": "Manual summary.",
        "schema": "schemas/summary.schema.json"
      }
    },
    "spaces": {
      "conversation_state_notes_with_intentionally_long_alias_tail": {
        "description": "Long-name note collection for alias truncation.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] }
      },
      "summaries": {
        "description": "Short summary collection for persistence review alternatives.",
        "model": "collection",
        "record_types": ["summary"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] }
      }
    }
  }
}
JSON
  cat > "$dir/schemas/note.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["body"],
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "tag": { "type": "string" }
  },
  "additionalProperties": false
}
JSON
  cat > "$dir/schemas/summary.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["summary"],
  "properties": {
    "summary": { "type": "string", "minLength": 1 }
  },
  "additionalProperties": false
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
  "$APM" memory build --manifest "$dir/agent.json" >/dev/null
}

write_loop_agent_lock() {
  local loop_package="@zack/m16a-semantic-action-loop"
  local loop_dir
  loop_dir="$(pkg_dir loops "$loop_package" "0.1.0")"
  mkdir -p "$loop_dir"
  cat > "$loop_dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "m16a-semantic-action-loop",
  "version": "0.1.0",
  "description": "M16a loop allowing all currently live semantic action surfaces.",
  "loop": {
    "archetype": "act_finish",
    "entry_phase": "exercise",
    "limits": { "max_steps": 8 },
    "phases": [
      {
        "id": "exercise",
        "objective": "Use each requested semantic action surface, then complete.",
        "access": {
          "tools": true,
          "knowledge": true,
          "memory": { "read": true, "write": true }
        },
        "outcomes": [
          { "id": "done", "description": "The M16a manual check is complete." }
        ]
      }
    ],
    "transitions": [
      { "from": "exercise", "on": "done", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$loop_dir/agent.json" >/dev/null

  cat > "$WORK/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "m16a-manual-agent",
  "version": "0.1.0",
  "description": "M16a manual Harness alias test agent.",
  "tools": ["@zack/m16a-search-tool-with-long-alias-truncation-target-name@0.1.0"],
  "skills": ["@zack/m16a-progressive-skill-with-long-alias-truncation-name@0.1.0"],
  "knowledge": ["@zack/m16a-contextual-knowledge-with-long-alias-target-name@0.1.0"],
  "memory": ["@zack/m16a-memory-package-with-long-alias-truncation-name@0.1.0"],
  "loop": "@zack/m16a-semantic-action-loop@0.1.0",
  "bindings": {
    "global": {
      "memory": [
        {
          "package": "@zack/m16a-memory-package-with-long-alias-truncation-name",
          "spaces": [
            "conversation_state_notes_with_intentionally_long_alias_tail",
            "summaries"
          ]
        }
      ]
    },
    "phases": {
      "exercise": {
        "tools": ["@zack/m16a-search-tool-with-long-alias-truncation-target-name"],
        "skills": ["@zack/m16a-progressive-skill-with-long-alias-truncation-name"],
        "knowledge": ["@zack/m16a-contextual-knowledge-with-long-alias-target-name"]
      }
    }
  }
}
JSON
  "$APM" lint "$WORK/agent.json" >/dev/null

  cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-09-12T00:00:00Z",
  "packages": {
    "tool:@zack/m16a-search-tool-with-long-alias-truncation-target-name@0.1.0": {
      "kind": "tool",
      "name": "@zack/m16a-search-tool-with-long-alias-truncation-target-name",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "skill:@zack/m16a-progressive-skill-with-long-alias-truncation-name@0.1.0": {
      "kind": "skill",
      "name": "@zack/m16a-progressive-skill-with-long-alias-truncation-name",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "knowledge:@zack/m16a-contextual-knowledge-with-long-alias-target-name@0.1.0": {
      "kind": "knowledge",
      "name": "@zack/m16a-contextual-knowledge-with-long-alias-target-name",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "memory:@zack/m16a-memory-package-with-long-alias-truncation-name@0.1.0": {
      "kind": "memory",
      "name": "@zack/m16a-memory-package-with-long-alias-truncation-name",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "loop:@zack/m16a-semantic-action-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/m16a-semantic-action-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "m16a-manual-agent",
      "version": "0.1.0",
      "tools": ["tool:@zack/m16a-search-tool-with-long-alias-truncation-target-name@0.1.0"],
      "skills": ["skill:@zack/m16a-progressive-skill-with-long-alias-truncation-name@0.1.0"],
      "knowledge": ["knowledge:@zack/m16a-contextual-knowledge-with-long-alias-target-name@0.1.0"],
      "memory": ["memory:@zack/m16a-memory-package-with-long-alias-truncation-name@0.1.0"],
      "profiles": [],
      "loop": "loop:@zack/m16a-semantic-action-loop@0.1.0"
    }
  }
}
JSON
}

write_model_and_configs() {
  mkdir -p "$WORK/runtime" "$WORK/inputs" "$WORK/scripts"
  cat > "$WORK/runtime/m16a_process_model.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

LOG_PATH = Path("runtime/m16a-process-model-calls.jsonl")
turn = 0

TOOL = "@zack/m16a-search-tool-with-long-alias-truncation-target-name"
SKILL = "@zack/m16a-progressive-skill-with-long-alias-truncation-name"
KNOWLEDGE = "@zack/m16a-contextual-knowledge-with-long-alias-target-name"
MEMORY = "@zack/m16a-memory-package-with-long-alias-truncation-name"
SPACE = "conversation_state_notes_with_intentionally_long_alias_tail"

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": "model",
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

def log(entry):
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a") as handle:
        handle.write(json.dumps(entry, sort_keys=True) + "\n")

def action(action_id, payload):
    return {"id": action_id, "action": payload}

def model_turn(actions, content=None):
    return {
        "assistant_content": content,
        "actions": actions,
        "usage": {},
        "finish_reason": "tool_calls",
        "provider_metadata": {},
    }

for line in sys.stdin:
    msg = json.loads(line)
    log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": "m16a-scripted-model",
            "model": "m16a-scripted",
            "capabilities": {
                "semantic_actions": True,
                "structured_output": True,
                "multimodal_input": False,
                "context_window_tokens": 128000,
                "usage_reporting": True
            }
        })
        continue

    if msg.get("kind") == "request" and msg.get("method") == "generate":
        payload = msg.get("payload") or {}
        aliases = (((payload.get("request") or {}).get("prompt") or {}).get("action_aliases") or [])
        alias_kinds = {alias.get("action_kind") for alias in aliases}
        if "persistence_review_complete" in alias_kinds:
            emit(msg, "response", model_turn([
                action("review-complete", {"type": "persistence_review_complete"})
            ]))
            continue

        turn += 1
        if turn == 1:
            emit(msg, "response", model_turn([
                action("agentpm-tool", {
                    "type": "agent_pm_tool",
                    "tool": TOOL,
                    "arguments": {"query": "m16a alias smoke"}
                }),
                action("skill-entrypoint", {
                    "type": "skill_resource_read",
                    "skill": SKILL,
                    "resource": "entrypoint"
                }),
                action("knowledge-overview", {
                    "type": "knowledge_request",
                    "package": KNOWLEDGE,
                    "mode": "context_document",
                    "document": "knowledge/docs/overview.md",
                    "return_citations": True
                }),
                action("memory-write", {
                    "type": "memory_write",
                    "package": MEMORY,
                    "space": SPACE,
                    "operation": "create",
                    "record_type": "note",
                    "content": {
                        "body": "M16a manual Memory write through a long alias surface.",
                        "tag": "m16a"
                    }
                })
            ]))
        elif turn == 2:
            emit(msg, "response", model_turn([
                action("memory-read", {
                    "type": "memory_read",
                    "package": MEMORY,
                    "space": SPACE,
                    "mode": "chronological",
                    "record_type": "note",
                    "limit": 5
                })
            ]))
        else:
            emit(msg, "response", model_turn([
                action("phase-complete", {
                    "type": "phase_completion",
                    "outcome": "done",
                    "output": {"summary": "m16a semantic alias manual run complete"}
                })
            ]))
        continue

    emit(msg, "error", error={"code": "unsupported_method", "message": "unsupported model request"})
PY
  chmod +x "$WORK/runtime/m16a_process_model.py"

  cat > "$WORK/inputs/all-actions.txt" <<'TXT'
Run the M16a manual semantic-action alias scenario.
Use the deterministic process model's requested Harness semantic actions only, then complete with outcome done.
TXT

  cat > "$WORK/agentpm.m16a.scripted.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "m16a-scripted-model",
    "model": "m16a-scripted"
  },
  "providers": {
    "models": {
      "m16a-scripted-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m16a_process_model.py"],
          "cwd": ".",
          "env": [],
          "startup_timeout_ms": 1000,
          "request_timeout_ms": 5000
        }
      }
    }
  },
  "scopes": {
    "user": "m16a-user"
  },
  "runtime": {
    "state_dir": ".agentpm-state-m16a-scripted",
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 8,
      "max_tool_calls_per_phase": 4,
      "max_actions_per_phase": 16,
      "max_structured_output_repairs": 2,
      "max_tool_call_repairs": 1
    }
  },
  "memory": {
    "write_review": {
      "points": ["run_end"]
    }
  },
  "trace": {
    "level": "verbose"
  }
}
JSON

  cat > "$WORK/agentpm.m16a.openai.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-4o-mini"
  },
  "scopes": {
    "user": "m16a-user-openai"
  },
  "runtime": {
    "state_dir": ".agentpm-state-m16a-openai",
    "limits": {
      "max_steps": 2,
      "max_model_calls_per_phase": 1,
      "max_tool_calls_per_phase": 4,
      "max_actions_per_phase": 16,
      "max_structured_output_repairs": 0,
      "max_tool_call_repairs": 0
    }
  },
  "trace": {
    "level": "verbose"
  }
}
JSON
}

write_assertions() {
  cat > "$WORK/scripts/m16a_assert_aliases.py" <<'PY'
#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
model_log = Path(sys.argv[2])
mode = sys.argv[3] if len(sys.argv) > 3 else "scripted"

def fail(message):
    raise AssertionError(message)

def load_jsonl(path):
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]

report = json.loads(report_path.read_text())
events = load_jsonl(Path(report["trace_path"]))
logs = load_jsonl(model_log)

def event_types():
    return [event.get("event_type") for event in events]

def nested_aliases(value):
    found = []
    if isinstance(value, dict):
        aliases = value.get("action_aliases")
        if isinstance(aliases, list):
            found.append(aliases)
        for child in value.values():
            found.extend(nested_aliases(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(nested_aliases(child))
    return found

alias_sets = []
for event in events:
    if event.get("event_type") == "model_runtime_request_prepared":
        alias_sets.extend(nested_aliases(event))
for row in logs:
    alias_sets.extend(nested_aliases(row))

if not alias_sets:
    fail("no action_aliases found in trace or model log")

def assert_alias_shape(alias):
    name = alias["alias"]
    if alias["action_kind"] in {"phase_completion", "persistence_review_complete"}:
        return
    if name.startswith("action_"):
        fail(f"positional alias survived: {name}")
    if len(name) > 64:
        fail(f"alias longer than 64 chars: {name}")
    if not re.fullmatch(r"[a-z][a-z0-9_]*", name):
        fail(f"alias is not provider-safe ASCII: {name}")
    suffix = name.rsplit("_", 1)[-1]
    if not re.fullmatch(r"[0-9a-f]{8}", suffix):
        fail(f"alias lacks 8-char hex suffix: {name}")

for aliases in alias_sets:
    for alias in aliases:
        assert_alias_shape(alias)

all_aliases = [alias for aliases in alias_sets for alias in aliases]
by_kind = {}
for alias in all_aliases:
    by_kind.setdefault(alias.get("action_kind"), []).append(alias)

def require_kind(kind):
    if kind not in by_kind:
        fail(f"missing alias kind {kind}; saw {sorted(by_kind)}")
    return by_kind[kind]

if mode == "scripted":
    if report.get("terminal_status") != "ended":
        fail(f"expected ended report, got {report.get('terminal_status')}")
    types = event_types()
    for expected in [
        "tool_invoked",
        "tool_completed",
        "skill_resource_requested",
        "skill_resource_loaded",
        "knowledge_request_started",
        "knowledge_retrieved",
        "memory_write_completed",
        "memory_read_completed",
        "memory_write_review_started",
        "memory_write_review_completed",
        "run_completed",
    ]:
        if expected not in types:
            fail(f"missing event {expected}")

    for kind in [
        "agentpm_tool",
        "skill_resource_read",
        "knowledge_request",
        "memory_read",
        "memory_write",
        "phase_completion",
        "persistence_review_complete",
    ]:
        require_kind(kind)

    if require_kind("phase_completion")[0]["alias"] != "phase_complete":
        fail("phase completion alias should be phase_complete")
    if require_kind("persistence_review_complete")[0]["alias"] != "persistence_review_complete":
        fail("review completion alias should be persistence_review_complete")

    memory_write_aliases = [a["alias"] for a in require_kind("memory_write")]
    if not any(a.startswith("memory_write_conversation_state_notes") and "_note_" in a for a in memory_write_aliases):
        fail(f"missing long Memory write alias with space and fixed record type: {memory_write_aliases}")
    if not any(a.startswith("agentpm_tool_m16a_search_tool") for a in [x["alias"] for x in require_kind("agentpm_tool")]):
        fail("missing semantic AgentPM Tool alias")
    if not any(a.startswith("skill_resource_m16a_progressive") for a in [x["alias"] for x in require_kind("skill_resource_read")]):
        fail("missing semantic Skill resource alias")
    if not any(a.startswith("knowledge_request_m16a_contextual") for a in [x["alias"] for x in require_kind("knowledge_request")]):
        fail("missing semantic Knowledge alias")

    review_sets = []
    for event in events:
        if event.get("event_type") != "model_runtime_request_prepared":
            continue
        fields = ((event.get("payload") or {}).get("fields") or {})
        aliases = fields.get("action_aliases") or []
        kinds = {alias.get("action_kind") for alias in aliases}
        if "persistence_review_complete" in kinds:
            review_sets.append(kinds)
    if not review_sets:
        fail("missing persistence review model request aliases")
    forbidden = {"phase_completion", "agentpm_tool", "external_mcp_tool", "knowledge_request", "skill_resource_read"}
    for kinds in review_sets:
        if forbidden & kinds:
            fail(f"review aliases include forbidden semantic action kinds: {sorted(forbidden & kinds)}")
        if not {"memory_read", "memory_write", "persistence_review_complete"} <= kinds:
            fail(f"review aliases did not include Memory read/write plus completion: {sorted(kinds)}")

    print("M16a aliases observed:")
    for kind in sorted(by_kind):
        for alias in sorted({a["alias"] for a in by_kind[kind]}):
            print(f"  {kind}: {alias}")
    print("ok: scripted M16a semantic action aliases and events verified")

elif mode == "native":
    provider_requests = [
        event for event in events
        if event.get("event_type") == "model_runtime_request_prepared"
        and (((event.get("payload") or {}).get("fields") or {}).get("request_kind") == "provider_wire_request")
    ]
    if not provider_requests:
        fail("missing native provider wire request")
    fields = (provider_requests[0]["payload"]["fields"])
    prompt = fields.get("prompt") or ""
    if "EFFECTIVE CAPABILITY CATALOG" in prompt:
        fail("native provider prompt leaked prose capability catalog")
    if "- phase_complete [phase_completion]" in prompt:
        fail("native provider prompt leaked prose action catalog row")
    aliases = fields.get("action_aliases") or []
    if not aliases:
        fail("native provider request did not expose alias mapping in trace")
    if not any(alias.get("alias", "").startswith("memory_write_conversation_state_notes") for alias in aliases):
        fail("native provider trace missing semantic Memory write alias")
    print("ok: native provider request keeps aliases structured and omits prose catalog")
else:
    fail(f"unknown assertion mode {mode}")
PY
  chmod +x "$WORK/scripts/m16a_assert_aliases.py"
}

write_tool
write_skill
write_knowledge
write_memory
write_loop_agent_lock
write_model_and_configs
write_assertions

cat > "$BASE/env.sh" <<TXT
export HARNESS_M16A_TEST_BASE="$BASE"
export M16A_WORK="$WORK"
export M16A_RUNS="$RUNS"
export APM="$APM"
export AGENTPM_MANUAL_PYTHON="$PYTHON_CMD"
TXT

cat <<TXT
M16a manual workspace created at:
  $BASE

Useful paths:
  workspace:   $WORK
  run outputs: $RUNS

To reuse this workspace in a new shell, run:
  source "$BASE/env.sh"
TXT
SH

chmod +x /tmp/setup-harness-m16a-manual.sh
/tmp/setup-harness-m16a-manual.sh

export HARNESS_M16A_TEST_BASE="${HARNESS_M16A_TEST_BASE:-$PWD/harness-m16a-test}"
export M16A_WORK="$HARNESS_M16A_TEST_BASE/workspace"
export M16A_RUNS="$HARNESS_M16A_TEST_BASE/runs"
```

If you open a new terminal or your shell loses these exports:

```bash
source "$PWD/harness-m16a-test/env.sh"
```

## Test 0: Focused Rust Smoke For External MCP Alias Coverage

Imported MCP execution is not available yet, but M16a does generate and normalize `external_mcp_tool` aliases. Verify the focused branch before running the live Harness fixture:

```bash
mkdir -p "$M16A_RUNS/rust"
cargo test -p agentpm-cli \
  provider_action_aliases_are_semantic_provider_safe_and_stable \
  -- --nocapture \
  | tee "$M16A_RUNS/rust/provider-action-aliases.txt"

cargo test -p agentpm-cli \
  provider_call_decoding_covers_all_non_phase_action_kinds \
  -- --nocapture \
  | tee "$M16A_RUNS/rust/provider-call-decoding.txt"

cargo test -p agentpm-cli \
  alias_truncation_prefers_component_boundaries_for_all_action_kinds \
  -- --nocapture \
  | tee "$M16A_RUNS/rust/alias-truncation.txt"
```

Expected:

- the model alias test covers `external_mcp_tool` with a `mcp_tool_..._<hash>` alias;
- the provider decoding test covers `ExternalMcpTool` normalization;
- long MCP names truncate at component boundaries, not arbitrary mid-token cuts.

## Test 1: Deterministic Run Covers Live Semantic Actions

```bash
rm -f "$M16A_WORK/runtime/m16a-process-model-calls.jsonl"
mkdir -p "$M16A_RUNS/scripted"

REPORT="$M16A_RUNS/scripted/all-actions.report.json"
(cd "$M16A_WORK" && "$APM" harness \
  --config agentpm.m16a.scripted.harness.json \
  --headless \
  --scope user=m16a-user \
  --input-file inputs/all-actions.txt \
  --report "$REPORT" \
  >"$M16A_RUNS/scripted/stdout.txt" \
  2>"$M16A_RUNS/scripted/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16A_WORK/scripts/m16a_assert_aliases.py" \
  "$REPORT" \
  "$M16A_WORK/runtime/m16a-process-model-calls.jsonl" \
  scripted \
  | tee "$M16A_RUNS/scripted/alias-summary.txt"
```

Expected:

- run ends `ended`;
- AgentPM Tool, Skill resource, Knowledge, Memory write, Memory read, PhaseCompletion, and PersistenceReviewComplete all appear through the Harness run/review path;
- every generated provider alias is provider-safe ASCII and at most 64 chars;
- no alias starts with `action_`;
- long Tool, Skill, Knowledge, and Memory aliases are readable and hash-suffixed;
- Memory aliases preserve the long space signal and fixed `note` record type where possible;
- persistence review request exposes only Memory read/write plus `persistence_review_complete`.

Useful manual inspection:

```bash
cat "$M16A_RUNS/scripted/alias-summary.txt"
"$AGENTPM_MANUAL_PYTHON" - "$REPORT" <<'PY'
import json
import sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines() if line.strip()]
for event in events:
    if event["event_type"] == "model_runtime_request_prepared":
        fields = event["payload"]["fields"]
        aliases = fields.get("action_aliases") or []
        if aliases:
            print(f"\nrequest_kind={fields.get('request_kind')} catalog_in_prompt={fields.get('capability_catalog_in_prompt')}")
            for alias in aliases:
                print(f"  {alias['action_kind']}: {alias['alias']} -> {alias['identity']}")
PY
```

## Test 2: Optional Native Provider Wire Request Shape

This check uses OpenAI to prove the M14c.1 invariant still holds with M16a aliases: native structured-action providers receive action declarations through the structured tool/function API rather than a duplicated prose capability catalog in ordinary prompt text.

Skip this test if `OPENAI_API_KEY` is not set.

```bash
mkdir -p "$M16A_RUNS/native"

REPORT="$M16A_RUNS/native/openai-alias-wire.report.json"
set +e
(cd "$M16A_WORK" && "$APM" harness \
  --config agentpm.m16a.openai.harness.json \
  --headless \
  --scope user=m16a-user-openai \
  --input "Inspect the available M16a semantic actions, then complete with outcome done." \
  --report "$REPORT" \
  >"$M16A_RUNS/native/stdout.txt" \
  2>"$M16A_RUNS/native/stderr.txt")
STATUS=$?
set -e

test -f "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M16A_WORK/scripts/m16a_assert_aliases.py" \
  "$REPORT" \
  "$M16A_WORK/runtime/m16a-process-model-calls.jsonl" \
  native \
  | tee "$M16A_RUNS/native/native-wire-summary.txt"

test "$STATUS" -eq 0 || {
  echo "Harness exited non-zero after preparing the native provider request. Inspect stderr:"
  cat "$M16A_RUNS/native/stderr.txt"
}
```

Expected:

- trace contains a `model_runtime_request_prepared` event with `request_kind: provider_wire_request`;
- the provider wire prompt does not contain `EFFECTIVE CAPABILITY CATALOG`;
- the provider wire prompt does not contain catalog rows like `- phase_complete [phase_completion] ...`;
- the trace still exposes the semantic alias mapping alongside canonical identities;
- structured action aliases include long Memory/Tool/Skill/Knowledge names rather than `action_N`.

## Test 3: Direct Alias Snapshot From Trace

Use this when reviewing with another model or by eye:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M16A_RUNS/scripted/all-actions.report.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines() if line.strip()]
seen = set()
for event in events:
    if event["event_type"] != "model_runtime_request_prepared":
        continue
    fields = event["payload"]["fields"]
    for alias in fields.get("action_aliases", []):
        key = (alias["action_kind"], alias["identity"], alias["alias"])
        if key in seen:
            continue
        seen.add(key)
        print(f"{alias['action_kind']:28} {alias['alias']:64} {alias['identity']}")
PY
```

Expected examples will look similar to:

```text
agentpm_tool                 agentpm_tool_m16a_search_tool_with_intentionally_long_1a2b3c4d
skill_resource_read          skill_resource_m16a_progressive_disclosure_skill_with_5e6f7a8b
knowledge_request            knowledge_request_m16a_contextual_knowledge_package_9a0b1c2d
memory_write                 memory_write_conversation_state_notes_with_intentionally_note_ab12cd34
phase_completion             phase_complete
persistence_review_complete  persistence_review_complete
```

The exact hash suffixes will differ. The stable requirements are provider-safe names, 64-character maximum, readable target signal, no `action_N`, and canonical identity preserved separately.

## Cleanup

The setup uses only a generated fixture under `harness-m16a-test`.

```bash
rm -rf "$HARNESS_M16A_TEST_BASE"
```

## Regression Tests

After manual testing, rerun the focused automated checks before handoff:

```bash
cargo fmt --all -- --check
cargo test -p agentpm-cli harness_runtime::model::tests -- --nocapture
cargo test -p agentpm-cli harness_runtime::provider::tests -- --nocapture
cargo clippy -p agentpm-cli --all-targets -- -A clippy::too_many_arguments -D warnings
git diff --check
```
