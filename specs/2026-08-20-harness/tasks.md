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

- [ ] Make execution surface part of provider/runtime readiness planning. For a configured `implementation.type = "host"`, classify readiness as pending host registration in machine/SDK mode, but unavailable in standalone TUI or one-shot headless mode because those surfaces have no external host implementation.

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

## Release Band 1: Contract, Config, and Preflight
Covered milestones: 1-2.
This gives us the Harness command shell, portable contract corrections, strict `agentpm.harness.json` validation, workspace discovery, lockfile-based Agent selection, cross-artifact preflight, and a structured resolved plan. It does not execute a Loop yet, but it gives authors early validation and makes runtime-readiness problems visible.
- [ ] After the Release Band 1 CLI/schema version is available, publish the updated `@zack/conversation-continuity` package so `refresh_saved_note` uses `output_mode: "replace_input"` in production examples.

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
- [ ] Enforce the single-active-Run invariant from `spec.md` at the Session/Engine level: reject an attempt to start a new Run while another Run is active (including while that Run is waiting on an approval checkpoint) without mutating the active Run's state, and only accept the next Run once the prior one reaches a terminal/runtime-terminal status.

- [ ] Emit events from Milestone 3 when applicable

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
    - rejecting a Run-start attempt while another Run is active, including while it is waiting on an approval checkpoint, without mutating the active Run,
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
> Scope note: replace the fake model side of Milestone 4 with real built-in ModelRuntime implementations and make the canonical prompt/action contract operational. Complete a text-only multi-phase Harness Run through one-shot plain headless mode before real Tool/Knowledge/Memory capability executors are added. Custom process/SDK-hosted model-provider configuration may already resolve in preflight, but its shared external-service transport becomes live in Milestone 9; this milestone must not invent a second provider protocol.
- [ ] Reuse the normalized `ModelRequest`, `ModelTurn`, semantic-action, usage, and repair contracts established in Milestone 4; do not introduce provider-specific Engine state/models.
- [ ] Expand Milestone 4's minimal `RunContext` and fake-runtime `ModelRequest` into the production immutable Run/model-request snapshots described in `spec.md`, sourced from `ResolvedHarnessPlan` and run inputs before provider prompt assembly. Include session/run IDs, workspace/state roots, resolved Agent/Loop/package graph, resolved runtime config/source metadata, runtime scope values, consumer-context snapshot, service/provider handles or readiness descriptors, hook registrations, run input, prior PhaseResults, EffectivePhase/capability catalog, phase-local transcript, repair feedback, and any remaining `ModelTurn` metadata needed for real provider responses/usage as applicable to this milestone.
- [ ] Expand Milestone 4's minimal `HarnessSession` into the production Session container for resolved plan state and Session-owned runtime resources. Preserve session ID, event sinks, cumulative usage, single-active-Run enforcement, and sequential Run reuse while adding immutable selected Agent/Loop/package/config context plus placeholders or handles for model/provider, Hook, Approval, Knowledge, Memory, MCP, trace, and report resources as they become live in later milestones.
- [ ] Add the production `ModelRuntime` boundary used by HarnessEngine: Engine creates `ModelRequest`; provider adapter translates it; provider response normalizes to `ModelTurn`; Engine alone validates/dispatches semantic actions and appends structured results before the next model call.
- [ ] Implement live selected-model capability advertisement equivalent to `semantic_actions`, `structured_output`, `multimodal_input`, optional `context_window_tokens`, and `usage_reporting`; keep capability reporting tied to the selected model/runtime rather than a closed AgentPM model catalog.
- [ ] Keep model provider IDs and concrete model IDs as open runtime strings. Reserve built-in provider IDs `openai`, `anthropic`, and `ollama` as defined by the config contract.
- [ ] Implement built-in OpenAI ModelRuntime using the current supported API pattern available at implementation time and normalize all responses through the common `ModelTurn` contract.
- [ ] Implement built-in Anthropic ModelRuntime with equivalent normalized semantics.
- [ ] Implement built-in Ollama/local HTTP ModelRuntime as the required free/local execution path.
- [ ] Resolve provider credentials/endpoints/options through scoped runtime configuration/environment without serializing secrets into prompts, events, reports, or provider diagnostics.
- [ ] Keep custom configured model-provider IDs represented in `ResolvedHarnessPlan`; until Milestone 9 activates process/host service transport, report those implementations as pending/unavailable rather than silently falling back to a built-in provider.
- [ ] Add provider capability/readiness checks sufficient to fail clearly when the selected model cannot support semantic actions or structured completion required by the current execution path; never rewrite Agent/Loop artifacts to compensate.
- [ ] Implement provider-safe temporary action/function aliases while preserving canonical AgentPM/MCP semantic identities in Engine state, events, Hooks, and reports.

- [ ] Implement the canonical six-layer logical prompt/request assembler from `spec.md`:
  - immutable Harness control/completion authority,
  - authored phase/Profile/Skill behavior,
  - Run input + Consumer Context,
  - prior PhaseResults,
  - Effective capability/action catalog,
  - current phase-local transcript.
- [ ] Keep provider message-role/function-schema translation entirely inside ModelRuntime adapters; provider limitations must not reorder or weaken the semantic authority of the canonical sections.
- [ ] Preserve the trust boundary in prompt serialization: Harness control remains authority; Profiles/Skills/phase objectives are authored guidance; Consumer Context/user input is contextual instruction; Tool/MCP/Knowledge/Memory results are lower-trust data and can never expand runtime authority merely because a provider serializes them into the same message stream.
- [ ] Keep Knowledge/Memory/Tool/Skill-resource content on-demand; only descriptors belong in the initial capability catalog until those actions actually return data.
- [ ] Add deterministic prompt/context budgeting behavior that never silently drops Harness control authority or authored required phase/Profile guidance; lower-priority truncation/summarization must produce diagnostics/verbose events according to `spec.md`.
- [ ] Preserve one canonical `before_model_request` interception seam after prompt assembly and before provider translation, but keep it inactive/no-op until HookRuntime is implemented in Milestone 9; do not create temporary Hook behavior in this milestone.
- [ ] Keep raw provider transcripts phase-local and start fresh provider context for a new phase execution/re-entry; cross-phase context comes from Run-level inputs and `PhaseResult` data.
- [ ] Record provider-reported token/usage metadata when available and preserve unknown values rather than fabricating them.

- [ ] Add **one-shot plain headless** execution: one Harness Session + exactly one Run from direct text, stdin, or input-file input, using the same HarnessEngine and final Run/report/event machinery as all later surfaces.
- [ ] Expose the Milestone 3 explicit report-path/export override on the one-shot headless CLI surface while retaining default `.agentpm-state/runs/<run-id>/report.json` generation.
- [ ] In one-shot plain headless mode, configured `type: host` implementations are unavailable because no machine/SDK host exists; return an actionable diagnostic rather than silently falling back.
- [ ] In plain headless mode, write only the user-facing terminal/final/handoff output to stdout; route diagnostics/status to stderr and durable report/trace output.
- [ ] Flush report/trace and deterministically shut down Session-owned resources before process exit.
- [ ] Use documented terminal behavior from `spec.md`: `ended` and `handed_off` are successful CLI outcomes; `aborted`, `failed`, `cancelled`, `limit_reached`, and `approval_required` are non-success outcomes according to repository CLI conventions.
- [ ] Preserve Milestone 4 approval semantics: a headless Run reaching a checkpoint without a controller terminates/returns `approval_required`; do not auto-approve or auto-deny.
- [ ] Return actionable missing-provider/model/scope requirements in non-interactive headless mode rather than silently choosing values.
- [ ] For future interactive mode, represent unresolved provider/model/scope inputs as promptable preflight requirements rather than resolving them arbitrarily here.

- [ ] Add provider contract tests using mocked transports/responses for action normalization, multiple ordered actions, structured completion, capability reporting, usage, alias mapping, malformed provider responses, and provider failures.
- [ ] Add optional real OpenAI/Anthropic/Ollama smoke tests gated by environment/runtime availability rather than required for offline CI.
- [ ] Add a representative three-phase text-only end-to-end test through the real HarnessEngine/ModelRuntime interface and one-shot headless path, including an approval-required case.

## Milestone 6: EffectivePhase, Profiles, and Consumer Context
> Scope note: make real phase composition operational for non-executable behavioral/context surfaces. Compute EffectivePhase on every phase entry, add Profile composition/compatibility, and snapshot/inject consumer-owned context once per Run. Tool/Skill-resource/Knowledge/Memory execution remains unavailable until later milestones, but their candidate/readiness slots must already fit the same EffectivePhase model.
- [ ] Finalize `EffectivePhase` as an ephemeral per-phase-entry model containing authored candidates, runtime augmentation candidates/placeholders, Loop access decisions, runtime readiness, effective/suppressed capabilities, suppression reasons, and deterministic ordering.
- [ ] Recompute EffectivePhase on every phase entry/re-entry from resolved immutable composition plus current runtime readiness; never persist it back into Agent/Loop manifests.
- [ ] Keep candidate composition, Loop restriction, and runtime readiness independently observable so diagnostics can explain whether a capability was absent, prohibited, unavailable, or pending.
- [ ] Do not expose capability kinds whose real executor is not implemented/ready yet merely because they appear in authored bindings.

