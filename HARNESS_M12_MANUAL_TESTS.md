# Harness Milestone 12 Manual Tests

Manual coverage for Harness M12: local KnowledgeRuntime, generic custom KnowledgeRuntime plumbing, and EmbeddingProvider execution.

This runbook focuses on the pieces that can be verified locally without standing up Pinecone/pgvector:

- context Knowledge readiness and on-demand document loading
- vector Knowledge artifact integrity/readiness
- local vector retrieval through the public `agentpm knowledge query` machinery
- configured process EmbeddingProvider fallback for local vector query embedding
- `before_knowledge_request` and `after_knowledge_retrieval` hooks
- citation/provenance propagation
- Knowledge surface suppression when no compatible EmbeddingProvider exists
- stale/mismatched vector artifacts being unavailable instead of exposed
- phase-local Knowledge access suppression from `Loop access.knowledge=false`
- Knowledge and EmbeddingProvider usage/report/event accounting
- SDK host registration/dispatch for model, embedding, Knowledge, and Knowledge hooks
- built-in model smoke tests with OpenAI and Anthropic

Not covered here: real Pinecone/pgvector KnowledgeRuntime behavior. That is Milestone 13 coverage. This doc includes only a small SDK-host custom KnowledgeRuntime smoke test so the M12 provider bridge is exercised.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

For live model tests:

```bash
export OPENAI_API_KEY="your OpenAI key"
export ANTHROPIC_API_KEY="your Anthropic key"
```

The setup uses deterministic 3-dimensional vectors for repeatable local Harness tests. An optional real OpenAI embedding check is included later and uses `scripts/knowledge_openai_embeddings.py`.

## Setup

Run this from the root of the `agentpm` repo. It creates `harness-m12-test/workspace` with local installed packages under `.agentpm/`, a v3 `agent.lock`, Harness configs, process service scripts, and one Python SDK manual runner.

