# Feature
Phase 7B: Built-In Harness Runtime

## Problem / Goal
AgentPM can now package the major authored building blocks of an agent system: Tools, Skills, Knowledge, Memory Blueprints, Instruction Profiles, Loops, Agents, and Templates. Phase 7A made orchestration and composition portable by adding Loop contracts and Agent bindings, but AgentPM still does not provide a canonical runtime that interprets those artifacts together.

Without a built-in runtime, users must still write framework-specific application code to resolve an Agent, assemble phase context, call models, execute Tools, retrieve Knowledge, persist Memory, evaluate Memory operations, enforce Loop access, handle approvals, expose or consume MCP, trace execution, and manage retries and terminal outcomes. That makes the package model harder to adopt and leaves the most important integration behavior to every application author.

Phase 7B introduces the **AgentPM Harness**: AgentPM's opinionated, transparent reference runtime for executing a composed Agent directly from the CLI while keeping the portable artifact contracts usable by third-party runtimes.

The Harness must be deliberately understandable. A developer should be able to inspect what AgentPM resolved, why a capability is available or suppressed, what model and runtime provider are active, what phase is executing, what Tool or retrieval action was requested, what Memory lifecycle operation ran, what approval is pending, and how the Loop reached its terminal result.

The phase must:

- Add `agentpm harness` as the canonical built-in Agent execution command.
- Keep the canonical orchestration engine in Rust; Node and Python SDKs launch and communicate with that engine rather than reimplementing Loop execution.
- Support interactive Ratatui, plain/headless, and machine/SDK execution over one UI-agnostic Harness engine.
- Resolve execution from `agent.lock` and installed `.agentpm/` artifacts, with local `agent.json` optional when the lockfile already identifies the runnable Agent root.
- Add optional workspace-owned `agentpm.harness.json` runtime configuration with strong defaults and transparent source/precedence reporting.
- Add a separate mutable `.agentpm-state/` runtime directory for local Memory state, traces, and run reports; never write live runtime state into installed package roots under `.agentpm/`.
- Execute Loop graphs, outcomes, limits, checkpoints, Tool/phase error policy, terminal targets, and additive Agent bindings.
- Compute a fresh `EffectivePhase` for every phase execution from authored bindings, inherited capabilities, Loop restrictions, runtime augmentation, and live runtime readiness.
- Treat Profiles, Skills, Knowledge, Memory, Tools, MCP, and consumer context according to the runtime semantics in this spec without rewriting the portable artifacts.
- Add stable structured events, typed Hook interception points, approval/control messages, cancellation, durable trace output, and an exportable JSON run report.
- Make hooks first-class in both Node and Python SDKs so application authors can register callbacks/providers without implementing the subprocess wire protocol themselves.
- Use the public `agentpm run` command as the canonical Harness Tool execution boundary and harden `agentpm run` with machine-readable results, schema enforcement, runtime-version enforcement, and nested-process cancellation safety.
- Use the public `agentpm serve --mcp` command to realize Agent-authored outward MCP surfaces and add a machine startup/event mode for Harness lifecycle management.
- Support runtime-configured external MCP servers as scoped inward Tool augmentation for already-published Agents.
- Provide built-in model support for OpenAI, Anthropic, and Ollama while keeping concrete model IDs open-ended runtime values.
- Provide a typed external provider mechanism for custom model, embedding, Knowledge, Memory, hook, and approval implementations.
- Ship usable reference external Knowledge providers for Pinecone and pgvector.
- Ship usable reference external Memory providers for PostgreSQL/pgvector and Redis/Redis Stack. Pinecone must not be presented as a complete MemoryRuntime because it cannot faithfully realize the full Memory Blueprint contract by itself.
- Make provider/hook contracts available through the language SDKs where application-hosted implementations are useful, so consumers can extend AgentPM without repeatedly writing transport boilerplate.
- Provide a built-in local SQLite MemoryRuntime as the zero-infrastructure reference implementation.
- Support lightweight TUI branding customization without turning the Harness UI into a theming framework.
- Add strong example Templates/workspaces that demonstrate minimal, SDK-hosted, external-provider, MCP, and full-feature Harness adoption.

### Core definitions

> The AgentPM Harness is the canonical AgentPM runtime that interprets an installed Agent, its resolved Loop and bindings, workspace runtime configuration, and live integrations. It is an opinionated reference implementation, not a new portable package kind.

> A Harness Session owns long-lived runtime services and may host multiple Runs sequentially, with at most one active Run per Session. A Run is one Loop traversal from entry phase to a terminal or runtime terminal state.

> An Effective Phase is the runtime-computed view of one Loop phase after authored global/phase bindings, Skill inheritance, runtime augmentation, Loop restrictions, and live capability readiness are applied. It is never written back to an Agent, Loop, or lockfile.

> Runtime configuration describes how this workspace realizes portable capabilities. It must not mutate or silently override the portable Agent/Loop contract.

## Non-goals

- Do not introduce a new `harness` package kind.
- Do not make Templates runnable units; `agentpm harness` executes Agents.
- Do not make Loop `archetype` executable or introduce archetype-specific engine branches.
- Do not turn Loop transitions, checkpoints, bindings, Memory triggers, or runtime config into an arbitrary expression/programming language.
- Do not implement separate orchestration engines in Node and Python.
- Do not mutate published Agent, Loop, Skill, Knowledge, Memory, Profile, or Tool packages during execution.
- Do not store live Memory records, trigger state, run reports, or traces inside installed package directories.
- Do not automatically invoke another Agent for `$handoff`.
- Do not add persistent Run resume/checkpoint restoration in this phase. JSON run reports are required; resumable RunState is deferred.
- Do not make README content executable behavior for any artifact.
- Do not eagerly inject all Skill, Knowledge, or Memory content into prompts.
- Do not infer a custom Knowledge or Memory backend from artifact metadata. Custom runtime realization is explicit workspace configuration.
- Do not silently fall back from an explicitly configured external runtime to a different runtime when the configured runtime fails.
- Do not allow hooks, retrieved content, Tool output, Memory content, consumer context, or model output to expand runtime authority.
- Do not automatically expose every runtime-discovered external MCP Tool globally; runtime config must explicitly scope imported MCP servers to phases/global scope.
- Do not treat outward MCP calls as Harness phase execution.
- Do not provision or synchronize Pinecone, pgvector, Redis, or other external stores automatically from package artifacts. Runtime providers consume already-prepared external infrastructure.
- Do not create a complex TUI extension/plugin system in this phase.

## Architectural invariants

1. **Agent is the runnable composition boundary.** Harness execution begins from one resolved Agent.
2. **`agent.lock` is required and authoritative for resolved execution versions.** `agent.json` is optional when the lockfile/install state already identifies the Agent root.
3. **Loop graph is authoritative.** `archetype` is descriptive only.
4. **HarnessEngine is the only writer of authoritative RunState.** Services and hooks return results/proposals; Engine validates and applies state changes.
5. **Portable artifacts remain immutable.** Effective runtime composition and mutable state are never written back into manifests or installed package roots.
6. **Dependency declaration is not runtime availability.** Agent bindings establish authored availability; runtime augmentation is explicit and separately identified.
7. **Global and phase bindings are additive.** Duplicate package identities are deduplicated; Memory selectors are unioned without inventing override precedence.
8. **Loop restrictions can narrow capability but never create it.** `true` permits an otherwise available capability, `false` suppresses it, omission expresses no Loop opinion.
9. **Runtime configuration may realize or narrow authored capability and may explicitly add scoped external MCP Tools.** It may not rewrite Loop graph/access/checkpoints or manufacture AgentPM package dependencies.
10. **Model-visible content can influence reasoning but cannot change authority.** The model proposes semantic actions; Harness validates and performs them.
11. **Provider API function/tool calling is transport, not semantics.** Harness distinguishes AgentPM Tool calls, external MCP Tool calls, Skill resource reads, Knowledge requests, Memory reads/writes, and phase completion even if the model provider represents all of them as function calls.
12. **The canonical engine is Rust.** SDKs are typed clients/hosts around the machine protocol.
13. **AgentPM public execution surfaces remain public.** Harness direct Tool calls invoke public `agentpm run`; Harness MCP export starts public `agentpm serve --mcp`; local Knowledge should reuse public AgentPM query machinery where practical.
14. **Defaults are observable.** TUI, events, reports, and machine preflight must identify whether a value came from authored metadata, runtime config, CLI/SDK override, environment, or Harness default.
15. **Unknown/unavailable optional capabilities degrade safely.** The Harness fails the whole Run only when execution cannot be interpreted coherently or a required runtime dependency for the active operation cannot be satisfied.
16. **A Session has at most one active Run.** A reusable Harness Session may execute multiple Runs, but they execute sequentially. Phase 7B does not queue or concurrently execute authoritative Run state machines inside one Session; callers that need concurrent Agent executions create multiple Sessions.

## Architecture overview and end-to-end flow

The Harness should be understandable as a small number of layers with explicit ownership boundaries. The implementation may use repository-appropriate module/type names, but it must preserve this shape:

```text
                       PORTABLE / INSTALLED INPUTS

       agent.lock + resolved Agent + Loop + bound package artifacts
                              |
                              v
+-----------------------------------------------------------------------+
|                         BOOTSTRAP / PREFLIGHT                         |
|                                                                       |
| workspace discovery -> package resolution -> config resolution        |
| -> cross-package validation -> service startup/handshake              |
| -> capability/readiness computation -> ResolvedHarnessPlan            |
+----------------------------------+------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------------------+
|                           HARNESS SESSION                             |
|                                                                       |
|  ModelRuntime     HookRuntime       KnowledgeRuntime                  |
|  MemoryRuntime    McpRuntime        ApprovalRuntime                   |
|  trace/events     machine host      long-lived provider processes     |
|                                                                       |
|      +---------------------------------------------------------+      |
|      |                         RUN                             |      |
|      |                                                         |      |
|      | RunContext snapshot + RunState + Loop traversal          |      |
|      |                                                         |      |
|      |   phase entry                                           |      |
|      |      |                                                  |      |
|      |      v                                                  |      |
|      |   EffectivePhase                                        |      |
|      |      |                                                  |      |
|      |      v                                                  |      |
|      |   phase-local agentic inner loop                        |      |
|      |      |                                                  |      |
|      |      +-> ModelRuntime -> normalized semantic actions     |      |
|      |      |        |                                         |      |
|      |      |        +-> ToolRuntime -> agentpm run            |      |
|      |      |        +-> McpRuntime -> imported MCP Tool       |      |
|      |      |        +-> Skill packaged-resource read          |      |
|      |      |        +-> KnowledgeRuntime                       |      |
|      |      |        +-> MemoryRuntime                          |      |
|      |      |        +-> PhaseCompletion                        |      |
|      |      |                                                  |      |
|      |      +<- structured action result appended to            |      |
|      |          phase-local transcript                          |      |
|      |                                                         |      |
|      |   PhaseResult -> Loop transition/checkpoint -> next phase|      |
|      +---------------------------------------------------------+      |
+-----------------------------------------------------------------------+
                                   |
                                   v
                       terminal result + report/trace
```

Events observe every layer but do not own state. Hooks intercept only typed decision points. HarnessEngine remains the only writer of RunState. Runtime/provider implementations perform typed capabilities but do not own Loop traversal or capability authority.

### Execution surfaces all use the same engine

```text
Ratatui interactive UI ---------+
                                |
plain one-shot --headless ------+--> HarnessSession / HarnessEngine
                                |
--machine + Node/Python SDK ----+
```

The three surfaces differ only in presentation/control transport:

- **interactive TUI** keeps a Session open, accepts user messages from a visible composer, starts one Run per submitted message, renders approvals/events/output, and may execute repeated Runs sequentially; while a Run is active (including while waiting at an interactive approval checkpoint), the composer does not start another Run;
- **plain headless** is a one-shot surface: bootstrap -> create Session -> execute exactly one Run from CLI/stdin/file input -> print the terminal user-facing result -> flush report/trace -> shut down the Session. It must not require the machine protocol and must never auto-approve;
- **machine/SDK** keeps stdin/stdout reserved for the versioned protocol and allows the host application to start Runs, receive events, answer approvals, host Hooks/providers, cancel, and invoke supported controls. A Session accepts at most one active Run; a second `start_run` request is rejected rather than queued.

For plain headless, normal final/handoff output goes to stdout while diagnostics/preflight warnings belong on stderr or the report/trace. Successful `ended`/`handed_off` results use a successful process exit; `aborted`, `failed`, `cancelled`, `limit_reached`, and `approval_required` use a non-success exit according to repository CLI conventions. Machine mode never places human prose on protocol stdout.

### Bootstrap -> Session -> Run sequence

A normal invocation follows this order:

1. discover workspace and resolve the selected Agent from `agent.lock`;
2. load/validate workspace runtime configuration and runtime scope inputs;
3. resolve installed artifacts and build the immutable `ResolvedHarnessPlan`;
4. start/handshake Session-owned providers, Hooks, Memory/Knowledge runtimes, MCP surfaces, approval transport, and trace sinks;
5. compute readiness/preflight diagnostics without executing the Loop;
6. create a RunContext snapshot, including current scope values and Consumer Context;
7. enter the Loop entry phase, evaluate its EffectivePhase, and execute the phase-local agentic inner loop;
8. produce a PhaseResult, evaluate checkpoints/transitions, and repeat until authored/runtime terminal state;
9. produce terminal output, RunReport, events/trace, and aggregate Run usage into Session usage;
10. after that Run is no longer active, either accept another Run sequentially (TUI/machine Session) or shut down owned Session services (one-shot headless).

## Runtime filesystem model

The workspace has three conceptually distinct areas:

```text
workspace/
├── agent.json                    # optional local authored Agent
├── agent.lock                    # required execution source of truth
├── agentpm.harness.json          # optional consumer runtime config
├── consumer-owned context/hooks/app code
├── .agentpm/                     # immutable installed package state
└── .agentpm-state/               # mutable Harness runtime state
    ├── memory.sqlite3            # default local MemoryRuntime
    └── runs/
        └── <run-id>/
            ├── events.jsonl
            └── report.json
```

