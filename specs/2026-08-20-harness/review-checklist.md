# Review Checklist

## Architectural boundary

- Confirm `agentpm harness` executes Agents, not Loops or Templates directly.
- Confirm Harness is a runtime feature, not a new package kind.
- Confirm the canonical orchestration engine exists only in Rust/core and Node/Python do not implement independent Loop traversal.
- Confirm Ratatui, headless, and machine/SDK modes are clients of the same engine/state machine.
- Confirm `agent.lock` is required and authoritative for resolved versions.
- Confirm local `agent.json` is optional when installed/lockfile Agent state is sufficient.
- Confirm an Agent without a Loop remains valid package metadata but is non-runnable by Harness.
- Confirm Harness never invents a default Loop.
- Confirm `loop.archetype` remains descriptive and there is no runtime switch/enumeration over archetype names.
- Confirm EffectivePhase/runtime state is never written back into Agent/Loop/bindings/lockfile.
- Confirm `.agentpm/` remains installed package state and no live Memory/trace/run state is written into installed package roots.
- Confirm mutable runtime state uses `.agentpm-state/` by default or a clearly configured equivalent.
- Confirm the implementation remains understandable enough that bootstrap, Engine, services, hooks/protocol, and TUI boundaries are visible in code rather than hidden in one monolithic executor.

## Intentional portable-contract changes

- Confirm Loop semantic validation now allows multiple checkpoints targeting the same phase.
- Confirm checkpoint IDs remain unique and existing phase/target validation remains intact.
- Confirm runtime evaluates multiple matching checkpoints in authored array order.
- Confirm first rejection stops later checkpoint evaluation and follows that checkpoint's `on_reject`.
- Confirm guarded phase does not consume a step on rejection.
- Confirm Memory transform supports optional `output_mode: create | replace_input`.
- Confirm omitted `output_mode` is backward-compatible `create`.
- Confirm `replace_input` is semantically restricted to matching single input/output pairing.
- Confirm flagship Memory examples/docs were updated where in-place transform is intended.
- Confirm document singleton identity is one current logical record per exact Memory package/version + space + complete resolved scope tuple; `record_type` is not part of document identity, document spaces may permit multiple record types as alternative schemas for that singleton document, `create` rejects when a current document already exists, and `upsert` replacement may change the current record type.
- Reject implementations that permit one simultaneous current document per record type within the same document space/scope.
- Confirm Memory filter semantics are conjunctive dot-path matching over durable record content: a path segment descends into an object by key or into an array existentially, and a match occurs when any traversal reaches a value exactly equal to the filter value, making leaf-array containment a case of the same rule.
- Reject MemoryRuntime implementations that advertise `filter` but return records outside the strict filter contract, including comparison operators, ranges, partial string matching, properties whose names contain a literal `.`, or backend-native richer query behavior.
- Reject any additional portable manifest/schema changes that are not justified by `spec.md` or explicitly raised during implementation.

## Harness configuration

- Confirm root config is versioned and consumer/workspace-owned.
- Confirm configuration groups model/providers, scopes, runtime limits/state, hooks, Knowledge, Memory, MCP, approvals, trace, and UI rather than using a generic untyped escape hatch.
- Confirm provider/model IDs remain open strings; concrete model names are not hardcoded enums.
- Confirm built-in provider IDs include OpenAI, Anthropic, and Ollama.
- Confirm runtime config cannot rewrite Loop graph, outcomes, transitions, access, checkpoints, or authored dependency identity.
- Confirm runtime config can explicitly augment model Tools only through scoped external MCP imports (or another explicitly approved runtime augmentation in spec), not arbitrary hooks/providers.
- Confirm explicit custom Knowledge/Memory runtime mappings win and do not silently fall back.
- Confirm config precedence is implemented/documented consistently: SDK/run override > CLI > config file > environment > default.
- Confirm resolved values carry source metadata for preflight/reporting.
- Confirm runtime safety limits use documented defaults and cannot loosen authored Loop `max_steps`.
- Confirm secrets are referenced/resolved at runtime rather than serialized into config-derived events/reports.
- Confirm state/config/workspace relative paths are canonicalized and unsafe traversal is rejected.

## Bootstrap and preflight