```bash
cat > /tmp/setup-harness-m12-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M12_TEST_BASE:-$ROOT/harness-m12-test}"
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

mkdir -p "$WORK/runtime"

pkg_dir() {
  local plural="$1"
  local package="$2"
  local version="$3"
  local without_at="${package#@}"
  local namespace="${without_at%%/*}"
  local name="${without_at#*/}"
  printf '%s/.agentpm/%s/%s/%s/%s' "$WORK" "$plural" "$namespace" "$name" "$version"
}

write_f32_vectors() {
  local output="$1"
  "$PYTHON_CMD" - "$output" <<'PY'
import struct
import sys
from pathlib import Path

vectors = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.70710678, 0.70710678, 0.0],
]

out = Path(sys.argv[1])
out.parent.mkdir(parents=True, exist_ok=True)
with out.open("wb") as handle:
    for row in vectors:
        for value in row:
            handle.write(struct.pack("<f", value))
PY
}

write_context_package() {
  local dir
  dir="$(pkg_dir knowledge '@zack/manual-context' '0.1.0')"
  mkdir -p "$dir/knowledge/docs"
  cat > "$dir/knowledge/docs/overview.md" <<'MD'
# Manual Context Overview

AgentPM Harness Milestone 12 should load this context document only after a model asks for it with a Knowledge request.

The key phrase for manual verification is: context package loaded on demand.
MD
  cat > "$dir/knowledge/docs/policy.md" <<'MD'
# Manual Context Policy

Use Knowledge package provenance honestly and do not claim unsupported sources.
MD
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "knowledge",
  "name": "manual-context",
  "version": "0.1.0",
  "description": "Manual M12 context Knowledge package.",
  "knowledge": {
    "mode": "context",
    "documents": [
      {
        "path": "knowledge/docs/overview.md",
        "content_type": "text/markdown",
        "role": "context",
        "description": "M12 manual overview."
      },
      {
        "path": "knowledge/docs/policy.md",
        "content_type": "text/markdown",
        "role": "context",
        "description": "M12 manual policy."
      }
    ]
  }
}
JSON
  "$APM" knowledge build --manifest "$dir/agent.json"
}

write_vector_package() {
  local package="$1"
  local dir
  dir="$(pkg_dir knowledge "$package" '0.1.0')"
  mkdir -p "$dir/knowledge/embeddings"
  cat > "$dir/knowledge/chunks.jsonl" <<'JSONL'
{"id":"chunk_alpha","source_id":"src_alpha","text":"Alpha launch checklist for Harness Milestone 12 Knowledge retrieval.","metadata":{"topic":"alpha","phase":"m12"}}
{"id":"chunk_beta","source_id":"src_beta","text":"Beta rollback guide for unrelated operational notes.","metadata":{"topic":"beta","phase":"m12"}}
{"id":"chunk_mixed","source_id":"src_alpha","text":"Mixed alpha and beta note used to verify ranked vector fallback behavior.","metadata":{"topic":"mixed","phase":"m12"}}
JSONL
  cat > "$dir/knowledge/sources.jsonl" <<'JSONL'
{"id":"src_alpha","title":"Alpha M12 Manual Source","uri":"agentpm://manual/m12/alpha"}
{"id":"src_beta","title":"Beta M12 Manual Source","uri":"agentpm://manual/m12/beta"}
JSONL
  write_f32_vectors "$dir/knowledge/embeddings/default.f32"
  local manifest_name="${package#*/}"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "knowledge",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "Manual M12 vector Knowledge package.",
  "knowledge": {
    "mode": "vector",
    "corpus": {
      "chunks_path": "knowledge/chunks.jsonl",
      "sources_path": "knowledge/sources.jsonl"
    },
    "embedding": {
      "id": "default",
      "provider": "manual",
      "model": "toy-3d",
      "dimensions": 3,
      "metric": "cosine",
      "normalized": true,
      "vectors_path": "knowledge/embeddings/default.f32"
    },
    "retrieval": {
      "strategy": "vector",
      "default_top_k": 2,
      "return_citations": true
    }
  }
}
JSON
  "$APM" knowledge build --manifest "$dir/agent.json"
}

write_context_package
write_vector_package '@zack/manual-vector'
write_vector_package '@zack/manual-vector-broken'

# Corrupt one built vector package after build. Preflight should suppress it with an
# artifact-integrity reason instead of exposing it as usable Knowledge.
BROKEN_DIR="$(pkg_dir knowledge '@zack/manual-vector-broken' '0.1.0')"
"$PYTHON_CMD" - "$BROKEN_DIR/knowledge/embeddings/default.f32" <<'PY'
import struct
import sys
from pathlib import Path

out = Path(sys.argv[1])
with out.open("wb") as handle:
    # Only two 3D rows for a manifest/corpus that declares three chunks.
    for value in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]:
        handle.write(struct.pack("<f", value))
PY

LOOP_DIR="$(pkg_dir loops '@zack/manual-knowledge-loop' '0.1.0')"
mkdir -p "$LOOP_DIR"
cat > "$LOOP_DIR/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "manual-knowledge-loop",
  "version": "0.1.0",
  "description": "Manual Harness loop for M12 Knowledge tests.",
  "loop": {
    "archetype": "research_then_answer",
    "entry_phase": "research",
    "limits": {
      "max_steps": 8
    },
    "phases": [
      {
        "id": "research",
        "objective": "Use available Knowledge packages when they help answer the user, then finish.",
        "access": {
          "tools": false,
          "knowledge": true,
          "memory": {
            "read": false,
            "write": false
          }
        },
        "outcomes": [
          {
            "id": "answer",
            "description": "Enough Knowledge has been gathered to answer."
          },
          {
            "id": "no-knowledge",
            "description": "Move to a phase where Knowledge access is intentionally disabled."
          },
          {
            "id": "handoff",
            "description": "The request should be handed off."
          }
        ]
      },
      {
        "id": "no-knowledge",
        "objective": "Answer without Knowledge access.",
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
            "id": "answer",
            "description": "Return the answer."
          }
        ]
      },
      {
        "id": "final",
        "objective": "Return the final concise user-facing response."
      }
    ],
    "transitions": [
      { "from": "research", "on": "answer", "to": "final" },
      { "from": "research", "on": "no-knowledge", "to": "no-knowledge" },
      { "from": "research", "on": "handoff", "to": "\$handoff" },
      { "from": "no-knowledge", "on": "answer", "to": "final" },
      { "from": "final", "on": "complete", "to": "\$end" }
    ],
    "error_policy": {
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
  "name": "manual-harness-m12-agent",
  "version": "0.1.0",
  "description": "Local manual Harness M12 test agent.",
  "tools": [],
  "knowledge": [
    "@zack/manual-context@0.1.0",
    "@zack/manual-vector@0.1.0",
    "@zack/manual-vector-broken@0.1.0"
  ],
  "loop": "@zack/manual-knowledge-loop@0.1.0",
  "bindings": {
    "phases": {
      "research": {
        "knowledge": [
          "@zack/manual-context",
          "@zack/manual-vector",
          "@zack/manual-vector-broken"
        ]
      },
      "no-knowledge": {
        "knowledge": [
          "@zack/manual-context",
          "@zack/manual-vector"
        ]
      }
    }
  }
}
JSON

cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-09-01T00:00:00Z",
  "packages": {
    "knowledge:@zack/manual-context@0.1.0": {
      "kind": "knowledge",
      "name": "@zack/manual-context",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "knowledge:@zack/manual-vector@0.1.0": {
      "kind": "knowledge",
      "name": "@zack/manual-vector",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "knowledge:@zack/manual-vector-broken@0.1.0": {
      "kind": "knowledge",
      "name": "@zack/manual-vector-broken",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "loop:@zack/manual-knowledge-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/manual-knowledge-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "manual-harness-m12-agent",
      "version": "0.1.0",
      "knowledge": [
        "knowledge:@zack/manual-context@0.1.0",
        "knowledge:@zack/manual-vector@0.1.0",
        "knowledge:@zack/manual-vector-broken@0.1.0"
      ],
      "loop": "loop:@zack/manual-knowledge-loop@0.1.0"
    }
  }
}
JSON

cat > "$WORK/runtime/toy_embedding_adapter.py" <<'PY'
#!/usr/bin/env python3
import json
import math
import sys

payload = json.load(sys.stdin)
text = str(payload.get("text") or "").lower()
if "beta" in text and "alpha" not in text:
    vector = [0.0, 1.0, 0.0]
elif "mixed" in text:
    vector = [0.70710678, 0.70710678, 0.0]
else:
    vector = [1.0, 0.0, 0.0]
norm = math.sqrt(sum(v * v for v in vector))
vector = [v / norm for v in vector]
json.dump({
    "vector": vector,
    "provider": "manual",
    "model": "toy-3d",
    "dimensions": 3,
    "normalized": True
}, sys.stdout)
sys.stdout.write("\n")
PY
chmod +x "$WORK/runtime/toy_embedding_adapter.py"

cat > "$WORK/runtime/toy_embedding_service.py" <<'PY'
#!/usr/bin/env python3
import json
import math
import sys

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": msg.get("service", "embedding"),
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

def vector_for(text):
    text = str(text or "").lower()
    if "beta" in text and "alpha" not in text:
        vector = [0.0, 1.0, 0.0]
    elif "mixed" in text:
        vector = [0.70710678, 0.70710678, 0.0]
    else:
        vector = [1.0, 0.0, 0.0]
    norm = math.sqrt(sum(v * v for v in vector))
    return [v / norm for v in vector]

for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": msg.get("payload", {}).get("registry_id", "toy-embedder"),
            "embedding_spaces": [
                {
                    "id": "default",
                    "provider": "manual",
                    "model": "toy-3d",
                    "dimensions": 3,
                    "metric": "cosine",
                    "normalized": True
                }
            ]
        })
        continue
    if msg.get("method") != "embed":
        emit(msg, "error", error={"code": "unsupported_method", "message": "unsupported embedding method"})
        continue
    payload = msg.get("payload", {})
    emit(msg, "response", {
        "vector": vector_for(payload.get("text")),
        "provider": payload.get("provider"),
        "model": payload.get("model"),
        "dimensions": payload.get("dimensions"),
        "normalized": payload.get("normalized")
    })
PY
chmod +x "$WORK/runtime/toy_embedding_service.py"

cat > "$WORK/runtime/knowledge_hooks.py" <<'PY'
#!/usr/bin/env python3
import json
import sys

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": msg.get("service", "hook"),
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("kind") == "initialize":
        hooks = msg.get("payload", {}).get("hooks", [])
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": msg.get("payload", {}).get("registry_id", "knowledge-hooks"),
            "hooks": hooks
        })
        continue

    method = msg.get("method")
    payload = msg.get("payload", {})
    input_payload = payload.get("input", {})

    if method == "before_knowledge_request":
        request = input_payload.get("request", {})
        patch = {}
        if request.get("mode") == "vector_query":
            patch["return_citations"] = True
        if request.get("mode") == "vector_query" and "hook rewrite" in str(request.get("query", "")).lower():
            patch.update({
                "query": "alpha launch checklist",
                "top_k": 1,
                "score_threshold": 0.0
            })
        emit(msg, "response", {"decision": "continue", "patch": patch})
        continue

    if method == "after_knowledge_retrieval":
        result = input_payload.get("result", {})
        patch = {}
        if isinstance(result.get("content"), str):
            patch["content"] = "[hooked] " + result["content"]
        if isinstance(result.get("results"), list) and result["results"]:
            patch["results"] = [
                {
                    "chunk_id": row["chunk_id"],
                    "source_id": row["source_id"],
                    "text": "[hooked] " + row.get("text", "")
                }
                for row in result["results"]
            ]
        emit(msg, "response", {"decision": "continue", "patch": patch})
        continue

    emit(msg, "error", error={"code": "unsupported_hook", "message": f"unsupported hook {method}"})
PY
chmod +x "$WORK/runtime/knowledge_hooks.py"

cat > "$WORK/runtime/sdk_m12_python.py" <<'PY'
#!/usr/bin/env python3
import json
import math
import os
import sys
from pathlib import Path
from typing import Any

from agentpm import HarnessClient

cli = os.environ["AGENTPM_HARNESS_CLI"]
workspace = os.environ["AGENTPM_HARNESS_WORKSPACE"]
calls: list[str] = []
model_calls = 0

def normalized_vector(text: str) -> list[float]:
    text = text.lower()
    if "beta" in text and "alpha" not in text:
        vector = [0.0, 1.0, 0.0]
    elif "mixed" in text:
        vector = [0.70710678, 0.70710678, 0.0]
    else:
        vector = [1.0, 0.0, 0.0]
    norm = math.sqrt(sum(v * v for v in vector))
    return [v / norm for v in vector]

def model_provider(_: Any) -> dict[str, Any]:
    global model_calls
    calls.append("model")
    model_calls += 1
    if model_calls == 1:
        return {
            "assistant_content": None,
            "actions": [
                {
                    "id": "sdk-context",
                    "action": {
                        "type": "knowledge_request",
                        "package": "@zack/manual-context",
                        "mode": "context_document",
                        "document": "knowledge/docs/overview.md",
                        "return_citations": True,
                    },
                },
                {
                    "id": "sdk-vector",
                    "action": {
                        "type": "knowledge_request",
                        "package": "@zack/manual-vector",
                        "mode": "vector_query",
                        "query": "hook rewrite sdk vector query",
                        "top_k": 2,
                        "return_citations": True,
                    },
                },
            ],
            "usage": {},
            "finish_reason": "tool_calls",
            "provider_metadata": {},
        }
    outcome = "answer" if model_calls == 2 else "complete"
    return {
        "assistant_content": None,
        "actions": [
            {
                "id": "sdk-complete",
                "action": {
                    "type": "phase_completion",
                    "outcome": outcome,
                    "output": {
                        "message": "SDK M12 manual run completed.",
                    },
                },
            }
        ],
        "usage": {},
        "finish_reason": "tool_calls",
        "provider_metadata": {},
    }

def embedding_provider(request: dict[str, Any]) -> dict[str, Any]:
    calls.append(f"embedding:{request['text']}")
    return {
        "vector": normalized_vector(request["text"]),
        "provider": request["provider"],
        "model": request["model"],
        "dimensions": request["dimensions"],
        "normalized": request["normalized"],
    }

def knowledge_runtime(request: dict[str, Any]) -> dict[str, Any]:
    calls.append(f"knowledge:{request['package']}:{request['mode']}")
    return {
        "ok": True,
        "package": request["package"],
        "version": request["version"],
        "mode": request["mode"],
        "document": request.get("document"),
        "query": request.get("query"),
        "content": "SDK host KnowledgeRuntime returned manual context.",
        "results": [],
        "citations": [],
    }

def before_knowledge_request(input_payload: dict[str, Any]) -> dict[str, Any]:
    calls.append("before_knowledge_request")
    request = input_payload["request"]
    if request["mode"] == "vector_query":
        return {
            "decision": "continue",
            "patch": {
                "query": "alpha launch checklist",
                "top_k": 1,
                "return_citations": True,
            },
        }
    return {"decision": "continue"}

def after_knowledge_retrieval(input_payload: dict[str, Any]) -> dict[str, Any]:
    calls.append("after_knowledge_retrieval")
    result = input_payload["result"]
    rows = result.get("results") or []
    if rows:
        return {
            "decision": "continue",
            "patch": {
                "results": [
                    {
                        "chunk_id": rows[0]["chunk_id"],
                        "source_id": rows[0]["source_id"],
                        "text": "[sdk-hooked] " + rows[0].get("text", ""),
                    }
                ]
            },
        }
    return {"decision": "continue"}

client = HarnessClient(agentpm_path=cli, cwd=workspace)
client.register_model_provider("company-model", model_provider)
client.register_embedding_provider(
    "embedder",
    embedding_provider,
    {
        "embedding_spaces": [
            {
                "provider": "manual",
                "model": "toy-3d",
                "dimensions": 3,
                "normalized": True,
            }
        ]
    },
)
client.register_knowledge_runtime(
    "kb",
    knowledge_runtime,
    {
        "modes": ["context_document"],
        "features": ["citations"],
        "packages": [
            {
                "package": "@zack/manual-context",
                "version": "0.1.0",
                "ready": True,
            }
        ],
    },
)
client.on_before_knowledge_request(before_knowledge_request)
client.on_after_knowledge_retrieval(after_knowledge_retrieval)

try:
    info = client.initialize()
    required = info.get("required_host_services") or []
    assert {"role": "embedding", "registry_id": "embedder"} in required, required
    assert {"role": "knowledge", "registry_id": "kb"} in required, required
    result = client.run("Run the SDK M12 manual fixture.")
    assert result["status"] == "ended", result
    assert "model" in calls, calls
    assert any(call.startswith("embedding:") for call in calls), calls
    assert "knowledge:@zack/manual-context:context_document" in calls, calls
    assert "before_knowledge_request" in calls, calls
    assert "after_knowledge_retrieval" in calls, calls
    report = result["report"]
    assert report["usage"]["knowledge_requests"] >= 2, report["usage"]
    assert report["usage"]["embedding_requests"] >= 1, report["usage"]
    print(json.dumps({"ok": True, "calls": calls, "usage": report["usage"]}, indent=2))
finally:
    client.shutdown()
PY
chmod +x "$WORK/runtime/sdk_m12_python.py"

cat > "$WORK/agentpm.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "company-model",
    "model": "sdk-scripted"
  },
  "providers": {
    "models": {
      "company-model": {
        "implementation": {
          "type": "host",
          "request_timeout_ms": 120000
        }
      }
    },
    "embeddings": {
      "embedder": {
        "implementation": {
          "type": "host",
          "request_timeout_ms": 120000
        }
      }
    }
  },
  "knowledge": {
    "runtimes": {
      "kb": {
        "implementation": {
          "type": "host",
          "request_timeout_ms": 120000
        }
      }
    },
    "packages": {
      "@zack/manual-context": {
        "runtime": "kb"
      }
    },
    "embedding_matches": [
      {
        "match": {
          "provider": "manual",
          "model": "toy-3d",
          "dimensions": 3,
          "normalized": true
        },
        "embedding_provider": "embedder"
      }
    ]
  },
  "runtime": {
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 8,
      "max_tool_calls_per_phase": 1,
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

write_live_config() {
  local output="$1"
  local provider="$2"
  local model="$3"
  cat > "$WORK/$output" <<JSON
{
  "version": 1,
  "model": {
    "provider": "$provider",
    "model": "$model"
  },
  "providers": {
    "embeddings": {
      "toy-embedder": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/toy_embedding_service.py"],
          "cwd": ".",
          "startup_timeout_ms": 10000,
          "request_timeout_ms": 30000,
          "restart": {
            "max_attempts": 0,
            "backoff_ms": 0
          }
        }
      }
    }
  },
  "hooks": {
    "implementations": {
      "knowledge-hooks": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/knowledge_hooks.py"],
          "cwd": ".",
          "startup_timeout_ms": 10000,
          "request_timeout_ms": 30000,
          "restart": {
            "max_attempts": 0,
            "backoff_ms": 0
          }
        }
      }
    },
    "bindings": [
      {
        "hook": "before_knowledge_request",
        "implementation": "knowledge-hooks",
        "failure_policy": "closed"
      },
      {
        "hook": "after_knowledge_retrieval",
        "implementation": "knowledge-hooks",
        "failure_policy": "closed"
      }
    ]
  },
  "knowledge": {
    "embedding_matches": [
      {
        "match": {
          "provider": "manual",
          "model": "toy-3d",
          "dimensions": 3,
          "normalized": true
        },
        "embedding_provider": "toy-embedder"
      }
    ]
  },
  "runtime": {
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 8,
      "max_tool_calls_per_phase": 1,
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
}

write_live_config "agentpm.openai.harness.json" "openai" "${AGENTPM_MANUAL_OPENAI_MODEL:-gpt-4o-mini}"
write_live_config "agentpm.anthropic.harness.json" "anthropic" "${AGENTPM_MANUAL_ANTHROPIC_MODEL:-claude-haiku-4-5-20251001}"

cat > "$WORK/agentpm.no-embedder.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-4o-mini"
  },
  "runtime": {
    "limits": {
      "max_steps": 4,
      "max_model_calls_per_phase": 4,
      "max_tool_calls_per_phase": 1,
      "max_actions_per_phase": 8,
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

cat > "$WORK/input-context.txt" <<'TXT'
Use the @zack/manual-context Knowledge package. Request document knowledge/docs/overview.md, then answer with the key phrase from that document. Do not use external knowledge.
TXT

cat > "$WORK/input-vector.txt" <<'TXT'
Use the @zack/manual-vector Knowledge package. Run a vector_query for "alpha launch checklist" with citations, then answer with the top source title and chunk id. Do not use external knowledge.
TXT

cat > "$WORK/input-hook-vector.txt" <<'TXT'
Use the @zack/manual-vector Knowledge package. Run a vector_query for "hook rewrite should become alpha" with top_k 2 and citations. The hook should rewrite the request. Answer with the retrieved chunk id.
TXT

cat > "$WORK/input-no-knowledge.txt" <<'TXT'
Move to the no-knowledge phase, then answer without making any Knowledge request.
TXT

cat > "$WORK/runtime/assert_m12_report.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
events_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
report = json.loads(report_path.read_text())
usage = report.get("usage", {})
assert usage.get("knowledge_requests", 0) >= 1, usage
if events_path:
    events = [json.loads(line) for line in events_path.read_text().splitlines() if line.strip()]
    types = {event.get("event_type") for event in events}
    assert "knowledge_surface_ready" in types or "knowledge_surface_unavailable" in types, types
    assert "knowledge_request_started" in types, types
    assert "knowledge_retrieved" in types or "knowledge_failed" in types, types
print(json.dumps({"ok": True, "usage": usage}, indent=2))
PY
chmod +x "$WORK/runtime/assert_m12_report.py"

echo "Linting generated manifests..."
(cd "$WORK" && "$APM" lint)

cat <<EOF
Created M12 manual workspace:
  $WORK

Next:
  cd "$WORK"
EOF
SH

bash /tmp/setup-harness-m12-manual.sh
cd harness-m12-test/workspace
```

