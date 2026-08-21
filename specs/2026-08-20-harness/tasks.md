# Tasks

## Milestone 1: Harness Contract Corrections and Runtime Configuration
> Scope note: establish the two portable-contract corrections discovered during Harness design and add the versioned workspace runtime-configuration contract. This milestone defines configuration/schema/types and semantic validation only; it does not execute Agents, call models, persist Memory, run MCP servers, implement hooks, or render a TUI.
- [ ] Update Loop semantic validation to allow multiple approval checkpoints with the same `before_phase`.
- [ ] Preserve authored `loop.checkpoints` array order and document that multiple checkpoints targeting one phase are evaluated in that order at runtime.
- [ ] Remove/replace tests and docs that reject duplicate `before_phase` checkpoints solely because they target the same phase.
- [ ] Retain checkpoint ID uniqueness, valid phase targets, valid `on_reject` targets, and all existing structural validation.
- [ ] Add optional Memory transform `output_mode` with exactly `create | replace_input`.
- [ ] Default omitted transform `output_mode` to `create` in typed/runtime interpretation for backward compatibility.
- [ ] Add semantic validation for `replace_input`: transform-only, exactly one input, output space/record type equal to the input pairing, and source-handling compatibility.
- [ ] Update the flagship `conversation-continuity` example so `refresh_saved_note` explicitly uses `output_mode: "replace_input"`.
- [ ] Add strict JSON Schema and Rust typed models for root-level `agentpm.harness.json` version 1.
- [ ] Implement the grouped config surfaces from `spec.md`: model, providers, scopes, runtime/state/limits, hooks, Knowledge, Memory, MCP, approvals, trace, and UI branding.
- [ ] Keep model/provider IDs open-ended strings while documenting/typing built-in provider IDs where appropriate.
- [ ] Add validation for workspace-relative paths, non-empty IDs, port/URL/transport shapes, phase-scope lists, hook failure policies, trace levels/content modes, and branding hex accents.
- [ ] Add a runtime-config loader using the workspace root and optional CLI-supplied config path override.
- [ ] Add resolved-config source metadata capable of distinguishing SDK/run override, CLI override, config file, environment, and Harness default.
- [ ] Implement default Harness safety limits from `spec.md` and ensure they are runtime defaults, not written into Loop manifests.
- [ ] Add unit/schema tests for minimal/empty valid config, full config, unknown top-level fields, malformed provider/runtime maps, unsafe paths, invalid branding, invalid limits, malformed scopes, and invalid MCP import scope.
- [ ] Confirm existing Agent/Loop/Memory manifests continue to validate except for the intentional semantic relaxation/addition above.

## Milestone 2: Harness Bootstrap, Workspace Discovery, and Preflight Plan
> Scope note: add the `agentpm harness` command shell and the bootstrap/preflight pipeline that resolves one runnable Agent from `agent.lock`, installed package state, and optional runtime configuration. Produce structured readiness data but do not execute the Loop or call a model yet.
- [ ] Add `agentpm harness [AGENT]` to CLI command routing/help.
- [ ] Reuse existing workspace-root discovery conventions rather than adding Harness-only root search behavior.
- [ ] Require `agent.lock` for execution and return an actionable error when missing.
- [ ] Support local `agent.json` when present but do not require it if a runnable installed Agent root is fully represented by lock/install state.
- [ ] Resolve an explicit Agent selector against lockfile/install state.
- [ ] If `AGENT` is omitted and exactly one runnable Agent root exists, select it deterministically.
- [ ] If multiple runnable Agent roots exist, return a structured selection requirement for headless/machine modes and leave interactive selection to the later TUI milestone.
- [ ] Reject Agents without a resolved Loop as non-runnable; do not invent a default Loop.
- [ ] Resolve exact installed Agent, Loop, Tool, Skill, Knowledge, Memory, and Profile package versions from lockfile relationships.
- [ ] Add Harness-only cross-package validation for binding phase names, bound package existence, Memory spaces/operations, Skill Tool inheritance, MCP Tool membership, and generated Knowledge/Memory metadata needed for runtime use.
- [ ] Classify diagnostics as fatal, warning, suppressed/unavailable capability, or informational according to `spec.md`.
- [ ] Warn/ignore unknown Agent phase-binding keys rather than failing an otherwise coherent Loop.
- [ ] Detect same-scope direct Tool + Skill-inherited Tool duplication, warn, and de-dupe.
- [ ] Resolve consumer-context path safely relative to workspace root but do not yet inject it into model prompts.
- [ ] Resolve configured runtime scope values without giving model code authority to choose them.
- [ ] Add `.agentpm-state` path resolution with config/CLI override support and keep it physically/logically separate from `.agentpm`.
- [ ] Add `PreflightReport`/`ResolvedHarnessPlan` Rust models usable by later TUI, headless, machine protocol, reports, and SDKs.
- [ ] Add tests for installed-Agent-only execution roots, local Agent roots, multiple Agents, missing Loop, bad binding phase, bad Memory selector, missing generated metadata, unsafe consumer context, missing optional context, and state-dir separation.

