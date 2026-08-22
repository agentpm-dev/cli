# Tasks

## Milestone 1: Harness Contract Corrections and Runtime Configuration
> Scope note: establish the two portable-contract corrections discovered during Harness design and implement the versioned workspace runtime-configuration subsystem. This milestone includes existing-manifest semantic validation for the Loop/Memory contract changes plus config-local structural and semantic validation for `agentpm.harness.json`. It does not resolve or execute a runnable Agent, perform Agent-specific cross-package/preflight validation, start runtime services, call models, persist live Memory, run MCP servers, invoke hooks, or render a TUI.
- [ ] Update existing Loop semantic validation to allow multiple approval checkpoints with the same `before_phase`.
- [ ] Preserve authored `loop.checkpoints` array order and document that multiple checkpoints targeting one phase are evaluated in that order at runtime.
- [ ] Remove/replace tests and docs that reject duplicate `before_phase` checkpoints solely because they target the same phase.
- [ ] Retain checkpoint ID uniqueness, valid phase targets, valid `on_reject` targets, and all existing Loop structural/semantic validation.
- [ ] Add optional Memory transform `output_mode` with exactly `create | replace_input`.
- [ ] Default omitted transform `output_mode` to `create` in typed/runtime interpretation for backward compatibility.
- [ ] Add existing-manifest semantic validation for `replace_input`: transform-only, exactly one input, output space/record type equal to the input pairing, and `source_handling` exactly `retain`.
- [ ] Update the flagship `conversation-continuity` example so `refresh_saved_note` explicitly uses `output_mode: "replace_input"`.
- [ ] Confirm the Loop and Memory changes are implemented in the existing portable-manifest schema/type/lint/semantic-validation paths rather than in Harness runtime-config validation.

- [ ] Add strict JSON Schema and Rust typed models for root-level `agentpm.harness.json` version 1. If the file exists, `version` is required and must equal `1`; `{ "version": 1 }` is the minimal valid file.
- [ ] Reject unknown version-1 fields (`additionalProperties: false`) except the intentionally open ID-keyed registries/maps and provider-specific `model.options` defined in `spec.md`; unused sections are omitted rather than required as empty objects.
- [ ] Implement the exact version-1 JSON field names/nesting from the Harness Configuration Contract in `spec.md`; do not rename/restructure them to repository preferences without a spec change.
- [ ] Implement the shared `implementation` union exactly as `process | host`, including process `command/args/cwd/env/startup_timeout_ms/request_timeout_ms/restart` semantics/defaults and host request-timeout semantics.
- [ ] Resolve all relative Harness-config paths from workspace root even when `--config` points elsewhere; allow an absolute path only where the spec explicitly permits it, notably `runtime.state_dir`.
- [ ] Treat process `env` entries as names projected through the scoped environment/`.env.local` resolver; never treat them as literal secret values or inherit the entire parent environment by default.

- [ ] Add `providers.models` and `providers.embeddings` typed registries; reserve built-in model IDs `openai`, `anthropic`, and `ollama` from custom redefinition while keeping concrete model IDs open strings.
- [ ] Add `hooks.implementations` plus ordered `hooks.bindings` with the exact version-1 Hook IDs and `failure_policy: closed | continue`; allow repeated Hook IDs and preserve binding-array order.
- [ ] Add `knowledge.runtimes`, versionless `knowledge.packages` mappings, and exact `knowledge.embedding_matches` tuples (`provider/model/dimensions/normalized -> embedding_provider`).
- [ ] Add `memory.runtimes`, versionless `memory.packages` mappings, and `memory.local.semantic` (`embedding_provider/model/dimensions`) for the built-in SQLite semantic-retrieval realization.
- [ ] Add exact MCP import unions: `stdio` with process-like command/env/lifecycle fields and `http` with absolute URL plus header values expressed as `{value}` or `{env}`.
- [ ] Require every MCP import to declare explicit `scope.mode = global | phases`; validate scope/filter structure in this milestone but defer checking phase names against a selected Loop and Tool names against a live MCP server.
- [ ] Add MCP export config `enabled` + `host` with defaults from `spec.md`; do not add static per-surface port mapping in config v1.
- [ ] Add optional `approvals.controller.implementation` plus `approvals.timeout_ms`; keep plain-headless-without-controller behavior fixed as `approval_required`, not a configurable auto-approve/deny mode.
- [ ] Implement exact trace enums `minimal | normal | verbose` and `none | redacted | full`, plus UI branding `name/subtitle/accent` with six-digit `#RRGGBB` validation.

- [ ] Add a Harness runtime-configuration subsystem that:
    - loads the optional default or CLI-selected config file,
    - validates JSON structure and deserializes typed version-1 models,
    - performs config-local semantic validation,
    - applies Harness defaults and explicit overrides,
    - tracks the source of resolved values,
    - and returns a typed resolved configuration for later bootstrap/execution.
- [ ] Keep config-local semantic validation separate conceptually from file loading/parsing even if repository organization places them in the same module.
- [ ] In config-local semantic validation, reject references to undefined config registry IDs, including Hook implementation IDs, Knowledge runtime IDs, Memory runtime IDs, custom model/embedding provider IDs, Memory local semantic embedding-provider IDs, and Approval controller implementations.
- [ ] Reject duplicate/ambiguous configuration-local declarations such as duplicate exact `knowledge.embedding_matches` tuples where more than one provider could satisfy the same tuple.
- [ ] Do not attempt Agent-aware validation in this milestone: `knowledge.packages` and `memory.packages` keys do not yet need to correspond to packages in a selected Agent, and MCP phase names do not yet need to correspond to a resolved Loop.
- [ ] Add resolved-config source metadata capable of distinguishing SDK/run override, CLI override, config file, environment, and Harness default.
- [ ] Implement default Harness safety limits and managed-process lifecycle defaults from `spec.md`; ensure they are runtime defaults, not written into portable Loop/Agent manifests.

- [ ] Add unit/schema tests for missing config, minimal config, complete populated config, unknown fields, invalid shared implementation descriptors, undefined config registry references, duplicate Hook/embedding mappings, unsafe paths, invalid branding/limits/scopes, malformed MCP stdio/HTTP/header/scope forms, and invalid approval controller config.
- [ ] Add portable-manifest regression tests covering the Loop checkpoint relaxation and Memory `output_mode` addition.
- [ ] Confirm existing Agent/Loop/Memory manifests continue to validate except for the intentional semantic relaxation/addition above.

## Milestone 2: Harness Bootstrap, Workspace Discovery, and Preflight Plan
> Scope note: add the `agentpm harness` command shell and the Agent-aware bootstrap/preflight pipeline. This milestone combines workspace/install state, `agent.lock`, an optional local Agent manifest, and the resolved Harness configuration from Milestone 1 into a structured `ResolvedHarnessPlan` and `PreflightReport`. It performs cross-artifact semantic validation and static capability-readiness planning but does not traverse the Loop, call a model, execute Tools, start external providers/hooks/MCP processes, persist live Memory, or render the full TUI.
- [ ] Add `agentpm harness [AGENT]` to CLI command routing/help.
- [ ] Add Harness CLI/config inputs required for bootstrap, including the optional runtime-config path override and state-directory/runtime overrides defined in `spec.md`.
- [ ] Reuse existing workspace-root discovery conventions rather than adding Harness-only root search behavior.
- [ ] Load the resolved Harness runtime configuration through the Milestone 1 configuration subsystem rather than duplicating configuration parsing/defaulting inside bootstrap.

- [ ] Require `agent.lock` for execution and return an actionable error when missing.
- [ ] Support local `agent.json` when present but do not require it if a runnable installed Agent root is fully represented by lock/install state.
- [ ] Resolve an explicit Agent selector against lockfile/install state.
- [ ] If `AGENT` is omitted and exactly one runnable Agent root exists, select it deterministically.
- [ ] If multiple runnable Agent roots exist, return a structured selection requirement for one-shot headless/machine modes and leave interactive selection to the later TUI milestone.
- [ ] Reject Agents without a resolved Loop as non-runnable by Harness while preserving that such an Agent remains a valid package artifact; do not invent a default Loop.

- [ ] Resolve exact installed Agent, Loop, Tool, Skill, Knowledge, Memory, and Profile package versions from lockfile relationships.
- [ ] Keep top-level Agent dependency arrays as dependency/install graph only; require an applicable binding for authored runtime availability and ensure an installed-but-unbound artifact is not surfaced automatically.
- [ ] Treat Agent `examples`, README, and license metadata as non-behavioral discovery/documentation data only.
- [ ] Load/resolve immutable package metadata and generated build artifacts needed for later runtime use without mutating installed package state.