Expected setup result:

- all generated manifests lint successfully;
- context and vector Knowledge packages have generated metadata in their `agent.json`;
- `@zack/manual-vector-broken` exists but has intentionally corrupted vector bytes after build.

## Test 1: Inspect context/vector packages

```bash
"$APM" knowledge inspect @zack/manual-context --json >inspect-context.json
"$APM" knowledge inspect @zack/manual-vector --json >inspect-vector.json
cat inspect-context.json
cat inspect-vector.json
```

Expected:

- `inspect-context.json` shows top-level `mode: "context"` and `build.document_count: 2`.
- `inspect-vector.json` shows top-level `mode: "vector"`, `build.dimensions: 3`, `build.chunk_count: 3`, `build.vector_count: 3`, and a fresh generated local index.

Structural check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

context = json.loads(Path("inspect-context.json").read_text())
vector = json.loads(Path("inspect-vector.json").read_text())
assert context["mode"] == "context"
assert context["build"]["document_count"] == 2
assert context["manifest_metadata_fresh"] is True
assert vector["mode"] == "vector"
assert vector["build"]["dimensions"] == 3
assert vector["build"]["chunk_count"] == 3
assert vector["build"]["vector_count"] == 3
assert vector["index"]["fresh"] is True
print("inspect checks passed")
PY
```

## Test 2: Query vector Knowledge directly with public query machinery

This verifies the `agentpm knowledge query` path that Harness reuses for local vector search.

With an explicit query vector:

```bash
cat >query-alpha-vector.json <<'JSON'
{
  "vector": [1.0, 0.0, 0.0],
  "provider": "manual",
  "model": "toy-3d",
  "dimensions": 3,
  "normalized": true
}
JSON