`.agentpm/` remains analogous to installed dependency state and must not contain mutable runtime records.

`.agentpm-state/` is runtime-managed, should be gitignored by generated/example workspaces, and may be overridden by runtime configuration or CLI/SDK options. Deleting `.agentpm/` must not be required to clear Memory, and reinstalling packages must not rewrite `.agentpm-state/`.

## Command and execution modes

Primary command:

```bash
agentpm harness [AGENT]
```

`AGENT` may be omitted when exactly one runnable Agent root can be selected from the workspace/lockfile.

If multiple runnable Agent roots exist:

- interactive mode prompts for the Agent;
- headless/machine mode requires an explicit Agent selector and fails with an actionable error rather than guessing.

Required modes:

- default TTY mode: Ratatui interactive UI;
- `--headless`: plain non-TUI execution suitable for scripts;
- `--machine`: structured bidirectional protocol suitable for SDK/application hosting.

The exact CLI flag spelling may follow repository conventions, but the three modes and one-engine architecture are required.

Useful supported inputs/options should include repository-appropriate equivalents of:

- Agent selector;
- input text or input file/stdin;
- config path override;
- model/provider override;
- repeated runtime scope overrides (`key=value`);
- trace/report path override;
- headless/machine selection.

Interactive mode may prompt for missing model/provider/scope/approval values. Headless mode must not invent them.

## Harness configuration contract

The default workspace config file is:

```text
agentpm.harness.json
```

It is consumer-owned, optional, source-control friendly, and not packaged with the Agent. If the file exists, `version` is required. Unused sections should be omitted rather than emitted as empty objects. Version 1 is strict: unknown fields are rejected except for the intentionally open ID-keyed registries/maps and provider-specific `model.options` object described below.

All relative paths in Harness config resolve from the discovered workspace root, not from the config file's own directory. Moving the config file with `--config` does not change path bases.

### Minimal valid configuration

```json
{
  "version": 1
}
```

A missing config file is equivalent to version-1 defaults plus values supplied by CLI/SDK/environment. It is not an error.

### Complete version-1 example

The following example is intentionally populated so every nested registry/mapping shape is concrete. A real workspace should omit sections it does not use.

```json
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-5",
    "options": {
      "temperature": 0.2
    }
  },
  "providers": {
    "models": {
      "company-model": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/company_model_provider.py"],
          "cwd": ".",
          "env": ["COMPANY_MODEL_API_KEY"],
          "startup_timeout_ms": 15000,
          "request_timeout_ms": 120000,
          "restart": {
            "max_attempts": 1,
            "backoff_ms": 250
          }
        }
      }
    },
    "embeddings": {
      "example-4d": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/example_embedder.py"],
          "env": []
        }
      }
    }
  },
  "scopes": {
    "user": "user-42",
    "conversation": "incident-abc"
  },
  "runtime": {
    "state_dir": ".agentpm-state",
    "limits": {
      "max_steps": 100,
      "max_model_calls_per_phase": 24,
      "max_tool_calls_per_phase": 16,
      "max_actions_per_phase": 64,
      "max_tool_call_repairs": 2,
      "max_structured_output_repairs": 2,
      "max_memory_operation_repairs": 2
    }
  },
  "hooks": {
    "implementations": {
      "workspace-policy": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/hooks.py"],
          "env": ["POLICY_SERVICE_TOKEN"]
        }
      }
    },
    "bindings": [
      {
        "hook": "before_model_request",
        "implementation": "workspace-policy",
        "failure_policy": "closed"
      },
      {
        "hook": "before_tool_call",
        "implementation": "workspace-policy",
        "failure_policy": "closed"
      },
      {
        "hook": "before_memory_write",
        "implementation": "workspace-policy",
        "failure_policy": "continue"
      }
    ]
  },
  "knowledge": {
    "runtimes": {
      "pinecone-prod": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/pinecone_knowledge.py"],
          "env": ["PINECONE_API_KEY"]
        }
      }
    },
    "packages": {
      "@zack/agentpm-docs": {
        "runtime": "pinecone-prod"
      }
    },
    "embedding_matches": [
      {
        "match": {
          "provider": "bring-your-own",
          "model": "example-normalized-4d",
          "dimensions": 4,
          "normalized": true
        },
        "embedding_provider": "example-4d"
      }
    ]
  },
  "memory": {
    "local": {
      "semantic": {
        "embedding_provider": "example-4d",
        "model": "example-normalized-4d",
        "dimensions": 4
      }
    },
    "runtimes": {
      "postgres-prod": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/postgres_memory.py"],
          "env": ["DATABASE_URL"]
        }
      }
    },
    "packages": {
      "@zack/conversation-continuity": {
        "runtime": "postgres-prod"
      }
    }
  },
  "mcp": {
    "imports": {
      "github": {
        "transport": "stdio",
        "command": "github-mcp-server",
        "args": [],
        "env": ["GITHUB_TOKEN"],
        "scope": {
          "mode": "phases",
          "phases": ["assess", "execute"]
        },
        "tools": ["get_issue", "search_issues"],
        "startup_timeout_ms": 15000,
        "request_timeout_ms": 120000,
        "restart": {
          "max_attempts": 1,
          "backoff_ms": 250
        }
      },
      "company-search": {
        "transport": "http",
        "url": "https://mcp.example.com/mcp",
        "headers": {
          "Authorization": {
            "env": "COMPANY_MCP_AUTHORIZATION"
          },
          "X-Workspace": {
            "value": "support"
          }
        },
        "scope": {
          "mode": "global"
        }
      }
    },
    "exports": {
      "enabled": true,
      "host": "127.0.0.1"
    }
  },
  "approvals": {
    "controller": {
      "implementation": {
        "type": "process",
        "command": "python",
        "args": ["runtime/approval_controller.py"],
        "env": []
      }
    },
    "timeout_ms": 300000
  },
  "trace": {
    "enabled": true,
    "level": "normal",
    "content": "redacted"
  },
  "ui": {
    "branding": {
      "name": "AgentPM Harness",
      "subtitle": null,
      "accent": null
    }
  }
}
```

The field names and nesting in this section are the Phase 7B version-1 contract. Implementation type/function/file names may follow repository conventions, but Codex should not rename/restructure these JSON fields without an explicit spec change.

### Shared service implementation descriptor

Custom model providers, embedding providers, Hook implementations, Knowledge runtimes, Memory runtimes, and optional approval controllers use the same implementation descriptor. The registry in which the implementation appears determines which runtime role it provides and which transport Harness must use to reach it.

A process-backed implementation is:

```json
{
  "type": "process",
  "command": "python",
  "args": ["runtime/provider.py"],
  "cwd": ".",
  "env": ["PROVIDER_API_KEY"],
  "startup_timeout_ms": 15000,
  "request_timeout_ms": 120000,
  "restart": {
    "max_attempts": 1,
    "backoff_ms": 250
  }
}
```

Rules:

- `command` is executed directly, never through an implicit shell. It may be an absolute executable path or a name resolved by the normal process PATH.
- `args` is an optional array of literal arguments; default `[]`. Harness does not interpolate shell variables/templates into arguments.
- `cwd` is optional, defaults to workspace root, and when present must be a safe workspace-relative path that canonicalizes inside the workspace root; absolute `cwd` is not allowed in config v1.
- `env` is an optional list of environment-variable names to project into that child; default `[]`. Values are resolved from the Harness secret/environment resolver (including `.env.local` when supported). Harness supplies only a minimal non-secret platform process baseline needed for normal execution (for example PATH/home/temp/system variables as appropriate) plus these declared variables; it must not inherit arbitrary parent variables. Literal secret values do not belong in this descriptor.
- `startup_timeout_ms` defaults to `15000` and must be positive.
- `request_timeout_ms` defaults to `120000` and must be positive. Individual semantic runtimes may apply a more specific authored/runtime timeout when their contract already provides one.
- `restart.max_attempts` defaults to `1`; `0` disables automatic process restart. `restart.backoff_ms` defaults to `250`.
- Automatic restart never automatically replays an in-flight request. The failed request fails visibly; a successful restart only makes the service available for subsequent requests.
- Process services use the versioned AgentPM service protocol over persistent stdin/stdout and must complete the service-specific handshake before becoming ready.

An application-hosted implementation is:

```json
{
  "type": "host",
  "request_timeout_ms": 120000
}
```

Rules:

- `host` means the parent machine/SDK client supplies the implementation over the Harness machine protocol under the configured registry ID.
- A configured `host` implementation without a connected/registered host provider is unavailable during preflight; Harness never silently replaces it.
- Host lifecycle/restart belongs to the parent application. Harness only applies request timeout/cancellation semantics.

Built-in implementations do not require entries in these registries.

### Runtime contracts and transports

The runtime contract is the semantic API. `agentpm-service` and the host-service portion of `agentpm-harness-machine` are two different transports for that same semantic API, not versions or subsets of each other.

For example, HarnessEngine semantically calls `ModelRuntime.generate(ModelRequest) -> ModelTurn`. A built-in provider satisfies that call directly through Rust types. A `process` implementation satisfies it over the common external service protocol using `protocol: "agentpm-service"`. A `host` implementation satisfies it over the Harness machine protocol using correlated host-service request/response frames supplied by the parent SDK/application. The Engine sees the same typed runtime abstraction in all three cases.

`ToolRuntime` and `McpRuntime` are intentional exceptions. Harness direct Tool calls use the public `agentpm run --machine` boundary, and MCP import/export uses MCP plus the public `agentpm serve --mcp --machine` lifecycle where applicable. They do not become generic `agentpm-service` providers.

### Common external service protocol

The implementation descriptor tells Harness **how to reach** an implementation; this section defines **how Harness communicates with it**. Local Rust implementations use the same typed Rust interfaces directly without serialization. Process implementations serialize those interfaces over persistent stdin/stdout. `host` implementations serialize the same semantic requests through the Harness machine protocol to the parent SDK/application.

#### Framing and envelope

Phase 7B uses newline-delimited UTF-8 JSON frames for AgentPM-owned process protocols. Each stdout line from a process service is exactly one protocol frame; human diagnostics from the child belong on stderr. A common envelope is required conceptually:

```json
{
  "protocol": "agentpm-service",
  "version": 1,
  "kind": "request",
  "id": "req-123",
  "service": "knowledge",
  "method": "retrieve",
  "payload": {}
}
```

Responses preserve `id` and use either a typed `result` or typed error:

```json
{
  "protocol": "agentpm-service",
  "version": 1,
  "kind": "response",
  "id": "req-123",
  "service": "knowledge",
  "result": {}
}
```

Service-originated informational events may use `kind: "event"`, but an event can never stand in for a required response. Requests are correlated by opaque IDs and may be in flight concurrently only where the typed service/runtime explicitly supports it; implementations must not rely on JSON line ordering as request identity.

Harness begins every process service with an `initialize` request containing Session/workspace-safe metadata, requested role/registry ID, protocol version, and any role-specific initialization data. The service returns a ready result containing its exact role plus its **live capability advertisement**. A configured process that reports the wrong role, incompatible protocol version, or capabilities insufficient for its mapped use is unavailable during preflight.

For example, a custom model service initialization response may be logically equivalent to:

```json
{
  "protocol": "agentpm-service",
  "version": 1,
  "kind": "response",
  "id": "init-1",
  "service": "model",
  "result": {
    "registry_id": "company-model",
    "ready": true,
    "capabilities": {
      "semantic_actions": true,
      "structured_output": true,
      "multimodal_input": false,
      "context_window_tokens": 32768,
      "usage_reporting": true
    }
  }
}
```

Other roles return their role-specific capability objects defined later in this spec. The exact field names inside typed role payloads may follow the published protocol schema generated by implementation, but they must preserve the semantic fields/authority described here rather than substituting untyped free-form JSON.

Cancellation uses a correlated `cancel` request where the service supports cooperative cancellation; Harness may terminate a Harness-owned child after the normal graceful timeout. Secrets are never placed in generic initialization payloads when they were projected through the child environment instead.

#### Required typed service methods/capabilities

The exact Rust/SDK type names may follow repository conventions, but the following semantic contracts must exist:

| Role | Minimum requests Harness needs | Readiness/capability advertisement |
| --- | --- | --- |
| `model` | `initialize`, `generate` | provider/model identity; native semantic-action/tool-call support; structured-output support; multimodal-input support; optional `context_window_tokens`; usage-reporting availability |
| `embedding` | `initialize`, `embed` | supported embedding-space tuples/patterns including provider/model/dimensions/normalization the implementation can satisfy |
| `hook` | `initialize`, `invoke` | exact version-1 Hook IDs implemented by this service |
| `knowledge` | `initialize`, `retrieve` | supported Knowledge modes/features plus package/version/corpus realization attestations for explicitly mapped packages |
| `memory` | `initialize`, direct record/read-write primitives, retrieval, count/state/batch primitives | normalized MemoryRuntime descriptor defined later, plus package/version realization readiness |
| `approval` | `initialize`, `request_approval` | approval capability and optional cancellation support |

`McpRuntime` speaks MCP to imported servers rather than the AgentPM service protocol. Harness-managed outward exports are controlled through the separate `agentpm serve --mcp --machine` contract. `ToolRuntime` invokes `agentpm run --machine` rather than registering as a persistent service.

#### Harness machine protocol

`agentpm harness --machine` also uses newline-delimited UTF-8 JSON with a separate protocol identifier such as `agentpm-harness-machine`, version `1`. Machine stdout is protocol-only.

The machine client and Harness negotiate once per Session. Required message families are:

- client -> Harness requests: initialize/register host capabilities, start Run, cancel Run, invoke externally-triggered Memory operation, shutdown;
- Harness -> client events: the same stable Harness events written to trace, subject to machine subscription/content policy;
- Harness -> client host-service requests: typed Hook/model/embedding/Knowledge/Memory/approval requests for configured/registered `type: "host"` implementations;
- client -> Harness host-service responses: correlated typed success/error responses;
- Harness -> client Run/preflight/terminal responses: structured results and report references.