- [ ] Compute global + phase Profile bindings additively, preserve authored order as global-then-phase, and de-dupe repeated package identity without treating ordering as precedence/override.
- [ ] Load resolved Profile metadata once during bootstrap/Session setup and reuse immutable data across Runs/phases.
- [ ] Serialize every authored behavioral Profile section present—identity, objectives/principles, audience, communication/formatting/vocabulary, boundaries, and constraints—as model-facing input; do not silently drop unsupported-looking sections.
- [ ] Serialize multiple Profiles as distinct labeled blocks; never merge them into one synthetic Profile/persona.
- [ ] Treat required/preferred Profile constraints as different prompt-strength guidance only; do not add fake post-response enforcement.
- [ ] Preserve stable Profile constraint IDs in prompt/event metadata where practical so traces can identify authored guidance.
- [ ] Evaluate Profile compatibility as advisory diagnostics only: `requires` mismatch warnings are stronger than `recommends` hints, but neither grants/suppresses runtime authority.
- [ ] Interpret capability-hint booleans exactly as positive assertions/hints when `true`; `false` must never be interpreted as a prohibition.
- [ ] Interpret `compatibility.minimum_context_tokens` as advisory selected-model context-window capacity: compare only when ModelRuntime reliably advertises `context_window_tokens`, warn strongly when known-insufficient, report unverified when unknown, and never reserve tokens/block/suppress the Profile.
- [ ] Warn on obvious mechanical Profile conflicts where practical but never invent identity/persona/semantic precedence to resolve them.

- [ ] Resolve and snapshot `bindings.consumer_context.file` exactly once at Run start after safe workspace-root canonicalization.
- [ ] Read the file into a Run-owned immutable snapshot and inject that exact snapshot into the canonical Consumer/Run Context section of every phase `ModelRequest` in that Run.
- [ ] Reload the consumer-context file for the next Run in the same Session; edits during an active Run must not affect that Run's snapshot.
- [ ] Keep missing/unreadable optional consumer context non-fatal and evented; invalid/escaping paths remain invalid.
- [ ] Record path, load status, byte size, approximate token count, and content hash in preflight/report subject to trace/content policy.
- [ ] Treat Consumer Context as model-visible context rather than Harness authority: its text cannot modify Loop graph/access/checkpoints, capability topology, trusted scopes, or runtime limits.
- [ ] Do not add a dedicated Consumer Context Hook; the normal `before_model_request` seam may shape assembled model-facing context once HookRuntime exists.

- [ ] Emit effective-phase/Profile/consumer-context activation, readiness, and suppression events using the Milestone 3 taxonomy.
- [ ] Add tests for per-entry EffectivePhase recomputation, global/phase Profile ordering/de-dupe, conflicting Profiles without invented resolution, capability-hint/minimum-context diagnostics, consumer-context prompt injection, missing context, mid-Run edit isolation, and next-Run reload.

## Release Band 2: Headless Loop Execution with Models
Covered milestones: 3-6.
This gives us stable events, traces, JSON run reports, the core HarnessEngine, fake-runtime execution coverage, real model adapters, one-shot headless execution, Profile composition, and Consumer Context snapshots. It proves the canonical runtime shape with text/model-only Runs before Tool, Knowledge, Memory, MCP, TUI, or SDK-hosted provider complexity is live.

## Milestone 7: `agentpm run` Machine Contract and Tool Runner Hardening
> Scope note: strengthen the existing public Tool execution surface before Harness depends on it. This milestone does not yet let the Harness model invoke Tools; it makes `agentpm run` authoritative, machine-readable, schema-safe, runtime-version-aware, and cancellation-safe for Harness and third-party consumers while preserving the shared internal runner used by other AgentPM surfaces.
- [ ] Add stable `agentpm run` machine mode with a versioned JSON success/failure envelope and stable runner error categories; machine stdout must contain only the documented machine result, with diagnostics on stderr.
- [ ] Preserve current human CLI behavior/output outside machine mode.
- [ ] Preserve JSON-stdin as the preferred machine invocation path while retaining existing supported human input forms where compatible.
- [ ] Validate Tool input JSON against the declared `inputs` schema before launching the Tool.
- [ ] Validate parsed successful Tool output against the declared `outputs` schema before returning machine success.
- [ ] Preserve the existing requirement that Tool stdout resolve to valid JSON.
- [ ] Treat schema-valid domain output such as `{ "ok": false }` as a successful Tool invocation unless the generic Tool output schema itself makes it invalid; do not infer domain failure from arbitrary fields.
- [ ] Enforce declared runtime minimum versions (Node/Python/etc.) rather than only checking executable presence or calling `--version`; produce a stable machine runtime-category failure when unsatisfied.
- [ ] Keep existing interpreter overrides such as `AGENTPM_NODE`/`AGENTPM_PYTHON` where supported.
- [ ] Preserve Tool environment defaults/required-variable semantics and avoid indiscriminate parent-env/secret injection.
- [ ] Preserve timeout/output-size/failure-artifact behavior where currently contractual and surface failures through stable machine categories.
- [ ] Preserve Unix process-group cleanup and harden signal/cancellation propagation so termination of `agentpm run` cannot orphan nested Tool children.
- [ ] Keep the shared internal Tool runner as the underlying implementation used by other AgentPM surfaces such as MCP serve; runner hardening should benefit those surfaces without forcing them to spawn `agentpm run` subprocesses internally.
- [ ] Keep human-facing diagnostics backward compatible where practical while ensuring Harness never needs to parse English error strings.

- [ ] Add tests for input/output schema failure, schema-valid domain failure output, runtime-version mismatch, environment failure, timeout, output limit, subprocess failure, malformed JSON output, machine serialization, stderr/stdout separation, signals/cancellation, and existing human-output regression.

## Milestone 8: Harness ToolRuntime and Skill Progressive Disclosure
> Scope note: replace the fake action dispatcher for AgentPM Tool and Skill-resource actions with real implementations. Direct AgentPM Tools execute only through public `agentpm run --machine`; bound Skills contribute progressive procedural resources and inherited Tools. Other semantic action kinds remain unavailable until their later runtime milestones rather than falling back to fake production behavior.
- [ ] Add Harness `ToolRuntime` that spawns public `agentpm run --machine` with finalized arguments over JSON stdin and consumes only the stable machine envelope/error categories.
- [ ] Route accepted `AgentPmTool` semantic actions from HarnessEngine to ToolRuntime; remove fake Tool execution from production paths while retaining deterministic fake runtimes for tests.
- [ ] Build model-facing AgentPM Tool descriptors from canonical package identity, description, and input schema only; never expose secrets, entrypoint internals, or environment values to the model.
- [ ] Replace the Milestone 5 placeholder provider-native `agentpm_tool` action schema with each resolved Tool's actual input schema; provider-native tool/function definitions and Harness pre-runtime validation must use the same authoritative schema.
- [ ] Keep provider-safe aliases separate from canonical Tool identity and map provider proposals back before Hooks/events/runtime execution.
- [ ] Do not infer destructive/read/write policy from arbitrary Tool action names/schema fields; use Agent bindings, Loop access/checkpoints, runtime readiness, and Hooks instead.
- [ ] Keep Tool entrypoint/files/cwd/timeout/environment/interpreter semantics owned by public `agentpm run`; Harness may inspect readiness but must not implement a second private runner.
- [ ] Perform early Harness argument-schema validation before ToolRuntime so malformed model arguments receive structured bounded repair rather than becoming Tool failures.
- [ ] Use `max_tool_call_repairs` as additional model repair attempts after the initial invalid proposal and revalidate any later Hook-modified arguments before ToolRuntime.
- [ ] Preserve the failure boundary: invalid/unauthorized proposals before ToolRuntime are model/action repair errors; failures while ToolRuntime attempts the invocation are Loop Tool failures.
- [ ] Map ToolRuntime machine failures into Loop Tool retry/error policy without parsing human strings.
- [ ] Retry as fresh `agentpm run` invocations with the same finalized arguments; a later model proposal with changed arguments is a new logical Tool action.
- [ ] Treat a schema-valid Tool result as phase-local Tool data and return it to the current phase transcript even when its domain content contains values such as `ok: false`.
- [ ] Populate `EffectivePhase` with ready/suppressed AgentPM Tool descriptors, preserving authored/global/phase/Skill-inherited ordering, Loop access decisions, runtime readiness, and explicit suppression reasons.
- [ ] Suppress known runtime-incompatible Tools during EffectivePhase computation with explicit reasons.
- [ ] Warn strongly but do not suppress solely for missing required Tool env during preflight; actual `agentpm run` invocation remains authoritative because runtime environment may change/be supplied.

- [ ] Add Skill activation descriptors containing compact manifest/name/description/resource inventory without eagerly injecting full `SKILL.md`, references, or scripts.
- [ ] Replace the Milestone 5 placeholder provider-native `skill_resource_read` action schema with an enum/list constrained to the active Skill's authorized resource IDs for the current phase.
- [ ] Route authorized Skill-resource semantic actions through a package-root-safe resource loader; support entrypoint/reference access on demand and keep resource content phase/Skill scoped.
- [ ] Resolve/canonicalize all Skill package-owned paths inside the exact installed Skill root and reject traversal/symlink escapes.
- [ ] Keep multiple active Skills distinct and deterministic rather than merging them into one synthetic procedural block.
- [ ] Expand each bound Skill's declared Tool dependencies into the Skill's global/phase binding scope without requiring duplicate direct Agent Tool bindings.
- [ ] Populate `EffectivePhase` with ready/suppressed Skill activation and Skill-resource descriptors, preserving authored ordering, resource readiness, and explicit suppression reasons.
- [ ] De-dupe same-scope direct + inherited Tool identity and emit a composition warning; do not treat global-direct + phase-inherited availability as inherently redundant.
- [ ] Never auto-execute Skill scripts or infer a script interpreter/executor from extension; scripts execute only through an independently authorized Tool capability.
- [ ] Emit Skill activation/resource events but do not add a mutable `before_skill_activation` Hook.
- [ ] Treat Skill compatibility metadata as advisory warnings only; it cannot grant/suppress authority.
- [ ] Enforce Loop `access.tools` over direct/inherited AgentPM Tool actions while Skill-resource reads remain a distinct semantic capability and are not gated by Tool access.
- [ ] Emit canonical Tool candidate/selection/invocation/retry/result/failure and Skill resource events, including source/inheritance/suppression reasons.