"$APM" knowledge query @zack/manual-vector \
  --vector-json query-alpha-vector.json \
  --top-k 2 \
  --include-text \
  --include-metadata \
  --json >query-alpha-direct.json
cat query-alpha-direct.json
```

With the local adapter contract:

```bash
"$APM" knowledge query @zack/manual-vector "alpha launch checklist" \
  --embedding-command "$AGENTPM_MANUAL_PYTHON runtime/toy_embedding_adapter.py" \
  --top-k 2 \
  --include-text \
  --include-metadata \
  --json >query-alpha-adapter.json
cat query-alpha-adapter.json
```

Expected:

- both queries return `chunk_alpha` as rank 1;
- result rows retain `chunk_id`, `source_id`, score, source metadata, and text when requested.

Structural check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

for path in ["query-alpha-direct.json", "query-alpha-adapter.json"]:
    data = json.loads(Path(path).read_text())
    assert data["results"][0]["chunk_id"] == "chunk_alpha", data
    assert data["results"][0]["source_id"] == "src_alpha", data
print("direct vector query checks passed")
PY
```

## Test 3: Preflight readiness and suppression

`agentpm harness --json` currently emits the preflight report shape only:

```json
{
  "status": "ready",
  "diagnostics": []
}
```

It does not include the resolved runtime Knowledge surface catalog. Knowledge surface availability/unavailability is verified later from run traces.