A machine Session permits at most one active Run. If the client sends `start_run` while another Run is active, Harness returns a stable structured session-busy/active-Run error and leaves the existing Run untouched. Phase 7B does not implicitly queue pending Run requests; a client that wants queueing owns that policy and may retry after the active Run terminates.

A host provider is registered by `(service role, configured registry ID)` and advertises the same capabilities as a process provider. The Engine must not care whether a typed runtime call was satisfied locally in Rust, by a child process, or by the parent SDK host.

### Configuration precedence

For runtime values that can be supplied in multiple places, use the following precedence from highest to lowest:

1. explicit SDK/application per-run option/control registration;
2. explicit CLI option;
3. `agentpm.harness.json`;
4. standard environment variables / provider-specific environment conventions;
5. Harness default.

Portable Agent/Loop metadata is not part of this override chain. Runtime config may realize/narrow it but cannot override authored graph/access/checkpoint semantics.

The resolved value and source must be inspectable in preflight/events/report.

### Model configuration and custom model providers

`model.provider` and `model.model` are open non-empty strings. Phase 7B ships built-in model-provider IDs:

- `openai`
- `anthropic`
- `ollama`

Concrete model IDs are never enumerated by AgentPM.

`model.options` is an optional provider-specific JSON object passed only to the selected ModelRuntime. It is intentionally open-ended because providers expose different generation controls. It must not be used as a secret store; secrets come from the runtime environment/resolver.

If `model.provider` is not a built-in ID, it must match a key in `providers.models`. Built-in IDs are reserved and cannot be redefined in `providers.models`.

Example custom provider:

```json
{
  "providers": {
    "models": {
      "company-model": {
        "implementation": {
          "type": "host"
        }
      }
    }
  },
  "model": {
    "provider": "company-model",
    "model": "support-v7"
  }
}
```

### Embedding providers

`providers.embeddings` is an ID-keyed registry of typed EmbeddingProvider implementations:

```json
{
  "providers": {
    "embeddings": {
      "corp-embedder": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/embedder.py"],
          "env": ["CORP_EMBEDDING_TOKEN"]
        }
      }
    }
  }
}
```

An EmbeddingProvider performs the typed capability `text -> compatible vector`; it does not own Knowledge retrieval unless separately configured as a KnowledgeRuntime.

### Static runtime scopes

`scopes` is an open map of authored scope key to non-empty runtime string value:

```json
{
  "scopes": {
    "tenant": "acme",
    "repository": "agentpm-dev/agentpm"
  }
}
```

Keys such as `user` and `conversation` have no built-in semantics. SDK/CLI per-run scope values override config values by key. The model never supplies authoritative scope values directly.

### Runtime state and limits

`runtime.state_dir` defaults to `.agentpm-state`. A relative value resolves from workspace root. An absolute path is allowed for this field specifically because durable runtime state may intentionally live outside the source workspace.

`runtime.limits` supports exactly:

- `max_steps` positive integer;
- `max_model_calls_per_phase` positive integer;
- `max_tool_calls_per_phase` positive integer;
- `max_actions_per_phase` positive integer;
- `max_tool_call_repairs` non-negative integer;
- `max_structured_output_repairs` non-negative integer;
- `max_memory_operation_repairs` non-negative integer.

Their meanings are deliberately distinct:

- `max_steps`: maximum Loop phase executions in the Run. One phase entry/re-entry consumes one step; model calls/actions inside that phase do not.
- `max_model_calls_per_phase`: maximum calls to ModelRuntime associated with one phase execution, including ordinary phase-turn calls, structured repair calls, and model-assisted Memory lifecycle calls triggered synchronously while that phase is active. It does not count embedding calls.
- `max_tool_calls_per_phase`: maximum **logical** Tool actions accepted in one phase across AgentPM Tools and imported MCP Tools. A Loop retry of the same finalized Tool action does not count as a new logical Tool call; a later model proposal with different arguments is a new Tool call.
- `max_actions_per_phase`: maximum normalized model semantic action proposals accepted for processing in one phase. It includes AgentPM Tool calls, imported MCP Tool calls, Skill resource reads, Knowledge requests, direct Memory reads/writes, and PhaseCompletion proposals. It excludes model calls themselves, Hook invocations, approval checks, Harness-internal Memory lifecycle operations, embedding calls, Tool retry attempts, and rejected/malformed proposals that never become accepted semantic actions. Those rejected proposals are bounded by model-call/repair limits instead.
- `max_tool_call_repairs`: additional model repair attempts after an invalid Tool argument proposal; the initial proposal is not a repair.
- `max_structured_output_repairs`: additional model repair attempts for invalid/missing phase completion/outcome structured output; the initial proposal is not a repair.
- `max_memory_operation_repairs`: additional model repair attempts for invalid transform/consolidate target content; the initial operation-model result is not a repair.

`max_tool_calls_per_phase` is therefore a subset of `max_actions_per_phase`; the latter protects phases that repeatedly retrieve/load/read without calling Tools.

If absent, use:

- `max_steps`: `100` Harness safety ceiling;
- `max_model_calls_per_phase`: `24`;
- `max_tool_calls_per_phase`: `16`;
- `max_actions_per_phase`: `64`;
- `max_tool_call_repairs`: `2` additional repair attempts;
- `max_structured_output_repairs`: `2` additional repair attempts;
- `max_memory_operation_repairs`: `2` additional repair attempts.

`loop.limits.max_steps`, when present, remains authoritative. Effective maximum steps is the stricter of the Loop value and runtime safety ceiling. Runtime config may tighten but never loosen the authored Loop limit. Reaching any Harness safety limit must emit a typed limit event and fail/terminate through the documented failure path rather than silently truncating work.

### Hook implementations and bindings

`hooks.implementations` defines reusable HookRuntime process/host implementations. `hooks.bindings` binds typed hook IDs to those implementations in deterministic authored array order:

```json
{
  "hooks": {
    "implementations": {
      "policy": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/hooks.py"]
        }
      }
    },
    "bindings": [
      {
        "hook": "before_model_request",
        "implementation": "policy",
        "failure_policy": "closed"
      },
      {
        "hook": "before_tool_call",
        "implementation": "policy",
        "failure_policy": "continue"
      }
    ]
  }
}
```

Version-1 hook IDs are:

- `before_model_request`;
- `before_tool_selection`;
- `before_tool_call`;
- `before_knowledge_request`;
- `after_knowledge_retrieval`;
- `before_memory_read`;
- `before_memory_write`;
- `before_memory_operation`.

`failure_policy` is exactly `closed | continue` and defaults to `closed` when omitted. Multiple bindings for the same hook ID are allowed and execute in `hooks.bindings` array order. Each successful patch/decision is validated/applied before the next Hook sees the updated safe snapshot. A `closed` failure stops the intercepted operation; `continue` records the failure and proceeds to the next binding/original operation.

SDK-hosted Hook registrations may be added per Session/Run without editing config. They execute after workspace-configured Hook bindings in host registration order and remain subject to the same constrained Hook contracts.

Approval callbacks are **not Hooks**. They are handled by ApprovalRuntime/control as described below.

### Knowledge runtime configuration

`knowledge.runtimes` is an ID-keyed registry of custom full KnowledgeRuntime implementations:

```json
{
  "knowledge": {
    "runtimes": {
      "pinecone-prod": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/pinecone_knowledge.py"],
          "env": ["PINECONE_API_KEY"]
        }
      }
    }
  }
}
```

`knowledge.packages` maps a **versionless AgentPM Knowledge package identity** to one configured runtime ID:

```json
{
  "knowledge": {
    "packages": {
      "@zack/agentpm-docs": {
        "runtime": "pinecone-prod"
      }
    }
  }
}
```

Rules:

- an entry must reference an ID defined in `knowledge.runtimes`;
- the exact installed version still comes from `agent.lock` and is supplied to/validated by the runtime;
- no package entry means the Harness attempts the AgentPM built-in/local Knowledge realization;
- an explicit package mapping never silently falls back to local retrieval if that runtime is unavailable or mismatched.

`knowledge.embedding_matches` is used only when AgentPM local vector retrieval needs an external query embedding. Each entry exactly matches one packaged embedding-space tuple and names a configured EmbeddingProvider:

```json
{
  "knowledge": {
    "embedding_matches": [
      {
        "match": {
          "provider": "bring-your-own",
          "model": "example-normalized-4d",
          "dimensions": 4,
          "normalized": true
        },
        "embedding_provider": "corp-embedder"
      }
    ]
  }
}
```

Every match requires `provider`, `model`, positive `dimensions`, and boolean `normalized`. Duplicate tuples are invalid. `embedding_provider` must reference `providers.embeddings`. The Harness also validates live provider capabilities and the actual installed index metadata before declaring the package ready.

### Memory runtime configuration

`memory.runtimes` and `memory.packages` follow the same explicit registry/mapping pattern as Knowledge:

```json
{
  "memory": {
    "runtimes": {
      "postgres-prod": {
        "implementation": {
          "type": "process",
          "command": "python",
          "args": ["runtime/postgres_memory.py"],
          "env": ["DATABASE_URL"]
        }
      }
    },
    "packages": {
      "@zack/conversation-continuity": {
        "runtime": "postgres-prod"
      }
    }
  }
}
```

Package keys are versionless AgentPM identities; exact versions are resolved from `agent.lock`. No mapping means built-in local MemoryRuntime. An explicit external mapping never silently falls back to SQLite.

The built-in local SQLite runtime advertises `semantic` retrieval only when `memory.local.semantic` is configured and ready:

```json
{
  "memory": {
    "local": {
      "semantic": {
        "embedding_provider": "corp-embedder",
        "model": "corp-memory-v1",
        "dimensions": 768
      }
    }
  }
}
```

`embedding_provider` must reference `providers.embeddings`; `model` is the embedding model ID requested from that provider; `dimensions` is a positive integer and becomes part of the local vector-space identity. The built-in local runtime uses cosine similarity for Phase 7B. If this block is absent, local Memory does not advertise `semantic` even if an embedding provider exists elsewhere in config.

### External MCP import configuration

`mcp.imports` is an ID-keyed registry of external MCP servers that add runtime Tool capability. Every import must declare an explicit scope.

Stdio form:

```json
{
  "mcp": {
    "imports": {
      "github": {
        "transport": "stdio",
        "command": "github-mcp-server",
        "args": [],
        "cwd": ".",
        "env": ["GITHUB_TOKEN"],
        "scope": {
          "mode": "phases",
          "phases": ["assess", "execute"]
        },
        "tools": ["get_issue", "search_issues"],
        "startup_timeout_ms": 15000,
        "request_timeout_ms": 120000,
        "restart": {
          "max_attempts": 1,
          "backoff_ms": 250
        }
      }
    }
  }
}
```

HTTP form:

```json
{
  "mcp": {
    "imports": {
      "company-search": {
        "transport": "http",
        "url": "https://mcp.example.com/mcp",
        "headers": {
          "Authorization": {
            "env": "COMPANY_MCP_AUTHORIZATION"
          },
          "X-Workspace": {
            "value": "support"
          }
        },
        "scope": {
          "mode": "global"
        },
        "tools": ["search"]
      }
    }
  }
}
```

Rules:

- `transport` is exactly `stdio | http` in config version 1.
- Stdio imports require `command`; `args`, `cwd`, `env`, timeouts, and restart use the same semantics/defaults as process implementations.
- HTTP imports require an absolute `http://` or `https://` URL. `headers` is optional. Each header value is exactly one of `{ "value": "non-secret literal" }` or `{ "env": "ENV_VAR_NAME" }`; secret header values should use `env`.
- `scope.mode` is exactly `global | phases`.
- `global` forbids `phases`.
- `phases` requires a non-empty unique list of authored Loop phase IDs. Unknown phase IDs are diagnosed during Agent-aware preflight; if none remain valid, the import is unavailable.
- `tools` is an optional unique list of advertised MCP Tool names. Omission means all Tools advertised by that explicitly configured server are eligible inside its configured scope.
- Discovery never expands scope beyond this config.

### Agent-authored MCP export configuration

Agent `bindings.mcp` defines the logical exported surfaces. Runtime config only controls how Harness-managed exports are hosted:

```json
{
  "mcp": {
    "exports": {
      "enabled": true,
      "host": "127.0.0.1"
    }
  }
}
```

Defaults are `enabled: true` and `host: "127.0.0.1"`. Harness always requests ephemeral port `0` independently for each logical exported surface. Version 1 does not provide a static per-surface port map; users who require custom standalone MCP hosting can run `agentpm serve --mcp` directly. Configuring a non-loopback host is allowed only explicitly and must produce a prominent security warning.

Exported MCP surfaces are Session-owned, not Run-owned. They may service external MCP calls while no Run is active and may also remain callable while the Session's single Harness Run is active. Such outward calls do not become part of that Run merely because their timing overlaps it: they do not mutate RunState, consume phase/Run action or Tool-call limits, participate in Loop access/checkpoints, or invoke phase-scoped Hooks. Their events/correlation carry Session/surface/call identity; `run_id` is absent unless a future explicit contract intentionally associates an outward call with a Run.

### Approval configuration

ApprovalRuntime remains separate from HookRuntime.

An optional workspace process/host controller may be configured:

```json
{
  "approvals": {
    "controller": {
      "implementation": {
        "type": "process",
        "command": "python",
        "args": ["runtime/approval_controller.py"]
      }
    },
    "timeout_ms": 300000
  }
}
```

Rules:

- `timeout_ms` is optional and positive; it applies when waiting on a process/host controller, not while a human is actively using the Ratatui approval screen.
- an explicit SDK/application approval callback has highest per-run precedence;
- otherwise an explicitly configured `approvals.controller` is used;
- otherwise Ratatui uses its built-in interactive ApprovalRuntime;
- otherwise machine mode may use the connected host controller if registered;
- otherwise plain headless reaches runtime terminal status `approval_required` at the checkpoint; it never auto-approves or auto-denies.