- [ ] Add Harness-only cross-artifact semantic validation after the runnable Agent and Loop are resolved.
- [ ] Validate Agent phase-binding keys against the resolved Loop; warn/ignore unknown phase-binding keys rather than failing an otherwise coherent Loop.
- [ ] Validate bound Tool, Skill, Knowledge, Memory, and Profile identities against the Agent dependency graph and lock/install state.
- [ ] Validate Memory binding `spaces` and `operations` against the resolved Memory Blueprint.
- [ ] Validate Skill Tool dependencies and compute inherited Tool candidates according to global/phase Skill binding scope.
- [ ] Detect same-scope direct Tool + Skill-inherited Tool duplication, warn, and de-dupe; do not treat global-direct + phase-inherited availability as inherently redundant.
- [ ] Validate Agent MCP export Tool membership against explicitly declared top-level Agent Tool dependencies; do not allow Skill-transitive Tools to become exported solely because they are installed.
- [ ] Validate Loop graph/checkpoint/outcome references needed for runtime interpretation, including entry phase, transition targets, duplicate/ambiguous transitions, and ordered multiple checkpoints.
- [ ] Validate generated Knowledge and Memory build/index/contract metadata required for later runtime use and classify missing/corrupt generated metadata according to `spec.md`.

- [ ] Resolve `knowledge.packages` and `memory.packages` configuration mappings against the selected Agent/lock graph; distinguish an undefined config runtime ID (Milestone 1 error) from a well-formed mapping to a package that is not relevant/resolved for the selected Agent (preflight diagnostic).
- [ ] Resolve configured model/provider identity and custom provider references but do not start/call ModelRuntime implementations yet.
- [ ] Resolve Hook implementation/binding references into the plan but do not start HookRuntime processes yet.
- [ ] Resolve Memory local semantic embedding-provider references into the plan but do not start embedding providers yet.
- [ ] Validate MCP import phase scopes against the resolved Loop.
- [ ] Preserve configured MCP Tool filters for later live `tools/list` validation; do not claim Tool readiness before the MCP server has actually been started/discovered in the MCP milestone.
- [ ] Resolve Approval controller configuration into the plan but do not start/invoke it yet.

- [ ] Resolve consumer-context path safely relative to workspace root but do not yet inject its contents into model prompts.
- [ ] Record consumer-context readiness as available, missing/unavailable warning, or invalid/unsafe path according to `spec.md`.
- [ ] Resolve configured runtime scope values without giving model code authority to choose them.
- [ ] Compute which Memory spaces require which authored scope keys and report unresolved required runtime scope values as capability-readiness diagnostics rather than letting the model invent values.
- [ ] Add `.agentpm-state` path resolution with config/CLI override support and keep it physically/logically separate from immutable `.agentpm` installed package state.

- [ ] Add a static capability-planning layer that distinguishes:
    - authored candidates from Agent bindings and Skill inheritance,
    - runtime-configured augmentation candidates such as scoped MCP imports,
    - Loop restrictions,
    - statically knowable runtime requirements,
    - and capability states that require later live service handshake/readiness checks.
- [ ] Do not mark a custom provider/runtime as fully ready merely because configuration exists; represent readiness as pending/unverified until its later runtime milestone performs the protocol handshake/capability advertisement.
- [ ] Suppress capabilities that are already conclusively impossible from static state, such as a missing installed package or known-incompatible Tool runtime requirement, while preserving a diagnostic explaining why.
- [ ] Do not fail the entire runnable Agent merely because an optional Tool/Knowledge/Memory/MCP capability is unavailable unless execution itself cannot be coherently interpreted.

- [ ] Classify preflight diagnostics as fatal, warning, suppressed/unavailable capability, pending runtime verification, or informational according to `spec.md`.
- [ ] Ensure fatal diagnostics are reserved for conditions that prevent coherent Harness interpretation, such as missing lock/root Agent/Loop, invalid graph references, or otherwise ambiguous authoritative runtime structure.
- [ ] Preserve safe-degradation behavior for non-fatal mismatches and unavailable optional capabilities.

- [ ] Add `PreflightReport` and `ResolvedHarnessPlan` Rust models that are UI-agnostic and reusable by later one-shot headless, machine/SDK, TUI, report, and execution milestones.
- [ ] Ensure `ResolvedHarnessPlan` contains the resolved Agent/Loop/package graph, resolved Harness config + source metadata, workspace/state paths, runtime scope state, consumer-context readiness, service/provider definitions, static capability candidates/readiness, and diagnostics needed by later milestones.
- [ ] Keep mutable RunState out of `ResolvedHarnessPlan`; this milestone produces immutable-ish bootstrap/session planning data only.

- [ ] Add tests for installed-Agent-only execution roots, local Agent roots, explicit Agent selection, zero/one/multiple runnable Agents, missing Loop, bad binding phase, missing bound package, bad Memory selector, Skill Tool inheritance/de-duplication, invalid MCP export Tool membership, missing generated Knowledge/Memory metadata, unsafe consumer context, missing optional context, unresolved scopes, irrelevant Knowledge/Memory config package mappings, MCP import phase-scope validation, state-dir separation, and static-vs-pending capability readiness.
- [ ] Add integration coverage proving Milestone 1 config-local validation and Milestone 2 Agent-aware validation remain separate—for example, a syntactically/semantically valid MCP phase scope passes config loading but produces a preflight diagnostic when that phase does not exist in the selected Loop.

## Milestone 3: Stable Events, Trace Sink, and JSON Run Report Foundation
> Scope note: establish the Harness observability and accounting contracts before complex execution is added. Define canonical event envelopes/taxonomy, event fan-out, durable JSONL traces, Run reports, redaction/content policy, Session usage aggregation, and the minimal Session/Run identities needed by those contracts. This milestone does not yet traverse a Loop, call a model, execute actions, resolve approvals, or implement Tool/Knowledge/Memory/MCP behavior.
- [ ] Add versioned Harness event-envelope types with:
    - stable event type,
    - event ID,
    - session ID,
    - optional run ID,
    - monotonically increasing session sequence,
    - optional monotonically increasing run-local sequence,
    - timestamp,
    - optional phase execution ID,
    - optional correlation ID and parent event ID,
    - and typed payload support.
- [ ] Use session sequence for deterministic ordering across the complete Session event stream, including bootstrap/service events that occur before a Run exists; reset only the optional run-local sequence for each Run.
- [ ] Treat correlation IDs as operation/request grouping rather than ordering authority; use parent event IDs where a direct causal parent is useful.
- [ ] Implement the minimum stable version-1 semantic event taxonomy from `spec.md`, covering:
    - Session/service/preflight lifecycle,
    - Run/phase lifecycle,
    - model requests/turns and semantic-action proposals/results,
    - outcome/transition selection,
    - Tool and Skill-resource activity,
    - Knowledge activity,
    - Memory access/lifecycle activity,
    - Hook and approval/control activity,
    - MCP import/export activity,
    - consumer-context loading,
    - cancellation,
    - terminal state,
    - and usage updates.
- [ ] Give canonical version-1 events typed payload contracts containing the stable semantic fields required by `spec.md`; allow later milestones to add more granular events without replacing or overloading the canonical event meanings.
- [ ] Keep events as immutable observability facts, not the authoritative source of RunState and not a requirement to rebuild RunState through event replay.

- [ ] Add one central event emitter/fan-out abstraction used by later CLI/TUI renderers, machine protocol subscribers, trace sinks, and report/accounting observers.
- [ ] Emit a canonical internal event before sink-specific filtering; trace level determines which otherwise-valid events a sink retains/displays rather than changing Harness execution or event semantics.
- [ ] Implement trace level exactly as `minimal | normal | verbose`.
- [ ] Implement trace content policy exactly as `none | redacted | full`.
- [ ] Keep event selection and content exposure separate: trace level controls event granularity while content policy controls whether eligible content-bearing fields are omitted, redacted, or retained.
- [ ] Make secret redaction unconditional regardless of trace level or content policy; `full` must never mean raw secret emission.
- [ ] Apply the same redaction/content rules consistently to JSONL traces, machine-protocol event delivery, TUI/event rendering data, and Run-report embedded summaries where applicable.

- [ ] Create `.agentpm-state/runs/<run-id>/events.jsonl` by default when tracing is enabled.
- [ ] Write JSONL incrementally so a crash/failure does not require the entire Run to finish before useful trace data exists.
- [ ] Keep trace output ordered according to the canonical event sequence and ensure each JSONL record is independently parseable.

- [ ] Add a versioned `RunReport` model and default `.agentpm-state/runs/<run-id>/report.json` path.
- [ ] Include at least:
    - Harness/report version,
    - session/run IDs,
    - Agent and Loop identities,
    - relevant preflight/runtime-source metadata,
    - start/end timestamps and duration,
    - terminal status,
    - warnings/diagnostics,
    - ordered phase summaries,
    - PhaseResult/outcome/transition summaries,
    - usage,
    - action summaries,
    - retry/error counts,
    - approval/cancellation summary where applicable,
    - and trace-file reference.
- [ ] Keep the Run report a compact execution summary derived from authoritative Harness state/accounting; do not require consumers to replay `events.jsonl` to understand the Run result.
- [ ] Add explicit report-path/export override while retaining default state-directory report generation.
- [ ] Ensure partial, failed, cancelled, `limit_reached`, and otherwise non-successful Runs can still flush a syntactically valid report and trace containing all data available up to termination.