For a human-readable preflight summary with per-capability static readiness details, use either config `trace.level = "verbose"` or the CLI flag:

```bash
"$APM" harness --config agentpm.openai.harness.json --verbose
```

Expected:

- `Static capabilities` includes `available` and `pending runtime activation` counts.
- `Static capability details` lists each counted capability, including available Knowledge packages and pending runtime-backed services such as the selected model provider, hook service, and EmbeddingProvider.
- Static `available` is not final runtime realization. Runtime Knowledge surface readiness is still verified from `knowledge_surface_ready` / `knowledge_surface_unavailable` trace events during a run.

Full process EmbeddingProvider config:

```bash
"$APM" harness --config agentpm.openai.harness.json --json >preflight-openai.json
cat preflight-openai.json
```

Expected:

- status is `ready`;
- diagnostics are empty.

No EmbeddingProvider config:

```bash
"$APM" harness --config agentpm.no-embedder.harness.json --json >preflight-no-embedder.json
cat preflight-no-embedder.json
```

Expected:

- status is `ready`;
- diagnostics are empty.
- This command proves the config and static manifest graph are valid. It does not prove Knowledge runtime-surface readiness by itself.

Structural check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

full = json.loads(Path("preflight-openai.json").read_text())
missing = json.loads(Path("preflight-no-embedder.json").read_text())
assert full["status"] == "ready", full
assert full["diagnostics"] == [], full
assert missing["status"] == "ready", missing
assert missing["diagnostics"] == [], missing
print("preflight config/static graph checks passed")
PY
```

## Test 4: Live OpenAI context Knowledge request

Requires `OPENAI_API_KEY`.

```bash
"$APM" harness --config agentpm.openai.harness.json \
  --headless \
  --input-file input-context.txt \
  --report openai-context-report.json \
  >openai-context.stdout.txt 2>openai-context.stderr.txt