## Milestone 3: Stable Events, Trace Sink, and JSON Run Report Foundation
> Scope note: establish the Harness observability contract before complex execution is added. Add event envelopes, event sequencing, durable JSONL trace files, run-report models, redaction policy, and a minimal session/run lifecycle. No model/tool/Knowledge/Memory execution yet.
- [ ] Add versioned Harness event envelope types with session ID, run ID, run-local sequence, timestamp, type, phase execution ID, correlation/parent IDs, and typed payload support.
- [ ] Add event categories/namespaces for bootstrap, session, run, phase, model, outcome/transition, Tool, Skill/resource, Knowledge, Memory, approval/control, Hook, MCP, consumer context, cancellation, and terminal state.
- [ ] Keep events as observability facts rather than authoritative event-sourced RunState.
- [ ] Add central event emitter/fan-out used by CLI renderers, machine protocol, trace files, and reports.
- [ ] Add trace configuration with `minimal | normal | verbose` and `none | redacted | full` content policies or repository-equivalent explicit enums.
- [ ] Make secret redaction unconditional regardless of trace content policy.
- [ ] Create `.agentpm-state/runs/<run-id>/events.jsonl` by default when tracing is enabled.
- [ ] Add versioned `RunReport` model and default `.agentpm-state/runs/<run-id>/report.json` path.
- [ ] Include preflight/runtime-source metadata, Agent/Loop identities, warnings, terminal state, phase summaries, usage placeholders, action summaries, error/retry counts, and trace reference in the report schema.
- [ ] Add explicit report-path/export override while retaining default state-directory report generation.
- [ ] Ensure partial/failed/cancelled Runs still flush a valid report and trace as far as possible.
- [ ] Add tests for sequence monotonicity, correlation IDs, redaction, trace-level filtering, report serialization, explicit output paths, state-dir creation, and failure-safe flush.

## Milestone 4: Core HarnessEngine and Loop Traversal with Fake ModelRuntime
> Scope note: implement the UI-agnostic Run/phase state machine using deterministic fake/test ModelRuntime responses. Prove Loop traversal, phase steps, outcomes, limits, terminals, retries/error-policy plumbing, and single-writer RunState before real provider APIs or capabilities are introduced.
- [ ] Add `HarnessSession`, `RunContext`, `RunState`, `PhaseResult`, runtime terminal status, and phase execution ID models.
- [ ] Enforce the single-writer invariant: only HarnessEngine mutates RunState.
- [ ] Execute `entry_phase`, phase completion, transition lookup, re-entry, and terminal targets from the Loop graph.
- [ ] Treat one phase execution as one Loop step; model/action work inside the phase does not increment Loop steps.
- [ ] Enforce effective max steps as the stricter of authored Loop max and Harness safety ceiling.
- [ ] Return `limit_reached` rather than authored `$abort` when limits are exhausted.
- [ ] Implement implicit `complete` for phases with omitted outcomes.
- [ ] Validate explicit model/host outcome IDs exactly and add bounded structured repair plumbing.
- [ ] Implement `$end`, `$abort`, and `$handoff` terminal result semantics, with `$handoff` returning control/context rather than invoking another Agent.
- [ ] Implement default Tool-failure -> phase failure and phase-failure -> runtime failed behavior for cases where Loop policy is absent; actual Tool invocation comes later.
- [ ] Implement Loop Tool retry policy counters/actions in an executor-neutral form for later Tool/MCP use.
- [ ] Add internal action-count/model-call/tool-call counters and safety-limit enforcement models.
- [ ] Add fake ModelRuntime/test harness capable of returning content, explicit outcomes, malformed outcomes, and failures.
- [ ] Emit lifecycle/phase/outcome/transition/terminal events and write populated reports.
- [ ] Add unit/integration tests for cycles, re-entered phases, implicit/explicit outcomes, invalid outcome repair/exhaustion, all terminals, max-step exhaustion, default failure behavior, and authored error policy.

