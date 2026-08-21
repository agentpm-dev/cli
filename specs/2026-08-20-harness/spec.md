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

> A Harness Session owns long-lived runtime services and may host multiple Runs. A Run is one Loop traversal from entry phase to a terminal or runtime terminal state.

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

It is consumer-owned, optional, source-control friendly, and not packaged with the Agent.

### Versioned top-level shape

Phase 7B should implement a strict versioned contract conceptually equivalent to:

```json
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-5",
    "options": {}
  },
  "providers": {
    "models": {},
    "embeddings": {}
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
    "implementations": {}
  },
  "knowledge": {
    "runtimes": {},
    "packages": {},
    "embedding_matches": []
  },
  "memory": {
    "runtimes": {},
    "packages": {}
  },
  "mcp": {
    "imports": {},
    "exports": {
      "host": "127.0.0.1"
    }
  },
  "approvals": {
    "headless": "require-controller"
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

This is the required semantic organization; implementation may adjust small field names to align with repository conventions, but must not collapse unrelated concepts into generic untyped maps.

### Configuration precedence

For runtime values that can be supplied in multiple places, use the following precedence from highest to lowest:

1. explicit SDK/application control message or per-run API option;
2. explicit CLI option;
3. `agentpm.harness.json`;
4. standard environment variables / provider-specific environment conventions;
5. Harness default.

Portable Agent/Loop metadata is not part of this override chain. Runtime config may realize/narrow it but cannot override authored graph/access/checkpoint semantics.

The resolved value and source must be inspectable in preflight/events/report.

### Model configuration

`model.provider` and `model.model` are open strings. Phase 7B ships built-in providers with IDs:

- `openai`
- `anthropic`
- `ollama`

Concrete model IDs are not enumerated by AgentPM and may change without AgentPM releases.

Standard provider environment variables should be used where established. Config may specify non-secret provider endpoints/options. Secrets must be resolved at runtime and never serialized into events or reports.

Unknown provider IDs may be satisfied by a configured custom model provider implementation.

### Runtime limits defaults

If absent, use conservative Harness defaults:

- `max_steps`: 100 Harness safety ceiling;
- `max_model_calls_per_phase`: 24;
- `max_tool_calls_per_phase`: 16;
- `max_actions_per_phase`: 64;
- `max_tool_call_repairs`: 2 additional repair attempts;
- `max_structured_output_repairs`: 2 additional repair attempts;
- `max_memory_operation_repairs`: 2 additional repair attempts.

`loop.limits.max_steps`, when present, remains authoritative. Effective maximum steps is the stricter of the Loop value and runtime safety ceiling. Runtime config may tighten but never loosen the authored Loop limit.

These values are defaults, not portable Loop semantics, and must be reported transparently.

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
10. establish Hook/Knowledge/Memory/MCP/provider service implementations;
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

A TUI Session may execute multiple Runs. A one-shot headless invocation may have one Session and one Run.

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

ModelRuntime owns provider communication and normalizes provider-specific messages/function calls into Harness semantic turns/actions.

Required built-in providers:

- OpenAI;
- Anthropic;
- Ollama/local HTTP runtime.

Requirements:

- concrete model IDs remain open strings;
- provider capability differences are adapted at runtime and surfaced diagnostically;
- provider-safe temporary function/tool names may be used, but Harness events/hooks always use canonical AgentPM/MCP identities;
- secrets are resolved from scoped runtime environment/config and never logged;
- response normalization must preserve structured usage when available;
- unsupported required provider capabilities produce readiness diagnostics rather than artifact mutation.

Ollama is the required local/open path so a user can run the Harness without a paid hosted model provider when a suitable local model is installed.

## Profiles

Profiles are resolved model-facing behavioral inputs; there is no ProfileRuntime.

- top-level Profile dependency alone does not activate behavior;
- global/phase bindings determine active Profiles;
- global + phase are additive and deduplicated;
- multiple Profiles remain distinct inputs, not merged into a synthetic Profile;
- ordering is deterministic serialization, not precedence/override;
- required/preferred constraints influence prompt wording only; Harness does not fake post-response enforcement;
- compatibility is advisory and may produce strong warnings;
- boundaries are authored model guidance and never replace Harness authority.

## Skills

There is no SkillRuntime.

A bound Skill contributes:

- compact manifest/description/resource inventory for initial discovery;
- progressive access to its entrypoint/reference resources as needed;
- inherited Tool dependencies in the Skill's binding scope.

Binding a Skill does not eagerly inject all Skill files.

Package-owned Skill paths resolve relative to the resolved Skill package root and must remain inside that root after canonicalization/symlink resolution.

Skill scripts never auto-execute. A script can execute only through independently authorized capability such as an AgentPM shell-executor Tool.

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

Failure before ToolRuntime invocation is a model/action repair concern. Failure while ToolRuntime attempts the invocation is a Loop Tool failure.

A schema-valid Tool output such as `{ "ok": false }` is still a successful Tool invocation unless the generic Tool contract itself defines it as invalid; Harness must not infer domain semantics from arbitrary output fields.

Known runtime-incompatible Tools are suppressed from the model with a diagnostic. Missing required environment variables produce strong preflight warnings but remain authoritative at invocation because runtime environment may change/be supplied.

Retries are fresh `agentpm run` invocations with the same finalized arguments. A model choosing different arguments is a new Tool call.

## KnowledgeRuntime

Knowledge bindings make packages available on demand; they never imply automatic retrieval.

### Context mode

Expose a compact package/document inventory initially. The model requests one declared document when needed. Document bodies are not eagerly injected.

Package paths resolve from the resolved Knowledge package root and must remain inside it.

### Vector mode

KnowledgeRuntime accepts a text query and returns normalized structured results with chunk/source/citation metadata.

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

Generated Memory contracts are the runtime durable record contracts. Author source schemas are build-time inputs.

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

MemoryRuntime advertises supported:

- space models;
- retrieval modes;
- retention actions;
- constraints;
- capacity handling;
- other protocol capabilities.

Harness compares the selected runtime's live capabilities with each Blueprint space/operation during preflight. A space is exposed only when its required contract can be faithfully realized.

### Local SQLite MemoryRuntime

Default mutable path:

```text
.agentpm-state/memory.sqlite3
```

Use a schema equivalent to the following logical tables (exact SQL types/index names may follow Rust/SQLite conventions):

#### `memory_meta`

- `key TEXT PRIMARY KEY`
- `value TEXT NOT NULL`

Must contain local store schema version.

#### `memory_records`

- `package TEXT NOT NULL`
- `package_version TEXT NOT NULL`
- `space TEXT NOT NULL`
- `record_type TEXT NOT NULL`
- `scope_hash TEXT NOT NULL`
- `scope_json TEXT NOT NULL` (canonical JSON)
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

Indexes at minimum:

- `(package, package_version, space, scope_hash, record_type)`;
- `(package, package_version, space, scope_hash, ordinal)` for sequence reads;
- `expires_at` for retention cleanup;
- active/archived lookup suitable for the chosen SQLite version.

Archived records remain in the table with `archived_at` set and are excluded from normal active retrieval.

#### `memory_operation_state`

- `package TEXT NOT NULL`
- `package_version TEXT NOT NULL`
- `operation TEXT NOT NULL`
- `scope_hash TEXT NOT NULL`
- `scope_json TEXT NOT NULL`
- `trigger_type TEXT NOT NULL`
- `armed INTEGER NOT NULL`
- `baseline_at TEXT NULL`
- `last_completed_at TEXT NULL`
- `next_eligible_at TEXT NULL`
- `last_observed_value INTEGER NULL`
- `watermark_json TEXT NULL`
- `updated_at TEXT NOT NULL`
- primary key: `(package, package_version, operation, scope_hash)`

#### `memory_vectors`

Used only when the local runtime implements semantic retrieval:

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

The local implementation may use exact vector search initially. It must not require a hosted vector database.

SQLite operations that combine record mutation and lifecycle trigger state should use transactions where practical so crashes do not create obvious record/trigger divergence.

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

Model-assisted operation calls receive operation description, authorized source content, target content schema, and Harness lifecycle instructions. They return target `content` only. Output is validated and repair-bounded before persistence.

### Memory schema amendment: transform output mode

Add optional `output_mode` to transform operations:

```text
create | replace_input
```

Omitted value defaults to `create` for backward compatibility.

`replace_input` means the transformed output updates/replaces the originating source record and is valid only when output space/record type matches the single input space/record type. It is explicit lifecycle authority even if the space is append-only for direct writes.

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

- `x-agentpm-persist: false` prevents durable persistence of that field and must be enforced before commit.
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

It does not add those Tools to phase execution and does not inherit Loop phase access/checkpoints.

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

Required hook families should include at least:

- prompt/context shaping before model request;
- Tool candidate/selection influence where applicable;
- before Tool call argument shaping/rejection;
- Knowledge request shaping and post-retrieval filtering/reranking;
- before Memory read/write;
- before participating Memory lifecycle operation execution;
- approval decision callback in application-hosted mode.

Do not add hooks for purely mechanical metadata reads with no meaningful decision point.

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

- Ratatui mode presents interactive approval controls.
- machine/SDK mode emits an approval request and awaits a typed approve/deny response.
- plain headless mode with no controller does not auto-approve or auto-deny. It terminates the Run with runtime status `approval_required` and records the checkpoint in the report.
- optional configured approval timeout is supported for controller modes; timeout is runtime/control failure, not authored rejection.

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

TUI/preflight must visibly show loaded/unavailable status and useful size/token estimate metadata.

## Events, tracing, and run reports

### Event envelope

Every significant event uses a stable versioned envelope containing at least:

- event schema version;
- session ID;
- run ID where applicable;
- monotonically increasing run-local sequence;
- timestamp;
- event type;
- phase execution ID where applicable;
- correlation/parent IDs where applicable;
- typed payload.

Events are observability records, not event-sourced authoritative state.

Required categories include bootstrap/session/run, phase, model, outcome/transition, Tool, Skill/resource, Knowledge, Memory, approval/control, Hook, MCP, consumer context, cancellation, and terminal status.

Decision events should explain candidate capability composition, inheritance, suppression, readiness, and hook influence.

### Trace content policy

Important occurrence metadata is always eventable. Full content capture is separately configurable.

Default:

- tracing enabled;
- `level: normal`;
- content policy `redacted`;
- secrets never captured.

Supported levels should include `minimal | normal | verbose` and content policies should include `none | redacted | full` or equivalent explicit values.

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

Template variables, `stack`, `execution_surfaces`, dependencies, and entrypoints remain generation/developer-time metadata and are not interpreted by Harness.

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

- `agentpm harness` can execute a resolved Agent with a Loop end-to-end in TUI and headless modes.
- A three-phase example can make multiple model calls within a phase, execute at least two AgentPM Tools through public `agentpm run`, transition by outcomes, enforce Loop access, require approval, and terminate correctly.
- `agent.lock` is authoritative and installed Agent roots can run without requiring a local `agent.json`.
- Missing Loop makes an Agent non-runnable with a clear error rather than inventing a default Loop.
- EffectivePhase composition correctly handles global/phase bindings, Skill Tool inheritance, Profiles, Knowledge, Memory, runtime MCP imports, Loop restrictions, and runtime readiness.
- OpenAI, Anthropic, and Ollama each complete the same representative Harness scenario using open model IDs/configuration rather than model enums.
- `agentpm run` has machine-readable output, input/output schema enforcement, runtime-version enforcement, and cancellation-safe nested process cleanup.
- Node and Python SDKs can launch Harness, consume events, provide at least prompt/Tool/approval hooks, cancel Runs, and receive terminal/report results without implementing the protocol manually.
- Hook failure behavior is explicit and fail-closed by default.
- Context Knowledge supports on-demand document loading.
- Vector Knowledge supports local AgentPM retrieval plus configurable EmbeddingProvider fallback.
- Pinecone and pgvector external Knowledge runtimes are usable reference implementations.
- A bound but unrealizable Knowledge surface is diagnosed/suppressed without automatically killing an otherwise runnable Agent.
- Built-in SQLite MemoryRuntime persists records across Harness processes under `.agentpm-state`, validates generated contracts, resolves arbitrary authored scopes, enforces retention/capacity/append-only semantics, and supports required retrieval modes according to advertised readiness.
- Memory lifecycle operations support automatic interval/record-count/capacity triggers, external invocation, durable trigger state, transform/consolidate model calls, delete semantics, provenance, and source handling.
- PostgreSQL/pgvector and Redis/Redis Stack external Memory runtimes are usable reference implementations or report unsupported Blueprint capabilities accurately.
- Agent-authored MCP bindings start one outward `agentpm serve --mcp` process per logical surface using loopback/ephemeral ports and structured machine readiness.
- Runtime-configured external MCP imports expose explicitly scoped Tools to the model and apply normal Loop Tool access/hooks/retry/failure behavior.
- Consumer context is snapshotted once per Run and visible in preflight/TUI status.
- TUI shows preflight readiness, phase execution, approvals, expandable trace detail, and lightweight configurable branding.
- Every Run writes a versioned JSON report and JSONL event trace with secrets redacted according to policy.
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