- [ ] Add `RunUsage` and in-memory `SessionUsage` accounting models.
- [ ] Define `SessionUsage` as the aggregation of completed/current Runs within one Harness Session, including:
    - Run count,
    - model-call count,
    - provider-reported input/output/total tokens,
    - accepted semantic actions,
    - logical Tool calls and retries,
    - useful Knowledge/Memory/embedding request counts,
    - duration,
    - and provider-reported or otherwise authoritative cost when available.
- [ ] Never fabricate token or cost values when a provider/runtime does not supply enough information; preserve unknown/unavailable values explicitly.
- [ ] Keep per-Run usage independently available even while Session totals accumulate across repeated Runs.
- [ ] Add `session_usage_updated` and terminal Session usage-summary exposure; do not require a separate durable Session report in Phase 7B.

- [ ] Add only the minimal Session/Run lifecycle primitives required to allocate stable IDs, attach events/usage to their owning Session/Run, open/flush sinks, and finalize reports; leave operational `RunState` and Loop traversal to Milestone 4.
- [ ] Provide synthetic/test lifecycle helpers so event, trace, report, and usage behavior can be exercised without a real ModelRuntime or HarnessEngine.

- [ ] Add tests for:
    - session and run sequence monotonicity,
    - sequence reset/isolation between Runs,
    - event IDs and correlation/parent relationships,
    - canonical event serialization,
    - trace-level filtering,
    - content-policy behavior,
    - unconditional secret redaction,
    - incremental JSONL writing,
    - RunReport serialization,
    - SessionUsage aggregation across multiple synthetic Runs,
    - unknown token/cost handling,
    - explicit output paths,
    - state-directory creation,
    - and failure-safe trace/report flush.

## Milestone 4: Core HarnessEngine, Agentic Inner Loop, and Loop Traversal with Fake Runtimes
> Scope note: implement the UI-agnostic Harness execution state machine using deterministic fake/test runtime boundaries. Prove Run creation, phase-local agentic execution, Loop traversal, ordered checkpoints, outcomes, transitions, safety limits, retry/error-policy plumbing, terminal behavior, Session reuse, and the single-writer RunState invariant before real model providers or real Tool/Knowledge/Memory/MCP implementations are introduced. Fake runtimes return canned normalized responses/results through the same interfaces later real implementations will use.

- [ ] Add operational `HarnessSession`, `RunContext`, `RunState`, `PhaseExecutionState`, `PhaseResult`, pending approval/control state, runtime terminal result/status, and phase execution ID models.
- [ ] Reuse Milestone 3 Session/Run IDs, events, reports, and usage accounting rather than introducing parallel lifecycle/accounting paths.
- [ ] Enforce the single-writer invariant: only `HarnessEngine` mutates authoritative `RunState`; runtimes/controllers return normalized results, proposals, or decisions for the Engine to validate and apply.
- [ ] Support multiple sequential Runs inside one `HarnessSession`; create fresh RunState, Loop-step state, phase transcripts, PhaseResults, and per-Run usage for each Run while preserving Session-level services/identity/usage aggregation.

- [ ] Add the normalized model-facing execution contracts used by both fake and later real ModelRuntime implementations:
    - `ModelRequest`,
    - `ModelTurn`,
    - assistant content,
    - ordered semantic action proposals,
    - structured repair feedback,
    - and provider-independent usage metadata.
- [ ] Add the normalized semantic Harness action model required by the phase inner loop, including at least:
    - AgentPM Tool call,
    - external MCP Tool call,
    - Skill resource read,
    - Knowledge request,
    - Memory read,
    - Memory write,
    - and PhaseCompletion.
- [ ] Keep semantic action identity independent from provider-native function/tool-call syntax; provider adapters added later must translate to/from these Harness-owned contracts rather than allowing provider response shapes to drive Engine behavior.
- [ ] Add a fake/scripted `ModelRuntime` capable of returning assistant content, one or more ordered semantic action proposals, explicit/implicit completion, malformed completion/outcome data, usage, and runtime failures.
- [ ] Add a fake/scripted action dispatcher/runtime boundary that can return deterministic structured action success/failure results without implementing real Tool, Skill, Knowledge, Memory, or MCP behavior.
- [ ] Keep the fake action path executor-neutral so later capability milestones replace the fake dispatcher with real routing rather than rewriting the inner-loop state machine.

- [ ] Implement the canonical phase-local agentic inner loop:
    - assemble normalized `ModelRequest`,
    - call `ModelRuntime`,
    - receive normalized `ModelTurn`,
    - record assistant content,
    - validate ordered semantic action proposals,
    - execute accepted non-terminal actions through the action-dispatch boundary,
    - append structured action results to the phase-local transcript,
    - call the model again,
    - and continue until valid PhaseCompletion, implicit completion, failure, cancellation, or safety-limit exhaustion.
- [ ] Do not model one phase execution as one model response; allow multiple model/action cycles within a single phase execution.
- [ ] Keep raw phase working transcripts phase-local. Cross-phase execution context flows through Run input, stable Run-level context, and `PhaseResult` rather than automatically forwarding one provider-native conversation transcript across the whole Loop.
- [ ] Build the canonical logical `ModelRequest` structure from `spec.md` even though most capability layers are empty/fake in this milestone, so later Profiles, Skills, consumer context, Knowledge, Memory, and Tools populate existing prompt/request layers rather than changing the request architecture.
- [ ] Process multiple non-terminal actions from one normalized `ModelTurn` deterministically in provider-preserved order.
- [ ] Do not execute actions in parallel in Phase 7B unless a later explicit spec change adds that behavior.
- [ ] Reject and issue structured repair feedback for ambiguous turns that combine PhaseCompletion with executable actions rather than guessing whether completion happens before or after those actions.
- [ ] Treat malformed/rejected proposals that never become accepted semantic actions as repairable model output rather than executed action failures.

- [ ] Execute `entry_phase`, phase completion, transition lookup, phase re-entry, and terminal targets strictly from the resolved Loop graph.
- [ ] Treat phase IDs as graph/runtime identity only, `phase.objective` as model-facing guidance, and `archetype` as descriptive metadata with no Engine branch.
- [ ] Implement generic EffectivePhase/access-policy inputs needed by later capability milestones.
- [ ] Enforce Loop access tri-state semantics:
    - `false` prohibits the corresponding semantic capability,
    - `true` permits an otherwise available capability but creates none,
    - omitted means the Loop expresses no opinion.
- [ ] Keep semantic capability gates distinct: provider-native "tool calling" must not cause Tool access policy to gate Skill-resource, Knowledge, or Memory actions.

- [ ] Treat one phase execution as one Loop step.
- [ ] Consume the Loop step only when the phase is actually entered; model calls, accepted semantic actions, retries, repairs, and approval checkpoints do not independently increment Loop steps.
- [ ] Enforce effective max steps as the stricter of authored Loop `limits.max_steps` and the Harness runtime safety ceiling.
- [ ] Return runtime terminal status `limit_reached` rather than authored `$abort` when the effective step ceiling is exhausted.

- [ ] Implement ordered Loop approval checkpoints before guarded phase entry using a fake/scripted approval decision boundary.
- [ ] Evaluate all checkpoints targeting the phase in authored `loop.checkpoints` array order.
- [ ] On approval, continue to the next checkpoint without consuming a Loop step.
- [ ] On the first rejection, stop evaluating later checkpoints, follow that checkpoint's `on_reject` target, and do not consume the guarded phase's Loop step.
- [ ] If an approval cannot be resolved by the current controller/test boundary, represent the Run as pending/`approval_required` according to the control contract rather than treating it as a rejection.
- [ ] Do not implement TUI prompts, SDK approval callbacks, or external ApprovalRuntime protocols yet; later milestones connect those surfaces to this same Engine checkpoint mechanism.

- [ ] Implement implicit `complete` for phases with omitted authored outcomes.
- [ ] For phases with explicit outcomes, require the selected outcome ID to match an authored outcome exactly.
- [ ] Add bounded structured repair when the model proposes an invalid/missing explicit outcome.
- [ ] Treat repair requests as ModelRuntime calls and account for them against the appropriate model-call/repair safety limits.
- [ ] Produce a first-class `PhaseResult` containing phase/execution identity, selected/implicit outcome, output content, usage contribution, and structured metadata required for later cross-phase prompt assembly.

- [ ] Implement deterministic transition selection from `(phase, outcome)` and reject ambiguous/missing graph interpretation rather than inventing transitions.
- [ ] Implement `$end`, `$abort`, and `$handoff` terminal semantics:
    - `$end` returns a successful completed result using the final PhaseResult/output by default,
    - `$abort` returns authored aborted state,
    - `$handoff` returns explicit handoff/control context and does not automatically invoke another Agent.