## Milestone 5: ModelRuntime, Prompt Assembly, OpenAI, Anthropic, and Ollama
> Scope note: add real model execution and normalized semantic turn/action plumbing, including the three required built-in providers. Complete a text-only multi-phase Harness Run in headless mode before Tool/Knowledge/Memory capabilities are added.
- [ ] Add `ModelRuntime` trait/interface and normalized model request/response/usage/action structures.
- [ ] Keep model provider IDs and concrete model IDs runtime strings; do not add a closed model enum.
- [ ] Implement built-in OpenAI provider using current supported API patterns in the repository/ecosystem at implementation time.
- [ ] Implement built-in Anthropic provider with equivalent normalized behavior.
- [ ] Implement built-in Ollama provider as the required local/open provider path.
- [ ] Resolve standard provider credentials/endpoints from scoped environment/config without serializing secrets.
- [ ] Support custom model provider IDs backed by configured process/SDK host provider contracts without requiring those custom implementations in this milestone.
- [ ] Add provider capability detection/adaptation sufficient to reject unsupported required structured-action behavior clearly.
- [ ] Add provider-safe temporary function/tool aliases while retaining canonical Harness identities internally.
- [ ] Add phase prompt assembler with Harness control contract, phase objective/outcomes, run input, prior PhaseResults, and placeholders for later Profile/Skill/capability composition.
- [ ] Keep raw provider transcripts phase-local and start a fresh provider context on phase re-entry/new phases.
- [ ] Account for provider usage/tokens when available.
- [ ] Add headless input/output flow and actionable missing-provider/model errors.
- [ ] In interactive-capable bootstrap, represent missing model/provider as promptable requirements for the future TUI rather than choosing silently.
- [ ] Add provider contract tests with mocked HTTP/process transports and optional real-provider smoke tests gated by environment variables.
- [ ] Add one representative three-phase text-only end-to-end test that runs against fake provider fixtures through the real ModelRuntime interface.

## Milestone 6: EffectivePhase, Profiles, and Consumer Context
> Scope note: compute real effective phase composition for non-executable behavioral/context surfaces. Add Profile composition and per-Run consumer-context snapshotting before Tools/Skills/Knowledge/Memory actions are layered in.
- [ ] Add `EffectivePhase` model including authored candidates, runtime augmentation placeholders, Loop access decisions, runtime readiness, suppression reasons, and deterministic ordering.
- [ ] Compute global + phase Profile bindings additively and de-dupe by package identity.
- [ ] Load Profile structured metadata once during bootstrap and reuse immutable resolved data.
- [ ] Serialize multiple Profiles as distinct model-facing inputs in deterministic global-then-phase authored order; do not merge/override into a synthetic Profile.
- [ ] Treat required/preferred Profile constraints as different prompt-strength guidance only.
- [ ] Evaluate Profile compatibility as advisory readiness diagnostics/warnings, never as hard behavioral enforcement.
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
- [ ] Perform early Harness Tool-argument schema validation before invoking ToolRuntime.
- [ ] Add bounded model argument repair using `max_tool_call_repairs`.
- [ ] Revalidate arguments after any later Hook modifications.
- [ ] Map ToolRuntime failures into Loop Tool failure policy without parsing human strings.
- [ ] Retry as fresh `agentpm run` invocations with the same finalized arguments.
- [ ] Suppress known runtime-incompatible Tools during EffectivePhase computation with reasons.
- [ ] Warn but do not necessarily suppress solely for missing required env at preflight; actual `agentpm run` invocation remains authoritative.
- [ ] Add Skill activation descriptors/inventory without eagerly loading full `SKILL.md`/references.
- [ ] Add model semantic action for authorized Skill entrypoint/reference read.
- [ ] Resolve/canonicalize Skill resource paths within installed Skill root and reject escapes/symlink escapes.
- [ ] Expand bound Skill Tool dependencies into the Skill's global/phase binding scope.
- [ ] De-dupe same-scope direct + inherited Tool identity and emit composition warning.
- [ ] Never auto-execute Skill scripts; script execution requires an independently authorized Tool.
- [ ] Enforce Loop `access.tools` over direct and inherited Tools while Skill resource reads remain distinct from Tool calls.
- [ ] Emit Tool candidate/selection/invocation/retry/result/failure and Skill resource events.
- [ ] Add end-to-end phase tests with two Tools, one Tool-backed Skill, Tool-disabled phase, invalid arguments, retry exhaustion, runtime suppression, and Skill resource access.