- Confirm bootstrap is separate from Run execution.
- Confirm workspace root discovery reuses existing AgentPM conventions.
- Confirm missing lockfile, unresolved Agent, missing Loop, invalid graph, and required noninteractive model/provider configuration fail clearly.
- Confirm cross-package checks deferred by Phase 7A lint happen at Harness preflight, not retroactively in publish/install lint unless required by the two explicit contract changes.
- Confirm unknown phase binding warns/ignores instead of unnecessarily making the whole Agent unrunnable.
- Confirm unavailable optional capability is surfaced/suppressed rather than hidden or automatically fatal.
- Confirm suppression records both authored Loop restriction and runtime readiness independently where both apply.
- Confirm same-scope direct Tool + Skill-inherited Tool duplication warns/de-dupes.
- Confirm Profile compatibility remains advisory.
- Confirm missing consumer context is non-fatal by default.
- Confirm missing Tool required environment is a strong warning but actual `agentpm run` invocation remains authoritative.
- Confirm known runtime-incompatible Tool is not exposed to the model.
- Confirm preflight data is reusable by TUI, headless, machine clients, trace, and report rather than separately recomputed with inconsistent logic.

## Session / Run / state ownership

- Confirm Harness Session can own long-lived model/provider, Hook, Knowledge, Memory, MCP, Approval, and trace services across multiple Runs.
- Confirm one Run means one Loop traversal from entry to terminal/runtime terminal.
- Confirm Consumer Context and RunState reset/snapshot per Run while durable Memory persists.
- Confirm only HarnessEngine mutates authoritative RunState.
- Confirm services return results/proposals rather than receiving unrestricted mutable RunState access.
- Confirm MemoryRuntime remains authoritative for external persistent Memory/trigger state and Engine stores only snapshots/references in RunState.
- Confirm Run IDs, phase execution IDs, counters, usage, pending approval, PhaseResults, and terminal status are represented explicitly.

## EffectivePhase and authority

- Confirm top-level dependency declaration alone never makes an artifact model-available.
- Confirm Agent global + phase bindings are additive.
- Confirm Skill Tool inheritance occurs in the Skill binding scope.
- Confirm runtime MCP imports are clearly marked runtime augmentation and require explicit scope.
- Confirm Loop `false` suppresses capability, `true` merely permits, omission means no Loop opinion.
- Confirm Loop access restrictions do not accidentally gate semantic actions by provider transport type.
- Confirm provider-native function calls normalize into distinct semantic Harness actions for AgentPM Tool, external MCP Tool, Skill resource, Knowledge, Memory read/write, and phase completion.
- Confirm `access.tools:false` suppresses AgentPM/imported MCP Tools but not Skill resource reads, Profiles, Knowledge, or Memory actions governed by their own semantics.
- Confirm no model-visible content can expand capability topology, graph, scopes, checkpoints, or runtime limits.
- Confirm hooks cannot add undeclared AgentPM capabilities or mutate authoritative scope.

## Loop execution

- Confirm one phase execution equals one Loop step.
- Confirm model/Tool/Knowledge/Memory calls inside a phase do not independently increment Loop steps.
- Confirm phase re-entry gets a new phase execution ID and fresh phase-local provider context.
- Confirm implicit `complete` only exists when outcomes are omitted.
- Confirm explicit outcomes require exact selected IDs and invalid selections use bounded repair.
- Confirm `$end`, `$abort`, and `$handoff` remain distinct authored terminals.
- Confirm `$handoff` does not invoke another Agent.
- Confirm runtime `failed`, `cancelled`, `limit_reached`, and `approval_required` are distinct from authored terminals.
- Confirm max-step exhaustion is `limit_reached`, not `$abort`.
- Confirm runtime max-step limit is the stricter of Harness config and Loop-authored limit.
- Confirm Loop Tool retry `max_retries` is interpreted as additional attempts after the initial call.
- Confirm default error policy is explicit/reportable when Loop omits one.
- Confirm phase working transcripts do not become an unbounded shared transcript across phases.
- Confirm prior PhaseResults, not raw prior provider transcript, are the intended cross-phase execution summary.

## ModelRuntime