- [ ] Add end-to-end phase tests with two Tools, Tool-backed Skill inheritance, Tool-disabled phase, invalid-argument repair, runner failure/retry exhaustion, runtime suppression, domain-level `ok:false` result, Skill resource access, and no Skill-resource leakage across phases.

## Release Band 3: Tools and Skills
Covered milestones: 7-8.
This gives us the hardened public `agentpm run --machine` surface, real Harness ToolRuntime execution through that boundary, and Skill progressive disclosure/resource reads. At this point the Harness can run practical AgentPM Tool/Skill scenarios while still treating Knowledge, Memory, MCP, and external providers as unavailable or pending where applicable.

## Milestone 9: HookRuntime, ApprovalRuntime, Machine Control Protocol, External Service Transport, and Cancellation
> Scope note: establish the persistent bidirectional Harness machine protocol plus both runtime service transports used by custom providers, Hooks, and approvals: `agentpm-service` for Harness-owned process implementations, and the host-service request/response lane inside `agentpm-harness-machine` for SDK/application-hosted implementations. Activate prompt/Tool Hooks and approval/control against the existing Engine seams; later Knowledge/Memory milestones activate their Hook/action methods on the same semantic runtime contracts. This milestone must preserve one HarnessEngine and transport-independent semantic runtime interfaces.
- [ ] Define and implement versioned `agentpm harness --machine` JSONL framing/envelopes from `spec.md` with protocol version, message kind/type, correlation IDs, typed request/response/event/error payloads, and protocol-only stdout; diagnostics use stderr.
- [ ] Implement machine message families for Session initialization/host capability registration, preflight, start Run, event streaming, terminal/Run/report results, cancellation, external Memory-operation control placeholder, shutdown, and correlated host-service request/response dispatch.
- [ ] Apply the Milestone 3 trace content policy and unconditional secret-redaction rules to machine-protocol event delivery and terminal Run/report payloads; machine subscribers must not receive content that would be suppressed from traces under the same policy.
- [ ] Reject a `start_run` request received while the Session already has an active Run with a stable structured session-busy/active-Run error per `spec.md`; do not queue it and do not mutate the active Run.
- [ ] Keep machine events distinct from control requests and service/provider requests even though all share the same framed transport.

- [ ] Implement the common persistent AgentPM process-service JSONL protocol for configured `model`, `embedding`, `hook`, `knowledge`, `memory`, and `approval` roles.
- [ ] Keep `agentpm-service` separate from the Harness machine protocol: process implementations speak `agentpm-service`, host implementations speak the host-service lane of `agentpm-harness-machine`, and both carry the same typed semantic runtime requests/results.
- [ ] Require `initialize`/handshake with protocol version, role identity, implementation/service ID, and live role-specific capability advertisement before a service becomes ready.
- [ ] Implement minimum role methods from `spec.md`: model `generate`; embedding `embed`; Hook `invoke`; Knowledge `retrieve`/attestation; Memory primitive record/retrieval/count/operation-state/batch methods; approval `request_approval`.
- [ ] Use the same semantic role contracts for SDK-hosted implementations over the machine protocol; only transport differs.
- [ ] Audit and formalize provider/runtime threading now that persistent surfaces are live: the Milestone 5 OS-thread bridge is only for one-shot plain headless execution; machine/TUI/service-backed Runs need explicit async/blocking boundaries, cancellation propagation, event/trace visibility, and no dependence on the opaque one-shot headless worker path.
- [ ] Activate configured custom **process** ModelRuntime providers through this service protocol and prove they normalize into the same `ModelTurn` path as built-in providers.
- [ ] Support generic registered **host** service dispatch at the machine-protocol level so Node/Python SDKs can provide ergonomic wrappers in Milestones 10/11; do not require raw host callbacks to know Harness RunState internals.
- [ ] Add correlation IDs, configured request timeouts, cancellation propagation where meaningful, typed service errors, duration/error events, and protocol validation.
- [ ] Implement managed-process lifecycle from `spec.md`: starting -> handshaking -> ready -> unhealthy -> restarting -> ready|failed -> stopped; one default restart for subsequent requests, never replay the failed in-flight request, and clean Session shutdown.

- [ ] Add `HookRuntime` using the exact version-1 Hook IDs and **closed per-Hook request/response contracts** from `spec.md`; never use generic RunState/config merge patch.
- [ ] Invoke a Hook only when its corresponding authorized action/decision is eligible; Hooks must not manufacture capabilities.
- [ ] Make the existing `before_model_request` seam live after canonical prompt assembly and before ModelRuntime provider translation.
- [ ] Make the `before_tool_selection` Hook live after `EffectivePhase` computes ready Tool candidates and before the model-visible Tool/action catalog is finalized; it may only reorder/subset existing effective Tool IDs and must not add capabilities.
- [ ] Make the `before_tool_call` Hook live after the model proposes an authorized AgentPM Tool action and before `ToolRuntime` dispatch; it may patch arguments or reject the call without changing Tool identity.
- [ ] Revalidate every successful Hook patch before passing the updated safe snapshot to the next binding/runtime.
- [ ] Keep Knowledge/Memory Hook contracts registered/transportable now, but invoke their actual execution points only when those runtimes become live in Milestones 12/14/15.
- [ ] Apply ordered configured Hook bindings first, then SDK-hosted registrations in host registration order.
- [ ] Make Hook failure `closed` by default; `continue` records the failure and proceeds without applying an invalid/failed patch. Preserve binding order and never silently fail open.
- [ ] Prevent Hooks from altering Loop graph/transitions/access/checkpoints/limits, arbitrary RunState, capability topology, or trusted Memory scope authority.

- [ ] A `host` implementation becomes ready only after the machine client registers the matching `(role, configured registry ID)` and successfully advertises/negotiates its required capabilities; never silently substitute a built-in or process implementation when that host is absent.

- [ ] Add `ApprovalRuntime` as a separate semantic service; approval callbacks are not Hook IDs/events.
- [ ] Route Engine checkpoint requests through one ApprovalRuntime resolution path and preserve Milestone 4 authored-order checkpoint semantics.
- [ ] Implement controller precedence from `spec.md`: explicit per-run SDK/host callback when present -> configured approval controller -> future Ratatui built-in UI -> registered machine host controller where applicable -> plain headless `approval_required` when none can resolve.
- [ ] Support configured process/host Approval controllers and machine approve/deny responses.
- [ ] Apply configured approval timeout as runtime/control failure, never as authored denial/rejection.

- [ ] Add first-class cancellation control; propagate cancellation through HarnessEngine, active ModelRuntime/service requests, `agentpm run` ToolRuntime processes, and owned child services where meaningful.
- [ ] Verify cancellation of an in-flight Harness ToolRuntime terminates the spawned `agentpm run --machine` process, and that `agentpm run` still cleans up its nested Tool process group.
- [ ] Graceful cancellation must produce `cancelled`, flush report/trace, and shut down owned processes; hard kill remains fallback only.
- [ ] Add canonical external Memory-operation control request shape now, returning a clear not-yet-live/unavailable response until Milestone 15 wires it to Memory operations.

- [ ] Add tests for protocol negotiation/framing, no human stdout leakage, malformed/correlated messages, process and host role handshakes/capability advertisement, custom process ModelRuntime execution, service timeout/restart-without-replay, Hook ordering/patch validation/fail-closed-vs-continue behavior, approval precedence/rejection/multiple checkpoints/timeout, `start_run` rejection with a structured busy error while a Run is active, event-control multiplexing, cancellation propagation, and clean Session shutdown.

## Milestone 10: Node SDK Harness, Hooks, and Host Provider APIs
> Scope note: make the persistent machine Harness ergonomic and first-class from Node/TypeScript without duplicating orchestration. The SDK owns subprocess/protocol convenience, typed callbacks/providers, and application-side lifecycle while Rust Harness remains authoritative for Loop execution, validation, readiness, events, and reports.
- [ ] Add public `Harness`/`HarnessClient` API using existing Node SDK CLI-location/subprocess conventions.
- [ ] Add typed Session/Run/preflight/config override/scope override/result/report/usage/event/control models matching the machine protocol.
- [ ] Spawn/manage `agentpm harness --machine` and hide JSONL framing, correlation IDs, host registration, and subprocess cleanup from normal application authors.
- [ ] Add explicit Session lifecycle and `run(...)`/start-Run APIs supporting Run input plus per-run model/provider/scope/runtime overrides allowed by `spec.md`.
- [ ] Add async event iteration/subscription APIs plus current/final Run and cumulative Session usage access.
- [ ] Surface preflight requirements/diagnostics and final report/trace paths without requiring users to parse stderr or files manually.

