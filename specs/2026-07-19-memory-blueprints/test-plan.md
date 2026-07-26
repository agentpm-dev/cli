# Test Plan

## Required verification
- Verify the shared manifest schema accepts valid `kind: "memory"` packages and continues to accept all existing package kinds.
- Verify Memory Blueprint semantic validation catches invalid scope, record-type, space, governance, retention, retrieval, operation, and trigger declarations.
- Verify `agentpm memory build` generates the exact expected resolved contracts, contract index, and build metadata.
- Verify build does not modify `agent.json` or source schemas.
- Verify build output is deterministic for unchanged authored inputs.
- Verify publish requires a fresh build and never performs a build itself.
- Verify Memory packages publish, install, and appear in lockfiles.
- Verify agents and templates resolve Memory Blueprint dependencies.
- Verify Node and Python SDKs load installed Memory Blueprint metadata and generated contracts.
- Verify registry APIs and UI support public and private Memory packages.
- Verify no regression in tool, agent, template, skill, or knowledge workflows.

## Automated checks
Run the repository’s actual commands and record exact results. At minimum, include the applicable equivalents of:

### CLI / Rust
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- Focused manifest tests for Memory Blueprint schema and semantic validation.
- Focused `commands::memory` tests for build, check, inspect, and freshness behavior.
- Focused publish tests for fresh, missing, and stale Memory builds.
- Focused install/lockfile tests for direct, agent, and template Memory dependencies.

### Backend
- Run the backend unit/integration test suite.
- Run focused publish/detail/search/private-access tests for `kind=memory`.
- Run dependency relationship tests for agents and templates referencing Memory packages.
- Run migration tests if package-kind persistence requires a schema migration.

### Frontend
- Run the frontend typecheck.
- Run the frontend lint command.
- Run the frontend unit/component test suite.
- Run focused Memory package detail tests.
- Run package-kind filter/card/detail regression tests.

### Node SDK
- Run Node SDK typecheck/build.
- Run Node SDK tests.
- Run focused `loadMemory` tests.

### Python SDK
- Run Python SDK formatting/lint/type checks used by the repository.
- Run Python SDK tests.
- Run focused `load_memory` tests.

### Documentation/examples
- Validate example `agent.json` files against the shared manifest schema.
- Run `agentpm lint` against the simple and advanced Memory Blueprint examples.
- Run `agentpm memory build` against the examples.
- Run `agentpm publish` in the repository’s mocked/test publish flow against a fresh example.

## Automated scenarios

### Manifest contract
- Valid minimal Memory Blueprint with one scope, record type, and document space passes.
- Valid advanced blueprint with document, collection, sequence, retention, capacity, governance, and operations passes.
- Memory package with top-level tools/skills/knowledge/agents/templates dependencies fails.
- Agent with `memory` dependency array passes.
- Template with `template.dependencies.memory` passes.
- Non-agent/non-memory use of top-level `memory` fails.
- Invalid map keys fail.
- Unsafe source schema paths fail.

### Scope and space validation
- Unknown scope reference fails.
- Duplicate scope reference fails.
- Unknown record type reference fails.
- Duplicate record type reference fails.
- Document without `key` retrieval fails.
- Sequence without `chronological` retrieval fails.
- Document with `append_only: true` fails.
- Collection and sequence with `append_only: true` pass.
- Empty retrieval modes fail.

### Governance annotations
- Every allowed data class passes.
- Every allowed sensitivity passes.
- Boolean persist/shareable values pass.
- Invalid enum values fail.
- Non-boolean persist/shareable values fail.
- Unknown `x-agentpm-*` keyword fails.
- Nested annotations inside objects, arrays, `$defs`, and composed schemas are validated.
- Generated contracts preserve governance annotations.

### Retention and triggers
- Valid ISO 8601 TTL and interval values pass.
- Invalid duration strings fail.
- Invalid retention action fails.
- Record-count threshold zero fails.
- Record-count unknown space fails.
- Capacity trigger without space capacity fails.
- External trigger passes without additional fields.
- Arbitrary content-condition fields fail because `additionalProperties` is false.

### Operations
- Valid consolidate operation passes.
- Valid transform operation passes.
- Valid delete operation passes.
- Consolidate with missing source/output space fails.
- Operation record type not accepted by its space fails.
- Transform with more than one input fails.
- Delete with empty targets fails.
- Unsupported operation type fails.
- Invalid source handling fails.

### Contract generation
- One document pairing generates one contract.
- A space with multiple record types generates one contract per pairing.
- A record type used in multiple spaces generates distinct contracts.
- Document contract contains exact scope keys and no ordinal.
- Collection contract contains exact scope keys and no ordinal.
- Sequence contract requires ordinal.
- Contract constants match space, record type, and schema version.
- Content constraints and governance metadata are preserved.
- Contract index is sorted and path-safe.
- Rebuild removes contracts for deleted pairings.
- Unchanged rebuild produces identical index and contract bytes.
- Check mode does not create or modify files.

