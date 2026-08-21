# Test Plan

## Required verification

Phase 7B is complete only when the same canonical Rust HarnessEngine can execute representative Agents through TUI, headless, and machine/SDK clients while preserving the portable Agent/Loop contracts, public Tool/MCP execution surfaces, provider extensibility, Memory durability, and traceability defined in `spec.md`.

Required end-to-end scenarios:

1. Run an installed Agent with no local `agent.json`, using `agent.lock` and installed `.agentpm` state only.
2. Run a local Agent through a three-phase Loop with implicit and explicit outcomes, phase re-entry, and `$end`.
3. Exercise `$abort`, `$handoff`, `limit_reached`, `failed`, `cancelled`, and `approval_required` terminal statuses.
4. Execute two AgentPM Tools through Harness ToolRuntime -> public `agentpm run --machine`, including Tool-backed Skill inheritance.
5. Verify a Tool-disabled Loop phase suppresses direct and Skill-inherited Tools while Skill resources remain accessible.
6. Run the same representative text/Tool scenario with OpenAI, Anthropic, and Ollama provider implementations (real-provider smoke tests where available; deterministic mocked provider tests are mandatory in CI).
7. Register first-class Node SDK prompt/Tool/approval Hooks, receive events, cancel a Run, and read the final report.
8. Run equivalent Python SDK Hook/event/approval/cancellation flow.
9. Load context Knowledge on demand without eager body injection.
10. Query vector Knowledge locally using existing AgentPM query machinery and a configured external EmbeddingProvider when required.
11. Map a Knowledge package explicitly to Pinecone and another to pgvector; verify normalized results and no silent local fallback on configured-provider failure.
12. Persist scoped Memory through SQLite, restart Harness, retrieve the same records, and verify `.agentpm` package state was never mutated.
13. Trigger `consolidate_recent_interactions` on the threshold transition, run a model-assisted consolidation, preserve provenance, apply source handling, and verify durable trigger state.
14. Exercise interval baseline/restart semantics and an externally invoked delete operation through the canonical Engine control path.
15. Run representative Memory behavior against PostgreSQL/pgvector and Redis provider conformance fixtures/integration environments.
16. Start multiple Agent-authored outward MCP surfaces through one `agentpm serve --mcp --machine` subprocess per logical binding using loopback/ephemeral ports.
17. Import an external MCP server through runtime config, explicitly scope it to one/more phases, and verify its Tools enter the normal Tool Hook/access/retry pipeline only in those phases.
18. Snapshot consumer context once per Run, edit it mid-Run, verify no change until the next Run, and verify TUI/preflight status.
19. Run multiple approval checkpoints targeting the same phase and verify authored-order evaluation, all-approve entry, and first-rejection routing.
20. Verify every Run writes `events.jsonl` and `report.json`, including partial/failed/cancelled Runs, with secret redaction.
21. Run the same representative Agent through headless, machine/SDK, and TUI modes and compare Engine outcomes/phase transitions.
22. Generate/run the Harness example Templates/workspaces and confirm `.agentpm-state/` is mutable/runtime-owned and gitignored while `.agentpm/` remains installed package state.

## Automated checks

Run commands from the repository that owns each implementation. If scripts differ from the examples below, use the repository's configured equivalent and report the exact command.

### Rust CLI/core/shared SDK