- Confirm OpenAI, Anthropic, and Ollama use the same normalized ModelRuntime contract.
- Confirm model IDs are passed as runtime strings and AgentPM releases are not required for new model IDs.
- Confirm provider-specific capability adaptation is localized to ModelRuntime/provider adapters.
- Confirm provider-safe Tool/function aliases map back to canonical identities in Hooks/events/reports.
- Confirm secrets/API keys are never emitted in trace/report.
- Confirm usage/token accounting is normalized when available.
- Confirm Ollama provides a genuinely local/open path rather than still requiring a hosted provider.
- Confirm custom model-provider support uses the same provider/service architecture and does not add another orchestration engine.

## Profiles

- Confirm bound Profiles are model-facing structured behavioral inputs, not a ProfileRuntime.
- Confirm multiple Profiles remain distinct and deterministic in global-then-phase authored order.
- Confirm duplicate Profile identity is removed without creating override precedence.
- Confirm identity/objectives/principles/audience/communication/boundaries/constraints are passed as authored behavior.
- Confirm required vs preferred changes instructional strength only.
- Confirm there is no fake output-scanning enforcement of required constraints.
- Confirm compatibility is warning/advisory only.
- Confirm Profile README is not used as behavior.

## Skills

- Confirm Skill activation exposes compact discovery metadata first rather than eagerly loading all Skill content.
- Confirm entrypoint/references load only when requested/needed.
- Confirm Skill package paths resolve from installed Skill root and traversal/symlink escapes are rejected.
- Confirm Skill Tool dependencies inherit the Skill binding scope and still obey Loop Tool access/readiness.
- Confirm same-scope redundant direct + inherited Tool warns/de-dupes.
- Confirm Skill scripts never auto-execute.
- Confirm script execution requires independently authorized Tool capability such as a shell executor.
- Confirm no SkillRuntime abstraction was added merely to read packaged procedural resources.

## Tool execution and public `agentpm run`

- Confirm Harness direct AgentPM Tool invocation literally spawns public `agentpm run --machine`; reject private-runner shortcuts inside Harness ToolRuntime.
- Confirm JSON arguments use stdin rather than shell-quoted command-line JSON.
- Confirm `agentpm run` machine mode provides stable typed success/failure category without English-string parsing.
- Confirm public `agentpm run` validates input schema and output schema.
- Confirm declared runtime minimum version is actually enforced.
- Confirm existing Tool environment defaults/required vars/interpreter overrides remain compatible.
- Confirm Harness validates model Tool args before ToolRuntime so malformed proposals are repairable model errors.
- Confirm Hook-modified args are revalidated.
- Confirm failures after ToolRuntime invocation boundary become Loop Tool failures.
- Confirm schema-valid domain-level error objects are not generically reclassified based on fields like `ok`.
- Confirm retries are fresh public run subprocesses with same finalized args.
- Confirm cancellation/outer-process termination cannot orphan Tool subprocess groups.

## Hooks and protocol

- Confirm Hook API is typed/interception-based rather than arbitrary mutable event callback.
- Confirm event observation and Hook control are separate protocol concepts.
- Confirm Hook process/SDK host protocols handshake/version capabilities.
- Confirm Hooks get safe snapshots and constrained response shapes.
- Confirm prompt, Tool selection/before-call, Knowledge, Memory read/write/operation, and approval-related application callbacks follow the contracts in spec.
- Confirm mundane metadata reads do not accumulate unnecessary Hook points.
- Confirm Hook failure is fail-closed by default.
- Confirm any fail-open/continue policy is explicit and evented.
- Confirm invalid Hook patch cannot silently mutate graph/access/limits/scopes/undeclared capability.
- Confirm process/host Hook timeouts and crashes are surfaced predictably.
- Confirm Node/Python SDK users do not need to implement framing/correlation IDs manually.
- Confirm public wire protocol remains documented enough for non-SDK third parties.

## Approvals and cancellation

- Confirm ApprovalRuntime is semantically separate from HookRuntime even if SDK callbacks share transport.
- Confirm TUI approval, SDK approval, and machine-control approval all feed the same Engine decision path.
- Confirm plain headless does not auto-approve or auto-deny.
- Confirm plain headless without controller ends `approval_required` and writes a report.
- Confirm controller timeout is runtime/control failure, not authored rejection.
- Confirm multiple matching checkpoints are ordered and first rejection semantics are correct.
- Confirm cancellation is first-class `cancelled`, not generic failure.
- Confirm graceful cancellation flushes trace/report and stops owned MCP/provider/Tool processes.
- Confirm resumable paused RunState was not accidentally implemented as a half-supported feature.

