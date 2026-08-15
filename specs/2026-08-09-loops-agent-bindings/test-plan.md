# Test Plan

## Required verification

Phase 7A is complete only when the Loop contract, Agent binding contract, CLI lifecycle, dependency graph, Template integration, registry behavior, database migration, web presentation, and both SDK metadata loaders are verified together without introducing runtime execution.

Required end-to-end scenarios:

1. Initialize and lint a Loop package.
2. Lint linear, branching, cyclic, approval, abort, and handoff Loop graphs.
3. Publish a valid Loop without a build step.
4. Install the Loop directly and verify `.agentpm/loops` layout.
5. Add/replace a Loop directly on a local Agent and verify singular manifest + lockfile behavior.
6. Lint an Agent using every binding surface and verify local binding-to-dependency membership checks.
7. Verify Agent lint deliberately does not resolve phase names, Memory selectors, or Loop access conflicts.
8. Install an Agent with a Loop dependency and full bindings metadata.
9. Generate a workspace from a Template with a direct Loop dependency and generated local Agents with their own Loops/bindings.
10. Search for and view the Loop in the registry and inspect Agent orchestration/binding presentation.
11. Load the installed Loop and an Agent with Loop/bindings through Node and Python SDKs.
12. Verify Loop search/trending/install statistics after the package-kind database migration.
13. Verify all seven existing package kinds and Agents/Templates without Loops continue to work.
14. Verify no Loop/Harness/runtime execution behavior was introduced.

## Automated checks

Run commands from the indicated repository unless repository scripts have changed. When scripts differ, use the configured equivalent and report the exact command used.

### CLI and Rust SDK