- [ ] Preserve runtime failures/cancellation/`limit_reached` as distinct terminal states from authored `$abort`.

- [ ] Add generic action failure and retry plumbing needed by later AgentPM Tool and external MCP Tool executors.
- [ ] Implement default Tool-failure -> phase failure and phase-failure -> runtime failed behavior when the Loop omits corresponding error policy; actual Tool execution remains fake in this milestone.
- [ ] Implement Loop Tool retry-policy counters/actions in an executor-neutral form.
- [ ] Define `max_retries` as additional attempts after the initial failed attempt (`2` means at most `3` total execution attempts).
- [ ] Treat retries as repeated attempts of the same finalized logical Tool call; retry attempts do not create new semantic actions.
- [ ] Keep a model-proposed new Tool call with changed arguments as a new logical action rather than a retry.

- [ ] Implement phase safety counters and enforcement using the exact semantics from `spec.md`.
- [ ] Define `max_actions_per_phase` as the count of accepted semantic action proposals in that phase execution: AgentPM Tool calls, external MCP Tool calls, Skill resource reads, Knowledge requests, Memory reads, Memory writes, and PhaseCompletion.
- [ ] Do not count model calls, Hook calls, approval checks, internal Memory lifecycle operations, embedding requests, retry attempts, or malformed proposals rejected before acceptance as separate semantic actions.
- [ ] Define `max_model_calls_per_phase` as ModelRuntime requests attributable to the phase, including structured/outcome repair requests.
- [ ] Define `max_tool_calls_per_phase` as accepted logical AgentPM/external-MCP Tool calls; execution retries of the same logical Tool call do not increment that logical-call count.
- [ ] Emit an explicit runtime limit/failure result and events when any phase safety limit is exceeded rather than silently truncating work.

- [ ] Emit the canonical Milestone 3 lifecycle, phase, model, action, approval, outcome, transition, limit, failure, terminal, and usage events as the fake execution proceeds.
- [ ] Populate/flush Milestone 3 Run reports from actual Engine state for completed, aborted, handed-off, failed, approval-required, cancelled/test-cancelled, and limit-reached Runs.
- [ ] Aggregate each Run's usage into SessionUsage and verify multiple Runs in one Session accumulate Session totals while resetting per-Run counters/state.

- [ ] Add unit/integration tests for:
    - multiple Runs in one Session,
    - cycles and phase re-entry,
    - phase-local transcript isolation,
    - multi-turn phases,
    - multiple ordered actions in one model turn,
    - ambiguous completion + action repair,
    - accepted-action/model-call/Tool-call limit accounting,
    - ordered multiple approval checkpoints,
    - approval rejection and unresolved approval,
    - implicit and explicit outcomes,
    - invalid-outcome repair and exhaustion,
    - all authored/runtime terminal states,
    - max-step exhaustion,
    - Tool retry counting (`max_retries + initial attempt`),
    - default failure behavior,
    - authored error policy,
    - single-writer RunState enforcement,
    - event/report integration,
    - and SessionUsage aggregation.

## Milestone 5: ModelRuntime, Prompt Assembly, OpenAI, Anthropic, and Ollama
> Scope note: add real model execution and normalized semantic turn/action plumbing, including the three required built-in providers. Complete a text-only multi-phase Harness Run in headless mode before Tool/Knowledge/Memory capabilities are added.
- [ ] Add `ModelRuntime` trait/interface and the normalized `ModelRequest`/`ModelTurn` contract from `spec.md`, including assistant content, ordered semantic action proposals, provider usage, finish metadata, canonical action identities, and provider-safe alias mapping.
- [ ] Make the ModelRuntime boundary explicit: Engine creates ModelRequest; provider adapter translates it; provider response is normalized to ModelTurn; the model only proposes actions; Engine validates/dispatches them and appends structured results before the next model call. No Engine code should parse provider-native response shapes.
- [ ] Add live selected-model capability advertisement (`semantic_actions`, `structured_output`, `multimodal_input`, optional `context_window_tokens`, `usage_reporting`) for built-in/custom providers and use it for readiness/compatibility diagnostics rather than a closed model catalog.
- [ ] Keep model provider IDs and concrete model IDs runtime strings; do not add a closed model enum.
- [ ] Implement built-in OpenAI provider using current supported API patterns in the repository/ecosystem at implementation time.
- [ ] Implement built-in Anthropic provider with equivalent normalized behavior.
- [ ] Implement built-in Ollama provider as the required local/open provider path.
- [ ] Resolve standard provider credentials/endpoints from scoped environment/config without serializing secrets.
- [ ] Support custom model provider IDs backed by configured process/SDK host provider contracts without requiring those custom implementations in this milestone.
- [ ] Add provider capability detection/adaptation sufficient to reject unsupported required structured-action behavior clearly.
- [ ] Add provider-safe temporary function/tool aliases while retaining canonical Harness identities internally.
- [ ] Add phase prompt assembler using the canonical logical structure from `spec.md`: immutable Harness control/outcome contract -> authored phase/Profile/Skill behavior -> Run input + Consumer Context -> prior PhaseResults -> Effective capability catalog -> current phase-local transcript. Keep provider message-role translation inside ModelRuntime.
- [ ] Represent effective Tool/MCP/Skill-resource/Knowledge/Memory/PhaseCompletion capabilities as Harness semantic action descriptors; provider-native function/tool definitions are only transport and must map back to canonical action identities.
- [ ] Keep Knowledge/Memory/Tool/Skill-resource contents on-demand and append their structured results to the current phase transcript rather than eager prompt injection.
- [ ] Apply `before_model_request` only after canonical prompt assembly and before provider translation; never allow it to remove Harness control authority, alter phase/outcome IDs, add action descriptors, or mutate EffectivePhase.
- [ ] Keep raw provider transcripts phase-local and start a fresh provider context on phase re-entry/new phases.
- [ ] Account for provider usage/tokens when available.
- [ ] Add **one-shot plain headless** input/output flow: one Session + exactly one Run from CLI text/stdin/file input, final/handoff user-facing output on stdout, diagnostics on stderr/report, report/trace flush, deterministic service shutdown, and no dependency on the machine protocol.
- [ ] Define/test headless terminal exit behavior consistent with `spec.md`: `ended`/`handed_off` successful; `aborted`/`failed`/`cancelled`/`limit_reached`/`approval_required` non-success according to CLI conventions.
- [ ] Return actionable missing-provider/model/scope errors in non-interactive headless mode rather than silently choosing values.
- [ ] In interactive-capable bootstrap, represent missing model/provider as promptable requirements for the future TUI rather than choosing silently.
- [ ] Add provider contract tests with mocked HTTP/process transports and optional real-provider smoke tests gated by environment variables.
- [ ] Add one representative three-phase text-only end-to-end test that runs against fake provider fixtures through the real ModelRuntime interface.

## Milestone 6: EffectivePhase, Profiles, and Consumer Context
> Scope note: compute real effective phase composition for non-executable behavioral/context surfaces. Add Profile composition and per-Run consumer-context snapshotting before Tools/Skills/Knowledge/Memory actions are layered in.
- [ ] Add `EffectivePhase` model including authored candidates, runtime augmentation placeholders, Loop access decisions, runtime readiness, suppression reasons, and deterministic ordering.
- [ ] Compute global + phase Profile bindings additively and de-dupe by package identity.
- [ ] Load Profile structured metadata once during bootstrap and reuse immutable resolved data.
- [ ] Serialize every authored behavioral Profile section present (identity, objectives/principles, audience, communication/formatting/vocabulary, boundaries, constraints) as model-facing input, and serialize multiple Profiles as distinct inputs in deterministic global-then-phase authored order; do not merge/override into a synthetic Profile.
- [ ] Treat required/preferred Profile constraints as different prompt-strength guidance only and preserve stable constraint IDs in prompt/trace metadata where practical.
- [ ] Evaluate Profile compatibility as advisory readiness diagnostics/warnings, never as hard behavioral enforcement; distinguish stronger `requires` mismatch warnings from softer `recommends` hints where the schema provides them.
- [ ] Interpret Profile capability-hint booleans correctly: `true` asserts/recommends the capability while `false` is absence of that positive requirement/recommendation, never a prohibition such as “Tool use must be disabled.”
- [ ] Implement `compatibility.minimum_context_tokens` exactly as advisory model context-window capacity: compare to ModelRuntime `context_window_tokens` only when reliably known, warn strongly when known-insufficient, report unverified when unknown, never reserve tokens/block/suppress, and evaluate multiple Profiles independently.
- [ ] Warn on obvious mechanical Profile conflicts where practical but never invent identity/persona/semantic precedence to resolve them.
- [ ] Snapshot `bindings.consumer_context.file` once at Run start and reuse that exact snapshot across all phase executions in the Run.
- [ ] Reload consumer context on the next Run within the same Session.
- [ ] Make missing/unreadable consumer context non-fatal and evented.
- [ ] Include context path, byte size, approximate token count, hash/status in preflight/report according to trace policy.
- [ ] Ensure consumer context cannot alter Loop/capability authority even when its text contains instructions requesting it.
- [ ] Add events for effective-phase computation, Profile activation, consumer-context loaded/unavailable, and suppression reasons.
- [ ] Add tests for global/phase Profile ordering, duplicates, conflicting Profiles without invented resolution, compatibility warnings, missing context, mid-Run file edits not affecting snapshot, and next-Run reload.