## Knowledge

- Confirm binding means on-demand availability, not automatic RAG/injection.
- Confirm context Knowledge initially exposes descriptors/inventory, not document bodies.
- Confirm vector Knowledge initially exposes searchable package/corpus identity, not every chunk.
- Confirm packages remain distinct retrieval surfaces and are not silently federated.
- Confirm package-owned paths resolve from installed package root and cannot escape.
- Confirm local vector runtime reuses public AgentPM Knowledge query machinery where practical.
- Confirm EmbeddingProvider is a service capability, not a Hook.
- Confirm embedding provider sees compatibility metadata/text and returns vector; it does not need local chunks/index files.
- Confirm dimensions/numeric compatibility are validated before retrieval.
- Confirm explicit custom KnowledgeRuntime mapping determines full retrieval routing.
- Confirm Pinecone and pgvector implementations return normalized AgentPM Knowledge results.
- Confirm provider/corpus package identity mismatch is detected where metadata supports it.
- Confirm explicit custom runtime failure never silently falls back to local.
- Confirm unrealizable bound Knowledge is suppressed/diagnosed rather than automatically killing the Agent.
- Confirm valid backend failure becomes Knowledge service failure to phase, not Tool failure.
- Confirm Knowledge Hooks cannot redirect to unauthorized packages.
- Confirm citations/source metadata are preserved when configured.

## Memory Blueprint boundary

- Confirm Memory artifacts remain Blueprints and installed package roots never contain live records.
- Confirm generated contracts, not source schemas alone, are the runtime durable record contracts.
- Confirm model proposes content only; IDs/scope/timestamps/schema version/ordinal/expiration/provenance are runtime-owned.
- Confirm arbitrary scope keys work and `user`/`conversation` are not hardcoded special literals.
- Confirm authoritative scope values come from trusted runtime context, never directly from model output.
- Confirm unresolved required scope makes only relevant Memory surfaces/operations unavailable where safe.
- Confirm Loop `memory.read/write` constrains direct model access only, not internal lifecycle operation authority.
- Confirm operation bindings and direct space bindings remain separate concepts.

## Local SQLite MemoryRuntime

- Confirm default DB path is `.agentpm-state/memory.sqlite3` (or configured state dir equivalent).
- Confirm local DB schema/versioning matches `spec.md` logical model.
- Confirm canonical scope JSON/hash is stable and used for scoped partitioning.
- Confirm record table distinguishes package/version/space/record type/scope/ID.
- Confirm sequence ordinal index/order is deterministic.
- Confirm active vs archived records are represented without losing auditability.
- Confirm TTL indexes/cleanup are practical.
- Confirm persistent operation state is stored with MemoryRuntime, not RunState or loose files.
- Confirm semantic vectors are keyed by provider/model/dimensions/content identity and do not contaminate other embedding spaces.
- Confirm SQLite transactions are used where record and trigger-state consistency matters.
- Confirm local semantic retrieval can remain exact/simple and does not require hosted vector infra.

## Memory direct semantics / governance

- Confirm document is one current logical record per complete scope tuple/record type.
- Confirm collection supports multiple identified records.
- Confirm sequence appends with runtime-owned ordinal.
- Confirm direct append-only prevents update/delete.
- Confirm explicit lifecycle operation may perform its authored mutation even on append-only direct space.
- Confirm MemoryRuntime advertises live capabilities and Harness checks Blueprint requirements before exposing a surface.
- Confirm a runtime does not claim semantic/full-text/archive/etc. capability merely because it could theoretically be added later.
- Confirm TTL is `(updated_at ?? created_at) + ttl` and updates refresh expiry.
- Confirm capacity is scoped per complete scope tuple.
- Confirm `x-agentpm-persist:false` is enforced before durable commit.
- Confirm `x-agentpm-shareable:false` controls semantic export/transfer, not ordinary owning-Agent reads or authorized inspection.
- Confirm trace sensitivity/redaction remains separate from shareability.