### Trace configuration

```json
{
  "trace": {
    "enabled": true,
    "level": "normal",
    "content": "redacted"
  }
}
```

Defaults are exactly the values above. `level` is exactly `minimal | normal | verbose`. `content` is exactly `none | redacted | full`. `full` still never serializes values classified by Harness as secrets.

### UI branding configuration

```json
{
  "ui": {
    "branding": {
      "name": "Acme Agent Console",
      "subtitle": "Internal AI Platform",
      "accent": "#2563EB"
    }
  }
}
```

All branding fields are optional. Defaults are `name: "AgentPM Harness"`, `subtitle: null`, `accent: null`. `accent`, when present, is exactly a six-digit `#RRGGBB` value. Branding changes presentation only.

### Managed service lifecycle

Harness-owned long-lived process services (custom providers, process Hooks, process approval controller, stdio MCP imports, and managed MCP export subprocesses) use a common observable lifecycle:

```text
starting -> handshaking -> ready -> unhealthy -> restarting -> ready|failed -> stopped
```

Requirements:

- a service is not ready until its protocol handshake/capabilities are validated;
- unexpected process exit or transport failure marks it unhealthy and fails any in-flight request visibly;
- automatic restart never replays the failed request;
- when restart is configured/defaulted, restart is only for subsequent requests;
- exhausted restart attempts mark the service failed/unavailable and recompute affected readiness where relevant;
- all transitions emit service lifecycle events;
- Session shutdown/cancellation stops owned processes cleanly.

## Bootstrap and preflight

Bootstrap is separate from orchestration execution.

It must:

1. discover workspace root according to existing AgentPM workspace conventions;
2. load required `agent.lock`;
3. select/resolve the Agent root;
4. resolve the exact installed Agent, Loop, Tool, Skill, Knowledge, Memory, and Profile package versions;
5. read optional `agentpm.harness.json`;
6. validate runtime configuration structurally/semantically;
7. resolve model/provider and runtime scope values;
8. validate cross-package binding references deliberately deferred by Phase 7A lint;
9. validate installed Knowledge and Memory build/index metadata required for runtime use;
10. establish Model/Embedding/Hook/Knowledge/Memory/MCP/Approval service implementations;
11. probe/handshake external services and determine capability readiness;
12. inspect Tool runtime/environment readiness where possible;
13. resolve Agent-authored outward MCP surfaces and runtime-configured inward MCP surfaces;
14. produce a structured preflight report used by TUI, headless output, machine clients, and the eventual run report.

### Preflight severity

Fatal errors include cases where execution cannot be coherently interpreted, including:

- missing/invalid lockfile;
- no selected runnable Agent;
- Agent has no Loop;
- unresolved/missing Loop;
- invalid entry phase or transition target/outcome;
- ambiguous duplicate transitions;
- missing provider/model in non-interactive mode;
- malformed required runtime configuration/protocol mismatch.

Warnings/safe degradation include:

- Agent phase binding references a phase that does not exist: warn, ignore that binding, offer did-you-mean where practical;
- bound capability is prohibited by Loop access: report suppression;
- optional consumer-context file missing/unreadable: warn, continue;
- Profile compatibility mismatch: strong warning, not enforcement;
- Tool missing required environment at preflight: warn; actual invocation remains authoritative;
- Knowledge or Memory surface cannot be realized: mark surface unavailable/suppress, continue unless active execution requires it;
- outward MCP surface cannot start: mark surface unavailable, continue;
- configured external MCP Tool subset contains unavailable Tools: expose ready subset and diagnose omissions; empty surface becomes unavailable.

Explicitly configured custom runtime failure must not silently fall back to a different runtime.

## Session, RunContext, and RunState

### Session

A Harness Session owns long-lived service lifecycle:

- ModelRuntime;
- HookRuntime processes/host connection;
- KnowledgeRuntime providers;
- MemoryRuntime;
- imported external MCP connections/processes;
- exported MCP server processes;
- ApprovalRuntime transport;
- event/trace sinks.

A TUI or machine/SDK Session may execute multiple Runs over its lifetime. A one-shot headless invocation has one Session and one Run.

#### Single-active-Run invariant

Phase 7B permits **at most one active authoritative Run per Harness Session**. Reusable Sessions execute Runs sequentially:

```text
HarnessSession
    |
    +-- Run 1: start --------------------------> terminal
    |
    +-- Run 2: start --------------------------> terminal
    |
    +-- Run 3: start --------------------------> terminal
```

The HarnessEngine must not execute two RunState machines concurrently inside the same Session, and Harness must not implicitly queue a second Run. A machine/SDK `start_run` request received while a Run is active is rejected with a stable structured busy/active-Run error and must not mutate the active Run. The TUI likewise prevents the message composer from starting another Run until the current Run is no longer active. Consumers that require concurrent Agent executions create multiple Harness Sessions. Concurrent Runs within one Session are outside Phase 7B.

This invariant is about **authoritative Harness Runs**, not about forbidding asynchronous I/O or internal concurrency inside an individual runtime implementation. A Runtime Service may use safe internal concurrency, and Session-owned outward MCP surfaces may continue serving independent external calls, without creating a second Harness RunState machine.

A Run that is waiting at an approval checkpoint remains the Session's active Run when the Session has an approval/control path capable of resolving that checkpoint. The Session cannot start another Run while that approval is pending. In plain one-shot headless mode with no approval controller, the existing `approval_required` behavior is terminal for that invocation: the Run report/trace is finalized and the Session shuts down rather than remaining open waiting for approval.

Session owns cumulative usage/accounting across its Runs. `SessionUsage` should aggregate, when available:

- completed/started Run count;
- ModelRuntime calls;
- input/output/total tokens;
- accepted semantic actions;
- logical Tool calls plus Tool retry attempts as separate counters;
- Knowledge/Memory/embedding request counts where useful;
- optional provider-reported or explicitly calculable cost; Harness must not hardcode stale model pricing merely to fabricate cost;
- Session elapsed duration.

Each Run keeps its own usage summary in RunState/RunReport and contributes to SessionUsage exactly once. Providers that do not report token usage leave those values unknown rather than guessed. Session usage is observable through events/machine/TUI and a terminal `session_summary`/shutdown result; Phase 7B does not require a separate durable Session report.

### RunContext

RunContext contains fixed/snapshotted execution inputs such as:

- run/session IDs;
- workspace/state roots;
- resolved Agent/Loop/package graph;
- resolved runtime config and source metadata;
- runtime scope values;
- consumer-context snapshot;
- service handles;
- hook registrations;
- run input.

### RunState

Only HarnessEngine mutates RunState. It contains:

- current phase/execution ID;
- step count;
- phase history and PhaseResults;
- current phase-local transcript/context state;
- pending approval/control state;
- Tool/action counters and repair counters;
- token/usage accounting;
- terminal/runtime status.

External Memory data and persistent Memory trigger state are not RunState; MemoryRuntime is authoritative for those resources.

## Agent runtime semantics

The Agent is the Harness runnable composition boundary. Harness runtime interpretation must preserve these Agent-specific rules explicitly:

- an Agent without `loop` remains a valid publishable/installable Agent artifact, but it is **not runnable by the built-in Harness** in Phase 7B; Harness must report that distinction rather than treating the package itself as invalid or inventing a Loop;
- top-level `tools`, `skills`, `knowledge`, `memory`, and `profiles` declare the dependency graph only; they do not make those artifacts model-available by themselves;
- Agent bindings establish authored runtime availability. If an artifact has no applicable global/phase binding, it is not surfaced merely because it is installed or listed as a dependency;
- binding package references remain versionless identities while exact runtime versions come from `agent.lock`;
- Agent `examples` and README/license metadata are discovery/documentation inputs only and never executable/model behavior; a TUI may surface examples as optional launch suggestions without injecting them into a Run;
- consumer context, outward MCP bindings, and phase/global bindings retain the specialized semantics defined later in this spec rather than becoming generic dependency activation.

## EffectivePhase

EffectivePhase is recomputed on each phase entry/re-entry.

Candidate authored composition is:

- global bindings;
- phase bindings;
- Skill-inherited Tools in the Skill's binding scope;
- distinct active Profiles/Skills/Knowledge/Memory selectors.

Runtime augmentation may add external MCP Tools only where the external server configuration explicitly scopes them.

Loop access and runtime readiness are computed independently and both suppression reasons are retained for traceability.

Effective phase computation must never mutate portable metadata.

### Ordering and deduplication

- Profiles: global authored order, then phase authored order; dedupe by package identity; no merge/override semantics.
- Skills: global authored order, then phase authored order; dedupe by package identity.
- Tools: combine direct bindings, Skill inheritance, and imported MCP candidates using canonical identities; de-dupe same AgentPM Tool identity.
- A directly bound Tool duplicated by Skill inheritance in the same scope should warn and de-dupe rather than fail startup.
- Knowledge packages remain distinct surfaces; do not silently federate/merge them.
- Memory selectors for the same Blueprint are unioned by spaces/operations while preserving global-versus-phase participation semantics.

## Loop execution model

Loop phase IDs are graph/runtime identities only; Harness must not attach built-in behavior to names such as `plan`, `execute`, or `review`. `phase.objective` is model-facing orchestration guidance. `loop.archetype` remains descriptive.

Loop access is tri-state: `false` prohibits the corresponding otherwise-available capability, `true` permits it without creating it, and omission expresses no Loop opinion.

For Loop Tool retry policy, `max_retries` means **additional attempts after the initial failed invocation**. For example, `max_retries: 2` permits at most three total attempts for that Tool call. Retries do not permit argument mutation; a model proposal with different arguments is a new Tool call.

### Phase steps

One execution of one phase equals one Loop step.

A phase execution may contain multiple model calls and runtime actions before producing a PhaseResult.

Before entering the next target phase:

1. evaluate any approval checkpoints targeting it;
2. if all approve, enter the phase and consume one step;
3. if a checkpoint rejects, stop evaluating remaining checkpoints, follow that checkpoint's `on_reject`, and do not consume the guarded phase step;
4. if approval cannot be resolved due to transport/timeout/runtime failure, treat it as runtime/control failure, not authored rejection.

### Multiple checkpoints per phase

Phase 7B intentionally supersedes the earlier Phase 7A semantic restriction that allowed only one checkpoint per `before_phase`.

Multiple approval checkpoints may target the same phase and are evaluated in authored `checkpoints` array order. All must approve. First rejection wins and follows its own `on_reject`.

Update Loop semantic lint/tests/docs accordingly; JSON shape does not need to change.

### PhaseResult

Every completed phase produces a first-class PhaseResult containing at least:

- phase ID;
- unique phase execution ID;
- Loop step number;
- selected outcome;
- output/content;
- usage summary;
- structured metadata needed by the engine/report.

Outcome and content are distinct.

If a phase omits explicit outcomes, Harness may assign implicit `complete` without requiring the model to choose it.

If explicit outcomes exist, model/host selection must match one declared ID exactly. Invalid outcome proposals receive structured bounded repair; exhausted repair becomes phase failure.

### Transcripts and cross-phase context

Raw provider transcripts are phase-local.

A new phase execution starts with fresh assembled context from:

- Harness control/protocol instructions;
- phase objective and valid outcome contract;
- active Profiles as distinct authored inputs;
- active Skills/descriptors and loaded Skill resources;
- run input;
- consumer-context snapshot;
- relevant prior PhaseResults;
- effective capability descriptors.

Tool/Knowledge/Memory results enter the current phase context when requested and do not automatically become global transcript history.

Re-entering a phase creates a new phase execution and context.

### Terminal states

Authored Loop terminal states:

- `$end` -> `ended` (successful authored completion; default final output is last PhaseResult content);
- `$abort` -> `aborted` (intentional authored unsuccessful termination);
- `$handoff` -> `handed_off` (returns explicit handoff result/context to caller; does not invoke another Agent).

Runtime terminal states include at least:

- `failed`;
- `cancelled`;
- `limit_reached`;
- `approval_required` for plain headless execution with no approval controller.

Runtime max-step exhaustion is `limit_reached`, not authored `$abort`.

## Semantic model actions

Provider-native function/tool calling must be normalized into Harness semantic actions rather than treated uniformly as Tool calls.

Required action categories:

- AgentPM Tool call;
- imported external MCP Tool call;
- Skill resource read;
- Knowledge request;
- Memory read;
- Memory write;
- phase completion/outcome proposal.

Loop `access.tools` controls AgentPM and imported MCP Tool calls only.

Loop `access.knowledge` controls Knowledge requests.

Loop `access.memory.read/write` controls direct model-facing Memory access only.

Skill resource access and Profile instructions are not governed by `access.tools`.

## ModelRuntime

ModelRuntime is the sole boundary between Harness semantic execution and a model provider API. Harness never parses OpenAI/Anthropic/Ollama responses directly in the Engine. The Engine creates a normalized `ModelRequest`; ModelRuntime translates it to the provider; the provider response is translated back into one normalized `ModelTurn`.

Required built-in providers:

- OpenAI;
- Anthropic;
- Ollama/local HTTP runtime.

### Normalized ModelRequest / ModelTurn

A normalized ModelRequest contains conceptually:

- selected provider/model and provider-specific `model.options`;
- canonical prompt/context sections assembled by Harness;
- the current phase-local transcript;
- model-visible semantic action descriptors currently authorized by EffectivePhase;
- provider-safe alias mapping for those actions where needed;
- optional structured completion/outcome schema/control descriptor;
- request correlation/run/phase identifiers for tracing, not as model authority.

A normalized ModelTurn contains conceptually:

```text
assistant_content: optional text
semantic_actions: ordered list of normalized action proposals
usage: optional provider usage
finish_reason/provider metadata: diagnostic only
```