- [ ] Add typed first-class registration APIs for every version-1 Hook ID; users register normal TypeScript callbacks/functions using Hook-specific request/result types.
- [ ] Preserve Hook registration order, automatically advertise host Hook capabilities, and execute SDK registrations after workspace-configured Hook bindings.
- [ ] Convert callback exceptions/timeouts into the documented Hook failure path rather than transport crashes.
- [ ] Add typed per-run/session approval callback registration using ApprovalRuntime semantics and precedence, separate from Hooks.
- [ ] Add cancellation API that maps to canonical machine cancellation and waits for terminal cleanup/result.
- [ ] Add typed external Memory-operation invocation API using the machine control contract; return not-yet-live/unavailable cleanly until Milestone 15 activates it.

- [ ] Add typed host-provider registration contracts for custom model, embedding, Knowledge, and Memory roles; SDK handles host capability advertisement/correlation/serialization automatically.
- [ ] Register host providers by `(service role, configured registry ID)` and require them to satisfy a resolved `type: host` implementation (or explicit per-run host override allowed by the config precedence contract); an SDK registration must not silently replace a configured process implementation.
- [ ] Make a registered host ModelRuntime fully usable now through Milestone 9 service dispatch and verify Harness remains authoritative for ModelTurn validation/action execution.
- [ ] Allow host provider methods whose Engine runtime is introduced later (Knowledge/Memory) to register early but fail/gate explicitly until that runtime milestone rather than being silently ignored.
- [ ] Preserve and expose host-service registration status in the SDK, including `active: false` plus an actionable reason for configured future roles whose Rust Engine dispatch is not live yet.
- [ ] Keep provider credentials/application state on the host side unless explicitly returned by the typed contract.
- [ ] Export all Harness/Hook/approval/provider types through public SDK entrypoints.

- [ ] Add fake-machine-process tests for Session lifecycle, Runs, events, usage, Hooks, approvals, cancellation, provider requests, malformed protocol, process exit, timeout, and cleanup.
- [ ] Add one real CLI integration test where Node launches Harness, supplies a host ModelRuntime plus prompt/Tool Hook and approval callback, executes a fake/simple Run, and receives terminal/report data without implementing transport framing.

## Milestone 11: Python SDK Harness, Hooks, and Host Provider APIs
> Scope note: provide Python parity with Milestone 10 using repository-idiomatic async/sync patterns while keeping Rust Harness as the only orchestration engine. Python users should implement Hooks/providers as ordinary callables/objects rather than process-protocol clients.
- [ ] Add public Harness client abstraction using existing Python SDK CLI-location/subprocess conventions.
- [ ] Add typed/dataclass/Pydantic-equivalent Session/Run/preflight/config override/scope override/result/report/usage/event/control models according to repository style.
- [ ] Spawn/manage `agentpm harness --machine` and hide JSONL framing, correlation IDs, host registration, and subprocess cleanup from normal users.
- [ ] Add repository-appropriate Session lifecycle and Run APIs supporting Run input plus allowed per-run model/provider/scope/runtime overrides.
- [ ] Add async event streaming/subscription and, if consistent with existing SDK style, a simple synchronous convenience facade without creating a second execution engine.
- [ ] Surface preflight diagnostics, current/final Run usage, cumulative Session usage, and report/trace paths as typed data.

- [ ] Add first-class typed registration APIs for every version-1 Hook ID; preserve registration order and advertise host Hook capabilities automatically after workspace-configured bindings.
- [ ] Convert callback exceptions/timeouts into documented Hook failure semantics rather than corrupting the protocol.
- [ ] Add separate typed approval callbacks using ApprovalRuntime precedence.
- [ ] Add cancellation API mapped to canonical machine control.
- [ ] Add external Memory-operation invocation API using the machine contract, with explicit unavailable behavior until Milestone 15.

- [ ] Add typed custom model, embedding, Knowledge, and Memory host-provider contracts and automatic capability advertisement/request dispatch.
- [ ] Register host providers by `(service role, configured registry ID)` and require them to satisfy a resolved `type: host` implementation (or explicit per-run host override allowed by config precedence); do not silently replace configured process implementations.
- [ ] Make a Python host ModelRuntime fully usable through the Rust Harness service dispatch now; keep Harness authoritative for validation/state/actions.
- [ ] Allow later Knowledge/Memory provider registrations to be gated explicitly until their Engine runtime is live rather than silently ignored.
- [ ] Preserve and expose host-service registration status in the SDK, including `active: false` plus an actionable reason for configured future roles whose Rust Engine dispatch is not live yet.
- [ ] Keep framing/correlation/process lifecycle and provider credentials/application state hidden from normal users.
- [ ] Export all public Harness/Hook/approval/provider APIs.
- [ ] Verify Node/Python field names, Hook/provider semantics, error categories, and registration precedence remain aligned.

- [ ] Add fake-process protocol tests equivalent to Node coverage and one real CLI integration test where Python supplies a host ModelRuntime plus Hook/approval callbacks and receives terminal/report results.

## Release Band 4: Machine Protocol and SDK Hosting
Covered milestones: 9-11.
This gives us the persistent machine protocol, HookRuntime, ApprovalRuntime, cancellation, external service transport, and first-class Node/Python Harness clients. SDK users can host Hooks, approvals, custom model providers, and other registered host services without implementing the wire protocol themselves.

## Milestone 12: Local KnowledgeRuntime, Generic Custom KnowledgeRuntime, and EmbeddingProvider Execution
> Scope note: make Knowledge semantic actions real. Add on-demand context/vector Knowledge using installed packages and existing public AgentPM query behavior, activate generic configured custom KnowledgeRuntime/EmbeddingProvider process-or-host services, and preserve explicit package/runtime routing. Pinecone and pgvector are reference provider implementations in the next milestone, not the point where the extension mechanism first appears.
- [ ] Add/activate the production `KnowledgeRuntime` boundary and normalized Knowledge request/result models from `spec.md`, including exact authorized package/version identity, context-document/vector-query intent, retrieval options, normalized source/chunk/citation metadata, and typed failures.
- [ ] Replace the Milestone 5 placeholder provider-native `knowledge_request` action schema with the finalized KnowledgeRuntime request contract, constrained to the bound package/surface and supported retrieval options.
- [ ] Route accepted Knowledge semantic actions from HarnessEngine to KnowledgeRuntime; remove fake Knowledge execution from production paths.
- [ ] Keep Knowledge semantic actions distinct from Tool actions and enforce Loop `access.knowledge` independently from `access.tools`.
- [ ] Keep bound Knowledge packages as distinct model-visible surfaces; never auto-federate all active packages into one search surface.
- [ ] Return successful Knowledge results/failures as structured phase-local transcript data for the next model turn.
- [ ] Populate `EffectivePhase` with ready/suppressed Knowledge surface descriptors, preserving bound package identity, authored ordering, Loop access decisions, runtime readiness, and explicit suppression reasons.

- [ ] Implement local **context** Knowledge readiness/descriptors with compact package/document inventory and on-demand loading of exactly one declared document.
- [ ] Treat document `role` as a discovery/reasoning hint only; do not infer eager loading or special authority.
- [ ] Resolve/canonicalize all package-owned Knowledge paths inside the exact installed package root and reject traversal/symlink escapes.
- [ ] Implement local **vector** Knowledge readiness from installed build/index/provenance metadata and validate index/corpus/vector compatibility before exposure.
- [ ] Treat packaged strategy/`top_k`/score threshold/citation settings as retrieval defaults/hints rather than Loop policy; authorized request/Hook shaping may narrow/adjust them within runtime constraints.
- [ ] Keep retrieval citation/provenance data separate from final-answer formatting; `return_citations` never forces the final model response to cite sources.
- [ ] Reuse public `agentpm knowledge query` behavior/machinery for local vector retrieval rather than adding a private Harness-only search implementation.
- [ ] Add/strengthen a machine-readable public Knowledge query surface if required so Harness consumes structured results/errors without parsing human output.

- [ ] Activate configured EmbeddingProvider process/host implementations through the Milestone 9 protocol with request tuple `provider/model/dimensions/normalized/text` and finite vector response.
- [ ] Require live EmbeddingProvider advertisement of compatible embedding-space tuples/patterns and validate the requested installed Knowledge embedding metadata against that advertisement.
- [ ] Apply the same readiness/capability enforcement to process and host EmbeddingProvider implementations: reject `ready:false`, provider/model/dimensions/normalization mismatches, malformed capability payloads, and unsupported requested embedding spaces before exposing or invoking the provider.
- [ ] Emit equivalent service health diagnostics/events for process and host EmbeddingProvider failures/timeouts; host transport errors must not disappear behind generic model/action failures.
- [ ] Resolve `knowledge.embedding_matches` by exact provider/model/dimensions/normalized tuple when local vector retrieval only lacks a query embedder; reject ambiguous matches and validate returned vector dimension/finiteness/normalization assumptions before search.