## Milestone 7: `agentpm run` Machine Contract and Tool Runner Hardening
> Scope note: strengthen the public Tool execution surface before the Harness depends on it. This milestone does not yet let the Harness model call Tools; it makes `agentpm run` authoritative, structured, schema-safe, runtime-version-aware, and cancellation-safe for later ToolRuntime use and third-party consumers.
- [ ] Add a stable `agentpm run` machine mode with versioned JSON success/failure envelope and stable runner error category.
- [ ] Preserve human CLI output outside machine mode.
- [ ] Validate JSON input against Tool `inputs` schema before launching the Tool.
- [ ] Validate parsed Tool output against Tool `outputs` schema before returning success.
- [ ] Preserve current requirement that Tool stdout resolve to valid JSON according to existing runner behavior.
- [ ] Enforce declared runtime minimum versions (Node/Python/etc.) rather than only invoking `--version`/checking executable presence.
- [ ] Keep interpreter override conventions such as `AGENTPM_NODE`/`AGENTPM_PYTHON` where already supported.
- [ ] Preserve existing environment-default/required-variable semantics and avoid indiscriminate secret injection.
- [ ] Preserve timeout behavior and Unix process-group cleanup.
- [ ] Harden signal/cancellation propagation so termination of `agentpm run` cannot orphan the Tool child/process group.
- [ ] Keep failure artifacts/diagnostics compatible where possible while machine output remains parseable and non-human-dependent.
- [ ] Add tests for input-schema failure, output-schema failure, runtime-version mismatch, environment failure, timeout, subprocess failure, malformed output, machine serialization, signals/cancellation, and existing human output regression.

## Milestone 8: Harness ToolRuntime and Skill Progressive Disclosure
> Scope note: add the first real model-executable capabilities. Direct AgentPM Tools execute only through public `agentpm run --machine`; bound Skills contribute progressive resources and inherited Tools. Loop Tool access, repair, retry/error policy, and Tool events become live.
- [ ] Add Harness `ToolRuntime` implementation that spawns public `agentpm run --machine` using JSON stdin.
- [ ] Build model-facing AgentPM Tool descriptors from canonical identity, description, and input schema; do not expose secrets/execution internals.
- [ ] Do not infer destructive/read/write safety policy from arbitrary Tool action names or schema fields; use authored Loop/binding/checkpoint policy and Hooks instead.
- [ ] Keep Tool entrypoint/files/cwd/timeout/environment/interpreter execution semantics owned by public `agentpm run`; Harness may inspect readiness but must not implement a second private runner.
- [ ] Perform early Harness Tool-argument schema validation before invoking ToolRuntime.
- [ ] Add bounded model argument repair using `max_tool_call_repairs`.
- [ ] Revalidate arguments after any later Hook modifications.
- [ ] Map ToolRuntime failures into Loop Tool failure policy without parsing human strings.
- [ ] Retry as fresh `agentpm run` invocations with the same finalized arguments.
- [ ] Suppress known runtime-incompatible Tools during EffectivePhase computation with reasons.
- [ ] Warn but do not necessarily suppress solely for missing required env at preflight; actual `agentpm run` invocation remains authoritative.
- [ ] Add Skill activation descriptors/inventory without eagerly loading full `SKILL.md`/references; keep multiple active Skills distinct and ordered rather than merging them into one synthetic Skill.
- [ ] Add model semantic action for authorized Skill entrypoint/reference read, keep loaded resources phase/Skill-scoped, and do not let previously loaded Skill content leak automatically into later phase contexts.
- [ ] Resolve/canonicalize Skill resource paths within installed Skill root and reject escapes/symlink escapes.
- [ ] Expand bound Skill Tool dependencies into the Skill's global/phase binding scope without requiring duplicate direct Agent Tool bindings.
- [ ] De-dupe same-scope direct + inherited Tool identity and emit composition warning.
- [ ] Never auto-execute Skill scripts or infer an interpreter/executor from file extension; script execution requires an independently authorized Tool.
- [ ] Emit Skill activation/resource events but do not add a mutable `before_skill_activation` Hook; activation remains authored composition.
- [ ] Treat Skill compatibility metadata as advisory warnings only; never use it to grant authority or silently suppress an otherwise valid Skill.
- [ ] Enforce Loop `access.tools` over direct and inherited Tools while Skill resource reads remain distinct from Tool calls.
- [ ] Emit Tool candidate/selection/invocation/retry/result/failure and Skill resource events.
- [ ] Add end-to-end phase tests with two Tools, one Tool-backed Skill, Tool-disabled phase, invalid arguments, retry exhaustion, runtime suppression, and Skill resource access.

## Milestone 9: HookRuntime, ApprovalRuntime, Machine Control Protocol, and Cancellation
> Scope note: establish the persistent bidirectional Harness protocol and typed interception/control contracts before language SDK wrappers are added. Implement prompt/Tool Hooks plus the separate ApprovalRuntime first; later capability milestones add Knowledge/Memory Hook points on the same contract.
- [ ] Define the versioned `agentpm harness --machine` JSONL envelope from `spec.md` with protocol/version/kind/correlation IDs and typed request/response/event/error payloads; human output must never share stdout with protocol frames.
- [ ] Implement machine message families for Session initialization/host registration, start Run, event streaming, Run/preflight/terminal responses, cancellation, external Memory-operation control, shutdown, and correlated host-service request/response dispatch.
- [ ] Add the common AgentPM process-service JSONL envelope/initialize handshake from `spec.md` for model, embedding, Hook, Knowledge, Memory, and approval roles; process stdout is protocol-only and diagnostics use stderr.
- [ ] Require role-specific live capability advertisement during initialization and validate configured role/protocol/readiness before marking a service ready. Host implementations over the SDK machine protocol must advertise the same semantic capabilities as process implementations.
- [ ] Implement the minimum role method contracts from `spec.md`: model `generate`; embedding `embed`; Hook `invoke`; Knowledge `retrieve`; Memory primitive record/retrieval/count/operation-state/batch methods; approval `request_approval`. MCP and ToolRuntime keep their separate public protocol boundaries.
- [ ] Implement common managed-process lifecycle states/events and defaults from `spec.md`: startup/handshake readiness, request timeout, one default restart for subsequent requests, no automatic replay of the failed in-flight request, exhausted-service unavailable state, and clean Session shutdown.
- [ ] Keep event messages distinct from control requests and service/provider requests.
- [ ] Add correlation IDs and bounded request timeouts where configured.
- [ ] Add `HookRuntime` with the exact version-1 Hook IDs from `spec.md`, ordered config/SDK registrations, and **closed per-Hook request/response patch schemas** rather than generic JSON merge-patch. Implement the allowed contracts for prompt shaping, Tool candidate subsetting/reordering, Tool argument patch/reject, Knowledge request/retrieval shaping, Memory read/write shaping, and Memory-operation allow/reject/model-guidance.
- [ ] Invoke a Hook only when its corresponding authorized decision/action is actually eligible; candidate/suppression events may still be emitted when no actionable capability exists, but Hooks must never be called as a capability-manufacturing escape hatch.
- [ ] Implement prompt/context-shaping Hook before model request.
- [ ] Implement Tool candidate/selection influence hook where applicable without granting new capabilities.
- [ ] Implement before-Tool-call argument shaping/rejection hook followed by schema revalidation.
- [ ] Make configured intercepting hook failure fail closed by default.
- [ ] Apply `hooks.bindings[].failure_policy` exactly as `closed | continue`, default `closed`, preserving binding-array order and validating/applying each successful patch before invoking the next binding.
- [ ] Prevent hooks from altering graph, checkpoints, Loop access, runtime limits, arbitrary RunState, or Memory scope authority.
- [ ] Add `ApprovalRuntime` separate from HookRuntime; approval callbacks must never be registered or reported as Hook IDs.
- [ ] Support optional configured process/host Approval controller using the shared implementation descriptor and precedence rules from `spec.md`.
- [ ] Implement machine approval request/approve/deny control messages.
- [ ] Implement deterministic multiple-checkpoint evaluation in authored order.
- [ ] For plain headless with no approval controller, terminate with `approval_required` rather than auto-approving/denying.
- [ ] Support optional controller approval timeout and classify timeout as runtime/control failure, not rejection.
- [ ] Add first-class cancellation control message and graceful shutdown path that flushes trace/report and stops child services/tools.
- [ ] Add canonical external Memory-operation control message placeholder for later Memory milestone.
- [ ] Add tests for protocol negotiation, malformed messages, event/control multiplexing, hook patch validation, fail-closed/open behavior, approval/rejection/multiple checkpoints, approval timeout, and cancellation.