Provider-native function/tool calls are mapped back through the alias table into the semantic actions defined by Harness. A model never calls Rust services directly; it only proposes an action in its response. Harness validates the proposal against EffectivePhase and the action's typed schema, executes the corresponding runtime, appends the structured result to the phase-local transcript, and then decides whether another ModelRuntime call is needed.

Multiple non-terminal semantic actions returned in one provider turn are processed in provider-preserved order in Phase 7B for deterministic behavior. Parallel action execution is deferred. A PhaseCompletion proposal must not compete with executable actions in the same normalized turn; if a provider response ambiguously proposes both, Harness requests bounded structured repair rather than guessing ordering.

If a phase has no explicit outcomes and the model returns final assistant content with no semantic actions, Harness may treat that content as PhaseResult output and assign implicit `complete`. If explicit outcomes exist, the turn must yield a valid PhaseCompletion/outcome proposal; missing/invalid completion is repaired according to `max_structured_output_repairs`.

### Model capability advertisement

For the **selected model**, ModelRuntime reports live capabilities equivalent to:

```json
{
  "semantic_actions": true,
  "structured_output": true,
  "multimodal_input": false,
  "context_window_tokens": 128000,
  "usage_reporting": true
}
```

`context_window_tokens` is optional because AgentPM deliberately does not maintain a closed global model-ID catalog. Built-in/custom providers should report it when they can determine it reliably for the selected model; otherwise compatibility is `unknown`, not guessed. `semantic_actions` means the provider/model can faithfully support the Harness's function/action interaction needed by the current phase, whether through native tool calling or a provider adapter that can produce equivalent validated structured actions. Unsupported required capabilities affect runtime readiness and Profile compatibility diagnostics, not portable artifact metadata.

Requirements:

- concrete model IDs remain open strings;
- provider capability differences are adapted at runtime and surfaced diagnostically;
- provider-safe temporary function/tool names may be used, but Harness events/hooks always use canonical AgentPM/MCP identities;
- secrets are resolved from scoped runtime environment/config and never logged;
- response normalization must preserve structured usage when available;
- unsupported required provider capabilities produce readiness diagnostics rather than artifact mutation.

Ollama is the required local/open path so a user can run the Harness without a paid hosted model provider when a suitable local model is installed.

## Prompt assembly and model-visible structure

Harness should have one canonical logical prompt/request structure even though different providers map those sections to system/developer/user messages or other API fields differently. Provider adaptation must not change the semantic ordering/authority of the sections.

Conceptually each phase ModelRequest is assembled as:

```text
1. HARNESS CONTROL (immutable authority)
   - Harness semantic-action protocol/instructions
   - current phase identity
   - valid completion/outcome contract
   - authority rule: model proposes; Harness validates/executes

2. AUTHORED PHASE + BEHAVIOR
   - Loop phase objective
   - active Profiles, each preserved as a distinct labeled block
   - active Skill descriptors/instructions/resources currently loaded,
     each preserved as a distinct Skill block

3. CONSUMER / RUN CONTEXT
   - Run input/user request
   - Consumer Context snapshot

4. CROSS-PHASE STATE
   - relevant prior PhaseResults in deterministic chronological order
   - no raw prior provider transcript unless it was intentionally captured
     into a PhaseResult/other authorized state

5. EFFECTIVE CAPABILITY CATALOG
   - AgentPM Tool descriptors
   - imported MCP Tool descriptors
   - Skill resource-read descriptors
   - Knowledge package/document/search descriptors
   - Memory read/write descriptors
   - PhaseCompletion descriptor where needed

6. CURRENT PHASE-LOCAL TRANSCRIPT
   - assistant turns already produced in this phase
   - structured Tool/MCP/Knowledge/Memory/Skill-resource results returned
     during this phase
```

The capability catalog may be carried through provider-native tool/function definitions rather than literal prompt text. Knowledge/Memory contents and Tool results are not inserted until retrieved/executed. Consumer Context is eager because that is its contract. Profiles/Skills are instructional; retrieved/generated results remain lower-trust data even if a provider serializes everything into one message stream.

`before_model_request` Hook executes after canonical assembly and before provider-specific translation. It may shape the mutable model-facing context/provider options allowed by its typed contract, but it cannot remove/replace Harness control authority, change canonical phase/outcome IDs, add action descriptors, or change EffectivePhase.

Prompt/context budgeting should preserve this section structure and produce diagnostics rather than silently dropping authoritative Harness control or authored required Profile/phase guidance. Phase 7B may use straightforward budgeting/truncation policies for lower-priority data, but any truncation/summarization must be observable in verbose trace/events.

## Agentic inner loop

Each phase execution is an agentic inner loop, not one model call. The canonical control flow is:

```text
enter phase / compute EffectivePhase
        |
        v
assemble fresh phase ModelRequest baseline
        |
        v
before_model_request Hooks
        |
        v
ModelRuntime.generate
        |
        v
normalized ModelTurn
        |
        +-- no action + implicit-complete phase --> PhaseResult
        |
        +-- PhaseCompletion ---------------------> validate/repair -> PhaseResult
        |
        +-- Tool/MCP/Skill/Knowledge/Memory action
                |
                v
          validate authorization + schema
                |
          action-specific Hooks
                |
          execute typed runtime/provider
                |
          append structured result to phase transcript
                |
          evaluate automatic Memory operation eligibility after
          relevant Memory state changes
                |
                +-------------------------------> ModelRuntime again
```

Rules:

- one pass through the Loop graph phase is one Loop step regardless of inner-loop model/action count;
- every ModelRuntime call increments model-call safety/accounting;
- every accepted semantic action increments `max_actions_per_phase`; logical AgentPM/imported-MCP Tool actions also increment `max_tool_calls_per_phase`;
- action execution results are data returned to the current phase model, not new Harness authority;
- malformed/unauthorized action proposals are returned as structured repair/error feedback when repair is possible and never executed speculatively;
- the Engine continues the inner loop until valid PhaseCompletion/implicit completion, phase failure, cancellation, approval/runtime terminal, or a safety limit is reached;
- phase-local raw transcripts are discarded from automatic cross-phase context after PhaseResult creation, though they may remain in trace/report according to content policy.

## Profiles

Profiles are resolved model-facing behavioral inputs; there is no ProfileRuntime.

All authored behavioral sections present in the resolved Profile—including identity, objectives/principles, audience, communication/formatting/vocabulary, boundaries, and constraints—remain model-facing inputs. Harness must not silently drop a Profile section merely because it does not have a special runtime implementation.

- top-level Profile dependency alone does not activate behavior;
- global/phase bindings determine active Profiles;
- global + phase are additive and deduplicated;
- multiple Profiles remain distinct inputs, not merged into a synthetic Profile;
- ordering is deterministic serialization, not precedence/override;
- required/preferred constraints influence prompt wording only; Harness does not fake post-response enforcement;
- compatibility is advisory and may produce strong warnings; `requires` mismatches should be stronger than `recommends` hints, but neither grants/suppresses authority by itself;
- in Profile capability hints, boolean `true` means the author is asserting/recommending that capability; boolean `false` means no positive requirement/recommendation for that capability and must **not** be interpreted as a prohibition (for example, `requires.tool_use: false` does not mean Tool use is forbidden);
- `compatibility.minimum_context_tokens` is an advisory minimum **model context-window capacity**, not a request to reserve that many tokens and not a limit on the assembled prompt. When the selected ModelRuntime reliably advertises `context_window_tokens`, a value below the Profile minimum produces a strong compatibility warning; when the provider cannot report the context window reliably, Harness reports compatibility as unverified/unknown rather than guessing or maintaining a closed model catalog. The hint never blocks the Run, suppresses the Profile, or changes capability authority. With multiple Profiles, evaluate each Profile independently and optionally summarize the highest requested minimum in preflight;
- stable Profile constraint IDs should be preserved in prompt assembly/events where practical so traces can identify which authored constraint influenced a request;
- obvious mechanical conflicts between active Profiles may be warned about, but Harness must not invent precedence or semantic conflict resolution, including for competing identity/persona guidance;
- boundaries are authored model guidance and never replace Harness authority.

## Skills

There is no SkillRuntime.

A bound Skill contributes:

- compact manifest/description/resource inventory for initial discovery;
- progressive access to its entrypoint/reference resources as needed;
- inherited Tool dependencies in the Skill's binding scope; those inherited Tools do not require a duplicate direct Tool binding on the Agent.

Binding a Skill does not eagerly inject all Skill files. Multiple active Skills remain distinct authored procedural inputs in deterministic order; Harness does not merge them into one synthetic Skill or apply override precedence. Skill resources are scoped to the Skill/phase in which the Skill is active and do not leak into later phase contexts merely because they were loaded previously.

Package-owned Skill paths resolve relative to the resolved Skill package root and must remain inside that root after canonicalization/symlink resolution. Skill resource loading may reuse a low-level packaged-file reader also used by other artifact implementations, but a Skill reference remains a Skill semantic action rather than Knowledge retrieval.

Skill scripts never auto-execute. Merely declaring a script grants no process-execution authority and Harness must not infer an interpreter/executor from `.sh`, `.py`, or other extensions. A script can execute only through independently authorized capability such as an AgentPM shell-executor Tool.

Skill activation itself is mechanical composition and needs an event, not a mutable `before_skill_activation` Hook. Hooks may shape later prompt/action decisions but cannot rewrite which Skill binding is active.

Skill `compatibility` metadata is advisory like Profile compatibility. Harness may emit readiness/quality warnings when the active runtime/model clearly does not match a declared compatibility hint, but compatibility does not grant/suppress authority or become hard execution policy by itself.

## ToolRuntime and `agentpm run`

Harness direct AgentPM Tool calls must execute through the public command boundary:

```text
Harness ToolRuntime -> agentpm run --machine -> Tool
```

Use JSON stdin for Tool arguments.

Phase 7B must strengthen `agentpm run` so the Harness and third parties can rely on it:

- add a stable machine-readable success/failure envelope and error category;
- validate Tool input JSON against the declared input schema;
- validate successful Tool output JSON against the declared output schema;
- enforce declared runtime minimum version rather than only checking executable presence;
- preserve timeout/process-group cleanup and harden cancellation so killing the outer run cannot orphan nested Tool processes;
- retain existing Tool environment/default semantics while avoiding indiscriminate secret projection.

Harness performs early input-schema validation before ToolRuntime so malformed model arguments can be repaired without counting as a Tool failure. Hook-modified arguments are validated again.

Harness must not infer read/write/destructive safety semantics from arbitrary Tool action names or schema fields. Tool availability/approval comes from Agent/Loop composition, runtime readiness, and Hooks rather than heuristics such as treating an `action: delete` field specially.

Tool package entrypoint, package files, `cwd`, timeout, environment-default expansion, interpreter selection, and concrete runtime launching remain responsibilities of the public Tool runner/`agentpm run` contract. Harness may inspect them for readiness but must not implement a second private Tool executor.

Failure before ToolRuntime invocation is a model/action repair concern. Failure while ToolRuntime attempts the invocation is a Loop Tool failure.

A schema-valid Tool output such as `{ "ok": false }` is still a successful Tool invocation unless the generic Tool contract itself defines it as invalid; Harness must not infer domain semantics from arbitrary output fields.

Known runtime-incompatible Tools are suppressed from the model with a diagnostic. Missing required environment variables produce strong preflight warnings but remain authoritative at invocation because runtime environment may change/be supplied.

Retries are fresh `agentpm run` invocations with the same finalized arguments. A model choosing different arguments is a new Tool call.

## KnowledgeRuntime

Knowledge bindings make packages available on demand; they never imply automatic retrieval.

### Context mode

Expose a compact package/document inventory initially. The model requests one declared document when needed. Document bodies are not eagerly injected. Declared document `role` values are discovery/reasoning hints only; they do not create hidden eager-load behavior or authority. Requests may address only documents declared by that Knowledge package.

Package paths resolve from the resolved Knowledge package root and must remain inside it.

### Vector mode

KnowledgeRuntime accepts a text query and returns normalized structured results with chunk/source/citation metadata. Packaged retrieval metadata such as strategy, `top_k`, score threshold, and citation preference are retrieval defaults/hints, not Loop/orchestration semantics; an authorized request/Hook may narrow or shape them within the package/runtime contract. `return_citations` controls retrieval-result provenance/citation data and does not force the final model response to contain citations.

Default resolution:

1. if an explicit custom KnowledgeRuntime mapping exists for the package, use it and do not silently fall back;
2. otherwise use AgentPM local KnowledgeRuntime;
3. if local retrieval can satisfy the query through existing public `agentpm knowledge query` behavior, use it;
4. if local retrieval only lacks a compatible query embedding, resolve a compatible EmbeddingProvider, obtain the vector, then use AgentPM local retrieval;
5. if no implementation can realize the package, mark the Knowledge surface unavailable and suppress it.

Embedding provider metadata identifies vector-space compatibility; it does not require AgentPM to hardcode every provider.

EmbeddingProvider is a typed capability, not a hook. It may be implemented by a persistent process or SDK host callback.

### External Knowledge providers

Phase 7B must ship reference implementations for:

- Pinecone full KnowledgeRuntime;
- pgvector full KnowledgeRuntime.

These providers must return normalized AgentPM Knowledge results and validate that the external index represents the expected package/version/corpus identity where the available build metadata supports that check.

Provider configuration maps packages explicitly. A custom provider may own embedding, retrieval, filtering, and reranking internally.

Pinecone/pgvector provisioning and corpus upload are outside runtime execution.

### Knowledge failure behavior

- malformed model request -> bounded structured repair;
- known unavailable package -> not exposed to model;
- unexpected backend failure on a valid request -> structured Knowledge access failure returned to the phase; phase may continue;
- repeated inability to complete eventually becomes phase failure, not Tool failure.

## MemoryRuntime and Memory Blueprints

Memory packages remain **Blueprints**, not stores.

Generated Memory contracts are the runtime durable record contracts. Author source schemas are build-time inputs. Generated build/index/contract paths are package-owned paths and resolve from the exact installed Memory package root; live records/state never do.

### Runtime-owned record envelope