## Memory lifecycle operations

- Confirm global operation bindings participate throughout Run and phase bindings narrow participation.
- Confirm operations may reach their declared internal spaces without directly exposing those spaces to phase model.
- Confirm `delete` is mechanical.
- Confirm `transform` and `consolidate` use ModelRuntime structured target-content generation.
- Confirm lifecycle model calls count usage but not Loop steps.
- Confirm transform produces one output per source record and supports create/replace-input explicitly.
- Confirm consolidate produces one destination output from active scoped input set.
- Confirm output is schema validated and repair bounded before commit.
- Confirm provenance/source handling is applied after successful output generation.
- Confirm record-count triggers are edge-triggered and re-arm below threshold.
- Confirm capacity triggers are edge-triggered/re-arm below capacity and hard-cap behavior is deterministic.
- Confirm interval baseline begins when relevant scoped state first exists, not at package install/Harness startup.
- Confirm interval scheduling state persists across Harness processes.
- Confirm external trigger never fires automatically.
- Confirm all TUI/SDK/host external operation requests route through one Engine invocation path.
- Confirm phase model does not automatically receive generic external-operation authority.

## External Memory providers

- Confirm PostgreSQL/pgvector and Redis/Redis Stack providers implement/advertise the same MemoryRuntime contract rather than redefining Blueprint operations.
- Confirm provider capability negotiation is precise and unsupported combinations are reported honestly.
- Confirm Harness still owns trigger eligibility and transform/consolidate/delete semantics.
- Confirm explicit package-to-runtime mapping is required for custom provider selection.
- Confirm no silent SQLite fallback for explicitly mapped provider failure.
- Confirm provider credentials remain scoped/provider-side.
- Confirm Node/Python provider adapters/helpers are optional-dependency friendly and useful as reference implementations.

## MCP export

- Confirm Agent `bindings.mcp` is interpreted as outward MCP server surfaces, not phase Tool availability.
- Confirm a Tool being both phase-bound and MCP-bound is valid/non-redundant.
- Confirm one public `agentpm serve --mcp --machine` subprocess is used per authored logical surface for MMP.
- Confirm loopback + ephemeral port default.
- Confirm `serve --mcp` keeps shared internal Tool runner and does not recursively spawn `agentpm run` per call.
- Confirm machine readiness/events remove any need to parse human stderr.
- Confirm outward calls do not run phase Hooks/checkpoints/Loop Tool access.
- Confirm outward activity is still traceable through MCP lifecycle/call events.
- Confirm known-unexecutable Tools are not falsely advertised.
- Confirm Session owns/stops server processes.

## MCP import

- Confirm external MCP server config is runtime-only and never written into Agent manifest.
- Confirm import direction is clearly distinct from Agent-authored export.
- Confirm explicit global/phase scope is required.
- Confirm Tool filtering works and omitted filter behavior is documented.
- Confirm discovered Tool capabilities use stable server-qualified internal identities.
- Confirm imported MCP Tools enter EffectivePhase only in configured scope.
- Confirm Loop Tool access, Tool Hooks, validation, retry/error policy, events, and phase-local result semantics apply.
- Confirm MCP protocol/invocation failure after call boundary is classified as Tool failure.
- Confirm external server lifecycle distinguishes owned stdio processes from remote connections.

## Consumer context

- Confirm path is workspace-relative, consumer-owned, and safe-canonicalized.
- Confirm file is snapshotted once per Run.
- Confirm all phases in a Run see the same snapshot.
- Confirm next Run reloads the file.
- Confirm missing file is warning/non-fatal by default.
- Confirm TUI/preflight visibly shows status/size/token estimate.
- Confirm consumer context is eager context but cannot alter Harness authority.
- Confirm no special standardized filename is introduced.

## Events, traces, and reports