## Milestone 10: Node SDK Harness, Hooks, and Host Provider APIs
> Scope note: make Harness use first-class and ergonomic from Node without duplicating orchestration. The SDK wraps `agentpm harness --machine`, exposes typed events/results/control, and lets users register callbacks/providers as normal TypeScript functions.
- [ ] Add public `Harness`/`HarnessClient` API using existing Node SDK subprocess/location conventions.
- [ ] Add typed session/run/preflight/config override/result/report/event models matching the machine protocol.
- [ ] Spawn and manage `agentpm harness --machine` without exposing JSON-line framing to application authors.
- [ ] Add async event iteration/subscription APIs.
- [ ] Add typed first-class registration APIs for every version-1 Hook ID; users register callbacks/functions without declaring protocol frames or correlation IDs.
- [ ] Preserve Hook registration order and automatically advertise host Hook capabilities to Harness; SDK registrations execute after workspace-configured Hook bindings as specified.
- [ ] Add typed approval callback registration.
- [ ] Add cancellation API.
- [ ] Add typed external Memory-operation invocation API placeholder that becomes live with Memory runtime milestones.
- [ ] Add typed host-provider registration contracts for custom model, embedding, Knowledge, and Memory providers; requests not yet supported by Rust services may be explicitly gated until their milestone rather than silently ignored.
- [ ] Ensure callbacks execute on the application side while Harness remains authoritative for validation/state.
- [ ] Surface subprocess/protocol errors with typed categories and include final report path/result.
- [ ] Export all Harness/Hook/provider types from public SDK entrypoint.
- [ ] Add tests with a fake Harness machine subprocess for events, Hooks, approvals, cancellation, provider requests, malformed protocol, process exit, and cleanup.
- [ ] Add one real CLI integration test proving Node can execute a fake-provider Harness Run and supply a Tool/prompt Hook without implementing transport code.

## Milestone 11: Python SDK Harness, Hooks, and Host Provider APIs
> Scope note: provide Python parity with Milestone 10 using language-idiomatic async/sync patterns while keeping the Rust Harness as the only orchestration engine.
- [ ] Add public Harness client abstraction using existing Python SDK CLI subprocess/location conventions.
- [ ] Add typed/dataclass/Pydantic-equivalent session/run/preflight/config/result/report/event models according to repository style.
- [ ] Add async event streaming/subscription.
- [ ] Add first-class registration APIs for every version-1 Hook ID plus separate approval callbacks; hide protocol/correlation plumbing and preserve registration order.
- [ ] Automatically advertise host Hook/provider capabilities to Harness and keep SDK Hook registrations ordered after workspace-configured bindings.
- [ ] Add cancellation API.
- [ ] Add external Memory-operation invocation API placeholder for later Memory support.
- [ ] Add typed custom model, embedding, Knowledge, and Memory host-provider contracts.
- [ ] Hide framing/correlation/process lifecycle from normal users.
- [ ] Preserve Harness authority/validation and avoid implementing Loop traversal in Python.
- [ ] Export public Harness/Hook/provider APIs.
- [ ] Add fake-process protocol tests equivalent to Node coverage.
- [ ] Add one real CLI integration test proving Python can execute a fake-provider Harness Run and supply Hook/approval callbacks.
- [ ] Verify Node/Python event and provider field semantics stay aligned.

## Milestone 12: Local KnowledgeRuntime and EmbeddingProvider Contract
> Scope note: add on-demand context/vector Knowledge to the Harness using installed Knowledge packages and existing public AgentPM query machinery. Introduce typed embedding-provider fallback but defer full Pinecone/pgvector runtimes to the next milestone.
- [ ] Add `KnowledgeRuntime` interface and normalized Knowledge request/result models using the runtime action contract from `spec.md` (`initialize/attest` + `retrieve`), including exact authorized package/version identity and normalized source/chunk/citation metadata.
- [ ] Add KnowledgeRuntime live capability/readiness advertisement for supported modes/features and mapped package/version/corpus realization attestations; an explicitly mapped runtime that cannot attest the expected package/corpus is unavailable rather than trusted implicitly.
- [ ] Add semantic model action for Knowledge access distinct from Tool calls.
- [ ] Enforce Loop `access.knowledge` independently from `access.tools`.
- [ ] Keep bound Knowledge packages distinct model surfaces rather than auto-federating them.
- [ ] For context Knowledge, expose compact package/document descriptors initially and load only the requested declared document; treat document `role` as a discovery/reasoning hint rather than eager-load behavior.
- [ ] Resolve package-owned Knowledge paths relative to installed package root and reject traversal/symlink escapes.
- [ ] For vector Knowledge, load/validate installed build/index/provenance metadata needed for compatibility/readiness.
- [ ] Treat packaged retrieval strategy/`top_k`/score threshold/citation settings as retrieval defaults or hints rather than Loop orchestration; allow authorized request/Hook shaping within runtime constraints.
- [ ] Keep retrieval citation/provenance output separate from final-answer formatting; `return_citations` does not force the model's final response to cite sources.
- [ ] Reuse public `agentpm knowledge query` behavior/machinery when it can satisfy the request rather than reimplementing search privately in Harness.
- [ ] Add a machine/query interface if existing public Knowledge query output is insufficient for Harness-safe structured consumption.
- [ ] Add typed `EmbeddingProvider` service request/response contract with provider/model/dimensions/normalization/text and returned numeric vector, plus live advertisement of the embedding-space tuples/patterns the provider can satisfy.
- [ ] Resolve `knowledge.embedding_matches` by exact provider/model/dimensions/normalized tuple when local query needs a compatible query vector; reject ambiguous/duplicate matches rather than choosing heuristically.
- [ ] Validate returned vector dimensions/numeric finiteness/declared compatibility before local retrieval.
- [ ] Suppress a vector Knowledge surface when neither local query nor a compatible embedding provider/custom runtime can realize it.
- [ ] Add before-Knowledge-request and after-retrieval Hook points using the existing HookRuntime contract; revalidate package/scope/options after Hook changes.
- [ ] Emit Knowledge availability/request/retrieval/citation/failure events with content governed by trace policy.
- [ ] Add tests for context progressive loading, vector local query, embedding fallback, incompatible vectors, unavailable suppression, Loop access, Hook query shaping/reranking, citations, and backend failure returned to phase.

## Milestone 13: Pinecone and pgvector Knowledge Providers + SDK Provider Helpers
> Scope note: prove the full custom KnowledgeRuntime extension path with two real external providers and expose provider implementations/helpers through the SDK ecosystem so consumers can copy/extend them without writing protocol boilerplate.
- [ ] Implement a Pinecone KnowledgeRuntime provider that accepts normalized AgentPM Knowledge requests and returns normalized results.
- [ ] Implement a pgvector KnowledgeRuntime provider with equivalent normalized semantics.
- [ ] Keep external index provisioning/upsert/synchronization outside Harness runtime execution.
- [ ] Add explicit `knowledge.packages` runtime mapping from package identity to configured provider/runtime ID.
- [ ] Validate package/version/corpus/hash identity against provider-advertised metadata where available; reject mismatched realization rather than serving unrelated data.
- [ ] Do not silently fall back to local Knowledge when an explicitly mapped provider is unavailable/mismatched.
- [ ] Add provider capability/handshake reporting and preflight readiness diagnostics.
- [ ] Keep provider credentials process/application-side and out of Harness event payloads.
- [ ] Add first-class Pinecone and pgvector provider helpers/adapters to Node and Python SDKs using optional dependencies/extras according to each SDK's packaging conventions.
- [ ] Make SDK provider adapters implement the same host-provider contract exposed in Milestones 10/11.
- [ ] Add runnable provider bridge/example code so CLI-only workspaces can launch an SDK-backed provider process through config without writing framing code.
- [ ] Add mocked provider tests for query/options/result mapping, metadata filters, citations/source IDs, service errors, and identity mismatch.
- [ ] Add optional real-provider integration tests gated by environment/config rather than required for offline unit suites.
- [ ] Add one end-to-end Harness test mapping one Knowledge package to Pinecone/pgvector fixture while another package uses local runtime.