From the AgentPM CLI workspace:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`

At minimum, targeted Rust tests must cover:

- Harness config parsing/validation/defaults/precedence;
- Loop checkpoint semantic relaxation;
- Memory transform `output_mode` schema/semantic validation;
- workspace/Agent/lockfile discovery;
- bootstrap/preflight severity and capability suppression;
- event sequencing/redaction/report serialization;
- core Loop engine transitions/outcomes/limits/terminal states;
- model provider normalization and aliases;
- EffectivePhase composition;
- consumer-context snapshots;
- public Tool run machine envelope/schema/version/cancellation behavior;
- Harness ToolRuntime subprocess behavior;
- Skill progressive disclosure/inheritance;
- machine protocol, Hooks, approvals, cancellation;
- local Knowledge context/vector retrieval and EmbeddingProvider fallback;
- local SQLite Memory direct access, retention/capacity, trigger state, lifecycle operations;
- MCP export lifecycle and import Tool integration;
- TUI state logic where practical.

### Node SDK

Use the repository's configured lint/test/build commands, expected forms if unchanged:

- `npm run lint`
- `npm test -- --run`
- `npm run build`

At minimum test:

- Harness subprocess discovery/start/stop;
- handshake/version mismatch;
- event streaming;
- terminal/report result;
- prompt Hook;
- Tool candidate/before-call Hook;
- approval callback;
- cancellation;
- external Memory operation invocation;
- custom model/embedding/Knowledge/Memory host-provider request routing;
- provider callback errors/timeouts;
- Pinecone/pgvector/Redis provider helpers where implemented in the SDK;
- cleanup after unexpected Harness process exit;
- parity with existing metadata loaders.

### Python SDK

Use configured formatter/linter/type checker plus:

- `pytest`

At minimum run equivalent coverage for:

- Harness lifecycle;
- event iteration;
- Hooks;
- approval;
- cancellation;
- provider callbacks;
- Memory external operation control;
- reference provider helpers;
- protocol/process failure;
- public exports/type models;
- existing loader regressions.

### Registry API / web

Phase 7B should not require broad registry product changes. Run existing API/web suites required by any manifest-schema/example/documentation changes and verify no package-kind/publish/install regressions.

At minimum:

- existing manifest/package validation integration remains compatible;
- updated Loop checkpoint and Memory transform metadata are accepted wherever generic manifests are persisted/displayed;
- no Harness runtime state is sent to or persisted by the registry unless an existing generic manifest field requires it;
- existing Agent/Loop/Memory pages do not misrepresent runtime-only `agentpm.harness.json` as portable package metadata.

## Contract tests

### Harness configuration

Verify valid:

- empty/minimal version-1 config using defaults;
- OpenAI/Anthropic/Ollama model configs;
- custom model/embedding provider definitions;
- scope defaults;
- runtime state-dir/limit overrides;
- Hook implementation definitions and failure policy;
- Knowledge package runtime mapping and embedding matches;
- Memory runtime mapping;
- stdio/HTTP external MCP imports with explicit phase/global scope;
- MCP export host override;
- trace config;
- branding name/subtitle/accent.

Verify rejection of:

- missing/unsupported config version;
- unknown forbidden properties;
- zero/negative safety limits;
- invalid trace levels/content modes;
- unsafe workspace-relative paths;
- invalid branding colors;
- external MCP import without explicit scope;
- malformed phase lists/tool filters/transports/URLs;
- package mapping to unknown configured runtime/provider IDs where validation can determine it;
- duplicate provider/Hook IDs where representation permits duplicates.

### Loop checkpoint change

Verify:

- multiple checkpoints may target one `before_phase`;
- checkpoint IDs still must be unique;
- unknown phase/target remains invalid;
- runtime evaluates matching checkpoints in authored array order;
- first rejection follows that checkpoint's `on_reject` and later checkpoints are not requested;
- guarded phase consumes no step on rejection.

### Memory transform change

Verify:

- omitted `output_mode` remains valid and resolves as `create`;
- explicit `create` valid;
- valid same-input/output `replace_input` valid;
- `replace_input` rejected for wrong operation type;
- `replace_input` rejected when output pairing differs from input;
- legacy Memory Blueprints remain readable/executable.

## Bootstrap and preflight checks

Verify:

- missing `agent.lock` is fatal;
- installed Agent root can run without local Agent manifest;
- local Agent plus lock resolves correctly;
- Agent without Loop is valid package metadata but non-runnable by Harness;
- multiple Agents require selection in noninteractive mode;
- unknown phase binding warns/ignores;
- invalid transition/entry/ambiguous graph is fatal;
- missing consumer context warns;
- Loop-prohibited bound capability is reported as suppressed;
- Profile compatibility mismatch warns;
- known Tool runtime incompatibility suppresses Tool;
- missing Tool required env warns but does not automatically suppress solely for that reason;
- unrealizable Knowledge/Memory surface is unavailable/suppressed without killing an otherwise runnable Agent;
- explicit custom provider failure never silently selects another provider;
- preflight includes source-of-value metadata for defaults/config/CLI/SDK overrides.

## Core Loop execution checks

Verify:

- one phase execution increments one Loop step;
- multiple model calls/actions inside a phase do not increment Loop steps;
- phase re-entry creates a new execution ID/context;
- omitted outcomes use `complete`;
- explicit outcomes remove implicit `complete` unless authored;
- malformed/unknown outcome uses bounded repair and then phase failure;
- every valid outcome transition is followed exactly;
- `$end` uses last PhaseResult output by default;
- `$abort` remains distinct from unexpected failure;
- `$handoff` returns to caller without Agent-to-Agent invocation;
- runtime max steps use the stricter authored/runtime ceiling;
- max-step exhaustion is `limit_reached`;
- Tool retry `max_retries` means additional attempts after the initial call;
- default error policy is visible when Loop omits it.

## Model provider checks

For each built-in provider adapter:

- model ID remains arbitrary/open string;
- standard credential/base URL resolution works without logging secrets;
- provider-native response normalizes to Harness semantic message/action structures;
- structured outcome/action requests work for supported models;
- unsupported capability is diagnosed clearly;
- provider function/tool alias maps back to canonical identity;
- usage/tokens are captured when provided;
- phase-local context is not accidentally reused across phase re-entry.

Real-provider smoke tests should be optional/gated by environment but run before release when credentials/local Ollama are available.

## EffectivePhase and prompt checks

Verify:

- top-level dependency without binding does not become model-available;
- no bindings means no bound package capabilities;
- global + phase bindings add;
- Loop `false` suppresses; `true` only permits; omission is no opinion;
- Profiles remain distinct and deterministic global-then-phase order;
- same Profile duplicate is removed;
- direct + Skill-inherited same Tool in same scope warns/de-dupes;
- runtime MCP imports are clearly marked runtime augmentation rather than authored dependency;
- capability readiness and Loop suppression reasons are independently observable;
- consumer context is lower authority than Harness/Loop/Profiles/Skills and cannot change capability topology.

## Tool and Skill checks

### `agentpm run --machine`

Verify:

- successful Tool returns stable machine envelope;
- resolution/runtime/timeout/malformed-output/subprocess/other categories remain machine-readable;
- invalid input schema fails before Tool process launch;
- invalid output schema fails after output parsing;
- runtime minimum mismatch fails before Tool launch;
- environment defaults/required vars remain compatible;
- cancellation/signal kills nested Tool process group;
- human mode remains usable and backward compatible.

### Harness ToolRuntime

Verify:

- direct Tools are invoked through an actual public `agentpm run` subprocess, not private runner shortcut;
- JSON input is piped through stdin;
- malformed model arguments are repaired before ToolRuntime and do not count as Tool failure;
- Hook-modified args are revalidated;
- ToolRuntime failure applies Loop Tool policy;
- retries create fresh public run processes;
- schema-valid domain `{ok:false}` output is treated as Tool output, not generic runner failure.

### Skills

Verify:

- bound Skill initially exposes description/resource inventory, not all content;
- entrypoint/reference loads happen on demand;
- package path escape/symlink escape rejected;
- phase-local Skill resource content does not leak automatically into later phases;
- Skill scripts never auto-run;
- inherited Tools receive Skill binding scope and still obey Loop Tool access.

## Hooks, machine protocol, and approval checks

Verify:

- version handshake/capability negotiation;
- events and control/provider requests can interleave without correlation loss;
- Hook sees only safe contract snapshot;
- Hook cannot add undeclared AgentPM Tool, alter graph/checkpoint/access/limits, or choose arbitrary Memory scope;
- invalid Hook patch is rejected/evented;
- Hook failure is fail-closed by default;
- explicit continue/fail-open behaves as configured and is visible;
- prompt/Tool Hooks operate from Node/Python and workspace process implementations through same semantic contract;
- approval callback approve/deny semantics;
- multiple checkpoints in authored order;
- plain headless without controller terminates `approval_required`;
- approval timeout is runtime failure, not rejection;
- cancellation flushes report/trace and cleans children.

## Knowledge checks

### Context

Verify:

- bound package descriptor/doc inventory appears without file bodies;
- only declared document can be loaded;
- package-relative safe path enforcement;
- document role is hint, not automatic eager semantics;
- Loop Knowledge prohibition suppresses access independently of Tool access.

### Local vector

Verify against fixture metadata similar to `devwork-maintainer-guide`:

- index/vector/chunk/source metadata consistency is checked enough for safe query;
- compatible query vector dimensions/normalization are enforced;
- public AgentPM query machinery is used where designed;
- configured EmbeddingProvider receives text + compatibility metadata and only returns vector;
- EmbeddingProvider never needs local chunks/index files;
- no compatible provider -> Knowledge surface unavailable/suppressed;
- retrieval returns normalized content/source/score/citation data.

### Full custom providers

Pinecone and pgvector:

- explicit package mapping selects provider;
- provider handshake advertises/validates served package/corpus identity where supported;
- mismatched corpus is unavailable/error rather than silent retrieval;
- no local fallback when explicitly mapped provider fails;
- backend result maps to same normalized KnowledgeResult as local;
- credentials never enter event payloads;
- provider Hook points still operate within already-authorized package.

## Memory checks

### SQLite storage

Verify actual SQLite schema/migration against `spec.md` logical model:

- store schema version;
- canonical scope JSON/hash;
- record primary keys/indexes;
- sequence ordinal ordering;
- archived records excluded from active retrieval;
- persistent operation state;
- semantic vector rows isolated by provider/model;
- database is under `.agentpm-state`, not installed package root.

### Envelope/contract

Verify:

- model can propose content only;
- runtime generates ID/scope/timestamps/version/ordinal/provenance/expiration;
- content and final generated contract validation;
- model cannot spoof another scope or provenance;
- arbitrary scope-key names work (tenant/repository/etc.);
- unresolved required scope makes relevant surface unavailable.

### Direct space models

Verify:

- document one-current logical record semantics;
- collection multiple records + declared retrieval modes;
- sequence append/chronological deterministic ordering;
- append-only direct update/delete rejection;
- lifecycle operation may apply explicit mutation despite direct append-only restriction;
- Loop read/write affects direct access only.

### Retention/capacity/governance

Verify:

- TTL from `updated_at` when present, else `created_at`;
- update refreshes expiration;
- lazy startup/read/write cleanup treats expired records inactive;
- archive vs delete behavior;
- capacity per complete scope tuple;
- `x-agentpm-persist:false` not durably stored;
- `x-agentpm-shareable:false` remains available to owning Agent/inspection but excluded from shareable export fixture behavior.

### Triggers/operations

Verify:

- record-count crossing triggers once and re-arms below threshold;
- unchanged above-threshold state does not storm-trigger;
- capacity trigger/re-arm and hard-cap write handling;
- interval baseline begins when relevant scoped state first exists;
- interval state persists across process restart;
- external operations never auto-run;
- global operation participates across phases;
- phase-bound operation only during active phase;
- operation can touch unbound declared internal spaces without exposing them directly to model;
- transform create/replace-input semantics;
- consolidate one destination output;
- delete mechanical path;
- model-assisted operation structured repair bound;
- provenance/source handling correctness;
- operation failure events and originating write/phase consequences.

### External providers

Run common contract/conformance fixtures against PostgreSQL/pgvector and Redis implementations for supported capability sets. A provider must report unsupported features rather than falsely claim compatibility.

## MCP checks

### Outward export

Verify:

- each Agent `bindings.mcp` ID becomes one Harness-managed `agentpm serve --mcp --machine` process;
- loopback default and ephemeral port selection;
- machine readiness gives actual endpoint without stderr parsing;
- only selected top-level Agent Tools exposed;
- MCP-safe name collision failure;
- runtime-incompatible Tool omission/partial surface behavior;
- external MCP call uses shared Tool runner and does not spawn nested `agentpm run` command;
- external MCP call does not invoke current Run Hooks/checkpoints/access;
- call activity appears in Harness trace/report;
- Session cleanup stops servers.

### Inward import

Verify:

- missing explicit scope rejected;
- stdio/HTTP server startup/connection;
- Tool discovery/filtering;
- phase/global scope honored;
- imported Tool identity includes server namespace;
- same Tool name from different servers does not collide internally;
- imported Tool obeys Loop `access.tools`;
- imported Tool uses Tool Hook/validation/retry/error pipeline;
- server failure is Tool failure when invocation boundary crossed;
- Agent manifest/lockfile remains unchanged.

## TUI manual/automated checks

- Start screen opens during/after bootstrap and updates readiness state.
- Agent/Loop/model/provider source visible.
- Consumer Context loaded/unavailable visible.
- Capability counts/suppressions/warnings understandable without expanding every detail.
- Missing model/provider prompts work.
- Multiple Agent selection works.
- Required trusted scope prompts work where enabled.
- Current phase/objective and recent actions visible.
- Approval UX routes through ApprovalRuntime.
- Detail toggles show prompt/Tool/Knowledge/Memory/Hook/MCP/raw event data according to trace policy.
- Cancellation works and cleans children.
- Repeated Runs reuse Session services and reload consumer context.
- Branding name/subtitle/accent displays without changing protocol/report identity.
- Small terminal/resize/loading/error states remain usable.

## Run report and trace checks

For ended, aborted, handed-off, failed, cancelled, limit-reached, and approval-required Runs verify:

- `report.json` exists and validates against report model/version;
- `events.jsonl` sequence is ordered and parseable;
- Agent/Loop/runtime/provider identities correct;
- preflight warnings/suppressions present;
- phase executions/outcomes/transitions/checkpoints present;
- Tool/MCP/Knowledge/Memory summaries present where used;
- usage/retry/repair/error counts correct;
- explicit report export path works;
- secrets absent under all content levels;
- redacted/default content policy does not unexpectedly store full prompts/results;
- report is not treated as resumable RunState.

## Template/example checks

For each Harness Template/example:

- generated workspace installs/resolves successfully;
- README instructions work from a clean environment;
- `.agentpm-state/` is gitignored and explained;
- portable Agent artifacts do not contain machine-specific provider/backend secrets/config;
- `agentpm.harness.json` contains runtime realization only;
- Template variables remain generation-time only;
- Template entrypoints are not auto-run by Harness;
- minimal example demonstrates near-zero config;
- SDK example demonstrates first-class Hook APIs;
- provider example demonstrates external provider mapping;
- MCP example demonstrates import/export direction clearly;
- full reference example exercises the promised artifact composition.

## Regression checks

- Run existing CLI/SDK suites for Tool, Agent, Template, Skill, Knowledge, Memory, Profile, and Loop behavior.
- Confirm publishing/installing artifacts does not create `.agentpm-state/` unless Harness/runtime behavior actually needs it.
- Confirm package installers never write live Memory/runtime trace state into `.agentpm` package roots.
- Confirm existing `agentpm run` human usage remains compatible despite machine/schema/version hardening.
- Confirm existing `agentpm serve --mcp` human usage/protocol remains compatible despite machine lifecycle additions.
- Confirm existing Knowledge query command remains usable outside Harness.
- Confirm Memory Blueprint build/publish still packages no live records.
- Confirm Profile/Skill/Knowledge README handling remains documentation-only.
- Confirm Phase 7A Agent/Loop metadata loaders remain metadata-only in both SDKs.
- Confirm registry web/API do not start implying that Harness config/runtime state is portable package metadata.
- Confirm Templates without Harness config continue to generate as before.

## Expected evidence

Report:

- exact commands run and pass/fail status;
- representative `agentpm harness` headless/TUI/machine output;
- preflight report snippets for ready and degraded scenarios;
- OpenAI/Anthropic/Ollama verification status and any environment-gated skips;
- Node and Python SDK Harness/Hook sample output;
- `agentpm run --machine` success/failure envelopes;
- `agentpm serve --mcp --machine` readiness/call events;
- local Knowledge/embedding provider result examples;
- Pinecone/pgvector Knowledge provider verification;
- SQLite schema/migration verification and persistent Memory examples;
- PostgreSQL/pgvector and Redis Memory provider verification;
- Memory trigger/operation traces including threshold and interval cases;
- MCP export/import endpoint/tool maps;
- TUI screenshots or terminal captures for preflight, approval, run, branding, and detail views where practical;
- sample redacted `events.jsonl` and `report.json`;
- generated Harness Template workspace trees/configs;
- any skipped external-service tests, credentials/runtime blockers, or deviations from `spec.md`.

## Out of scope

Do not require/test in Phase 7B:

- persisted Run resume/checkpoint restoration;
- automatic Agent-to-Agent invocation on `$handoff`;
- arbitrary Loop expressions/scripts/conditions;
- model-selected Memory scope authority;
- automatic external Knowledge/Memory infrastructure provisioning or index synchronization;
- arbitrary TUI plugins/layout scripting/themes beyond documented branding;
- automatic execution of Template entrypoints or Skill scripts;
- Profile semantic compliance scoring/enforcement;
- globally exposing external MCP Tools without explicit runtime scope;
- writing live runtime state into published/installed package roots.