- Confirm every important decision/action emits through one central event model used by CLI/TUI/SDK/trace.
- Confirm event envelope is versioned, ordered, correlated, and phase-aware.
- Confirm full content capture is separate from event occurrence metadata.
- Confirm default trace content is redacted rather than full.
- Confirm secrets are never captured even under full content mode.
- Confirm decision events explain direct/inherited/runtime Tool origins, Loop suppression, readiness, and Hook influence.
- Confirm every Run writes a versioned JSON report and JSONL event trace by default when trace enabled.
- Confirm reports exist for failed/cancelled/approval-required Runs.
- Confirm report includes preflight warnings, phases/outcomes/transitions, action summaries, usage, repairs/retries, terminal output/status, and trace reference.
- Confirm report is diagnostic/export output, not resumable RunState.
- Confirm explicit report export path works.

## SDKs

- Confirm Node and Python clients spawn `agentpm harness --machine` rather than implement Loop logic.
- Confirm both expose typed config/run/preflight/event/result/report models.
- Confirm both expose first-class Hook registration APIs that hide framing/correlation details.
- Confirm both expose approval callbacks and cancellation.
- Confirm both expose external Memory-operation control.
- Confirm both expose custom model/embedding/Knowledge/Memory host-provider contracts.
- Confirm callbacks/providers cannot bypass Harness validation/state authority.
- Confirm provider helpers for Pinecone/pgvector/Redis are available in useful open-source form according to `spec.md` and dependency packaging remains reasonable.
- Confirm field semantics are aligned across languages.
- Confirm existing metadata loaders remain metadata-only and unchanged in meaning.

## TUI

- Confirm TUI owns presentation/control only, never graph traversal or authoritative state mutations.
- Confirm preflight/start screen is genuinely useful and not just a loading spinner.
- Confirm Agent, Loop, model/provider/source, Consumer Context, capabilities, Hooks, MCP, warnings/readiness are visible.
- Confirm default run view is concise and detail can be expanded quickly.
- Confirm prompt/Tool/Knowledge/Memory/Hook/MCP/raw event detail obeys trace policy.
- Confirm interactive approval uses ApprovalRuntime.
- Confirm cancellation uses Engine cancellation.
- Confirm repeated Runs reuse Session services and reload per-Run context.
- Confirm branding supports only name/subtitle/accent and remains cosmetic.
- Confirm branding cannot alter event/report/protocol/package identities.
- Reject arbitrary TUI plugins/layout scripting/theme engines in this phase.

## Templates and examples

- Confirm Templates remain generation/install/developer-time artifacts and are never interpreted by Harness at runtime.
- Confirm Template variables are not runtime Harness variables.
- Confirm Template entrypoints are not auto-executed.
- Confirm generated `agentpm.harness.json` becomes normal consumer runtime config after generation.
- Confirm examples include minimal, SDK-hosted, custom-provider, MCP, and full-reference Harness workspaces.
- Confirm README instructions are runnable from a clean generated workspace.
- Confirm examples clearly separate portable Agent artifacts from runtime config/provider code.
- Confirm `.agentpm-state/` is gitignored/documented.
- Confirm examples demonstrate observability/report paths rather than hiding runtime behavior.

## Failure behavior

- Confirm malformed model proposal before runtime service is repairable model/action error.
- Confirm AgentPM/imported MCP invocation failure after Tool boundary uses Loop Tool failure policy.
- Confirm Knowledge/Memory service failure is not mislabeled Tool failure.
- Confirm lifecycle Memory operation failure is its own event/result and propagates only as needed.
- Confirm Hook failure is fail-closed by default.
- Confirm approval transport failure is not authored rejection.
- Confirm cancellation is not generic failure.
- Confirm runtime limit exhaustion is not authored abort.
- Confirm unavailable optional capability does not automatically kill whole Run.
- Confirm fatal bootstrap/runtime ambiguity does not get silently downgraded.

## Security / trust

- Confirm authority hierarchy is enforced structurally rather than only described in prompts.
- Confirm retrieved Knowledge/Memory/Tool/MCP output cannot enable capabilities or rewrite graph.
- Confirm model cannot select arbitrary Memory scope partition.
- Confirm consumer context cannot change Tool/Memory/Knowledge authority.
- Confirm Hooks/providers receive only scoped secrets/data required by their contract.
- Confirm all package-relative file reads remain inside resolved package roots.
- Confirm consumer/workspace relative paths remain inside workspace unless explicitly designed otherwise.
- Confirm external MCP imports require explicit scope and do not become accidental global authority.
- Confirm trace/report content redaction has dedicated tests with representative secrets.