- [ ] Honor explicit `knowledge.packages` mappings now: initialize/attest the configured custom KnowledgeRuntime against the exact installed package/version/corpus identity, route retrieval to it when ready, and **never silently fall back** to local retrieval on mismatch/failure.
- [ ] For unmapped packages, follow the local-resolution order from `spec.md`: context/local query where possible -> compatible EmbeddingProvider fallback for query vector -> unavailable/suppressed when no realization exists.
- [ ] Require custom KnowledgeRuntime live capability/readiness advertisement for supported modes/features and package/corpus attestations; configuration existence alone is not readiness.
- [ ] Apply the same readiness/capability enforcement to process and host custom KnowledgeRuntime implementations: reject `ready:false`, registry/package/version/corpus attestation mismatches, malformed capability payloads, and unsupported Knowledge modes/features before exposing a surface.
- [ ] Emit equivalent service health diagnostics/events for process and host KnowledgeRuntime failures/timeouts, and keep explicit custom-runtime failures from silently falling back to local retrieval.
- [ ] Suppress a known-unrealizable Knowledge surface with a diagnostic rather than exposing a model action that cannot succeed.

- [ ] Make `before_knowledge_request` and `after_knowledge_retrieval` Hook points live through HookRuntime; Hooks remain confined to the already-authorized package/mode/options and result identities, and all changes are revalidated.
- [ ] When activating Knowledge/Memory Hook seams in Milestones 12/14/15, drain and emit queued nonfatal Hook failures before every terminal rejection/failure exit, including engine-side patch-validation failures.
- [ ] Preserve failure semantics: malformed/unauthorized model request -> bounded structured repair; valid backend/runtime failure -> structured Knowledge failure returned to the phase, not Loop Tool failure; repeated inability to complete may eventually become phase failure.
- [ ] Emit Knowledge surface/readiness/request/retrieval/citation/failure events with content governed by trace policy.

- [ ] Add tests for context progressive loading, undeclared-document rejection, local vector query, embedding fallback/capability mismatch, incompatible vectors, explicit custom runtime attestation/mismatch/no-fallback, unavailable suppression, distinct-package surfaces, Loop access, Hook shaping/reranking, citations, and backend failure returned to the phase.

## Milestone 13: Pinecone and pgvector Knowledge Reference Providers + SDK Helpers
> Scope note: prove the full custom KnowledgeRuntime path with two usable open-source reference integrations. Provider-specific logic belongs in provider bridges/SDK helpers speaking the public process/host contracts; do not add Pinecone/pgvector-specific branches to HarnessEngine or portable Knowledge artifacts.
- [ ] Implement a Pinecone KnowledgeRuntime reference provider that accepts normalized AgentPM Knowledge requests, advertises/attests the package/corpus it serves, and returns normalized AgentPM results.
- [ ] Implement a pgvector KnowledgeRuntime reference provider with equivalent normalized semantics.
- [ ] Build the reference providers on the public Milestone 9/10/11 service/provider contracts so third parties can copy/replace them without Harness-core changes.
- [ ] Keep external index provisioning, corpus upload/upsert, schema creation, and synchronization outside Harness runtime execution; the provider must assume the external realization is already prepared.
- [ ] Use the existing `knowledge.packages` mapping from Milestone 1/12; do not introduce a second provider-selection mechanism.
- [ ] Validate package/version/corpus/hash identity against provider metadata where available and reject mismatch rather than serving unrelated data.
- [ ] Preserve explicit-mapping no-fallback semantics.
- [ ] Keep provider credentials inside the provider process/application environment and out of Harness event/report payloads.
- [ ] Add first-class Pinecone and pgvector provider adapters/helpers to both Node and Python SDK ecosystems using optional dependencies/extras according to repository packaging conventions.
- [ ] Provide runnable process-bridge examples so CLI-only workspaces can launch the same SDK-backed provider implementation through `agentpm.harness.json` without hand-writing JSONL framing.
- [ ] Keep normalized result fields/source identities/citations consistent across Node/Python implementations.

- [ ] Add mocked provider tests for capability advertisement/attestation, query/options/result mapping, metadata filters, citations/source IDs, provider errors, and identity mismatch.
- [ ] Add optional real-provider integration tests gated by environment/configuration.
- [ ] Add end-to-end Harness coverage where one Knowledge package is explicitly mapped to a Pinecone/pgvector fixture and another package simultaneously uses local KnowledgeRuntime.

## Release Band 5: Knowledge Runtime and Reference Providers
Covered milestones: 12-13.
This gives us real Knowledge semantic actions, local context/vector retrieval, embedding-provider execution, explicit custom KnowledgeRuntime routing, and Pinecone/pgvector reference providers. Knowledge becomes runtime-usable while preserving the rule that explicit external mappings never silently fall back to local retrieval.

## Milestone 14: Built-In SQLite MemoryRuntime, Generic Custom MemoryRuntime, and Direct Memory Access
> Scope note: make direct Memory semantic actions real. Implement persistent local SQLite Memory Blueprint realization plus generic configured custom MemoryRuntime routing, generated-contract enforcement, trusted scopes, direct document/collection/sequence access, retention/capacity, semantic retrieval, and live capability advertisement. Lifecycle operations/triggers remain Milestone 15.
- [ ] Add/activate production `MemoryRuntime` boundary with normalized primitive contracts from `spec.md`: direct record mutation, retrieval, scoped counts/capacity, sequence allocation, durable operation-state access, and atomic batch capability.
- [ ] Replace the Milestone 5 placeholder provider-native `memory_read`/`memory_write` action schemas with bound-space-aware schemas; write schemas must use generated Memory content contracts for the selected package/space/record type.
- [ ] Route accepted MemoryRead/MemoryWrite semantic actions from HarnessEngine to MemoryRuntime; remove fake Memory execution from production paths.
- [ ] Keep Blueprint trigger/lifecycle meaning in Harness rather than delegating it to storage providers.
- [ ] Build model-facing direct Memory descriptors only for bound/ready spaces and declared record types/retrieval modes; Memory remains on-demand and is never eagerly dumped into the prompt.
- [ ] Populate `EffectivePhase` with ready/suppressed direct Memory read/write descriptors, preserving bound package/space/record-type identity, authored ordering, Loop access decisions, runtime readiness, and explicit suppression reasons.
- [ ] Enforce Loop `memory.read`/`memory.write` only over direct model-facing Memory actions.
- [ ] Return Memory read/write success/failure as structured phase-local transcript data; valid backend failures are Memory service failures, not Tool failures.

- [ ] Implement built-in SQLite runtime at default `.agentpm-state/memory.sqlite3`, physically separate from immutable installed `.agentpm` package state.
- [ ] Add local store schema versioning/migrations and implement the version-1 logical tables/keys/indexes from `spec.md`: `memory_meta`, `memory_records`, `memory_sequence_state`, `memory_operation_state`, and `memory_vectors`.
- [ ] Store canonical lexicographically-keyed compact `scope_json` plus exact `sha256:<hex>` `scope_hash`; verify hash/content agreement and never accept model-supplied scope serialization/hash as authority.
- [ ] Resolve arbitrary Blueprint scope keys from trusted RunContext/config/SDK/CLI inputs; unresolved required scopes make the relevant direct surface unavailable rather than inviting model invention.
- [ ] Load generated Memory build/contract index/contracts from package-root-relative paths in the exact installed package and validate build integrity before runtime use.
- [ ] Accept model-proposed record `content` only; validate content before persistence, construct runtime-owned ID/space/record type/scope/schema version/timestamps/expiration/ordinal/provenance, then validate the complete envelope against the generated contract.
- [ ] Treat invalid model-proposed Memory write content as bounded model/action repair before backend mutation; do not persist speculative invalid records.

- [ ] Implement `document` one-current-record create-or-replace semantics per complete scope tuple/record type.
- [ ] Implement `collection` create/read/update/delete/filter semantics according to declared constraints/retrieval modes.
- [ ] Implement `sequence` append/chronological retrieval with transactionally allocated zero-based never-reused ordinals through `memory_sequence_state`.
- [ ] Enforce `append_only` for ordinary direct model mutation while preserving later explicit lifecycle-operation exceptions.
- [ ] Implement `key`, `filter`, `chronological`, and practical local `full_text` retrieval only where declared by the Blueprint.

- [ ] Implement local `semantic` retrieval only when `memory.local.semantic` resolves a ready EmbeddingProvider/model/dimensions; otherwise omit `semantic` from live local capability advertisement.
- [ ] Store little-endian contiguous `f32` vectors and canonical content hashes in `memory_vectors`; do not require sqlite-vec or another SQLite vector extension.
- [ ] Generate/update/invalidate vectors transactionally with record mutation where part of the same semantic write; treat a content-hash mismatch as stale and regenerate through the configured EmbeddingProvider before returning that record from semantic retrieval.
- [ ] Perform exact cosine similarity in Rust for Phase 7B local Memory semantics.

- [ ] Advertise the normalized live MemoryRuntime capability descriptor from `spec.md` (`space_models`, `retrieval_modes`, `retention_actions`, `constraints`, `capacity`, `durable_trigger_state`, `atomic_batches`) containing only currently realizable capabilities.
- [ ] Compare selected runtime capabilities to every bound space and suppress direct spaces it cannot faithfully realize, with explicit readiness diagnostics.
- [ ] Honor explicit `memory.packages` mappings now: initialize the configured custom process/host MemoryRuntime, use its live capability descriptor, route direct operations to it, and never silently fall back to SQLite on failure/mismatch.
- [ ] Apply the same readiness/capability enforcement to process and host custom MemoryRuntime implementations: reject `ready:false`, registry/package/version/Blueprint realization mismatches, malformed capability payloads, and unsupported space/retrieval/write/batch requirements before exposing direct Memory surfaces.
- [ ] Emit equivalent service health diagnostics/events for process and host MemoryRuntime failures/timeouts, and keep explicit custom-runtime failures from silently falling back to SQLite.
- [ ] For unmapped packages, use the built-in SQLite runtime.