## Milestone 14: Built-In SQLite MemoryRuntime and Direct Memory Access
> Scope note: implement persistent local Memory Blueprint realization, generated-contract enforcement, trusted scopes, direct document/collection/sequence access, retention/capacity, and capability advertisement. Lifecycle operations/triggers come in the next milestone.
- [ ] Add `MemoryRuntime` interface and normalized primitive contracts from `spec.md`: direct create/upsert/update/delete/archive, retrieval, scoped count/capacity, deterministic sequence allocation, durable operation-state load/store, and atomic batch/transaction capability. Keep Blueprint trigger/lifecycle interpretation in Harness.
- [ ] Implement built-in SQLite runtime at default `.agentpm-state/memory.sqlite3`.
- [ ] Add local store schema versioning/migration mechanism.
- [ ] Implement the version-1 SQLite schema from `spec.md`: `memory_meta`, `memory_records`, `memory_sequence_state`, `memory_operation_state`, and `memory_vectors`, including required indexes/uniqueness and schema-version migration handling.
- [ ] Store canonical lexicographically-keyed compact `scope_json` plus `sha256:<hex>` `scope_hash` exactly as defined in `spec.md`; verify hash/content agreement and never let model-supplied content choose scope.
- [ ] Resolve arbitrary Blueprint scope keys from trusted RunContext/config/SDK/CLI inputs.
- [ ] Load generated build/contract index/contracts from package-root-relative paths in the exact installed Memory package and validate runtime records against generated envelope contracts; never place live records/state beside those package files.
- [ ] Accept model-proposed record `content` only and construct IDs, scope, timestamps, schema version, ordinal, expiration, and provenance in Harness/MemoryRuntime.
- [ ] Implement document one-current-record semantics per complete scope tuple.
- [ ] Implement collection create/read/update/delete by ID/filter according to declared constraints/retrieval modes.
- [ ] Implement sequence append/chronological retrieval with `memory_sequence_state`, zero-based never-reused ordinals, and ordinal reservation/insertion in one transaction.
- [ ] Enforce `append_only` for direct model mutations.
- [ ] Implement `key`, `filter`, `chronological`, and practical local `full_text` retrieval where declared.
- [ ] Implement local `semantic` retrieval only when `memory.local.semantic` resolves a ready configured EmbeddingProvider/model/dimensions; store little-endian f32 vectors/content hashes and use exact cosine search in Rust; otherwise do not advertise semantic. **Do not require sqlite-vec or any SQLite vector extension** for Phase 7B correctness/tests; an extension may only be a future optional optimization.
- [ ] Add the normalized MemoryRuntime capability descriptor from `spec.md` (`space_models`, `retrieval_modes`, `retention_actions`, `constraints`, `capacity`, `durable_trigger_state`, `atomic_batches`) and compare live resolved capabilities to each Blueprint space/operation during preflight.
- [ ] Suppress direct Memory spaces the selected runtime cannot faithfully realize.
- [ ] Enforce Loop `memory.read/write` for direct model access only.
- [ ] Implement TTL anchor `(updated_at ?? created_at) + ttl`, lazy expiration enforcement, delete/archive actions, and active-record filtering.
- [ ] Enforce capacity per complete resolved scope tuple.
- [ ] Enforce `x-agentpm-persist:false` before durable commit.
- [ ] Preserve `x-agentpm-shareable` metadata for later export/share semantics without hiding it from normal authorized reads.
- [ ] Add before-Memory-read/write Hooks and revalidate after Hook changes.
- [ ] Add events for Memory readiness, direct reads/writes, validation failures, retention/capacity actions, and suppressions.
- [ ] Add restart persistence tests, arbitrary-scope tests, contract-validation tests, all three space models, append-only, TTL/archive/delete, capacity, retrieval readiness, semantic-provider availability, and `.agentpm` immutability.

## Milestone 15: Memory Lifecycle Operations, Durable Trigger State, and External Invocation
> Scope note: complete the canonical Harness interpretation of Memory Blueprint operations and triggers using persistent MemoryRuntime state and ModelRuntime-assisted transform/consolidate execution.
- [ ] Load participating global/phase Memory operation bindings separately from direct space bindings.
- [ ] Allow operations to access their declared internal source/output/target spaces even when those spaces are not directly model-bound in the current phase.
- [ ] Implement persistent trigger-state read/write through `memory_operation_state` (or equivalent provider state API).
- [ ] Implement `record_count` edge-trigger/re-arm semantics from `spec.md`.
- [ ] Implement capacity edge-trigger/re-arm semantics and pre-overflow eligible operation handling.
- [ ] Implement interval baseline starting when relevant scoped Memory first exists, dormant-empty semantics, successful-completion-based next eligibility, and persistence across Harness restarts.
- [ ] Evaluate relevant automatic operation eligibility immediately after Memory state changes, including mid-phase.
- [ ] Implement transform as one output per active scoped source record.
- [ ] Support `output_mode=create` and `replace_input`, including append-only lifecycle exception for explicit replace operation.
- [ ] Implement consolidate over active scoped input records with one destination output.
- [ ] Implement delete operation mechanically without ModelRuntime generation.
- [ ] For transform/consolidate, call ModelRuntime with operation description, authorized source content, target content schema, and lifecycle control instructions; require target content only.
- [ ] Count lifecycle ModelRuntime calls in provider usage/token totals and Harness model-call safety accounting, emit them distinctly in trace/report, and never count them as phase turns or Loop steps.
- [ ] Validate generated target content and perform bounded `max_memory_operation_repairs` structured repair.
- [ ] Construct provenance from operation/source record IDs and enforce `preserve_provenance`/source-handling semantics.
- [ ] Apply `retain`, `retain_until_expiration`, and `delete_after_success` consistently; for the built-in SQLite runtime, record/output/source/trigger-state mutations belonging to one lifecycle operation must commit or roll back together.
- [ ] Add `memory_operation_eligible/started/completed/failed` and detailed source/output events.
- [ ] Add before-Memory-operation Hook without allowing Hook to rewrite trigger/input/output/targets/source handling/scope authority.
- [ ] Implement canonical Engine `invoke_memory_operation` for `trigger.type=external`.
- [ ] Route machine protocol control request, later TUI control, and SDK APIs through the same Engine path.
- [ ] Reject external invocation of non-external, unbound/non-participating, unresolved-scope, or backend-unready operations.
- [ ] Add tests for trigger persistence/re-arm, 12th-record mid-phase consolidation, interval across process restarts, transform replace/create, source handling, provenance, operation failures, external invocation, Loop memory access not blocking internal operations, and transactional consistency.

## Milestone 16: PostgreSQL/pgvector and Redis Memory Providers + SDK Helpers
> Scope note: prove Memory Blueprint portability beyond SQLite with two external backends that advertise capabilities honestly. Keep the Harness operation scheduler/Blueprint semantics canonical rather than delegating semantic meaning to providers.
- [ ] Implement PostgreSQL/pgvector MemoryRuntime provider covering document/collection/sequence, scope partitioning, retrieval, retention, capacity, durable operation state, and semantic retrieval where configured.
- [ ] Implement Redis/Redis Stack MemoryRuntime provider with equivalent supported semantics and explicit unsupported-capability reporting where necessary.
- [ ] Keep lifecycle operation trigger interpretation/execution in Harness; providers expose primitive persistence/retrieval/trigger-state services rather than redefining operations.
- [ ] Add explicit `memory.packages` package-to-runtime mapping and configured runtime definitions.
- [ ] Do not silently fall back to SQLite when an explicitly mapped external Memory runtime is unavailable.
- [ ] Make external providers advertise the same normalized MemoryRuntime capability descriptor; add readiness diagnostics and suppress only unsupported spaces/operations where safe instead of pretending unsupported semantics exist.
- [ ] Add Node and Python SDK provider adapters/helpers for PostgreSQL/pgvector and Redis using optional dependencies/extras.
- [ ] Add runnable provider bridges/examples compatible with workspace process provider configuration.
- [ ] Keep backend credentials provider-side/scoped and out of events.
- [ ] Add mocked provider contract tests plus optional live integration suites gated by environment.
- [ ] Add one cross-backend conformance suite running the same representative Blueprint semantics against SQLite and external provider fixtures.

## Milestone 17: MCP Export and `agentpm serve --mcp` Machine Lifecycle
> Scope note: realize Agent-authored `bindings.mcp` as outward AgentPM MCP server surfaces. Preserve current shared-runner implementation while adding machine readiness/events and Harness Session lifecycle management.
- [ ] Add machine mode to `agentpm serve --mcp` with a documented versioned protocol envelope and structured handshake/ready/shutdown/error/event messages; Harness must not parse human stderr/stdout text to determine readiness or call activity.
- [ ] Support `--port 0` and return the actual bound endpoint in machine readiness.
- [ ] Keep default host loopback and let Harness choose ephemeral ports for managed surfaces.
- [ ] Keep existing `serve --mcp` Tool invocation through shared runner code; do not spawn public `agentpm run` per MCP request.
- [ ] Add machine events for MCP Tool call started/completed/failed with canonical AgentPM identity and MCP-normalized name.
- [ ] Preserve MCP protocol behavior and human serve output outside machine mode.
- [ ] Add Harness `McpRuntime` export lifecycle that starts one `agentpm serve --mcp --machine` subprocess per authored MCP binding ID.
- [ ] Pass exactly the bound top-level Agent Tools for that logical surface.
- [ ] Treat phase-binding and MCP-export binding of the same Tool as valid/non-redundant because they expose different surfaces; allow MCP-only exported Tools without making them phase capabilities.
- [ ] Keep exported MCP surfaces Session-owned and callable even when no Harness Run is currently active.
- [ ] Validate MCP-safe normalized name collisions during preflight/startup.
- [ ] Suppress known runtime-incompatible Tools from managed MCP exposure; realize ready subset with strong warnings when non-empty and mark empty surface unavailable.
- [ ] Keep outward calls independent from active Run phase hooks/access/checkpoints.
- [ ] Emit Harness surface start/ready/activity/failure/stop events and include endpoint/tool mapping in TUI/report.
- [ ] Ensure Session shutdown/cancellation terminates all owned MCP server processes cleanly.
- [ ] Apply the managed-process lifecycle policy to Harness-owned `serve --mcp` children: an in-flight call is never replayed; default one restart may restore the surface for later calls; exhausted restart marks the surface unavailable.
- [ ] Add tests for multiple surfaces, ephemeral ports, Tool filtering, collisions, partial readiness, call events, process failure/restart-without-replay, and cleanup.