From the AgentPM CLI workspace:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`

At minimum, targeted tests must cover:

- manifest schema and typed parsing
- Loop graph semantic linting
- Agent binding semantic linting
- Loop init
- direct Loop install
- Agent singular Loop dependencies
- lockfile v3 singular Loop relationships
- Template direct Loop dependencies
- download/extraction roots
- Loop publish dry-run
- workspace/new flows
- shared SDK package-kind DTO serialization

### Registry API

From the API repository:

- run the current publish helper tests
- run the current install/resolve helper tests
- run the current search/trending/statistics tests
- run the current package detail/authorization tests
- run the complete API test suite using the repository's normal command, typically `pytest`

Run migration verification through the repository's established database-test process and verify upgrade/downgrade where supported.

### Registry web

From the web repository:

- run the configured formatter/linter
- run the configured TypeScript type check
- run the configured component/unit test suite
- run the production build

Expected script forms, if unchanged:

- `npm run lint`
- `npm test -- --run`
- `npm run build`

At minimum, targeted tests must cover:

- Explore Loop filter
- global search Loop dispatch
- Loop card links
- public/private Loop detail loading
- Loop Overview optional fields
- Agent Loop dependency link
- Agent global/phase bindings
- Memory binding display
- MCP surface display
- consumer-context display

### Node SDK

From the Node SDK repository:

- `npm run lint` if configured
- `npm test -- --run`
- `npm run build`

At minimum, run Loop loader and Agent loader tests covering singular Loop relationships and complete bindings metadata.

### Python SDK

From the Python SDK repository:

- run the configured formatter/linter/type checker
- `pytest`

At minimum, run new Loop loader tests and Agent loader tests covering singular Loop relationships and complete bindings metadata.

## Contract tests

### Valid Loop manifests

Verify:

- minimal Loop with one phase and implicit `complete`
- linear multi-phase Loop
- branching Loop with explicit outcomes
- cyclic Loop
- Loop with optional open-ended archetype
- Loop with `max_steps`
- Loop with Tool/Knowledge/Memory access declarations
- Loop using all three standardized terminal targets across fixtures
- Loop with approval checkpoint
- Loop with Tool retry then phase failure
- Loop using common README/license metadata

### Invalid Loop manifests / semantics

Verify rejection of:

- missing top-level structured `loop`
- structured `loop` on the wrong package kind
- Agent package reference form on the wrong package kind
- missing `entry_phase`, phases, or transitions
- empty/whitespace-only phase objective
- duplicate phase IDs
- duplicate explicit outcome IDs
- explicit outcome missing description
- unknown entry phase
- unknown transition source phase
- unknown non-terminal transition destination
- unsupported `$...` terminal target
- transition `on` not valid for source phase
- transition using implicit `complete` when explicit outcomes are present and `complete` was not declared
- missing transition for a valid phase/outcome pair
- multiple transitions for one phase/outcome pair
- unreachable phase
- graph with no reachable terminal target
- duplicate checkpoint IDs
- checkpoint before unknown phase
- checkpoint rejection to unknown target
- more than one approval checkpoint targeting the same phase
- invalid/zero/negative `max_steps`
- invalid Tool failure action
- retry without `max_retries` or `on_exhausted`
- retry with zero/negative retries
- non-retry Tool action containing retry-only fields
- `fail_phase` path without phase failure policy
- unsupported phase failure action
- extra Loop properties
- Loop-owned package dependencies
- `display_name` on Loop if current common schema still rejects it

### Agent binding schema

Verify valid Agents containing:

- Loop dependency with no bindings
- global-only bindings
- phase-only bindings with Loop present
- both global and phase bindings
- Tool bindings
- Skill bindings
- Knowledge bindings
- Profile bindings
- Memory package + spaces
- Memory package + operations
- Memory package + spaces + operations
- operation-only Memory binding
- multiple MCP surfaces
- consumer context
- an Agent with Loop but no phase bindings for some/all phases

Verify schema rejection of:

- versions in binding package identities
- object-form package references in bindings
- unsafe consumer-context paths
- empty Memory binding with neither spaces nor operations
- empty present `spaces` or `operations`
- invalid Memory key syntax
- empty MCP Tool list
- invalid MCP ID syntax
- bindings on non-Agent package kinds
- more than one direct Template Loop dependency

## Loop graph semantic checks

### Implicit outcomes

Use a phase with omitted `outcomes` and transition:

```json
{ "from": "investigate", "on": "complete", "to": "review" }
```

Verify it is valid.

Add explicit outcomes that do not contain `complete` and keep the same transition. Verify lint fails because explicit outcomes replace the implicit outcome set.

Add explicit `{ "id": "complete", ... }` and verify the transition becomes valid again.

### Deterministic transitions

For every phase, verify the validator requires exactly one transition for each valid outcome and rejects both missing and duplicate mappings.

### Reachability

Verify:

- every authored phase must be reachable from `entry_phase`
- cycles remain valid
- at least one `$end`, `$abort`, or `$handoff` target is reachable from the entry graph
- no mandatory `max_steps` is required solely because a graph contains a cycle

### Checkpoints

Verify approval semantics structurally:

- valid before-phase + rejection-to-phase
- valid rejection to `$abort`
- valid rejection to `$handoff`
- valid rejection to `$end` if retained by the final schema
- invalid unknown phase/terminal
- duplicate approval target phase rejected

No test should attempt to request actual approval or resume a run.

## Agent binding semantic lint checks

### Dependency membership

Create an Agent where each top-level dependency uses an exact/ranged package reference and each binding uses the versionless identity.

Verify successful matching for:

- Tools
- Skills
- Knowledge
- Memory
- Profiles
- MCP Tools

Verify rejection when a binding references a package absent from its corresponding top-level collection.

Verify wrong-collection membership does not count. Example: a package listed only in `skills` does not satisfy a Tool binding even if the name matches.

### Duplicate binding identities

Verify rejection of duplicate canonical identities within one global/phase collection and duplicate Memory package entries within the same binding scope.

Verify the same identity may appear once globally and once in a phase because global + phase association is additive.

### Phase binding prerequisite

Verify `bindings.phases` fails when the Agent has no top-level `loop`.

Verify `bindings.global`, `bindings.mcp`, and `bindings.consumer_context` may exist without a top-level Loop if the final schema/spec permits them independently.

### Deliberately non-resolving lint

Use fixtures where:

- phase key `totally-made-up-phase` is syntactically valid but not known locally
- Memory space `does_not_exist` is syntactically valid
- Memory operation `does_not_exist` is syntactically valid
- a Tool is bound to a phase that the referenced Loop would declare `access.tools: false`

Verify normal Agent lint does not resolve external packages and therefore does not fail or warn solely for these cases.

The only relevant binding requirement is that package identities are declared in the Agent's top-level dependency collections.

## CLI lifecycle checks

### Init and lint

Run a flow equivalent to:

```bash
mkdir /tmp/agentpm-loop-test
cd /tmp/agentpm-loop-test
agentpm init --kind loop --name research-review --description "Reusable research review loop."
agentpm lint
```

Verify:

- `agent.json` exists and uses `kind: "loop"`
- `README.md` exists and matches `readme`
- starter graph is semantically valid
- no build/generated/runtime directory exists
- no package dependencies or Agent bindings exist
- no `display_name` is introduced
- README clearly states declarative/non-execution boundaries
- `agentpm lint --strict` follows existing warning policy

### Publish

Run Loop publish dry-run and a real publish against the test registry.

Verify:

- no build command/readiness step is requested
- semantic graph validation runs before archive creation
- README/license common files are included as expected
- dependency-bearing Loops fail
- invalid graphs fail before upload/finalize
- published package kind is `loop`
- archive contains only expected common authored files according to current generic packaging rules
- canonical detail URL follows `/loops/<package-id>/v<version>/overview`

### Direct install

Install a Loop with no local Agent and with a local Agent.

Verify:

- package extracts under `.agentpm/loops`
- no Loop execution is attempted
- local Agent receives singular top-level `loop`
- reinstall/update of same Loop follows existing range semantics
- direct installation of a different Loop replaces the singular Agent Loop reference
- bindings are not auto-created or modified
- package key is `loop:@namespace/name@version`

### Agent graph

Publish or fixture an Agent with:

- one Loop dependency
- Tools/Skills/Knowledge/Memory/Profiles
- complete bindings metadata

Verify:

- backend resolve expands the singular Loop dependency
- Loop remains a leaf
- lock root contains exactly one singular Loop package key
- authored bindings remain manifest metadata rather than lockfile relationships
- a direct package spec returned as `kind: "loop"` is accepted when outbound resolution uses existing generic behavior
- wrong-kind Loop dependency fails
- an Agent without `loop` remains valid and installs according to prior behavior

### Frozen/refresh/reachability

Verify:

- frozen install succeeds with a complete v3 singular Loop relationship
- frozen install fails when required Loop package/relationship is absent or wrong-kind
- refresh updates Loop dependency resolution normally
- changing/removing the Agent Loop prunes unreachable old Loop packages according to existing behavior
- the same exact Loop referenced by multiple reachable Agent roots is deduplicated
- older v3 locks omitting `loop` still deserialize
- no invented reserved Loop migration occurs unless current repository data requires it

### Template/new

Create a Template declaring one direct Loop and generated local Agents declaring same/different Loops.

Verify:

- resolver request includes direct and generated-Agent Loop requirements
- Loops install under `.agentpm/loops`
- synthesized root `agent.json` receives exact resolved direct Template Loop
- generated local Agents retain their own authored `loop` and `bindings`
- direct Template Loop is not copied into every local Agent
- workspace lock roots contain correct singular Loop relationships
- Template with more than one direct Loop fails
- no Loop-specific prompt is shown
- Template variables do not mutate installed Loop package content or runtime binding semantics

## API and database checks

### Package-kind support

Verify Loop support in:

- publish init/finalize
- install resolve/init/finalize
- package/version details
- namespace listings
- search/trending filters/results
- yanking/signing/security metadata
- statistics responses
- public/private authorization

### Agent/Template dependency relationships

Verify:

- Agent singular `loop` persists and expands in install graphs
- only stored kind `loop` may satisfy Agent Loop relationships
- Loop packages have no outgoing package relationships
- Template direct Loop relationship validates/persists and remains deferred to `agentpm new` for actual Template workspace expansion
- backend does not evaluate `bindings` against Loop/Memory package contents

### Migration

Apply the migration to a database containing rows for all seven existing package kinds.

Verify:

- existing rows/views remain valid
- Loop row can be inserted/published after upgrade
- `tools_kind_check` accepts exactly the intended eight kinds and rejects unsupported kinds
- `trending_tools` contains Loop rows and partitions rank by `kind`
- `tool_search_index` contains Loop rows using existing indexed text fields
- Loop installs increment per-package and aggregate counters
- all recreated indexes/triggers/views exist
- downgrade follows repository policy and does not silently corrupt data

## Registry web checks

Verify:

- Loop appears as a first-class Explore filter/card/badge
- global search routes Loop results correctly
- public/private Loop detail pages use existing authorization behavior
- Loop Overview renders phases, objectives, implicit/explicit outcomes, transitions, terminals, access, limits, checkpoints, and error policy
- README remains a separate documentation surface
- UI does not present archetype as a runtime enum or special execution mode
- Agent detail shows resolved Loop dependency
- Agent detail shows global bindings
- Agent detail shows phase bindings grouped by authored phase key
- Agent detail shows Memory spaces/operations without claiming cross-package validation
- Agent detail shows named MCP surfaces and member Tools
- Agent detail shows consumer-context filename without trying to load it
- UI does not flag Loop-access-versus-binding conflicts as invalid package state
- no run/approval/model/provider/MCP-port controls appear

## SDK checks

### Node

Verify `loadLoop`:

- resolves installed versioned/unversioned specs according to current loader behavior
- supports `loopDirOverride`
- returns typed common + Loop metadata
- rejects wrong-kind/malformed/missing manifests
- does not read README as orchestration instructions

Verify `loadAgent`:

- exposes resolved singular Loop relationship
- exposes locked-but-missing Loop paths using existing nullable conventions
- preserves typed authored `bindings`
- exposes Memory binding selectors, MCP bindings, and consumer context as metadata
- does not resolve phase keys, Memory selectors, access conflicts, or effective global+phase capabilities

Verify generic Tool `load()` guides Loop callers to `loadLoop`.

### Python

Run equivalent checks for `load_loop`, `load_agent`, public exports, wrong-kind guidance, and metadata-only behavior.

Confirm Node/Python agree on Loop and binding field names/relationship semantics.

## Manual checks

- Review an initialized Loop and confirm it reads as an orchestration contract rather than a runnable workflow program.
- Publish a Loop through the normal CLI flow and confirm no build-related messaging appears.
- Inspect a full Loop manifest and confirm a new custom archetype can be understood entirely through phases/outcomes/transitions.
- Browse Explore and Loop detail pages and confirm the graph is understandable without implying registry execution.
- Open an Agent with a Loop and bindings and confirm the composition is transparent: dependency versions live at the Agent top level while bindings use versionless identities.
- Confirm Memory selectors use real Blueprint snake_case identifiers and remain visibly authored references rather than validated live store state.
- Confirm MCP surfaces show logical groupings but no host/port/transport values.
- Confirm consumer-context display makes clear the file is consumer-owned and optional.
- Generate a Template workspace with direct/root and local-Agent Loops and inspect `agent.json`, local Agent manifests, `.agentpm/loops`, and `agent.lock`.
- Load the same installed Loop and Agent through Node and Python SDKs and compare metadata.
- Confirm no CLI/API/UI/SDK surface claims that Phase 7A executes, enforces, approves, retries, starts MCP, reads consumer context, or selects a model/provider.

## Regression checks

- Initialize, lint, publish, and install representative fixtures for Tool, Agent, Template, Skill, Knowledge, Memory, and Profile.
- Generate a Template workspace with no Loop dependencies and confirm existing behavior remains unchanged.
- Run existing Skill Tool-dependency tests.
- Run existing Knowledge and Memory build/publish tests to ensure Loop work does not alter their build requirements.
- Run existing Profile lint/publish/install/SDK tests to ensure Loop access/binding concepts do not leak into Profile semantics.
- Verify Agents without `loop` and `bindings` remain valid.
- Verify Agents still require top-level `tools` according to the existing invariant.
- Verify Templates without `dependencies.loops` remain valid.
- Verify old v3 lock fixtures without Loop relationship fields remain readable.
- Verify package-kind search/trending/statistics continue to include all existing kinds.
- Verify private namespace behavior remains consistent across all kinds.

## Expected evidence

Report:

- exact commands run and pass/fail status
- CLI output for Loop init, lint, publish dry-run, direct install, Agent install, and Template `new`
- representative valid/invalid Loop lint output with instance paths
- Agent binding lint output for missing top-level dependency references
- evidence that intentionally unknown phase/Memory selector/access-conflict fixtures do not fail local Agent lint
- generated/updated `agent.json` snippets showing singular Loop refs and versionless bindings
- `agent.lock` snippets showing singular Loop relationships/package keys
- database migration upgrade/downgrade results
- query output showing Loop rows in search/trending/statistics
- screenshots of Explore, Loop Overview/README/Security, and Agent orchestration/binding presentation
- Node and Python loader output snippets showing equivalent Loop and binding metadata
- any skipped commands, environmental blockers, or unverified migration/UI behavior
- any deviation from `spec.md`

## Out of scope

Do not test or implement:

- `agentpm harness`
- Loop execution
- model/provider selection or calls
- prompt/system-message compilation
- Tool invocation from Loop phases
- Knowledge retrieval execution
- live Memory CRUD/persistence/scope resolution
- Memory lifecycle trigger evaluation or operation execution
- approval prompting/suspension/resume
- retry/backoff execution
- handoff execution or Agent-to-Agent delegation
- MCP process startup, host/port/transport behavior
- reading consumer-context files
- effective binding calculation/enforcement
- cross-package phase-name validation during Agent lint/publish
- cross-package Memory selector validation during Agent lint/publish
- access-versus-binding conflict rejection/warnings during package lifecycle
- Profile merge/precedence semantics
- custom expression languages or transition conditions
- nested Loop/binding full-text search