- [ ] Implement TTL anchor `(updated_at ?? created_at) + ttl`, lazy expiry enforcement, delete/archive retention actions, and exclusion of expired/archived records from active reads/counts/retrieval.
- [ ] Enforce capacity per complete scope tuple with authoritative backend checks; lifecycle capacity relief is added in Milestone 15.
- [ ] Enforce `x-agentpm-persist:false` before durable commit.
- [ ] Preserve `x-agentpm-shareable` metadata for semantic export/transfer policy without hiding non-shareable fields from normal owning-Agent use or authorized inspection.
- [ ] Make `before_memory_read`/`before_memory_write` Hooks live and revalidate every permitted request/content patch before runtime execution.
- [ ] Emit Memory readiness/direct read/write/validation/retention/capacity/vector/suppression events according to trace policy.

- [ ] Add restart-persistence, arbitrary-scope, generated-contract, invalid-write-repair, all-three-space-model, append-only, TTL/archive/delete, capacity, retrieval-mode readiness, semantic vector lifecycle/provider availability, explicit custom-runtime/no-fallback, and `.agentpm` immutability tests.

## Milestone 15: Memory Lifecycle Operations, Durable Trigger State, and External Invocation
> Scope note: complete the canonical Harness interpretation of Memory Blueprint lifecycle operations and automatic/external triggers. Harness owns participation, trigger meaning, model-assisted transform/consolidate semantics, source handling, provenance, and external invocation; MemoryRuntime supplies primitive durable operations, trigger state, and atomic batches.
- [ ] Resolve participating global/phase Memory operation bindings separately from direct space bindings; global operations participate for the Run and phase-bound operations only while that phase execution is active.
- [ ] Populate `EffectivePhase`/Run operation state with ready/suppressed participating Memory operation descriptors, preserving global-versus-phase participation, operation identity, backend readiness, and explicit suppression reasons without making lifecycle operations ordinary model actions.
- [ ] Allow a participating operation to access its declared internal input/output/target spaces even when those spaces are not directly bound/model-visible in the current phase.
- [ ] Keep Loop `memory.read/write` restrictions limited to direct model access; they must not disable internally authorized lifecycle operation reads/writes.
- [ ] Require live backend readiness for all referenced operation spaces plus durable trigger state/atomic batches where the operation needs them; suppress only the unavailable operation where safe.

- [ ] Compute each operation scope tuple from the union of scope keys required by its declared inputs/output/targets and resolve those values only from trusted RunContext/runtime scope state.
- [ ] Persist trigger state through MemoryRuntime (`memory_operation_state` for SQLite or equivalent provider API) keyed by exact package/version/operation/resolved operation scope.
- [ ] Implement `record_count` edge trigger: fire on below-threshold -> threshold-or-higher, disarm after firing, re-arm only after active count falls below threshold.
- [ ] Implement `capacity` edge trigger: fire when active count reaches capacity and re-arm after falling below; before rejecting a would-overflow write, run an eligible participating capacity operation first and reject if capacity remains unavailable.
- [ ] Implement interval semantics: remain dormant until relevant scoped state first exists; establish first baseline then; after successful operation execution set next eligibility to completion + interval; persist across Harness restarts.
- [ ] Evaluate relevant automatic operation eligibility after every related Memory state change, including synchronously mid-phase after direct writes.

- [ ] Implement `transform` as one structured output per active scoped record matching its single input pairing.
- [ ] Support `output_mode=create` and `replace_input`; `replace_input` preserves source identity, requires matching input/output pairing + `source_handling=retain`, and is explicit lifecycle authority even on direct append-only spaces.
- [ ] Implement `consolidate` over active scoped records matching its declared inputs, producing one destination record.
- [ ] Implement `delete` mechanically without ModelRuntime generation.
- [ ] For transform/consolidate, call ModelRuntime with Harness lifecycle control, operation description, authorized source content, and target content schema; request/accept target `content` only, never model-owned durable envelope/scope/provenance.
- [ ] Count lifecycle ModelRuntime calls in provider usage/token totals and `max_model_calls_per_phase` when synchronously associated with an active phase, but never as ordinary phase turns, accepted model semantic actions, or Loop steps.
- [ ] Validate lifecycle model output and apply `max_memory_operation_repairs` as additional structured repair attempts.
- [ ] Construct operation provenance/source record IDs in Harness and apply `preserve_provenance` plus `retain | retain_until_expiration | delete_after_success` source handling deterministically.
- [ ] For built-in SQLite, commit output/source/vector/trigger-state mutations belonging to one successful lifecycle operation atomically or roll them all back.
- [ ] Treat lifecycle operation failure as a first-class Memory operation failure, not Tool failure; propagate failure to an originating write/phase only where execution cannot safely continue (for example capacity could not be relieved).

- [ ] Make `before_memory_operation` Hook live after an operation is eligible and before execution; Hooks may allow/reject/add model guidance but cannot rewrite trigger/type/input/output/targets/source handling/scope authority.
- [ ] Implement canonical Engine `invoke_memory_operation(package, operation, current_resolved_scope)` for `trigger.type=external`.
- [ ] Route machine control, Node/Python SDK APIs, and later TUI controls through that same Engine method.
- [ ] Do not automatically expose external Memory operations as model semantic actions merely because they are bound.
- [ ] Reject external invocation of non-external, unbound/non-participating, unresolved-scope, or backend-unready operations with typed control errors.
- [ ] Emit trigger-evaluated/operation-eligible/started/completed/failed/source/output events and include lifecycle usage in Run reports.

- [ ] Add tests for durable trigger persistence/re-arm, 12th-record mid-phase consolidation, capacity relief/failure, interval baseline across process restarts, transform create/replace, consolidate/delete, source handling/provenance, invalid lifecycle output repair, operation failure mapping, external invocation through canonical control, Loop direct-memory restrictions not blocking internal operations, and transactional rollback consistency.

## Milestone 16: PostgreSQL/pgvector and Redis Memory Reference Providers + SDK Helpers
> Scope note: prove Memory Blueprint portability beyond SQLite with two usable external reference backends. Provider-specific logic implements the public MemoryRuntime contract and advertises capabilities honestly; HarnessEngine remains the canonical Blueprint interpreter and must not gain PostgreSQL/Redis-specific lifecycle semantics.
- [ ] Implement a PostgreSQL/pgvector MemoryRuntime reference provider covering the supported document/collection/sequence, scope partitioning, retrieval, retention, capacity, durable trigger state, atomic batch, and semantic retrieval capabilities.
- [ ] Implement a Redis/Redis Stack MemoryRuntime reference provider for the semantics it can faithfully support; explicitly omit unsupported capabilities rather than emulating them incorrectly.
- [ ] Build both providers on the public Milestone 9/10/11 MemoryRuntime process/host contracts; do not add backend-specific branches to HarnessEngine or portable Memory Blueprints.
- [ ] Reuse the existing `memory.packages` mapping activated in Milestone 14; do not introduce another provider-selection mechanism.
- [ ] Preserve explicit-mapping no-fallback semantics: unavailable external runtime does not silently route to SQLite.
- [ ] Advertise the same normalized live MemoryRuntime capability descriptor and let Harness suppress unsupported spaces/operations based on real capability mismatches.
- [ ] Keep lifecycle trigger interpretation, transform/consolidate/delete orchestration, source handling, trusted scopes, and model-assisted generation in Harness; providers expose primitive persistence/retrieval/state/transaction services only.
- [ ] Keep backend credentials provider-side/scoped and out of events/reports.

- [ ] Add Node and Python SDK provider adapters/helpers for PostgreSQL/pgvector and Redis/Redis Stack using optional dependencies/extras.
- [ ] Provide runnable process-bridge examples compatible with `agentpm.harness.json` so CLI-only consumers can use the same reference implementations without hand-writing protocol framing.
- [ ] Keep Node/Python provider request/result/capability semantics aligned.

- [ ] Add mocked provider contract tests plus optional live integration suites gated by environment.
- [ ] Add one cross-backend conformance suite exercising the same representative Blueprint direct-access + lifecycle semantics against SQLite and external provider fixtures, allowing expected unsupported-capability skips only when the provider advertises them honestly.

## Release Band 6: Memory Runtime and Reference Providers
Covered milestones: 14-16.
This gives us the built-in SQLite MemoryRuntime, direct Memory read/write semantics, generated-contract enforcement, trusted scopes, retention/capacity, semantic retrieval, lifecycle operations, durable trigger state, and PostgreSQL/pgvector plus Redis reference providers. This band should be treated as a major runtime subsystem release because it introduces durable local state.