cat openai-context.stdout.txt
cat openai-context.stderr.txt
```

Expected:

- exit code `0`;
- the model requests `@zack/manual-context`;
- the next model request includes the loaded context result;
- `openai-context-report.json` records at least one Knowledge request;
- the trace includes Knowledge surface/request/retrieval events.

Find the generated event trace path:

```bash
OPENAI_CONTEXT_EVENTS="$(python3 - <<'PY'
import json
from pathlib import Path
print(json.loads(Path("openai-context-report.json").read_text()).get("trace_path", ""))
PY
)"
echo "$OPENAI_CONTEXT_EVENTS"
python3 runtime/assert_m12_report.py openai-context-report.json "$OPENAI_CONTEXT_EVENTS"
```

Provider behavior can vary. If the model answers without issuing the requested Knowledge action, rerun the same command once and inspect `openai-context-report.json` before treating it as a Harness failure.

## Test 5: Live OpenAI vector Knowledge request with process EmbeddingProvider

Requires `OPENAI_API_KEY`.

```bash
"$APM" harness --config agentpm.openai.harness.json \
  --headless \
  --input-file input-vector.txt \
  --report openai-vector-report.json \
  >openai-vector.stdout.txt 2>openai-vector.stderr.txt

cat openai-vector.stdout.txt
cat openai-vector.stderr.txt
```

Expected:

- exit code `0`;
- `toy-embedder` initializes as an `embedding` process service;
- local vector retrieval returns `chunk_alpha`/`src_alpha` for the alpha query;
- citations/provenance are present in the Knowledge result;
- `usage.knowledge_requests >= 1`;
- `usage.embedding_requests >= 1`;
- events include `knowledge_request_started`, `knowledge_retrieved`, `embedding_request_started`, and `embedding_request_completed`; embedding failures should appear as `embedding_request_failed` plus the Knowledge failure event.

Structural check:

```bash
OPENAI_VECTOR_EVENTS="$(python3 - <<'PY'
import json
from pathlib import Path
print(json.loads(Path("openai-vector-report.json").read_text()).get("trace_path", ""))
PY
)"

python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("openai-vector-report.json").read_text())
usage = report["usage"]
assert usage["knowledge_requests"] >= 1, usage
assert usage["embedding_requests"] >= 1, usage
text = json.dumps(report)
assert "chunk_alpha" in text or "src_alpha" in text, text[:1000]
print("OpenAI vector report usage/provenance checks passed")
PY

python3 runtime/assert_m12_report.py openai-vector-report.json "$OPENAI_VECTOR_EVENTS"

python3 - "$OPENAI_VECTOR_EVENTS" <<'PY'
import json
import sys
from pathlib import Path

events = [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]
text = json.dumps(events)
types = {event.get("event_type") for event in events}
assert "knowledge_surface_ready" in {event.get("event_type") for event in events}, text[:1000]
assert "knowledge_surface_unavailable" in {event.get("event_type") for event in events}, text[:1000]
assert "embedding_request_started" in types, types
assert "embedding_request_completed" in types, types
assert "@zack/manual-vector-broken" in text, text[:1000]
assert "installed vector artifact integrity failed" in text or "vector" in text, text[:1000]
print("Knowledge surface readiness/suppression trace checks passed")
PY
```

## Test 6: Live OpenAI Knowledge hooks

Requires `OPENAI_API_KEY`.

```bash
"$APM" harness --config agentpm.openai.harness.json \
  --headless \
  --input-file input-hook-vector.txt \
  --report openai-hook-vector-report.json \
  >openai-hook-vector.stdout.txt 2>openai-hook-vector.stderr.txt