## Milestone 9: HookRuntime, ApprovalRuntime, Machine Control Protocol, and Cancellation
> Scope note: establish the persistent bidirectional Harness protocol and typed interception/control contracts before language SDK wrappers are added. Implement prompt/Tool/approval hooks first, then later capability milestones add Knowledge/Memory hook points on the same contract.
- [ ] Define a versioned Harness machine protocol over stdin/stdout or the repository's equivalent persistent subprocess transport.
- [ ] Add handshake/version/capability negotiation for machine clients and workspace process providers/hooks.
- [ ] Keep event messages distinct from control requests and service/provider requests.
- [ ] Add correlation IDs and bounded request timeouts where configured.
- [ ] Add `HookRuntime` with typed hook registrations and constrained request/response patch types.
- [ ] Implement prompt/context-shaping Hook before model request.
- [ ] Implement Tool candidate/selection influence hook where applicable without granting new capabilities.
- [ ] Implement before-Tool-call argument shaping/rejection hook followed by schema revalidation.
- [ ] Make configured intercepting hook failure fail closed by default.
- [ ] Add explicit per-hook `continue`/fail-open configuration and visible diagnostics when chosen.
- [ ] Prevent hooks from altering graph, checkpoints, Loop access, runtime limits, arbitrary RunState, or Memory scope authority.
- [ ] Add `ApprovalRuntime` separate from HookRuntime.
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
- [ ] Add typed first-class prompt Hook registration.
- [ ] Add typed Tool selection/before-call Hook registration.
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
- [ ] Add first-class prompt, Tool selection/before-call, and approval callbacks.
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
- [ ] Add `KnowledgeRuntime` interface and normalized Knowledge request/result models.
- [ ] Add semantic model action for Knowledge access distinct from Tool calls.
- [ ] Enforce Loop `access.knowledge` independently from `access.tools`.
- [ ] Keep bound Knowledge packages distinct model surfaces rather than auto-federating them.
- [ ] For context Knowledge, expose compact package/document descriptors initially and load only the requested declared document.
- [ ] Resolve package-owned Knowledge paths relative to installed package root and reject traversal/symlink escapes.
- [ ] For vector Knowledge, load/validate installed build/index/provenance metadata needed for compatibility/readiness.
- [ ] Reuse public `agentpm knowledge query` behavior/machinery when it can satisfy the request rather than reimplementing search privately in Harness.
- [ ] Add a machine/query interface if existing public Knowledge query output is insufficient for Harness-safe structured consumption.
- [ ] Add typed `EmbeddingProvider` service request/response contract with provider/model/dimensions/normalization/text and returned numeric vector.
- [ ] Resolve configured embedding-provider matches against Knowledge embedding metadata when local query needs a compatible query vector.
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
- [ ] Add `MemoryRuntime` interface and normalized direct Memory read/write/update/delete request/result types.
- [ ] Implement built-in SQLite runtime at default `.agentpm-state/memory.sqlite3`.
- [ ] Add local store schema versioning/migration mechanism.
- [ ] Implement `memory_meta`, `memory_records`, `memory_operation_state`, and `memory_vectors` logical tables from `spec.md` with suitable indexes/constraints.
- [ ] Store canonical `scope_json` plus stable `scope_hash`; never let model-supplied content choose scope.
- [ ] Resolve arbitrary Blueprint scope keys from trusted RunContext/config/SDK/CLI inputs.
- [ ] Load generated contract index/contracts and validate runtime records against generated envelope contracts.
- [ ] Accept model-proposed record `content` only and construct IDs, scope, timestamps, schema version, ordinal, expiration, and provenance in Harness/MemoryRuntime.
- [ ] Implement document one-current-record semantics per complete scope tuple.
- [ ] Implement collection create/read/update/delete by ID/filter according to declared constraints/retrieval modes.
- [ ] Implement sequence append/chronological retrieval with runtime-assigned ordinal and deterministic ordering.
- [ ] Enforce `append_only` for direct model mutations.
- [ ] Implement `key`, `filter`, `chronological`, and practical local `full_text` retrieval where declared.
- [ ] Implement local `semantic` retrieval using configured/local embedding provider plus `memory_vectors` exact search; advertise semantic only when resolved and ready.
- [ ] Add MemoryRuntime capability advertisement and preflight comparison to Blueprint space requirements.
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
- [ ] Validate generated target content and perform bounded `max_memory_operation_repairs` structured repair.
- [ ] Construct provenance from operation/source record IDs and enforce `preserve_provenance`/source-handling semantics.
- [ ] Apply `retain`, `retain_until_expiration`, and `delete_after_success` consistently and transactionally where practical.
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
- [ ] Add runtime capability handshake/readiness diagnostics and suppress only unsupported spaces/operations where safe.
- [ ] Add Node and Python SDK provider adapters/helpers for PostgreSQL/pgvector and Redis using optional dependencies/extras.
- [ ] Add runnable provider bridges/examples compatible with workspace process provider configuration.
- [ ] Keep backend credentials provider-side/scoped and out of events.
- [ ] Add mocked provider contract tests plus optional live integration suites gated by environment.
- [ ] Add one cross-backend conformance suite running the same representative Blueprint semantics against SQLite and external provider fixtures.