The model proposes `content` only. Harness/MemoryRuntime owns:

- record ID;
- record type/space identity;
- scope values;
- schema version;
- created/updated timestamps;
- expiration;
- sequence ordinal;
- provenance.

Model-proposed content is validated against the content schema before persistence; the completed envelope is validated against the generated contract.

### Scope resolution

Blueprint scope keys are arbitrary authored identifiers. `user` and `conversation` are examples, not special literals.

Authoritative scope values come only from trusted runtime context (SDK/CLI options, config, trusted resolver/application). The model may mention/propose a possible scope value but may never directly select the Memory partition used for persistence/retrieval.

A direct space or operation requiring unresolved scope keys is unavailable and diagnosed.

### Direct space semantics

Per complete resolved scope tuple:

- `document`: one current logical document per space/record type; direct write is create-or-replace/update;
- `collection`: multiple identified records; direct create/read/update/delete where constraints permit;
- `sequence`: ordered records; direct creation appends and runtime assigns ordinal; mutation/deletion only where constraints permit.

`append_only: true` forbids ordinary direct update/delete but does not prohibit an explicitly authored lifecycle operation from applying its declared source handling/output semantics.

Loop `memory.read/write` constrains direct model access only. It does not prohibit participating lifecycle operations from reading/writing their declared internal spaces.

### Runtime capability advertisement

Every MemoryRuntime exposes a normalized capability descriptor equivalent to:

```json
{
  "space_models": ["document", "collection", "sequence"],
  "retrieval_modes": ["key", "filter", "chronological", "full_text", "semantic"],
  "retention_actions": ["delete", "archive"],
  "constraints": ["append_only"],
  "capacity": true,
  "durable_trigger_state": true,
  "atomic_batches": true
}
```

The arrays contain only capabilities the runtime can actually realize in its **current resolved configuration**. For example, the built-in SQLite runtime omits `semantic` unless `memory.local.semantic` resolves a ready EmbeddingProvider.

`capacity` means the runtime can atomically enforce scoped hard record limits. `durable_trigger_state` means operation scheduling state survives Harness process restarts. `atomic_batches` means the runtime can commit/rollback the related multi-record/source/trigger-state mutations required by lifecycle operations as one semantic batch.

Harness compares this live descriptor with every Blueprint space and participating operation during preflight. A direct space is exposed only when all of its declared model/retrieval/retention/constraint/capacity requirements can be faithfully realized. A lifecycle operation is ready only when all referenced spaces are realizable for internal access and the backend provides durable trigger state plus atomic batches for operations that mutate multiple records/state entries.

### Local SQLite MemoryRuntime

Default mutable path:

```text
.agentpm-state/memory.sqlite3
```

Use the following version-1 logical schema. SQL spelling/index names may follow the existing Rust migration conventions, but the columns, keys, uniqueness semantics, and state separation below are required.

#### `memory_meta`

- `key TEXT PRIMARY KEY`
- `value TEXT NOT NULL`

On creation/migration, store `key = 'schema_version'`, `value = '1'`. Future local-store schema changes must use explicit migrations; opening a newer unsupported schema version fails clearly rather than attempting best-effort interpretation.

#### Scope identity

For every persisted scoped row, `scope_json` is the complete scope tuple required by that space/operation, serialized as a JSON object with keys sorted lexicographically and no insignificant whitespace. Scope values are strings. `scope_hash` is:

```text
sha256:<lowercase-hex SHA-256 of UTF-8 scope_json>
```

The hash is an index key only; `scope_json` remains stored for diagnostics/collision verification. Harness never accepts a model-provided `scope_hash` or `scope_json` as authority.

For operation state spanning multiple spaces, the operation scope tuple is the union of scope keys required by its declared inputs/output/targets, resolved from the same trusted RunContext.

#### `memory_records`