cat openai-hook-vector.stdout.txt
cat openai-hook-vector.stderr.txt
```

Expected:

- `before_knowledge_request` runs and patches the vector query to `alpha launch checklist`, `top_k: 1`, and `return_citations: true`;
- `after_knowledge_retrieval` runs and patches model-visible result text with `[hooked]`;
- Hook events appear in the trace;
- the phase continues after successful hook shaping and retrieval.

Structural check:

```bash
OPENAI_HOOK_EVENTS="$(python3 - <<'PY'
import json
from pathlib import Path
print(json.loads(Path("openai-hook-vector-report.json").read_text()).get("trace_path", ""))
PY
)"

python3 - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("openai-hook-vector-report.json").read_text())
text = json.dumps(report)
assert "alpha launch checklist" in text or "chunk_alpha" in text, text[:1000]
assert report["usage"]["knowledge_requests"] >= 1, report["usage"]
assert report["usage"]["embedding_requests"] >= 1, report["usage"]
print("OpenAI hook/vector report checks passed")
PY

python3 - "$OPENAI_HOOK_EVENTS" <<'PY'
import json
import sys
from pathlib import Path

events_path = Path(sys.argv[1])
events = [json.loads(line) for line in events_path.read_text().splitlines() if line.strip()]
types = {event["event_type"] for event in events}
assert "hook_started" in types, types
assert "hook_completed" in types, types
assert "knowledge_request_started" in types, types
assert "embedding_request_started" in types, types
assert "embedding_request_completed" in types, types
print("OpenAI hook trace checks passed")
PY
```

## Test 7: Live Anthropic context/vector smoke tests

Requires `ANTHROPIC_API_KEY`.

Context:

```bash
"$APM" harness --config agentpm.anthropic.harness.json \
  --headless \
  --input-file input-context.txt \
  --report anthropic-context-report.json \
  >anthropic-context.stdout.txt 2>anthropic-context.stderr.txt

cat anthropic-context.stdout.txt
cat anthropic-context.stderr.txt
```

Vector:

```bash
"$APM" harness --config agentpm.anthropic.harness.json \
  --headless \
  --input-file input-vector.txt \
  --report anthropic-vector-report.json \
  >anthropic-vector.stdout.txt 2>anthropic-vector.stderr.txt

cat anthropic-vector.stdout.txt
cat anthropic-vector.stderr.txt
```

Expected:

- both runs exit `0`;
- Anthropic emits compatible semantic actions for Knowledge requests;
- vector run records `usage.embedding_requests >= 1`;
- both traces include Knowledge events.

Structural check:

```bash
python3 - <<'PY'
import json
from pathlib import Path

for report_name in ["anthropic-context-report.json", "anthropic-vector-report.json"]:
    report = json.loads(Path(report_name).read_text())
    assert report["usage"]["knowledge_requests"] >= 1, (report_name, report["usage"])
    if "vector" in report_name:
        assert report["usage"]["embedding_requests"] >= 1, report["usage"]
print("Anthropic report checks passed")
PY
```

As with OpenAI, if the model ignores the explicit instruction to call Knowledge, inspect the trace and rerun once before treating it as a Harness failure.

## Test 8: Loop access suppresses Knowledge in the disabled phase

This test is model-dependent because the model must choose the `no-knowledge` outcome. It verifies that Harness suppresses Knowledge descriptors when the active phase has `access.knowledge=false`.

```bash
"$APM" harness --config agentpm.openai.harness.json \
  --headless \
  --input-file input-no-knowledge.txt \
  --report openai-no-knowledge-report.json \
  >openai-no-knowledge.stdout.txt 2>openai-no-knowledge.stderr.txt

cat openai-no-knowledge.stdout.txt
cat openai-no-knowledge.stderr.txt
```

Expected:

- if the run enters `no-knowledge`, the trace shows Knowledge capability suppression with reason `Loop access.knowledge=false for this phase`;
- no Knowledge request is accepted in that phase.

Structural check:

```bash
NO_KNOWLEDGE_EVENTS="$(python3 - <<'PY'
import json
from pathlib import Path
print(json.loads(Path("openai-no-knowledge-report.json").read_text()).get("trace_path", ""))
PY
)"

python3 - "$NO_KNOWLEDGE_EVENTS" <<'PY'
import json
import sys
from pathlib import Path

events = [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]
text = json.dumps(events)
assert "Loop access.knowledge=false for this phase" in text, text[:1000]
print("Loop knowledge suppression check passed")
PY
```

If the model does not transition to `no-knowledge`, this test is inconclusive rather than failed.

## Test 9: Node SDK real-CLI host embedding and Knowledge

This uses the SDK repo’s gated real-CLI integration test against the fixture workspace. The default `agentpm.harness.json` in the fixture is intentionally SDK-host based.

From `agentpm/harness-m12-test/workspace`:

```bash
export AGENTPM_HARNESS_CLI="$(cd ../.. && pwd)/target/debug/agentpm"
export AGENTPM_HARNESS_WORKSPACE="$(pwd)"
export AGENTPM_HARNESS_EMBEDDING_PROVIDER="embedder"
export AGENTPM_HARNESS_EMBEDDING_SPACE_PROVIDER="manual"
export AGENTPM_HARNESS_EMBEDDING_SPACE_MODEL="toy-3d"
export AGENTPM_HARNESS_EMBEDDING_DIMENSIONS="3"
export AGENTPM_HARNESS_EMBEDDING_NORMALIZED="true"
export AGENTPM_HARNESS_KNOWLEDGE_RUNTIME="kb"
export AGENTPM_HARNESS_KNOWLEDGE_PACKAGE="@zack/manual-context"
export AGENTPM_HARNESS_KNOWLEDGE_VERSION="0.1.0"
export AGENTPM_HARNESS_EMBEDDING_KNOWLEDGE_PACKAGE="@zack/manual-vector"