## Milestone 17: MCP Export and `agentpm serve --mcp` Machine Lifecycle
> Scope note: realize Agent-authored `bindings.mcp` as outward AgentPM MCP server surfaces. Preserve current shared-runner implementation while adding machine readiness/events and Harness Session lifecycle management.
- [ ] Add machine mode to `agentpm serve --mcp` with structured handshake/ready/shutdown/error messages.
- [ ] Support `--port 0` and return the actual bound endpoint in machine readiness.
- [ ] Keep default host loopback and let Harness choose ephemeral ports for managed surfaces.
- [ ] Keep existing `serve --mcp` Tool invocation through shared runner code; do not spawn public `agentpm run` per MCP request.
- [ ] Add machine events for MCP Tool call started/completed/failed with canonical AgentPM identity and MCP-normalized name.
- [ ] Preserve MCP protocol behavior and human serve output outside machine mode.
- [ ] Add Harness `McpRuntime` export lifecycle that starts one `agentpm serve --mcp --machine` subprocess per authored MCP binding ID.
- [ ] Pass exactly the bound top-level Agent Tools for that logical surface.
- [ ] Validate MCP-safe normalized name collisions during preflight/startup.
- [ ] Suppress known runtime-incompatible Tools from managed MCP exposure; realize ready subset with strong warnings when non-empty and mark empty surface unavailable.
- [ ] Keep outward calls independent from active Run phase hooks/access/checkpoints.
- [ ] Emit Harness surface start/ready/activity/failure/stop events and include endpoint/tool mapping in TUI/report.
- [ ] Ensure Session shutdown/cancellation terminates all owned MCP server processes cleanly.
- [ ] Add tests for multiple surfaces, ephemeral ports, Tool filtering, collisions, partial readiness, call events, process failure/restart policy if implemented, and cleanup.

## Milestone 18: External MCP Import and Runtime Tool Augmentation
> Scope note: let runtime config add environment-specific MCP functionality to a published Agent. Imported Tools become normal phase Tool capabilities only in explicitly configured scope and are governed by the Harness Tool pipeline.
- [ ] Implement `mcp.imports` runtime config for at least stdio and supported HTTP MCP transport according to current MCP ecosystem/repository choices.
- [ ] Require every import to declare explicit global or phase scope; reject omitted/empty scope.
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
- [ ] Gitignore `.agentpm-state/` in generated Harness workspaces while documenting how to inspect/export reports and local Memory safely.
- [ ] Document `agentpm harness` modes/options, Agent selection, config precedence/defaults, runtime-state directory, safety limits, approvals, cancellation, and terminal statuses.
- [ ] Publish a complete `agentpm.harness.json` reference with examples for OpenAI, Anthropic, Ollama, Hooks, scopes, Knowledge providers, Memory providers, MCP imports/exports, trace policy, and branding.
- [ ] Document the public machine protocol sufficiently for third-party clients/providers without requiring Node/Python SDKs.
- [ ] Document first-class Node/Python Harness and Hook/provider APIs with runnable examples.
- [ ] Document Pinecone/pgvector Knowledge and PostgreSQL/pgvector/Redis Memory provider setup boundaries, including that Harness does not provision/sync external indexes/stores.
- [ ] Document local SQLite schema/location/migration expectations and Memory inspection/export/shareable semantics.
- [ ] Document `agentpm run --machine` and `agentpm serve --mcp --machine` as public integration surfaces.
- [ ] Run end-to-end scenarios against OpenAI, Anthropic, and Ollama where credentials/runtime are available; use deterministic mocks for required automated CI coverage.
- [ ] Run the same representative Loop across headless, machine/SDK, and TUI paths and confirm identical Engine outcomes/events modulo UI/transport.
- [ ] Verify Node/Python SDK parity for Hooks/events/approvals/cancellation/providers/reports.
- [ ] Verify all required run reports/traces are generated and contain no secrets.
- [ ] Verify all existing package kinds, publish/install/new/build/query flows, registry/API/web behavior, and metadata-only SDK loaders remain compatible.
- [ ] Update version markers/release notes/docs according to repository conventions.
- [ ] Record exact verification evidence required by `test-plan.md`, including skipped external-provider tests and environment blockers.