## Milestone 18: External MCP Import and Runtime Tool Augmentation
> Scope note: let runtime config add environment-specific MCP functionality to a published Agent. Imported Tools become normal phase Tool capabilities only in explicitly configured scope and are governed by the Harness Tool pipeline.
- [ ] Implement the exact config-v1 `mcp.imports` union from `spec.md`: `transport: stdio | http`; stdio uses direct command/args/cwd/env/timeouts/restart, HTTP uses an absolute URL and `{value}|{env}` header references.
- [ ] Require every import to declare `scope.mode: global | phases`; `global` forbids `phases`, while `phases` requires a non-empty unique phase list and becomes unavailable if Agent-aware preflight leaves no valid phases.
- [ ] Support optional allowed Tool-name filter; if omitted, expose all advertised Tools within the explicitly configured scope.
- [ ] Start/connect external MCP servers at Session bootstrap, initialize protocol, discover Tool descriptors, and validate configured filters.
- [ ] Assign canonical internal identities such as `mcp:<server-id>/<tool-name>` and provider-safe aliases separately.
- [ ] Add imported MCP Tools as runtime augmentation candidates in EffectivePhase.
- [ ] Apply Loop `access.tools`, Tool selection/before-call Hooks, input-schema validation, retry/error policy, phase-local result handling, and Tool events uniformly to imported MCP Tools.
- [ ] Classify valid external MCP invocation/protocol failures as Loop Tool failures.
- [ ] Keep external MCP server lifecycle Session-owned; distinguish owned stdio processes from remote connections.
- [ ] Surface discovered/exposed/suppressed Tools and phase scope in preflight/report/TUI data.
- [ ] Add tests for explicit scoping, Tool filters, duplicate Tool names across servers, provider alias mapping, Loop Tool-disabled phase, Hook-modified arguments, Tool retry, server disconnect, session cleanup, and no mutation of Agent manifest.

## Milestone 19: Ratatui Harness TUI, Interactive Approvals, and Branding
> Scope note: build a focused TUI client over existing engine/events/control. Do not move orchestration logic into UI code and do not build arbitrary theming/plugin systems.
- [ ] Add Ratatui frontend that starts early enough to display bootstrap/preflight progress.
- [ ] Add start screen showing Agent/Loop, model/provider/source, consumer context, Profiles/Skills/Tools, Knowledge, Memory, Hooks, outward/inward MCP, warnings, and readiness.
- [ ] Clearly show loaded/unavailable Consumer Context with path, size, and approximate tokens.
- [ ] Show capability suppression/readiness reasons without overwhelming the default view.
- [ ] Add Agent selection prompt when multiple runnable Agents exist and no selector was supplied.
- [ ] Add provider/model prompt when required values are missing.
- [ ] Add trusted scope-value prompts for unresolved required Memory scopes where interactive resolution is appropriate.
- [ ] Add run view centered on current phase/objective, recent model/action activity, selected outcome/transition, and terminal result.
- [ ] Add a clearly visible message composer as the primary user input surface when the Session is ready for a new Run; submitting a message starts that Run, while active-run state exposes working/cancel/approval affordances instead of accepting ambiguous mid-Run chat input.
- [ ] Show the latest assistant/PhaseResult user-facing output prominently in the Run column so the TUI is an agent interaction surface first and an observability dashboard around it, not only a debugger.
- [ ] Show both current-Run usage and cumulative Session usage where space permits, with unknown token/cost values shown as unknown rather than estimated.
- [ ] Add interactive approval checkpoint view and approve/deny controls routed through ApprovalRuntime.
- [ ] Add cancellation/quit behavior through the canonical Engine cancellation path.
- [ ] Add expandable/toggleable views for prompts, Tool args/results, Knowledge results, Memory events, Hook decisions, MCP activity, and raw events subject to trace policy.
- [ ] Add repeated Run support within one Session and ensure Consumer Context reloads on each new Run.
- [ ] Show report/trace paths after/between Runs.
- [ ] Implement lightweight config branding: name, optional subtitle, optional hex accent.
- [ ] Keep protocol/event/report identifiers canonical AgentPM values regardless of branding.
- [ ] Add TUI state/component tests where practical and manual verification for resize/small-terminal/error/loading/approval/cancel/repeated-run paths.

## Milestone 20: Templates, Examples, Documentation, End-to-End Hardening, and Release Verification
> Scope note: close Phase 7B by proving the complete runtime in realistic workspaces, teaching adoption through Templates, documenting public protocols/configuration, and running cross-repository regression/conformance suites. Do not add new architecture unless required to satisfy the existing spec.
- [ ] Create/update a minimal Harness Template/workspace that runs a published Agent with near-zero config and documents provider/model setup.
- [ ] Create/update an SDK-hosted Harness example showing first-class Node or Python Hooks, event streaming, approvals, cancellation, and report access.
- [ ] Create/update a custom-provider example showing an EmbeddingProvider and one external Knowledge/Memory runtime.
- [ ] Create/update an MCP example showing both Agent-authored outward surfaces and explicitly scoped external MCP import.
- [ ] Create/update a full reference Harness example exercising a 3+ phase Loop, 2+ Tools, 2+ Skills, Profiles, context/vector Knowledge, Memory spaces/operations, consumer context, approvals, hooks, tracing, reports, and TUI.
- [ ] Ensure generated Template README copy teaches `Agent artifacts = portable definition`, `agentpm.harness.json = workspace runtime realization`, and `agentpm harness = reference executor`.
- [ ] Document/prove that Template dependencies do not become Harness bindings, Template entrypoint commands are never auto-executed by Harness, generated files become ordinary consumer-owned workspace inputs, and multi-Agent Template scaffolding still executes one selected Agent per Harness Run.
- [ ] Gitignore `.agentpm-state/` in generated Harness workspaces while documenting how to inspect/export reports and local Memory safely.
- [ ] Document `agentpm harness` modes/options, Agent selection, config precedence/defaults, runtime-state directory, safety limits, approvals, cancellation, and terminal statuses, including that Consumer Context is snapshotted once per Run and shaped only through normal prompt Hooks rather than a dedicated context-loading Hook.
- [ ] Publish the exact `agentpm.harness.json` version-1 reference from `spec.md`, including shared process/host descriptors, Hook implementations/bindings, Knowledge/Memory mappings, local Memory semantic config, stdio/HTTP MCP imports, approvals, lifecycle defaults, trace policy, and branding.
- [ ] Document the public machine protocol sufficiently for third-party clients/providers without requiring Node/Python SDKs.
- [ ] Document first-class Node/Python Harness and Hook/provider APIs with runnable examples.
- [ ] Document Pinecone/pgvector Knowledge and PostgreSQL/pgvector/Redis Memory provider setup boundaries, including that Harness does not provision/sync external indexes/stores.
- [ ] Document local SQLite schema/location/migration expectations and Memory inspection/export/shareable semantics.
- [ ] Document `agentpm run --machine` and `agentpm serve --mcp --machine` as public integration surfaces.
- [ ] Run end-to-end scenarios against OpenAI, Anthropic, and Ollama where credentials/runtime are available; use deterministic mocks for required automated CI coverage.
- [ ] Run the same representative Loop across **one-shot plain headless**, persistent machine/SDK, and interactive TUI paths and confirm identical HarnessEngine phase/outcome/runtime semantics modulo presentation/control transport.
- [ ] Verify one-shot headless specifically works from direct text, stdin, and input-file forms; creates one Session/Run; prints only the user-facing terminal result to stdout; routes diagnostics separately; writes trace/report; maps terminal states to documented exit behavior; and shuts down all owned services deterministically.
- [ ] Verify Node/Python SDK parity for Hooks/events/approvals/cancellation/providers/reports.
- [ ] Verify all required run reports/traces are generated and contain no secrets.
- [ ] Verify all existing package kinds, publish/install/new/build/query flows, registry/API/web behavior, and metadata-only SDK loaders remain compatible.
- [ ] Update version markers/release notes/docs according to repository conventions.
- [ ] Record exact verification evidence required by `test-plan.md`, including skipped external-provider tests and environment blockers.