## Regressions

- Run existing CLI/core, API, web, Node SDK, and Python SDK suites appropriate to changed repos.
- Confirm Tool/Agent/Template/Skill/Knowledge/Memory/Profile/Loop init/lint/publish/install/load flows remain valid.
- Confirm Memory build artifacts still contain no live records.
- Confirm Knowledge build/query outside Harness remains functional.
- Confirm public `agentpm run` human mode remains backward compatible.
- Confirm public `agentpm serve --mcp` MCP behavior remains backward compatible.
- Confirm older Memory Blueprints without `output_mode` retain `create` semantics.
- Confirm older valid Loops with single checkpoint continue unchanged.
- Confirm registry/API do not gain unintended runtime state/config persistence.
- Confirm no new broad dependency/runtime framework was introduced where existing Rust/SDK patterns suffice.

## Tests and verification

- Confirm implementation was verified against `test-plan.md`, not only unit-tested locally.
- Confirm high-risk boundaries have integration tests: public Tool subprocess, protocol Hooks, local Memory persistence/restart, MCP child processes, external provider mapping, and report redaction.
- Confirm external-service tests are clearly separated into deterministic mocks vs optional live integration suites.
- Confirm OpenAI/Anthropic/Ollama each have provider-level coverage and release smoke-test evidence when available.
- Confirm Pinecone/pgvector Knowledge and PostgreSQL/pgvector/Redis Memory providers have conformance coverage.
- Confirm Node/Python SDK parity is demonstrated with real CLI integration, not only mocked wire data.
- Confirm TUI behavior has manual evidence in addition to state/unit tests.
- Review all skipped commands/environment blockers and ensure no required acceptance criterion was silently waived.

## Pattern adherence / scope control

- Prefer existing AgentPM package loaders, lockfile models, workspace discovery, Tool runner, Knowledge query, and MCP server code before adding duplicate implementations.
- Keep `agentpm run` and `agentpm serve --mcp` public integration improvements reusable outside Harness.
- Keep provider/hook transport generic enough for third parties but typed enough that application code is not repeatedly writing boilerplate.
- Do not add a generic runtime DSL or scripting engine to `agentpm.harness.json`.
- Do not make external providers a reason to move portable backend details into Agent/Knowledge/Memory manifests.
- Do not make SDK convenience APIs a reason to split execution semantics across languages.
- Do not over-build TUI customization.
- Do not add automatic Agent-to-Agent execution, persisted Run resume, or infrastructure provisioning under the guise of completing existing requirements.

## Notes for reviewer

- The most important design test is: **can a third party understand/reproduce the AgentPM Harness interpretation using public artifacts and execution surfaces without relying on hidden AgentPM-only authority?**
- Inspect any code path that takes `&mut RunState` outside HarnessEngine very carefully; it likely violates the single-writer invariant.
- Inspect any model/provider function-call dispatch that treats every function call as a Tool; Knowledge, Memory, Skill resource access, and phase completion must remain semantically distinct.
- Search for direct calls to the private Tool runner from Harness ToolRuntime. Harness direct Tool execution must use public `agentpm run`; only `agentpm serve --mcp` may continue using the shared internal runner as its existing implementation.
- Inspect `.agentpm` write paths during Harness Runs. Runtime state belongs in `.agentpm-state` or configured state backend.
- Inspect fallback behavior around explicitly configured custom Knowledge/Memory runtimes. Silent fallback is a correctness bug.
- Inspect Memory trigger scheduling for restart/unchanged-threshold storms and ensure durable state is backend-owned.
- Inspect Memory scope handling for any model-provided scope value reaching persistence without trusted runtime adoption.
- Inspect outward vs inward MCP carefully: outward `bindings.mcp` is independent external server authority; inward runtime MCP Tools are phase capabilities and must obey normal Tool controls.
- Inspect SDKs for orchestration logic creep. They should be rich clients/hosts, not alternate Harness engines.
- Inspect trace/report fixtures for leaked API keys, authorization headers, environment secrets, private provider config, and unredacted content under default policy.
- Treat the example Templates as part of product quality, not incidental sample code; they are the primary adoption path for showing developers how portable Agent artifacts and runtime configuration fit together.