## Milestone 17: MCP Export and `agentpm serve --mcp` Machine Lifecycle
> Scope note: realize Agent-authored `bindings.mcp` as outward AgentPM MCP server surfaces. Preserve the existing shared-runner MCP implementation while adding a stable machine lifecycle/event contract and Session-owned Harness management. Outward MCP remains independent of active Run phase semantics.
- [ ] Add `agentpm serve --mcp --machine` with a documented versioned machine envelope and structured startup/ready/shutdown/error/event messages; protocol stdout must not require Harness to parse human stderr/stdout text.
- [ ] Support `--port 0` and report the actual bound host/port/endpoint in machine readiness.
- [ ] Preserve existing human `serve --mcp` behavior outside machine mode.
- [ ] Keep default managed host loopback and honor `mcp.exports.host`; use ephemeral ports per logical surface rather than static config mapping.
- [ ] Keep existing `serve --mcp` Tool invocation through the shared internal Tool runner; do **not** spawn public `agentpm run` per MCP request.
- [ ] Ensure Milestone 7 runner hardening (schema/runtime/env/timeout/cancellation semantics) is inherited by MCP calls through the shared runner.
- [ ] Add `serve --mcp` lifecycle cleanup for concurrent shared-runner Tool invocations: SIGINT/SIGTERM or managed Session shutdown must terminate any nested child process groups started by in-flight Tool calls without installing a permanent process-global `_exit` handler that bypasses graceful MCP/Harness cleanup.
- [ ] Emit machine Tool-call started/completed/failed events containing canonical AgentPM identity and external MCP-safe normalized name.

- [ ] Add Harness McpRuntime export lifecycle that honors `mcp.exports.enabled`; when enabled, start one managed `agentpm serve --mcp --machine` subprocess per authored Agent `bindings.mcp` surface.
- [ ] Pass exactly the top-level Agent Tools explicitly listed by that logical MCP surface; do not export Skill-transitive Tools by accident.
- [ ] Treat a Tool being both phase-bound and MCP-exported as valid/non-redundant, and allow MCP-only exported Tools without making them phase capabilities.
- [ ] Keep surfaces Session-owned and externally callable even when no Harness Run is active.
- [ ] Validate MCP-normalized name collisions and surface Tool readiness before/at startup.
- [ ] Suppress known runtime-incompatible Tools from the managed surface; expose the ready subset with strong diagnostics when non-empty and mark an empty surface unavailable.
- [ ] Keep missing Tool env semantics authoritative at actual shared-runner invocation rather than pretending successful readiness guarantees env presence forever.
- [ ] Keep outward calls outside active Run Tool Hooks, Loop `access.tools`, checkpoints, Tool retry policy, and phase transcripts.
- [ ] Feed surface lifecycle and external call activity back through Harness events/preflight/report/TUI models without treating those calls as Run actions.
- [ ] Apply managed-process restart policy: failed in-flight call is never replayed; optional restart restores only subsequent calls; exhausted restart makes the surface unavailable.
- [ ] Ensure Session shutdown/cancellation terminates all Harness-owned MCP export subprocesses cleanly.

- [ ] Add tests for exports disabled/enabled, multiple surfaces, ephemeral ports/host, Tool filtering, normalized-name collisions, ready subset/empty surface behavior, call events, shared-runner failures, concurrent in-flight Tool cleanup on MCP server termination, process restart-without-replay, calls with no active Run, and cleanup.

## Milestone 18: External MCP Import and Runtime Tool Augmentation
> Scope note: let workspace runtime configuration add environment-specific MCP functionality to an already-published Agent. Imported MCP Tools become normal phase Tool capabilities only in explicitly configured scope and run through the same Harness Tool selection/validation/Hook/retry/failure pipeline as AgentPM Tools, while retaining distinct McpRuntime transport.
- [ ] Implement config-v1 `mcp.imports` exactly as defined in Milestone 1/spec: `transport: stdio | http`; stdio uses direct command/args/cwd/env/timeouts/restart, HTTP uses an absolute URL and `{value}|{env}` header references.
- [ ] Resolve stdio env/header `{env}` references through the scoped secret/environment resolver and never emit resolved secrets in events, reports, or diagnostics.
- [ ] Require every import to declare explicit `scope.mode: global | phases`; global forbids `phases`, while phase scope requires a non-empty unique list already validated against the selected Loop.
- [ ] Support optional allowed Tool-name filter; omitted means all currently advertised Tools are eligible within the explicitly configured scope.
- [ ] Start/connect imports at Session bootstrap, perform MCP initialization and `tools/list`, validate configured filters, and normalize discovered Tool name/description/input schema into runtime Tool descriptors.
- [ ] Replace the Milestone 5 placeholder provider-native `external_mcp_tool` action schema with each discovered MCP Tool's advertised input schema after `tools/list`, preserving the configured filter/scope.
- [ ] Replace any combined-string `server/tool` alias-decoding placeholder with structured external MCP Tool identity metadata before imported MCP Tool actions become live; server IDs and Tool names must not depend on `/` splitting such as `rsplit_once('/')`.
- [ ] Apply managed-service lifecycle to owned stdio imports and appropriate connection/readiness failure handling to remote HTTP imports; never replay an in-flight Tool call automatically after reconnect/restart.
- [ ] Assign stable canonical internal identities such as `mcp:<server-id>/<tool-name>` and keep provider-safe model aliases separate.
- [ ] Add discovered imported Tools as runtime augmentation candidates only in configured global/phase scope; never mutate Agent manifest/bindings to represent them.
- [ ] Reconcile imported-MCP scope encoding before `mcp_import` candidates become live in `EffectivePhase`: config currently labels phase scope as `phases:a,b`, while existing runtime candidate matching expects `global` or `phase:<id>`. Prefer typed scope metadata, or normalize to one string format, so phase-scoped imports are not silently dropped.
- [ ] Populate `EffectivePhase` with ready/suppressed imported MCP Tool augmentation descriptors, preserving configured global/phase scope, discovered Tool identity, Loop `access.tools`, runtime readiness, and explicit suppression reasons.

- [ ] Route imported MCP Tool actions through the same logical Tool pipeline as AgentPM Tools for EffectivePhase `access.tools`, candidate/selection Hooks, argument schema validation, `max_tool_calls_per_phase`, Loop retry/error policy, phase-local result handling, and Tool events.
- [ ] Dispatch the actual call through McpRuntime rather than `agentpm run` and normalize MCP result/protocol failure into the common Tool action result/failure model.
- [ ] Treat valid MCP invocation/transport/protocol failures as Loop Tool failures after the McpRuntime invocation boundary is crossed.
- [ ] Revalidate arguments after Hook changes before `tools/call`.
- [ ] Surface server/discovered/exposed/suppressed Tool readiness, configured phase scope, and endpoint/transport-safe metadata in preflight/report/TUI data.
- [ ] Keep lifecycle Session-owned and distinguish Harness-owned stdio child termination from remote HTTP connection cleanup.

- [ ] Add tests for stdio + HTTP imports, secret-header/env handling, explicit scoping, Tool filters, duplicate Tool names across servers, canonical/provider alias mapping, Tool-disabled phases, Hook-modified arguments, logical Tool retry, server disconnect/restart-without-replay, phase-local results, Session cleanup, and no Agent-manifest mutation.

## Milestone 19: Ratatui Harness TUI, Interactive Resolution, Approvals, and Branding
> Scope note: build a focused Ratatui client over the existing bootstrap/engine/event/control interfaces. TUI code owns presentation and interactive resolution only; it must not duplicate Loop traversal, capability composition, runtime execution, Hook logic, or approval semantics already implemented below it.
- [ ] Add Ratatui frontend as the default TTY mode for `agentpm harness` and start it early enough to render bootstrap/preflight/service progress rather than showing a blank terminal until readiness completes.
- [ ] Add start/readiness screen showing selected Agent/Loop, provider/model + value source, Consumer Context, Profiles/Skills/Tools, Knowledge, Memory, Hooks, outward/inward MCP, warnings, pending live checks, and effective readiness.
- [ ] Give Tools its own readiness row on the start screen (ready/suppressed counts with expandable reasons) alongside Knowledge/Memory/Hooks/MCP; the per-phase Effective Capabilities list inside the Run view is not a substitute for Agent-level Tool readiness.
- [ ] Show a compact source tag (for example `config`, `cli`, `env`, `default`) next to the resolved Model/provider value on the start screen per the Milestone 1/`spec.md` resolved-value source-metadata requirement; a bare model name with no source is insufficient.
- [ ] Clearly show Consumer Context loaded/unavailable state with path/size/approximate token metadata.
- [ ] Show capability suppression/unavailability/pending reasons with a concise default view plus expandable detail.

- [ ] Add interactive Agent selection when multiple runnable roots exist and no selector was supplied.
- [ ] Add provider/model prompts when required values are unresolved.
- [ ] Add trusted scope-value prompts for unresolved required Memory scope keys where interactive resolution is appropriate.
- [ ] Treat interactive answers as trusted runtime overrides with source metadata for the current Session/Run; do not rewrite `agentpm.harness.json`, Agent artifacts, or portable manifests implicitly.
- [ ] Re-run/recompute affected preflight/readiness after interactive Agent/model/provider/scope resolution before allowing the Run to start.