- `package TEXT NOT NULL`
- `package_version TEXT NOT NULL`
- `space TEXT NOT NULL`
- `space_model TEXT NOT NULL` (`document | collection | sequence`)
- `record_type TEXT NOT NULL`
- `scope_hash TEXT NOT NULL`
- `scope_json TEXT NOT NULL`
- `id TEXT NOT NULL`
- `schema_version TEXT NOT NULL`
- `ordinal INTEGER NULL`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NULL`
- `expires_at TEXT NULL`
- `archived_at TEXT NULL`
- `provenance_json TEXT NULL`
- `content_json TEXT NOT NULL`
- primary key: `(package, package_version, space, scope_hash, id)`

Timestamps are UTC RFC 3339 strings. `content_json`, `scope_json`, and `provenance_json` use deterministic compact JSON serialization for stable hashing/debugging.

Required indexes/constraints:

- index `(package, package_version, space, scope_hash, record_type, archived_at)` for active scoped lookup;
- index `(package, package_version, space, scope_hash, ordinal)` for sequence reads;
- index `expires_at` for retention cleanup;
- partial unique index `(package, package_version, space, scope_hash, record_type)` where `space_model = 'document' AND archived_at IS NULL`, enforcing one active document per scope/record type;
- sequence records require non-null `ordinal`; non-sequence records require null `ordinal`, enforced in runtime validation if a portable SQLite CHECK would make migrations awkward.

Archived records remain in `memory_records` with `archived_at` set and are excluded from normal active reads/counts/retrieval. Delete semantics physically remove the record and any local vector row.

#### `memory_sequence_state`

- `package TEXT NOT NULL`
- `package_version TEXT NOT NULL`
- `space TEXT NOT NULL`
- `scope_hash TEXT NOT NULL`
- `scope_json TEXT NOT NULL`
- `next_ordinal INTEGER NOT NULL`
- `updated_at TEXT NOT NULL`
- primary key: `(package, package_version, space, scope_hash)`

Sequence ordinal allocation starts at `0`. Appending reserves the current `next_ordinal` and increments it in the same SQLite transaction as record insertion. Ordinals are never reused after deletion/archive.

#### `memory_operation_state`

- `package TEXT NOT NULL`
- `package_version TEXT NOT NULL`
- `operation TEXT NOT NULL`
- `scope_hash TEXT NOT NULL`
- `scope_json TEXT NOT NULL`
- `trigger_type TEXT NOT NULL`
- `armed INTEGER NOT NULL` (`0 | 1`)
- `baseline_at TEXT NULL`
- `last_completed_at TEXT NULL`
- `next_eligible_at TEXT NULL`
- `last_observed_value INTEGER NULL`
- `watermark_json TEXT NULL`
- `updated_at TEXT NOT NULL`
- primary key: `(package, package_version, operation, scope_hash)`

`watermark_json` is reserved for trigger-specific durable state that cannot be represented by the scalar columns. Harness owns its typed contents; models/hooks never write it directly.

#### `memory_vectors`

Used only when the local runtime has a ready `memory.local.semantic` configuration:

- `package TEXT NOT NULL`
- `package_version TEXT NOT NULL`
- `space TEXT NOT NULL`
- `scope_hash TEXT NOT NULL`
- `record_id TEXT NOT NULL`
- `embedding_provider TEXT NOT NULL`
- `embedding_model TEXT NOT NULL`
- `dimensions INTEGER NOT NULL`
- `content_hash TEXT NOT NULL`
- `vector BLOB NOT NULL`
- `updated_at TEXT NOT NULL`
- primary key: `(package, package_version, space, scope_hash, record_id, embedding_provider, embedding_model)`

`vector` stores contiguous little-endian IEEE-754 `f32` values, matching the established AgentPM local-vector representation. `content_hash` is SHA-256 of the exact canonical model-visible record content used to create the embedding. A mismatched hash makes the vector stale and it must be regenerated before semantic retrieval returns that record.

The local implementation uses exact cosine search in Phase 7B. It must not require a hosted vector database. It also must **not require a SQLite vector extension** such as sqlite-vec for correctness: vectors are stored as the `BLOB` representation above, loaded by the Rust MemoryRuntime, and exact cosine similarity is computed in Rust. A SQLite vector extension may be explored later as an optional optimization, but Phase 7B tests/behavior cannot depend on it being installed.

All SQLite writes that combine record mutation, sequence allocation, vector invalidation/update, and/or lifecycle trigger state must execute in one transaction where they are part of the same semantic operation. SQLite foreign-key/cascade usage is allowed, but transaction semantics remain authoritative.

### Retention and capacity

TTL anchor:

```text
expires_at = (updated_at if present else created_at) + ttl
```

Local runtime may enforce expiry lazily on startup/read/write/trigger evaluation; expired records must not participate as active Memory after expiry.

Capacity applies per complete resolved scope tuple.

### Lifecycle operations

Participating operations are determined by Agent bindings:

- global operation binding participates throughout the Run;
- phase-bound operation participates only while that phase execution is active;
- operation may internally access its declared input/output/target spaces even when those spaces are not directly bound to the phase.

Operation semantics:

- `delete`: mechanical deletion/archival behavior; no model generation required;
- `transform`: model-assisted structured transformation of each active scoped record matching its single input pairing, producing one output per source;
- `consolidate`: model-assisted structured synthesis over active scoped records matching its declared inputs, producing one destination record.

Model-assisted operation calls receive operation description, authorized source content, target content schema, and Harness lifecycle instructions. They return target `content` only. Output is validated and repair-bounded before persistence. These internal lifecycle model calls are first-class usage/trace events and count toward provider token/usage totals and Harness model-call safety accounting, but they are not phase model turns and do not consume Loop steps.

### Memory schema amendment: transform output mode

Add optional `output_mode` to transform operations:

```text
create | replace_input
```

Omitted value defaults to `create` for backward compatibility.

`replace_input` means the transformed output updates/replaces the originating source record and is valid only when output space/record type matches the single input space/record type **and** `source_handling` is `retain`. It is explicit lifecycle authority even if the space is append-only for direct writes. `delete_after_success` or `retain_until_expiration` with `replace_input` is invalid because the transformed record is the retained source identity itself.

Update the flagship `refresh_saved_note` example to declare `output_mode: "replace_input"`.

### Trigger semantics

Trigger state is persistent MemoryRuntime state.

- `external`: never automatic; invoked only through the canonical Harness external-operation invocation path.
- `record_count`: edge-trigger when active scoped count moves from below threshold to threshold-or-higher; disarm after firing and re-arm once count falls below threshold.
- `capacity`: edge-trigger when active scoped count reaches capacity; re-arm once count falls below capacity. A write that would exceed a hard capacity may first run an eligible participating capacity operation; if capacity is not freed, reject the write.
- `interval`: dormant until relevant scoped input/target state first exists. First baseline starts when that state first exists. After successful execution, next eligibility is successful completion time plus `every`. If no relevant state exists, remain dormant.

Automatic trigger eligibility is evaluated at relevant state changes, including mid-phase immediately after Memory writes.

### External operation invocation

TUI, SDK, and parent applications must all route through one canonical Engine request equivalent to:

```text
invoke_memory_operation(package, operation, current_resolved_scope)
```

Harness validates that the operation exists, is bound/participating in the current scope, has `trigger.type = external`, has resolved scopes, and has a ready backend before execution.

The phase model does not automatically receive authority to invoke external Memory operations.

### Governance

#### Durable Memory Projection and `x-agentpm-persist`

`x-agentpm-persist: false` marks a schema-governed Memory value as ephemeral with respect to durable Memory persistence.

For every direct or lifecycle-created Memory record, Harness applies the following canonical sequence:

1. Validate the full proposed `content` against the generated canonical record content contract.
2. Apply any authorized Memory Hooks and revalidate the resulting content.
3. Recursively apply persistence governance to produce the durable content projection, removing values governed by `x-agentpm-persist: false`.
4. Validate the durable content projection against the same canonical record contract.
5. Only then dispatch the durable mutation to the selected `MemoryRuntime`.

Removed values must not be persisted as `null`, redacted text, sentinels, encrypted copies, alternate representations, or other placeholders.

The canonical generated record contract remains authoritative before and after persistence projection; Phase 7B does not introduce a second durable-record schema.

##### Authoring validity

A property that is deterministically structurally required by an applicable object schema must not also be governed by `x-agentpm-persist: false`, because the resulting durable record could never satisfy its canonical contract.

Memory lint/build semantic validation must reject such deterministically identifiable conflicts, including nested/referenced cases supported by the existing schema traversal machinery.

The validator is not required to solve arbitrary Draft 2020-12 schema satisfiability for complex conditional/composed schemas. Runtime durable-projection validation remains the final authority for actual record instances.

##### Backend boundary

Persistence governance is Harness/shared Memory semantic behavior, not SQLite-specific behavior.

Built-in, process-backed, and SDK-hosted `MemoryRuntime` implementations receive only the governed durable projection for durable mutations. A custom MemoryRuntime must never receive `x-agentpm-persist: false` values as part of a durable create/update/upsert request.

##### Derived durable state

Any durable representation derived from Memory content must be based only on the durable content projection. This includes:

- persisted content hashes;
- semantic embedding input;
- persisted vectors;
- durable full-text/search index material;
- equivalent backend-derived durable representations.

Values governed by `x-agentpm-persist: false` must not influence persisted vectors or other durable indexes.

This rule is distinct from Harness trace/content-redaction policy. `x-agentpm-persist` governs durable Memory persistence; trace visibility is controlled independently by trace/redaction policy.

- `x-agentpm-shareable: false` prevents generic semantic export/transfer outside the owning Memory/Agent boundary, including future Agent handoff/agent-to-agent sharing and shareable Memory export. It does not prevent normal owning-Agent use or authorized local inspection.
- trace/log redaction is controlled independently by trace/sensitivity policy; `shareable` must not be overloaded as a trace flag.

### External Memory providers

Phase 7B must ship usable reference external MemoryRuntime implementations for:

- PostgreSQL/pgvector;
- Redis/Redis Stack.

They must advertise capabilities and implement the portable MemoryRuntime semantics required by the supported Blueprint features. Provider implementations may report unsupported capability combinations rather than pretending to support them.

The SDK/provider architecture must make these implementations readable reference examples for custom backends.

## MCP Runtime

MCP has two distinct directions.

### Agent-authored outward MCP export

Agent `bindings.mcp` means: expose selected top-level AgentPM Tools outward as named MCP server surfaces.

It does not add those Tools to phase execution and does not inherit Loop phase access/checkpoints. A Tool may validly be both phase-bound and MCP-exported; that is not redundant composition because the two bindings grant different execution surfaces. Likewise an MCP-exported Tool need not be phase-bound at all. Outward surfaces are Session-owned and may serve external calls even when no Run is currently active.

For MMP, realize each authored MCP surface as one public subprocess:

```text
agentpm serve --mcp --tool ... --host 127.0.0.1 --port 0 --machine
```

Use loopback and ephemeral ports by default.

`agentpm serve --mcp` keeps using the shared internal Tool runner for MCP calls; it must not spawn `agentpm run` per request.

Add machine startup/events so McpRuntime never parses human stderr. Machine events should include readiness, surface lifecycle, and Tool call start/completion/failure sufficient for Harness trace visibility.

Outward MCP calls are not Run phase calls and therefore do not execute phase hooks or Loop approval/access semantics.

### Runtime-configured inward MCP import

`agentpm.harness.json` may configure external MCP servers consumed by the Harness.

Each import must explicitly declare scope, for example global or one/more phase IDs. Scope is required; no implicit global import.

Config may optionally filter advertised external Tool names.

At Session startup, McpRuntime connects/starts the server, performs MCP initialization and Tool discovery, validates configured filters, and produces runtime Tool descriptors.

Imported MCP Tools enter EffectivePhase as runtime-supplied Tool capabilities and obey:

- Loop `access.tools`;
- Tool selection hooks;
- argument schema validation;
- Tool retry/error policy;
- phase-local result semantics;
- Tool events/tracing.

External MCP Tool failures are Loop Tool failures.

Harness assigns stable canonical internal identities such as `mcp:<server-id>/<tool-name>` and provider-safe temporary aliases where required.

## Runtime/provider action contracts

The Engine should depend on typed semantic interfaces rather than provider-specific code. The following are the minimum actions each boundary must expose. Local Rust implementations, process providers, and SDK-hosted providers implement the same semantics; only transport differs.

### ToolRuntime

- `check_readiness(tool)` -> executable/suppression diagnostics;
- `invoke(tool_identity, finalized_arguments, cancellation)` -> structured success/failure from public `agentpm run --machine`.

ToolRuntime has no provider capability advertisement beyond Tool-specific readiness because the Tool manifest/runner contract defines its capabilities.

### KnowledgeRuntime

- `initialize/attest(mapped package versions)`;
- `retrieve(request)` where request identifies exactly one authorized Knowledge package plus context-document or vector-query intent/options;
- return normalized ranked/document results with package/source/chunk identity, score where applicable, content, and citation/provenance metadata.

KnowledgeRuntime readiness advertises supported modes/features and, for explicitly mapped runtimes, which exact package/version/corpus identities it can serve. A custom runtime that cannot attest the mapped package is unavailable rather than silently returning unrelated search results.

### EmbeddingProvider

- `embed(provider, model, dimensions, normalized, text)` -> finite numeric vector;
- advertise which embedding-space tuples/patterns it can faithfully satisfy.

EmbeddingProvider only creates vectors. It does not become a KnowledgeRuntime or MemoryRuntime implicitly.

### MemoryRuntime

The backend exposes primitive durable semantics needed by Harness's canonical Blueprint interpreter, including:

- record create/upsert/update/delete/archive as allowed by the requested space semantics;
- key/filter/chronological/full-text/semantic retrieval capabilities it advertises;
- scoped active-record count/capacity queries;
- deterministic sequence ordinal allocation where sequence is supported;
- durable lifecycle-operation state load/store;
- atomic batch/transaction boundary for multi-record lifecycle mutations where advertised/required.

Harness owns Blueprint trigger meaning, transform/consolidate/delete orchestration, scope authority, source handling, and model-assisted lifecycle generation. A provider must not reinterpret those semantics privately.

### McpRuntime

- outward/export: start/stop/observe one managed `agentpm serve --mcp --machine` surface per Agent MCP binding;
- inward/import: connect/initialize/discover Tools and invoke one selected external MCP Tool;
- expose discovered imported Tool descriptors/readiness to EffectivePhase.

Imported MCP protocol capability advertisement comes from MCP initialization/`tools/list`, not the AgentPM service protocol.

### HookRuntime

- advertise supported Hook IDs;
- `invoke(hook_id, safe_snapshot)` -> `continue` with optional typed patch, or `reject` with reason;
- never mutate RunState directly.

### ApprovalRuntime

- `request_approval(checkpoint, safe_run_context)` -> `approve | deny` plus optional non-secret metadata;
- inability/timeout is a runtime/control failure, never an authored denial.

### ModelRuntime

- `generate(ModelRequest)` -> normalized `ModelTurn` described above;
- advertise selected-model capabilities described in the ModelRuntime section.

Every runtime/provider call must have correlation IDs, cancellation propagation where meaningful, duration/error events, and typed error categories sufficient for Engine failure mapping. Runtime-specific third-party error payloads may be preserved as diagnostics but must not be parsed by Engine control logic.

## Hooks

Hooks are typed interception contracts, not mutable event callbacks.

Hook flow:

```text
Harness snapshot -> HookRuntime -> constrained patch/decision -> Harness validation -> apply/reject -> event
```

Hooks cannot:

- add AgentPM package capabilities;
- change Loop graph/transitions;
- change Loop access/checkpoints/limits;
- choose arbitrary Memory scope partitions;
- mutate authoritative RunState directly;
- expose secrets that were not explicitly projected to the hook.

Version-1 Hook IDs and their constrained contracts are:

| Hook | Safe input | Allowed result/patch |
| --- | --- | --- |
| `before_model_request` | assembled mutable model-facing sections, selected model/options, canonical phase metadata | add/shape mutable context and provider options; may reject request; cannot alter Harness control contract, phase/outcome IDs, EffectivePhase/action catalog, or scope authority |
| `before_tool_selection` | existing effective Tool candidates and safe phase snapshot | reorder/subset existing candidate canonical IDs or reject selection step; cannot add a Tool |
| `before_tool_call` | selected canonical Tool identity + proposed arguments | replace arguments or reject; cannot change Tool identity; Harness revalidates schema afterward |
| `before_knowledge_request` | one authorized package + document/query/options | shape document/query/top-k/threshold/other supported retrieval options inside that same package; cannot switch package or grant a mode not declared/ready |
| `after_knowledge_retrieval` | normalized results/provenance from one request | filter/reorder and optionally transform model-visible result text while retaining source/result identity; cannot introduce unrelated-package results or falsify authoritative provenance |
| `before_memory_read` | authorized Blueprint/space/retrieval request and trusted scope snapshot | shape query/filter/limit/mode within already-bound/ready space and declared retrieval modes; cannot change authoritative scope or package/space |
| `before_memory_write` | authorized Blueprint/space/record type + proposed content | patch record `content` or reject; cannot set envelope/scope/ID/timestamps/provenance; Harness revalidates afterward |
| `before_memory_operation` | eligible participating operation, safe source summary, trusted scope | reject/allow and add operation-model guidance where applicable; cannot rewrite trigger, inputs/output/targets, source handling, operation type, or scope |

A Hook response uses a typed decision equivalent to `continue` (optional allowed patch) or `reject` (reason). Hook-specific patch schemas are closed; arbitrary JSON merge-patch of RunState/config is explicitly not the contract.

Do not add hooks for purely mechanical metadata reads with no meaningful decision point. A Hook runs only when its corresponding authorized decision/action is actually eligible. For example, a phase with zero effective Tools may emit Tool-candidate/suppression events but does not invoke Tool-selection or before-Tool-call Hooks merely to give the Hook a chance to manufacture capability.

Configured intercepting hooks fail closed by default. A hook may explicitly configure a continue/fail-open policy, but silent implicit fail-open is not allowed.

### Hook transport and SDK ergonomics

Workspace process hooks use a persistent structured stdin/stdout protocol with version/capability handshake.

Machine/SDK-hosted callbacks use the Harness machine protocol.

Node and Python SDKs must provide first-class APIs so users register functions/objects rather than parse JSON lines or manage correlation IDs manually. APIs must cover:

- starting/stopping Harness;
- async event iteration/subscription;
- hook registration with typed request/response models;
- approval callbacks;
- cancellation;
- external Memory operation invocation;
- custom embedding/Knowledge/Memory/model provider callbacks where supported;
- final result/report access.

The wire protocol remains public/documented enough that third parties can implement it without the SDKs.

## Approvals and control

ApprovalRuntime is semantically separate from HookRuntime.

Controller resolution follows the configuration precedence defined above:

- an explicit SDK/application per-run approval callback wins;
- otherwise a configured `approvals.controller` process/host implementation is used;
- otherwise Ratatui mode uses the built-in interactive approval UI;
- otherwise machine mode may use the connected host controller when one was registered;
- otherwise plain headless does not auto-approve or auto-deny and terminates with runtime status `approval_required`, recording the pending checkpoint in the report.

Configured controller timeout is runtime/control failure, not authored rejection. Multiple checkpoints targeting one phase remain distinct sequential approval requests in authored order.

Cancellation is first-class. Graceful cancellation produces `cancelled`, flushes report/trace, stops owned MCP/provider processes, and terminates nested Tool process groups. Hard kill remains fallback only.

Persistent paused-Run resume is deferred.

## Consumer context

`bindings.consumer_context.file` is consumer-owned workspace-relative context.

- resolve/canonicalize relative to workspace root and reject escapes;
- snapshot once at Run start;
- use the same snapshot for every phase execution in that Run;
- reload on the next Run;
- missing/unreadable file is a visible warning and non-fatal by default;
- eager model context rather than Knowledge-style retrieval;
- model cannot change the authoritative file during a Run.

TUI/preflight must visibly show loaded/unavailable status and useful size/token estimate metadata. Loading the declared file is mechanical and does not require a dedicated Consumer Context Hook; the normal prompt/context-shaping Hook may operate on the assembled model request afterward without changing which file was authoritative for the Run.

## Events, tracing, and run reports

### Event envelope

Every significant event uses a stable versioned envelope containing at least:

- event schema version;
- stable opaque event ID;
- session ID;
- run ID where applicable;
- monotonically increasing Session sequence for deterministic ordering across the entire Session, including bootstrap/service events before a Run exists;
- optional monotonically increasing Run-local sequence when the event belongs to a Run, reset for each Run;
- timestamp;
- event type;
- phase execution ID where applicable;
- correlation ID where applicable;
- parent event ID where a direct causal parent is useful;
- typed payload.

Correlation IDs group one logical request/operation and are not ordering authority. Session/Run sequences provide ordering; parent event IDs express direct causality where useful. Events are observability records, not event-sourced authoritative state.

Required categories include bootstrap/session/run, phase, model, outcome/transition, Tool, Skill/resource, Knowledge, Memory, approval/control, Hook, MCP, consumer context, cancellation, and terminal status.

The implementation may add more granular events, but the stable version-1 taxonomy should contain semantic equivalents of at least:

- Session/bootstrap: `session_starting`, `service_starting`, `service_ready`, `service_unhealthy`, `service_restarting`, `service_failed`, `preflight_completed`, `session_started`, `session_usage_updated`, `session_stopping`, `session_stopped`;
- Run/phase: `run_started`, `consumer_context_loaded|unavailable`, `phase_enter_requested`, `effective_phase_computed`, `phase_started`, `phase_result_ready`, `phase_failed`, `run_completed|failed|cancelled|limit_reached|approval_required`;
- Model/action: `prompt_prepared`, `model_request_started`, `model_request_completed|failed`, `semantic_action_proposed`, `semantic_action_rejected`, `semantic_action_completed`, `model_repair_requested`;
- Outcome/graph: `outcome_proposed|selected|invalid`, `transition_selected`, `loop_limit_reached`;
- Tool/Skill: `tool_candidates_computed`, `tool_invoked`, `tool_retrying`, `tool_completed|failed`, `skill_activated`, `skill_resource_requested|loaded|failed`;
- Knowledge: `knowledge_surface_ready|unavailable`, `knowledge_request_started`, `knowledge_retrieved|failed`;
- Memory: `memory_surface_ready|unavailable`, `memory_read_started|completed|failed`, `memory_write_started|completed|failed`, `memory_trigger_evaluated`, `memory_operation_eligible|started|completed|failed`;
- Hooks/approval: `hook_started|completed|rejected|failed`, `approval_requested|approved|denied|failed`;
- MCP: `mcp_surface_starting|ready|failed|stopped`, `mcp_import_connected|failed`, `mcp_tool_invoked|completed|failed`;
- Control: `cancellation_requested`, `cancellation_completed`.

Exact payload structs are typed by event type. Events that represent decisions should include the safe reason/candidates/source needed to explain candidate capability composition, inheritance, suppression, readiness, Hook influence, and transition selection. Full prompt/result/argument bodies remain governed by trace content policy.

### Trace content policy

Important occurrence metadata is always eventable. Full content capture is separately configurable.

Default:

- tracing enabled;
- `level: normal`;
- content policy `redacted`;
- secrets never captured.

Supported levels are exactly `minimal | normal | verbose`; content policies are exactly `none | redacted | full`.

### Durable JSON run report

Every Run must produce an exportable structured JSON report. By default write:

```text
.agentpm-state/runs/<run-id>/report.json
```

and a JSONL event trace:

```text
.agentpm-state/runs/<run-id>/events.jsonl
```

The report must include at least:

- report format version;
- run/session IDs and timestamps;
- resolved Agent/Loop identity/version;
- terminal status and final/handoff/abort output;
- effective runtime/provider/model identifiers with secrets removed;
- runtime config source summary;
- preflight warnings/unavailable capabilities;
- scope key names and values only when trace policy permits; sensitive values must be redactable;
- consumer-context file metadata/hash/status, not necessarily content;
- phase executions, outcomes, transitions, checkpoints;
- Tool/MCP/Knowledge/Memory operation summaries;
- usage/token totals when available;
- errors/retries/repair counts;
- trace path/reference.

CLI should allow an explicit report path/export override while preserving the default state-directory report.

Run reports are diagnostic/audit output only and are not resumable checkpoints in Phase 7B.

## TUI

Ratatui is a client of the engine/event/control interfaces, not a separate runtime.

The TUI should remain focused and practical:

- start/preflight screen with Agent, Loop, provider/model, consumer context, Tools, Knowledge, Memory, hooks, MCP, warnings, and readiness;
- run view centered on current phase, concise model/action activity, approvals, terminal result;
- a clearly visible message composer in the primary Run view whenever the Session is ready to accept a new Run; submitting the message creates the next Run, while an active Run uses that area/state for working/cancel/approval feedback rather than accepting ambiguous mid-Run conversation;
- prominent user-facing assistant/PhaseResult output in the primary Run column so the TUI is an agent interaction surface with observability around it, not merely a debugger/status dashboard;
- current-Run usage plus cumulative Session usage where space permits, with unknown provider usage/cost displayed as unknown rather than estimated;
- easy toggles/expansion for prompts, Tool arguments/results, Knowledge results, Memory events, hook decisions, and raw events according to trace policy;
- repeated Runs within one Session;
- cancellation;
- approval controls;
- report/trace location visibility.

### Branding customization

`agentpm.harness.json` may provide lightweight branding:

```json
{
  "ui": {
    "branding": {
      "name": "Acme Agent Console",
      "subtitle": "Internal AI Platform",
      "accent": "#2563EB"
    }
  }
}
```

Requirements:

- `name` changes the visible Harness header/product label for that workspace;
- optional short subtitle appears on start/run surfaces where space permits;
- optional hex accent controls restrained terminal accent styling;
- invalid/unsupported color values fall back safely with a warning;
- branding never changes execution semantics, event types, report schema, protocol identifiers, or AgentPM package identity;
- no arbitrary TUI plugin/layout/theme scripting in Phase 7B.

## Templates and examples

Templates have no direct Harness runtime semantics. Their Phase 7B importance is developer adoption.

Create/upgrade high-quality example Templates/workspaces that demonstrate:

1. **Minimal Harness**: installed Agent + lockfile + `agentpm harness`, little/no runtime config.
2. **SDK-hosted Harness**: Node and/or Python host, typed events/hooks/approval callbacks.
3. **Custom provider Harness**: embedding provider and external Knowledge/Memory runtime configuration.
4. **MCP Harness**: outward Agent MCP surfaces plus scoped external MCP import.
5. **Full reference Harness**: Loop, Tools, Skills, Profiles, Knowledge, Memory, consumer context, approvals, hooks, tracing, TUI, and reports.

Generated README files should explicitly teach:

```text
Agent artifacts       = portable definition
agentpm.harness.json  = this workspace's runtime realization
agentpm harness       = AgentPM reference execution
```

Template variables, `stack`, `execution_surfaces`, dependencies, and entrypoints remain generation/developer-time metadata and are not interpreted by Harness. Template dependencies never become Harness bindings or global capability merely because they were declared by the Template, and Template entrypoint commands are never auto-executed by Harness. Once Template files are generated into a workspace they are consumer-owned/runtime inputs according to their resulting role; Harness does not retain special runtime behavior based on Template provenance. A Template that scaffolds multiple Agents still does not create multi-Agent Harness execution: one resolved Agent remains the runnable unit.

## Trust and security model

Authority order:

1. Harness control/protocol, Loop graph/access/checkpoints, runtime safety;
2. authored Agent behavior: phase objective, Profiles, Skills;
3. consumer context and user input;
4. retrieved/generated data: Knowledge, Memory, Tool/MCP outputs.

Lower classes cannot alter higher-class authority.

Additional requirements:

- secrets are runtime-resolved and component-scoped;
- do not send all process environment variables to hooks/providers by default;
- `.env.local` may be parsed once into an internal resolver, but secrets are projected only to declared components;
- secrets never appear in events/reports;
- package-relative path access must canonicalize and remain within the resolved package root;
- consumer context/config-relative paths must remain within workspace root unless a runtime configuration field explicitly allows an external path;
- externally imported MCP capability must be explicitly scoped;
- externally configured providers are trusted runtime components but receive only contract-relevant data/secrets.

## Failure taxonomy

Keep failures semantically distinct:

- malformed model proposal before a runtime service -> repairable model/action validation failure;
- ToolRuntime invocation failure -> Loop `tool_failure` policy;
- phase cannot complete after repairs/service failures -> Loop `phase_failure` policy/default;
- Knowledge/Memory backend request failure -> structured service failure returned to phase; not automatically Tool failure;
- Memory lifecycle operation failure -> first-class Memory operation failure; may cause originating write/phase failure when required;
- Hook failure -> fail closed by default unless explicit continue policy;
- approval transport/timeout -> runtime/control failure, not rejection;
- Harness infrastructure/service protocol failure -> runtime `failed`;
- user/host cancellation -> `cancelled`;
- runtime safety/Loop step ceiling -> `limit_reached`.

If Loop error policy is absent, default canonical Harness behavior is:

- Tool failure: fail the current phase;
- phase failure: runtime `failed`;

These defaults must be visible in preflight/report.

## SDK architecture

Node and Python SDKs must not reimplement orchestration.

Both SDKs should expose parity around a `Harness`/`HarnessClient` abstraction with language-idiomatic naming and typed models for:

- Harness config overrides;
- run/session options;
- events;
- preflight result;
- terminal/run result;
- run report;
- approvals;
- Hook request/response types;
- cancellation;
- external Memory operation invocation;
- host-provider request/response types.

The SDK starts `agentpm harness --machine` as a subprocess and manages the structured protocol.

Hooks must be first-class ergonomic APIs, for example conceptually:

```text
client.onBeforeToolCall(fn)
client.onPrompt(fn)
client.onApproval(fn)
```

rather than requiring users to implement stdio framing.

Provider interfaces should make the open-source Pinecone, pgvector, Redis, and custom embedding examples reusable and readable. SDK provider helpers may use optional dependencies/extras so core SDK installation remains lightweight.

Built-in OpenAI/Anthropic/Ollama execution remains canonical in Rust; SDKs expose typed configuration and may provide host-provider examples without creating a second orchestration engine.

## Acceptance criteria

Phase 7B is complete when all of the following are true:

- `agentpm harness` can execute the same resolved Agent/Loop semantics end-to-end through interactive TUI, one-shot plain headless, and persistent machine/SDK surfaces using one HarnessEngine.
- One-shot headless accepts direct/stdin/file input, runs exactly one Run, emits user-facing terminal output without machine framing, writes report/trace, returns documented terminal exit behavior, and shuts down owned Session services.
- `agentpm harness --machine` exposes the versioned bidirectional protocol, event stream, host-provider/Hook/approval requests, cancellation/control, and terminal/report results without human text on protocol stdout.
- A three-phase example can make multiple model calls within a phase, execute at least two AgentPM Tools through public `agentpm run`, transition by outcomes, enforce Loop access, require approval, and terminate correctly.
- `agent.lock` is authoritative and installed Agent roots can run without requiring a local `agent.json`.
- Missing Loop makes an Agent non-runnable with a clear error rather than inventing a default Loop.
- EffectivePhase composition correctly handles global/phase bindings, Skill Tool inheritance, Profiles, Knowledge, Memory, runtime MCP imports, Loop restrictions, and runtime readiness.
- OpenAI, Anthropic, and Ollama each complete the same representative Harness scenario using open model IDs/configuration rather than model enums.
- ModelRuntime normalizes provider-native responses into Harness ModelTurns/semantic action proposals; the Engine never parses provider-specific response shapes, and a representative phase demonstrates multiple model/action cycles before PhaseResult.
- Runtime safety-limit counters follow the exact semantics in this spec, including `max_actions_per_phase`.
- `agentpm run` has machine-readable output, input/output schema enforcement, runtime-version enforcement, and cancellation-safe nested process cleanup.
- Node and Python SDKs can launch Harness, consume events, provide prompt/Tool Hooks plus separate approval callbacks, cancel Runs, and receive terminal/report results without implementing the protocol manually.
- Hook failure behavior is explicit and fail-closed by default.
- Process and SDK-hosted model/embedding/Hook/Knowledge/Memory/approval implementations use the documented version-1 typed service contracts and live capability advertisements; Engine behavior is transport-independent.
- Required Hook IDs use closed typed patch/decision contracts and required semantic events are emitted through the stable event taxonomy.
- Context Knowledge supports on-demand document loading.
- Vector Knowledge supports local AgentPM retrieval plus configurable EmbeddingProvider fallback.
- Pinecone and pgvector external Knowledge runtimes are usable reference implementations.
- A bound but unrealizable Knowledge surface is diagnosed/suppressed without automatically killing an otherwise runnable Agent.
- Built-in SQLite MemoryRuntime persists records across Harness processes under `.agentpm-state`, validates generated contracts, resolves arbitrary authored scopes, enforces retention/capacity/append-only semantics, and supports required retrieval modes according to advertised readiness.
- Built-in SQLite semantic Memory stores portable f32 BLOB vectors and performs exact cosine search in Rust without requiring sqlite-vec or another SQLite vector extension.
- Memory lifecycle operations support automatic interval/record-count/capacity triggers, external invocation, durable trigger state, transform/consolidate model calls, delete semantics, provenance, and source handling.
- PostgreSQL/pgvector and Redis/Redis Stack external Memory runtimes are usable reference implementations or report unsupported Blueprint capabilities accurately.
- Agent-authored MCP bindings start one outward `agentpm serve --mcp` process per logical surface using loopback/ephemeral ports and structured machine readiness.
- Runtime-configured external MCP imports expose explicitly scoped Tools to the model and apply normal Loop Tool access/hooks/retry/failure behavior.
- Consumer context is snapshotted once per Run and visible in preflight/TUI status.
- TUI shows preflight readiness, phase execution, approvals, expandable trace detail, and lightweight configurable branding.
- Every Run writes a versioned JSON report and JSONL event trace with secrets redacted according to policy.
- Run usage is reported per Run and aggregated into observable Session usage across repeated Runs without fabricating unavailable token/cost data.
- Multiple approval checkpoints targeting one phase are evaluated deterministically in authored order.
- Memory transform `output_mode` is supported with backward-compatible default `create`.
- High-quality Harness Templates/examples demonstrate minimal, SDK-hosted, external-provider, MCP, and full-reference workflows.
- Existing package publish/install/registry behavior remains compatible except for the intentional Loop checkpoint semantic relaxation and Memory transform schema addition.

## Risks / edge cases

- **Runtime monolith risk:** keep Engine, bootstrap, services, protocol, TUI, and SDK host boundaries modular; do not let Ratatui own orchestration.
- **Silent authority expansion:** imported MCP is the only planned runtime capability augmentation. Hooks/providers must not accidentally become generic capability injection.
- **Provider lock-in:** built-in OpenAI/Anthropic/Ollama are conveniences; model IDs and provider contracts remain runtime-swappable.
- **Duplicate execution engines:** SDKs must never implement their own Loop traversal.
- **Tool subprocess recursion:** Harness uses public `agentpm run`; `agentpm serve --mcp` continues using the shared runner internally and must not recursively spawn `agentpm run` for each MCP request.
- **Provider-name normalization:** canonical Tool/MCP identity must survive provider-safe aliasing for debugging and hooks.
- **Prompt bloat:** consumer context is eager, but Skills/Knowledge/Memory must remain progressive/on-demand.
- **Profile over-enforcement:** required constraints remain authored prompt guidance, not fake post-hoc enforcement.
- **Memory store confusion:** Blueprints never contain live records; generated contracts are runtime contracts only.
- **Memory crash consistency:** local records and trigger watermarks must not diverge easily; use SQLite transactions.
- **Trigger storms:** edge-trigger/re-arm rules and persistent state must prevent repeated firing on an unchanged condition.
- **TTL ambiguity:** use updated-at-first anchoring consistently.
- **External provider mismatch:** explicitly mapped Knowledge/Memory runtimes must validate package/corpus/capability identity where possible and never silently serve unrelated state.
- **MCP authority confusion:** outward Agent MCP is independent external authority; inward MCP is phase Tool authority and must be scoped/enforced accordingly.
- **Trace privacy:** full observability does not mean logging secrets or every sensitive value by default.
- **Headless approvals:** never auto-approve; terminate `approval_required` or use a controller.
- **State directory lifecycle:** generated Templates/docs must explain `.agentpm-state` and gitignore it.
- **Over-themed TUI:** branding remains name/subtitle/accent, not a UI plugin framework.
- **Scope-value injection:** Memory scope values are trusted runtime context, never direct model authority.
- **Configuration sprawl:** keep `agentpm.harness.json` versioned, typed, and grouped by runtime concern; avoid arbitrary nested escape hatches.

## Open questions

No blocking product questions remain for Phase 7B planning.

Implementation may adapt exact type/function/file names to existing repository structure, but the semantic boundaries in this spec are authoritative. If repository constraints require changing a portable artifact contract beyond the two explicit changes below, the implementation should stop and raise the change rather than silently expand it.

Intentional portable-contract changes discovered by Phase 7B:

1. Relax Loop semantic validation to allow multiple approval checkpoints targeting the same phase, evaluated in authored array order.
2. Add optional Memory transform `output_mode: create | replace_input`, defaulting to `create` when absent.

Resumable persisted RunState remains explicitly deferred. JSON run reports and durable traces are required in this phase.

## Related Specs

- Phase 2: Agent packages and Agent dependency installation
- Phase 4: Workflow Templates and `agentpm new`
- Phase 6A: Skills
- Phase 6B: Knowledge
- Phase 6C: Memory Blueprints
- Phase 6D: Instruction Profiles
- Phase 7A: Loops & Agent Bindings