cd ../../../agentpm-sdk-node
pnpm test -- --run test/harness.spec.ts -t 'real agentpm harness process'
```

Expected:

- test is not skipped;
- SDK registers required host services for `embedding:embedder` and `knowledge:kb`;
- host model dispatches two Knowledge actions;
- host KnowledgeRuntime receives the mapped context request;
- host EmbeddingProvider receives the local vector query embedding request;
- registration status for both new M12 services is active.

## Test 10: Python SDK manual host embedding and Knowledge

From `agentpm/harness-m12-test/workspace`:

```bash
export AGENTPM_HARNESS_CLI="$(cd ../.. && pwd)/target/debug/agentpm"
export AGENTPM_HARNESS_WORKSPACE="$(pwd)"

cd ../../../agentpm-sdk-python
uv run python ../agentpm/harness-m12-test/workspace/runtime/sdk_m12_python.py
```

Expected:

- script exits `0`;
- JSON output includes calls for `model`, `embedding:...`, `knowledge:@zack/manual-context:context_document`, `before_knowledge_request`, and `after_knowledge_retrieval`;
- reported usage has `knowledge_requests >= 2` and `embedding_requests >= 1`.

## Optional: real OpenAI embeddings through the existing script

This optional check verifies the real OpenAI embedding utility and `agentpm knowledge query --embedding-command` path with a 1536-dimensional package. It is separate from the deterministic 3D Harness fixture so normal M12 manual tests stay fast and repeatable.

Requires `OPENAI_API_KEY`.

From the root of the `agentpm` repo:

```bash
cd harness-m12-test/workspace
OPENAI_VEC_DIR=".agentpm/knowledge/zack/manual-vector-openai/0.1.0"
mkdir -p "$OPENAI_VEC_DIR/knowledge/embeddings"
cp .agentpm/knowledge/zack/manual-vector/0.1.0/knowledge/chunks.jsonl "$OPENAI_VEC_DIR/knowledge/chunks.jsonl"
cp .agentpm/knowledge/zack/manual-vector/0.1.0/knowledge/sources.jsonl "$OPENAI_VEC_DIR/knowledge/sources.jsonl"

"$AGENTPM_MANUAL_PYTHON" ../../scripts/knowledge_openai_embeddings.py chunks-to-f32 \
  --chunks "$OPENAI_VEC_DIR/knowledge/chunks.jsonl" \
  --output "$OPENAI_VEC_DIR/knowledge/embeddings/default.f32" \
  --model text-embedding-3-small

cat > "$OPENAI_VEC_DIR/agent.json" <<'JSON'
{
  "kind": "knowledge",
  "name": "manual-vector-openai",
  "version": "0.1.0",
  "description": "Manual M12 vector Knowledge package using real OpenAI embeddings.",
  "knowledge": {
    "mode": "vector",
    "corpus": {
      "chunks_path": "knowledge/chunks.jsonl",
      "sources_path": "knowledge/sources.jsonl"
    },
    "embedding": {
      "id": "default",
      "provider": "openai",
      "model": "text-embedding-3-small",
      "dimensions": 1536,
      "metric": "cosine",
      "normalized": true,
      "vectors_path": "knowledge/embeddings/default.f32"
    },
    "retrieval": {
      "strategy": "vector",
      "default_top_k": 2,
      "return_citations": true
    }
  }
}
JSON

"$APM" knowledge build --manifest "$OPENAI_VEC_DIR/agent.json"

"$APM" knowledge query "$OPENAI_VEC_DIR" "alpha launch checklist" \
  --embedding-command "$AGENTPM_MANUAL_PYTHON ../../scripts/knowledge_openai_embeddings.py adapter" \
  --top-k 2 \
  --include-text \
  --json >query-openai-embeddings.json

cat query-openai-embeddings.json
```

Expected:

- OpenAI writes three 1536-dimensional vectors;
- `agentpm knowledge build` succeeds and generates local index metadata;
- query returns ranked rows with chunk/source identities.

## Baseline automated checks worth rerunning before M13

From the root of `agentpm`:

```bash
cargo test -p agentpm-cli knowledge
cargo test -p agentpm-cli harness_runtime::knowledge
cargo test -p agentpm-cli harness_runtime::hook
cargo test -p agentpm-cli commands::harness
cargo fmt --all --check
```

From SDK repos:

```bash
cd ../agentpm-sdk-node
pnpm test -- --run
pnpm build
pnpm lint

cd ../agentpm-sdk-python
uv run pytest
uv run mypy agentpm tests
```

## Failure interpretation

- If direct `agentpm knowledge query` fails, investigate the package artifacts/build metadata first; Harness depends on that public query path for local vector retrieval.
- If preflight marks `@zack/manual-vector` unavailable with the full config, inspect `knowledge.embedding_matches` and `toy-embedder` process initialization.
- If only live OpenAI/Anthropic runs fail to call Knowledge, inspect the trace before changing code; model action choice can vary.
- If SDK tests fail before dispatch, check that the fixture default `agentpm.harness.json` is still SDK-host based and that `AGENTPM_HARNESS_CLI`/`AGENTPM_HARNESS_WORKSPACE` point at this fixture.
- If `embedding_requests` stays `0` after a vector query that needed text embedding, that is an M12 accounting regression.