### Build metadata and freshness
- Build metadata contains all required fields and supported format/type.
- Manifest edit makes build stale.
- Source schema edit makes build stale.
- Contract edit makes build stale.
- Contract index edit makes build stale.
- Missing build metadata fails publish.
- Missing index fails publish.
- Missing indexed contract fails publish.
- Extra unindexed contract fails publish.
- Unsupported format version fails publish.
- Contract count mismatch fails publish.
- Fresh build passes publish readiness.
- Publish readiness check leaves all local files byte-identical.

### Inspect
- Local package directory resolves.
- Local `agent.json` path resolves.
- Installed `@namespace/name` package resolves through lockfile/install layout.
- Optional `memory:` prefix resolves.
- Text inspect reports core model details.
- JSON inspect returns structured details.
- Stale generated files are reported as stale rather than silently rebuilt.
- Non-memory target fails clearly.

### Dependency/install/lockfile
- Direct Memory package installs to `.agentpm/memory/...`.
- Agent Memory dependency resolves and installs.
- Template Memory dependency resolves during `agentpm new`.
- Lockfile uses `memory:@namespace/name@version` package keys.
- Version ranges resolve correctly.
- Missing Memory packages fail clearly.
- Conflicting Memory versions follow existing dependency-resolution rules.
- Existing lockfiles remain readable.

### SDKs
- `loadMemory` resolves a valid installed package.
- `load_memory` resolves a valid installed package.
- Both expose manifest metadata, build metadata, contract index, and contracts according to the chosen API shape.
- Both reject wrong package kinds.
- Both reject missing packages clearly.
- Neither exposes live record CRUD behavior.

### Backend/private access
- Public Memory package publish/detail/search/download works.
- Private Memory package is available to authorized namespace members.
- Private Memory package is hidden or forbidden for unauthorized users according to existing policy.
- Agent/template dependency validation accepts accessible Memory packages.
- Unauthorized private Memory dependencies are rejected.

### Frontend
- Memory badge/card/filter renders.
- Simple Memory Blueprint detail renders.
- Advanced Memory Model sections render.
- Governance annotations render readably.
- Resolved contract viewer renders envelope and content fields.
- Missing optional retention/capacity/operations sections do not break layout.
- Private access/error states match existing packages.
- Existing Knowledge and Skill detail pages remain unchanged.

## Manual checks

### Author flow
1. Run `agentpm init --kind memory --name conversation-continuity`.
2. Inspect the generated manifest, README, and source schema.
3. Run `agentpm lint` and confirm the starter passes.
4. Run `agentpm memory build`.
5. Confirm `agent.json` and the source schema were not modified.
6. Inspect `memory/contracts/index.json`, a resolved contract, and `memory/build.json`.
7. Run `agentpm memory inspect .` and `agentpm memory inspect . --json`.
8. Modify a source schema and confirm inspect reports stale state.
9. Confirm publish fails and instructs the author to run `agentpm memory build`.
10. Rebuild and confirm publish readiness succeeds.

### Contract readability
- Verify a generated document contract clearly communicates one logical document per scope tuple.
- Verify a generated sequence contract requires `ordinal`.
- Verify the exact required scope keys are obvious.
- Verify field-level governance annotations remain visible.
- Verify a consumer can identify every valid space-and-record-type contract through the index without reparsing all source files.

### Dependency flow
- Publish or use test fixtures for a Memory package.
- Add it to an agent’s top-level `memory` array and install the agent.
- Confirm the Memory package installs and appears correctly in the lockfile.
- Add it to `template.dependencies.memory`, generate a workspace, and confirm installation.

### Registry UI
- Open a public Memory Blueprint detail page.
- Verify overview, scopes, record types, spaces, operations, governance, contracts, and README are understandable.
- Verify the UI does not imply that AgentPM currently stores live memory.
- Verify semantic retrieval does not show a provider/model requirement.
- Verify a private Memory Blueprint is not exposed to unauthorized users.
- Verify responsive layout at narrow and desktop widths.

### Regression checks
- Publish and install one existing tool fixture.
- Publish and install one existing skill fixture.
- Build/publish/install one existing Knowledge fixture.
- Generate one existing template fixture.
- Confirm an existing agent with `memory: []` still validates.

## Expected evidence
Report back:

- Exact commands run and whether each passed.
- Focused test names added for manifest, build, freshness, publish, dependency, SDK, backend, and UI behavior.
- A directory tree of a built example Memory Blueprint.
- A representative generated resolved contract.
- A representative `memory/contracts/index.json`.
- A representative `memory/build.json`.
- Output from `agentpm memory inspect` in text and JSON modes.
- Output from publish failing on a stale build and succeeding after rebuild.
- A lockfile excerpt containing a Memory package and an agent/template relationship.
- Screenshots of the registry Memory Model and Record Contracts views.
- Any checks that could not be run, with the exact reason.

## Out of scope
- Live memory persistence tests.
- Store-adapter tests.
- Runtime trigger evaluation.
- TTL enforcement.
- Summarization, consolidation, transformation, or deletion execution.
- Agent bindings between operations and skills/tools.
- Harness-generated agent loop tests.
- Embedding provider or vector-index compatibility tests for Memory Blueprints.
- Cross-store synchronization or live record import/export.
- Automatic schema migration tests.
- Graph memory tests.