- [ ] Add primary Run view centered on current phase/objective, concise assistant/model/action activity, selected outcome/transition, approval state, errors/limits, and terminal result.
- [ ] Add a clearly visible message composer only while the Session has no active Run (idle/ready-for-next-Run); submitting the message creates the next Run through the canonical Engine path. Use inviting placeholder copy (for example "Type a message to start the next Run…") rather than pre-filled draft text with a live cursor, so an idle composer is never visually confusable with an in-progress one.
- [ ] While a Run is active — including while it is waiting on an approval checkpoint per the Milestone 4/9 single-active-Run invariant — replace the composer entirely with a non-editable working-status bar: a working/progress indicator naming the current phase plus a `[C] Cancel Run` control routed through canonical cancellation. Do not merely grey out or disable the composer in place; a dimmed text box still reads as an input field, which is the exact ambiguity this element exists to remove.
- [ ] Bind `Enter` to Send only while the composer is shown (Session idle) and bind `C` to Cancel only while a Run is active; the footer keybind legend must reflect whichever state is current rather than always advertising both.
- [ ] On a Run's transition from active to terminal, keep the most recently completed PhaseResult/assistant output visible without an intermediate blank state, and reveal the idle composer only once the Run has actually reached a terminal/runtime-terminal status.
- [ ] Replace the Phase Objective block with a compact Run Summary once the Run is terminal — terminal status, duration, checkpoint outcomes, and the realized phase path (for example `assess -> execute -> respond -> $end`) — rather than leaving a stale in-progress phase objective on screen after the Run has ended.
- [ ] Build the active-vs-terminal composer/working-bar/Run-Summary behavior above against the reviewed TUI reference mockup (provided at implementation time) demonstrating one Run shown at both points in its lifecycle; treat visual/interaction fidelity to that reference as part of this milestone's acceptance, not only the underlying state routing.
- [ ] Show the latest user-facing assistant/PhaseResult output prominently so the TUI is an Agent interaction surface first with observability around it, not only a debugger.
- [ ] Show current-Run usage and cumulative Session usage where space permits; display unavailable token/cost data as an explicit "unknown" label (for example "cost: unknown (provider does not report pricing)") rather than omitting the field or estimating a value.
- [ ] Add interactive checkpoint approval/deny controls routed through the existing ApprovalRuntime/Engine request path.
- [ ] Add cancellation/quit through canonical cancellation and wait for graceful trace/report/service cleanup when possible.
- [ ] Add expandable/toggleable views for canonical prompt sections, Tool args/results, Skill resources, Knowledge results/citations, Memory reads/writes/lifecycle, Hook decisions, MCP activity, and raw events according to trace/content policy.
- [ ] Apply the Milestone 3 trace content policy and unconditional secret-redaction rules to every TUI event/detail/rendering path; expanded views may reveal more event categories, but must not bypass configured content exposure.
- [ ] Render event/action labels in the trace/detail view using the exact canonical Milestone 3 event type names (for example `memory_write_completed`); do not introduce TUI-only event name variants.
- [ ] Ensure approval decision events (`approval_requested`/`approval_approved`/`approval_denied`) are visible in the trace/detail view whenever an approval outcome is also shown in the Run view, so the two panels can never disagree about whether or when an approval occurred.
- [ ] Support repeated Runs in one Session; Consumer Context reloads at each Run start and Session usage accumulates.
- [ ] Surface report/trace paths and terminal status after/between Runs.

- [ ] In standalone TUI execution, treat configured `type: host` providers/runtimes/hooks/controllers as unavailable and show an actionable preflight diagnostic directing the user to configure a `process` implementation or launch the Harness through a Node/Python SDK host. Built-in implementations remain available normally.

- [ ] Implement lightweight branding from config: visible name, optional subtitle, optional `#RRGGBB` accent with safe terminal fallback; branding never alters protocol/event/report/package identity.
- [ ] Do not add arbitrary layout/theme/plugin scripting in Phase 7B.
- [ ] Add TUI state/component tests where practical plus manual verification for small terminals/resizing, bootstrap loading/failure, interactive resolution, approval, cancellation, repeated Runs, trace-content modes, and branding.

## Milestone 20: Templates, Examples, Documentation, End-to-End Hardening, and Release Verification
> Scope note: close Phase 7B by proving the complete architecture through realistic workspaces and all three execution surfaces, documenting the public configuration/protocol/provider contracts, and running cross-repository regression/conformance suites. Do not introduce new runtime architecture here unless required to satisfy the existing spec.
- [ ] Create/update a **minimal Harness Template/workspace** that runs a published Agent with little/no runtime config and demonstrates the shortest credible `install -> agentpm harness -> message -> result` path.
- [ ] Include a local/free Ollama-oriented variant or setup path so the minimal Harness story can be demonstrated without requiring paid hosted-model credentials when a suitable local model is installed.
- [ ] Create/update an **SDK-hosted Harness** example showing first-class Node or Python Hooks, event streaming, approval callback, cancellation, trusted scope/run overrides, Session usage, and report access.
- [ ] Create/update a **custom-provider Harness** example showing a configured EmbeddingProvider plus external Knowledge and Memory runtime realization through the public provider bridge contracts.
- [ ] Create/update an **MCP Harness** example showing both Agent-authored outward MCP surfaces and explicitly scoped external MCP import augmentation.
- [ ] Create/update a **full reference Harness** example exercising a 3+ phase Loop, 2+ AgentPM Tools, 2+ Skills, Profiles, context/vector Knowledge, Memory direct spaces + lifecycle operations, consumer context, approvals, Hooks, tracing/reports, MCP import/export, repeated Runs, and TUI.
- [ ] Ensure generated Template README copy teaches `Agent artifacts = portable definition`, `agentpm.harness.json = workspace runtime realization`, and `agentpm harness = AgentPM reference executor`.
- [ ] Document/prove that Template dependencies do not become Harness bindings, Template entrypoint commands are never auto-executed by Harness, generated files become ordinary consumer-owned workspace inputs, and multi-Agent Template scaffolding still executes one selected Agent per Run.
- [ ] Gitignore `.agentpm-state/` in generated Harness workspaces while documenting safe inspection/export of Run reports/traces/local Memory.

- [ ] Document all `agentpm harness` execution surfaces/options: default TUI, `--headless`, `--machine`, Agent selection, direct/stdin/file Run input, config/model/provider/scope overrides, config precedence/source metadata, state directory, limits, approvals, cancellation, terminal statuses, and report/trace output.
- [ ] Publish the exact `agentpm.harness.json` version-1 reference from `spec.md`, including process/host descriptors, providers, Hook implementations/bindings, Knowledge/Memory mappings, local Memory semantic config, MCP import/export, approvals, trace, lifecycle defaults, and branding.
- [ ] Document the public Harness machine protocol and common process-service protocol sufficiently for third-party clients/providers without requiring the official SDKs.
- [ ] Document first-class Node/Python Harness, Hook, approval, and provider APIs with runnable examples.
- [ ] Document `agentpm run --machine`, public Knowledge query machine behavior used by Harness, and `agentpm serve --mcp --machine` as public integration surfaces.
- [ ] Document Pinecone/pgvector Knowledge and PostgreSQL/pgvector/Redis Memory provider setup/attestation boundaries, including that Harness does not provision/synchronize external indexes/stores.
- [ ] Document local SQLite Memory state location/schema/migrations, arbitrary trusted scopes, semantic f32-vector behavior without sqlite-vec, lifecycle trigger persistence, Memory inspection, and `x-agentpm-shareable` export semantics.
- [ ] Document that Consumer Context is snapshotted once per Run and is shaped only through normal prompt Hooks, not a dedicated context-loading Hook.

- [ ] Run the same representative Loop across **one-shot plain headless**, persistent machine/Node SDK, persistent machine/Python SDK, and interactive TUI paths and confirm identical HarnessEngine phase/outcome/action/runtime semantics modulo presentation/control transport.
- [ ] Verify one-shot headless from direct text, stdin, and input-file: one Session/Run, user-facing terminal output only on stdout, diagnostics separately, report/trace written, documented exit behavior, `approval_required` behavior, and deterministic shutdown.
- [ ] Verify repeated TUI/SDK Runs preserve Session-owned services/usage while resetting RunState/phase transcripts/Run usage and reloading Consumer Context.
- [ ] Verify the built TUI against the Milestone 19 reference mockup specifically for the active-Run vs. terminal-Run distinction: no editable composer and a visible Cancel control while active, composer restored with placeholder-only text once terminal, and no intermediate blank/ambiguous state during the transition.
- [ ] Run representative OpenAI, Anthropic, and Ollama scenarios where credentials/runtime are available; retain deterministic mocked coverage in required CI.
- [ ] Verify configured process and SDK-hosted provider implementations use the same semantic contracts/capability advertisement and that custom provider failures never silently trigger unconfigured fallback.
- [ ] Verify Node/Python parity for Hooks/events/approvals/cancellation/model+embedding+Knowledge+Memory providers/reports/usage.
- [ ] Verify Pinecone + pgvector Knowledge reference providers and PostgreSQL/pgvector + Redis Memory reference providers against mocked/offline conformance, recording any optional live-test skips/environment blockers.
- [ ] Verify inward/outward MCP lifecycle, Tool scoping, Hook/retry semantics, no active-Run export calls, and cleanup.
- [ ] Verify every Run terminal path writes a valid report/trace as far as possible and no secrets appear under any trace content mode.
- [ ] Verify all existing package kinds and publish/install/new/build/query/registry/API/web behavior plus metadata-only SDK loaders remain compatible except for the intentional Loop checkpoint relaxation and Memory transform `output_mode` addition.
- [ ] Update version markers/release notes/docs according to repository conventions.
- [ ] Record exact verification evidence required by `test-plan.md`, including skipped optional external-provider tests and environment blockers.

## Release Band 7: MCP, TUI, Templates, and Final Hardening
Covered milestones: 17-20.
This gives us Harness-managed outward MCP export, explicitly scoped external MCP imports, the Ratatui interactive UI, branding, realistic Templates/examples, full documentation, and end-to-end release verification across headless, machine/SDK, and TUI surfaces.
