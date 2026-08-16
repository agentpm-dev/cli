# Tasks

## Milestone 1: Loop Manifest Contract
> Scope note: establish `kind: "loop"` as the final first-class package contract and define the complete authored Loop vocabulary in JSON Schema and Rust typed-manifest layers. Overload top-level `loop` so Loop packages use structured metadata while Agents use a singular package reference. Add Agent `bindings` shape and singular Template `loop` dependency shape at the schema/type level. This milestone validates manifest shape and typed parsing only; it does not perform graph semantic linting, scaffold files, resolve dependencies, write lockfiles, publish/install packages, render registry UI, load Loops through SDKs, or execute orchestration.
- [ ] Add `loop` to the shared manifest package-kind enum and add a `kind: "loop"` branch to the top-level `oneOf` requiring structured top-level `loop` metadata.
- [ ] Add or extract a reusable stable lowercase kebab-case identifier definition matching `^[a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,63}$` for Loop phase IDs, outcome IDs, checkpoint IDs, and MCP binding IDs.
- [ ] Add a versionless `packageIdentity` schema definition accepting only `@namespace/package` and rejecting versions, ranges, and object-form package references.
- [ ] Add strict Loop schema definitions for `archetype`, `entry_phase`, `limits`, phases, access metadata, explicit outcomes, transitions, standardized terminal targets, checkpoints, and error policy exactly as defined in `spec.md`.
- [ ] Keep `archetype` optional and open-ended; do not define a closed enum or attach runtime semantics to archetype values.
- [ ] Require `loop.entry_phase`, non-empty `loop.phases`, and non-empty `loop.transitions`.
- [ ] Require each phase to have `id` and non-empty `objective`.
- [ ] Define optional phase `access.tools`, `access.knowledge`, and `access.memory.read|write` booleans; require `access.memory` to contain at least one field when present.
- [ ] Define explicit `outcomes` as non-empty arrays of objects requiring `id` and `description`; keep implicit `complete` behavior as a semantic rule when `outcomes` is omitted.
- [ ] Define transition objects with exactly `from`, `on`, and `to` and no expression, callback, metadata, or code escape hatches.
- [ ] Define terminal targets as exactly `$end`, `$abort`, and `$handoff` where transition/checkpoint targets permit terminals.
- [ ] Define optional `limits.max_steps` as a positive integer and do not introduce `max_turns`.
- [ ] Define approval checkpoints with required `id`, `type: "approval"`, `before_phase`, and `on_reject`.
- [ ] Define Tool failure policy union/cross-field shape for `retry`, `fail_phase`, `abort`, and `handoff`, including retry-only `max_retries` and `on_exhausted` fields.
- [ ] Define phase failure policy as `abort | handoff`.
- [ ] Use `additionalProperties: false` on all Loop-specific and Agent-binding-specific objects.
- [ ] Overload top-level `loop` so `kind: "loop"` uses Loop metadata and `kind: "agent"` uses the existing package-reference shape.
- [ ] Reject top-level `loop` for package kinds other than Agent and Loop.
- [ ] Preserve existing Agent compatibility: `loop` remains optional and the existing required top-level `tools` array is unchanged.
- [ ] Add top-level Agent-only `bindings` with optional `global`, `phases`, `mcp`, and `consumer_context` fields.
- [ ] Define global and phase binding objects supporting versionless `tools`, `skills`, `knowledge`, `profiles`, and structured `memory` arrays.
- [ ] Require present binding arrays to be non-empty and schema-unique where exact uniqueness can be expressed.
- [ ] Define Memory binding objects with required versionless `package`, optional non-empty unique `spaces`, optional non-empty unique `operations`, and a requirement that at least one selector is present.
- [ ] Reuse the Memory Blueprint snake_case key contract for bound `spaces` and `operations`.
- [ ] Define MCP bindings as a non-empty array of objects requiring stable `id` and non-empty unique versionless Tool identities.
- [ ] Define `consumer_context.file` using the existing `safeRelativePath` schema and no other v1 fields.
- [ ] Restrict `bindings` to `kind: "agent"`; reject bindings on all other package kinds.
- [ ] Add optional singular `loop` to `templateDependencyGroup` using the existing package-reference shape.
- [ ] Add defaulted `loop: Option<PackageReference>` to Rust `TemplateDependencies` or the repository-equivalent type.
- [ ] Add Rust typed Loop structs/enums and Agent binding structs matching the schema.
- [ ] Add `LoopManifest` and `parse_loop_manifest`, including clear wrong-kind errors.
- [ ] Extend Agent typed-manifest parsing with optional singular Loop dependency and optional typed bindings.
- [ ] Add schema tests for a minimal valid Loop using implicit `complete` outcomes.
- [ ] Add schema tests for a full valid Loop using every supported field and all three terminal targets.
- [ ] Add schema tests for Agents using every binding surface and every bindable package kind.
- [ ] Add schema tests proving binding identities reject versions/ranges/object-form package refs while top-level dependencies continue to accept normal package refs.
- [ ] Add schema failure tests for invalid stable IDs, empty outcomes, unsafe consumer-context paths, empty Memory selectors, malformed MCP bindings, unsupported terminal names, unsupported access fields, and extra properties.
- [ ] Add schema tests proving Templates accept zero or one direct Loop and reject plural/array-shaped Template Loop declarations.
- [ ] Add tests proving Loop packages cannot declare package dependencies or Agent bindings.
- [ ] Update embedded-schema tests and hardcoded seven-kind schema assertions for the new eighth package kind.

## Milestone 2: Loop and Agent Binding Semantic Linting + Initialization
> Scope note: add semantic validation for Loop graph determinism and the intentionally local Agent binding-to-dependency integrity rules, then provide a valid `agentpm init --kind loop` starter. This milestone does not resolve Loop dependencies while linting Agents, inspect Memory Blueprint contents, evaluate access-versus-binding conflicts, create build outputs, resolve packages, write lockfiles, publish/install packages, execute Loops, start MCP servers, or read consumer-context files.
- [ ] Add a reusable Loop semantic validator invoked by normal manifest lint after schema validation and typed Loop parsing succeed.
- [ ] Reject required Loop text values that are empty after trimming, including phase objectives and explicit outcome descriptions.
- [ ] Reject optional authored Loop text such as `archetype` when present but empty after trimming.
- [ ] Reject duplicate phase IDs with precise issue paths identifying the later duplicate.
- [ ] Reject duplicate explicit outcome IDs within the same phase with precise issue paths.
- [ ] Validate that `entry_phase` references a declared phase.
- [ ] Define the implicit valid outcome set for phases without `outcomes` as exactly `{complete}`.
- [ ] Define the explicit valid outcome set for phases with `outcomes` as exactly the authored outcome IDs, including `complete` only when explicitly authored.
- [ ] Validate every transition `from` phase exists.
- [ ] Validate every non-terminal transition `to` target exists.
- [ ] Validate every transition `on` belongs to the valid outcome set of its source phase.
- [ ] Reject duplicate/ambiguous transitions with the same `from` + `on` pair.
- [ ] Require exactly one transition for every valid phase/outcome pair.
- [ ] Reject unreachable phases using graph traversal from `entry_phase`.
- [ ] Require at least one standardized terminal target to be reachable from `entry_phase`.
- [ ] Permit cycles and do not require `max_steps`; do not flag intentional iterative graphs as invalid solely because they can loop.
- [ ] Validate checkpoint IDs are unique.
- [ ] Validate every checkpoint `before_phase` exists.
- [ ] Validate checkpoint `on_reject` is either a declared phase or one of the three terminal targets.
- [ ] Reject multiple approval checkpoints targeting the same phase in Phase 7A.
- [ ] Validate Tool error-policy cross-field requirements and forbidden fields for retry versus non-retry actions.
- [ ] Require `phase_failure` when `tool_failure` can resolve to `fail_phase`.
- [ ] Keep graph/semantic issues rooted under `/loop` using existing `LintIssue` conventions.
- [ ] Add Agent binding semantic validation to the shared Agent lint path without resolving external packages.
- [ ] Canonicalize top-level Tool/Skill/Knowledge/Memory/Profile dependency references to versionless package identities for membership comparison.
- [ ] Reject any global or phase Tool binding whose package identity is not declared in top-level `tools`.
- [ ] Reject any global or phase Skill binding whose package identity is not declared in top-level `skills`.
- [ ] Reject any global or phase Knowledge binding whose package identity is not declared in top-level `knowledge`.
- [ ] Reject any global or phase Memory binding whose `package` identity is not declared in top-level `memory`.
- [ ] Reject any global or phase Profile binding whose package identity is not declared in top-level `profiles`.
- [ ] Reject any MCP Tool binding whose package identity is not declared in top-level `tools`.
- [ ] Reject duplicate canonical package identities within one binding array where schema exact uniqueness is insufficient.
- [ ] Reject duplicate Memory package entries within the same global or phase binding scope.
- [ ] Reject duplicate MCP IDs with precise issue paths.
- [ ] Reject `bindings.phases` when top-level Agent `loop` is absent.
- [ ] Do not resolve the referenced Loop or validate phase keys against actual Loop phases.
- [ ] Do not resolve Memory packages or validate bound space/operation names against Blueprint contents.
- [ ] Do not compare Loop `access` metadata with Agent bindings and do not emit compatibility warnings for conflicts.
- [ ] Do not inspect Profile/Skill/Knowledge/Tool contents or define effective capability/precedence behavior.
- [ ] Do not read or require the consumer-context file during lint.
- [ ] Add `Loop` to `InitKind` and update `agentpm init` help text.
- [ ] Add a Loop starter `agent.json` with a small but representative graph using an entry phase, an iterative phase, an explicit decision outcome, and terminal transition.
- [ ] Include `readme: "README.md"` and no dependencies, bindings, generated outputs, runtime/provider configuration, or `display_name`.
- [ ] Add a Loop README asset explaining Loop versus Agent versus Harness responsibilities, implicit `complete`, graph-defined control flow, and that the README is documentation only.
- [ ] Make `agentpm init --kind loop` create only `agent.json` and `README.md`.
- [ ] Keep `--mode` Knowledge-only and reject non-default mode usage with Loops using existing behavior.
- [ ] Add Loop init tests for generated files, starter schema/semantic validity, README content, and absence of generated directories.
- [ ] Add Loop semantic lint tests for valid linear, branching, cyclic, approval, handoff, and abort graphs.
- [ ] Add Loop semantic failure tests for every graph/checkpoint/error-policy rule above.
- [ ] Add Agent binding lint tests for every dependency-kind membership rule and the intentionally non-resolving cases.
- [ ] Verify lint pretty/JSON/NDJSON/strict/fix behavior remains consistent with existing shared lint conventions and `--fix` never rewrites authored Loop or binding content.

## Milestone 3: Loop Package Kind, Install Root, and Resolver Plumbing
> Scope note: make `loop` a recognized package kind across Rust CLI/shared SDK resolver and installer structures, preserve backend-authoritative Loop kinds, and establish `.agentpm/loops` extraction behavior. This milestone provides foundational package-kind/download plumbing only; it does not yet activate Agent singular Loop dependency resolution, mutate Agent manifests, create first-class lockfile Loop relationships, publish Loops, process Template Loops in `agentpm new`, render UI, load Loops through language SDKs, or execute orchestration.
- [ ] Add `Loop` to CLI semver `PackageKind` and shared Rust SDK install `PackageKind` with snake_case serialization as `"loop"`.
- [ ] Update exhaustive package-kind matches, conversions, displays, parsers, supported-kind messages, API DTO conversions, and tests required for workspace compilation.
- [ ] Ensure package-key construction/parsing supports `loop:@namespace/name@1.0.0`.
- [ ] Preserve backend-authoritative direct package kind behavior so a direct bare package spec resolved as `kind: "loop"` remains Loop throughout conversion/download/extraction.
- [ ] Treat Loop manifests as dependency leaves and do not inspect structured `loop` metadata for dependencies.
- [ ] Add `.agentpm/loops` to shared install-directory creation and `InstallRoots`.
- [ ] Thread Loop install roots through every existing constructor/call site, including workspace/template paths, without yet enabling Template Loop dependency processing.
- [ ] Route Loop artifacts to `.agentpm/loops/<namespace>/<name>/<version>` using existing safe extraction behavior.
- [ ] Add installed Loop root/manifest helper functions parallel to other metadata-only kinds where shared code needs them.
- [ ] Reuse namespace/name/version sanitization, archive traversal defense, integrity verification, atomic replacement, and cleanup behavior.
- [ ] Keep Template artifacts rejected by generic `agentpm install` according to existing semantics.
- [ ] Add unit tests for Loop package-kind serialization/deserialization, package keys, resolver conversions, authoritative response-kind preservation, and installed path construction.
- [ ] Add download/extraction tests covering canonical paths, integrity success/failure, malformed artifacts, traversal rejection, replacement, and cleanup.
- [ ] Add a mocked direct Loop install extraction test without local Agent mutation; defer root/lock/dependency semantics to Milestone 4.
- [ ] Add regression coverage for all seven existing package kinds.

## Milestone 4: Agent Loop Dependencies, Bindings, and Lockfile Relationships
> Scope note: activate singular Agent `loop` dependency resolution, preserve authored `bindings`, and add first-class singular Loop relationships to lockfile v3. Follow existing Agent dependency/install patterns while keeping Loops as leaves. This milestone also handles direct Loop installation into a local Agent manifest. It does not resolve binding phase names or Memory selectors, execute bindings, process Template direct Loops, publish/persist Loops, or add language SDK loaders.
- [ ] Add optional singular `loop` relationships to `LockedRoot`, `LockRoot::LocalAgent`, `LockRoot::RegistryAgent`, and repository-equivalent root/relationship models while preserving locks that omit the field.
- [ ] Keep lockfile version 3 unless an actual serialization incompatibility is discovered.
- [ ] Confirm whether any existing `reserved.loop`/`reserved.loops` compatibility field exists; if none exists, do not invent one or add migration code.
- [ ] Parse Agent top-level `loop` into exactly one Loop package requirement.
- [ ] Resolve local Agent Loop dependencies and record the canonical Loop package key on the local Agent root.
- [ ] Read Loop dependencies from installed registry Agent manifests and record them on registry Agent roots.
- [ ] Require Agent Loop references to resolve to stored/resolved package kind `loop`.
- [ ] Treat Loop packages as leaves.
- [ ] Include singular Loop relationships in lock construction, normalization, comparison, serialization, reachability, package retention, pruning, refresh, root replacement, and deterministic output.
- [ ] Include Loop relationships in transitive traversal of installed registry Agents.
- [ ] Add direct dependency-kind detection for Loops and update a local Agent's singular top-level `loop` when a Loop is installed directly.
- [ ] Preserve existing range/update behavior when reinstalling/updating the same Loop package.
- [ ] Replace the previous singular Loop reference when a different Loop package is directly installed into the same local Agent rather than creating multiple Loop references.
- [ ] Do not create or mutate `bindings` automatically during Loop install.
- [ ] Ensure direct standalone Loop installs follow established leaf-package behavior rather than Agent/Skill root semantics.
- [ ] Ensure frozen installs require the expected singular Loop relationship and fail clearly for missing, wrong-kind, or stale lock data.
- [ ] Ensure Loop upgrades/removal prune unreachable Loop packages according to current graph behavior.
- [ ] Preserve authored Agent `bindings` as manifest metadata; do not duplicate bindings into lockfile relationships.
- [ ] Add tests for local and registry Agent Loop dependencies, no-Loop Agents, direct Loop install/update/replacement, wrong-kind/missing dependencies, frozen mode, refresh, reachability, pruning, shared exact Loop packages across Agent roots, and deterministic serialization.
- [ ] Add binding-regression tests confirming normal install/lock processing preserves Agent manifests and does not cross-validate phase names, Memory selectors, access conflicts, MCP runtime details, or consumer-context files.

## Milestone 5: Loop Publishing, Backend Persistence, and Dependency Resolution
> Scope note: make Loop packages publishable and persistable as immutable leaf artifacts, add backend/database package-kind support, and recognize Agent singular Loop and Template direct Loop relationships. Reuse existing package publishing, common README/license, signing, namespace, and dependency-resolution patterns. This milestone does not add generated outputs, runtime execution, cross-package binding validation, registry UI, or language SDK loaders.
- [ ] Add `PublishManifest::Loop`, Loop parse dispatch, supported-kind messaging, and exhaustive CLI publish handling.
- [ ] Add Loop archive packaging containing `agent.json` and supported manifest-declared README/license files through existing common behavior only.
- [ ] Require normal schema + Loop semantic validation before Loop archive creation.
- [ ] Do not add a Loop build check, generated metadata, entrypoint, script payload, or package-specific archive layout.
- [ ] Reject Loop package dependencies in CLI publish preparation.
- [ ] Ensure Agent publish accepts top-level `loop` and `bindings` after local schema/semantic validation and does not resolve phase/Memory/policy conflicts for binding validation.
- [ ] Add CLI publish tests for minimal/full Loops, README/license handling, invalid graphs, forbidden dependencies, exact archive contents, and no-build behavior.
- [ ] Add `loop` to backend package-kind types, validators, publish dispatch, supported-kind errors, detail URL helpers, and resolver models.
- [ ] Allow Loop publishing through existing generic package publish authorization while preserving any legacy Tool-only scope behavior.
- [ ] Support Loop publish-init/finalize through existing upload, signing, attestation, malware scan, private namespace, idempotency, and immutable version flows.
- [ ] Validate server-side Loop package invariants needed for defense in depth without building an independent graph compiler beyond established backend validation patterns.
- [ ] Preserve complete Loop manifests in existing manifest JSON storage; add no Loop-specific database columns.
- [ ] Add Agent singular top-level `loop` to backend dependency extraction, relationship persistence, and normal Agent install-graph expansion.
- [ ] Require Agent Loop dependencies to resolve to stored package kind `loop`.
- [ ] Keep Loop packages as dependency leaves.
- [ ] Add Template `dependencies.loop` to publish-time dependency extraction/validation/relationship persistence.
- [ ] Require Template Loop references to resolve to stored package kind `loop`.
- [ ] Keep Template direct Loop installation deferred to `agentpm new`; generic Template install behavior remains unchanged.
- [ ] Do not parse/evaluate Agent `bindings` for runtime policy or cross-package content validation on the backend.
- [ ] Add a package-kind database migration parallel to the latest Profile migration.
- [ ] Update `tools_kind_check` to allow `loop`, producing the exact eight-kind allowlist in `spec.md`.
- [ ] Update install-completion statistics triggers/functions for Loop installs.
- [ ] Recreate `trending_tools` with Loop as its own `kind` partition and recreate required indexes/views in repository-prescribed order.
- [ ] Recreate `tool_search_index` without indexing nested Loop or Agent binding metadata.
- [ ] Provide downgrade behavior consistent with existing artifact-kind migrations.
- [ ] Add backend tests for Loop publish, public/private namespaces, signing, common files, direct resolution, authoritative kind, Agent Loop expansion, Template Loop relationships, wrong-kind/missing/private cases, and Loop leaf behavior.
- [ ] Add migration verification for Loop rows, search inclusion, per-kind trending, install statistics, and preservation of all existing kinds.

## Milestone 6: Backend Read APIs, Search, Trending, and Statistics
> Scope note: complete first-class read/discovery support for persisted Loops and expose authored Agent orchestration metadata through existing generic manifest/detail APIs. Reuse existing package endpoints and authorization behavior. This milestone does not alter the database migration, execute Loops, add cross-package validation, render frontend pages, or add SDK loaders.
- [ ] Add `loop` to remaining backend package-kind enums, Literals, DTOs, serializers, query validators, namespace listings, route allowlists, and read-side hardcoded lists.
- [ ] Ensure generic package/version detail APIs support public and authorized private Loops and preserve `kind: "loop"`.
- [ ] Return complete structured Loop metadata through the stored manifest without adding Loop-specific API families or duplicated fields unless existing generic package detail conventions require a thin presentation wrapper.
- [ ] Support common README, license, signing, security, yanking, version-listing, and latest-version behavior for Loops.
- [ ] Add Loop support to namespace listings, search filters/results, trending filters/results, and generic package statistics.
- [ ] Keep full-text search limited to existing package name/namespace/description fields.
- [ ] Preserve private namespace authorization and safe-not-found behavior.
- [ ] Ensure canonical Loop links use `/loops/<package-id>/v<version>/overview` where backend URL helpers emit frontend routes.
- [ ] Ensure artifact-specific APIs for other kinds reject Loops according to established wrong-kind behavior.
- [ ] Do not add Loop run/inspect/build/execution/graph-simulation/compatibility APIs.
- [ ] Confirm Agent generic manifest/detail responses preserve top-level `loop` and `bindings` exactly as authored.
- [ ] Add backend tests for Loop details, README/security, namespace visibility, search/trending/stats, private auth, yanking, serialization, and negative artifact-specific endpoints.
- [ ] Add regression coverage for all existing package kinds and Agent manifests without Loop/bindings.

## Milestone 7: Template `new` and Workspace Integration
> Scope note: add direct Template Loop support to `agentpm new`. Resolve/install at most one direct Template Loop, write the exact resolved reference into the synthesized root Agent, and preserve Loop dependencies/bindings authored by generated local Agents. Do not create workspace-level Loop roots, generate bindings, execute orchestration, or reinterpret Template variables as runtime configuration.
- [ ] Include `template.dependencies.loop` in `agentpm new` resolver requests using `PackageKind::Loop`.
- [ ] Enforce singular direct Template Loop handling in schema and runtime defensive validation.
- [ ] Include Loop requirements declared by rendered local Agent manifests through the normal Agent dependency parser.
- [ ] Apply existing version resolution, wrong-kind validation, private access, deduplication, and conflict behavior.
- [ ] Install resolved Loops under `.agentpm/loops/<namespace>/<name>/<version>`.
- [ ] Materialize the direct Template Loop as an exact-version top-level `loop` reference on the synthesized root Agent.
- [ ] Preserve Loop dependencies and bindings explicitly declared by generated local Agents.
- [ ] Do not copy the direct Template Loop or root bindings into every generated Agent.
- [ ] Include root/local Agent singular Loop relationships in workspace lock generation using Milestone 4 support.
- [ ] Deduplicate shared exact Loop packages across Agent roots according to existing lock behavior.
- [ ] Do not add Loops to `WorkspacePackageRoots` or `agentpm.workspace.json` if the existing synthesized-root-Agent pattern is sufficient.
- [ ] Do not prompt for Loop/binding values, interpolate Template variables into installed Loop package content, or synthesize Agent bindings automatically.
- [ ] Ensure subsequent workspace install/frozen flows preserve Loop relationships and authored bindings.
- [ ] Add tests for direct Template Loop, no direct Loop, generated local Agents with same/different Loops, private/missing/wrong-kind Loops, synthesized root Agent output, workspace locks, reinstall, frozen mode, and bindings preservation.
- [ ] Add regression coverage for existing Tool/Agent/Skill/Knowledge/Memory/Profile Template behavior.

## Milestone 8: Registry Web Experience and Agent Orchestration Presentation
> Scope note: expose Loops as a first-class registry package type and make Agent Loop/binding composition inspectable using existing package UI patterns. This milestone renders authored metadata only; it does not execute graphs, calculate effective bindings, validate cross-package phase/Memory references, merge Profiles, start MCP servers, read consumer context, or add runtime configuration controls.
- [ ] Add `loop` to frontend package-kind unions for search, trending, statistics, namespace lists, package details, dependency rendering, and route dispatch.
- [ ] Add typed Loop manifest/API models matching the structured contract.
- [ ] Add typed Agent Loop/bindings models matching the Agent manifest contract.
- [ ] Add a thin Loop fetch helper using existing generic package/version/README/Security APIs.
- [ ] Add Loop cards, Explore filters, global search dispatch, trending, namespace listings, badges, route generation, and any all-package-kind landing/discovery surfaces.
- [ ] Add canonical versioned Loop pages at `/loops/<loopId>/v<version>/overview` following current route conventions.
- [ ] Provide Overview, README, and Security tabs only unless current generic package layout requires an equivalent minimal set.
- [ ] Render Loop archetype, entry phase, phases/objectives, implicit/explicit outcomes, transitions, access metadata, limits, checkpoints, error policy, and terminal targets defensively.
- [ ] Make graph/control-flow presentation understandable without inventing runtime state or requiring a new visualization library; reuse existing cards/tables/code patterns before introducing custom graph infrastructure.
- [ ] Clearly label Loop content as declarative orchestration metadata rather than an executing workflow.
- [ ] Add Loop dependency presentation to Agent and Template detail surfaces, preserving resolved versions when relationship data provides them.
- [ ] Add Agent orchestration/bindings presentation when authored: global bindings, phase bindings, Memory spaces/operations, MCP surfaces, and consumer-context path.
- [ ] Keep binding package identities visibly separate from resolved dependency version links where possible; do not fabricate resolved phase/Memory validity.
- [ ] Do not present Loop access conflicts as package errors or claim AgentPM registry enforcement.
- [ ] Do not add run buttons, model/provider controls, prompt previews, approval actions, MCP host/port settings, or consumer-context editors.
- [ ] Add component/route tests for Loop cards/filters/detail pages, optional fields, public/private packages, wrong-kind/missing packages, Agent Loop links, all binding surfaces, and responsive/empty states.
- [ ] Audit canonical URLs, breadcrumbs, metadata, loading, error, 404, and version-not-found behavior.

## Milestone 9: Node SDK
> Scope note: add typed Node SDK support for locating/loading installed Loop metadata and exposing Agent singular Loop relationships plus authored binding metadata. `loadLoop` and `loadAgent` remain metadata loaders only; they do not execute or validate cross-package orchestration semantics, calculate effective bindings, start MCP servers, read consumer context, compile prompts, or invoke the future harness.
- [ ] Add `loop` to public package-kind unions, package models, lockfile package/root types, and installed-root handling.
- [ ] Add complete public Loop interfaces for archetype, phases, access, outcomes, transitions, terminals, limits, checkpoints, and error policy.
- [ ] Add complete public Agent binding interfaces for global/phase package identities, Memory bindings, MCP bindings, and consumer context.
- [ ] Add typed Agent manifest support for optional top-level singular `loop` and `bindings`.
- [ ] Add loaded Loop result and `LoadLoopOptions` types consistent with existing metadata-only loaders.
- [ ] Implement `loadLoop` using installed package resolution parallel to `loadProfile` / closest current metadata-only loader.
- [ ] Support `loopDirOverride` following existing test conventions.
- [ ] Return package identity/key/integrity/root/manifest path/typed manifest/typed `loop` metadata according to current SDK conventions.
- [ ] Reject missing packages, malformed manifests, wrong kinds, and missing structured Loop metadata with clear errors.
- [ ] Update generic Tool `load()` detection/guidance to direct Loop callers to `loadLoop`.
- [ ] Update `loadAgent` to expose resolved singular Loop relationships from first-class lock roots, including locked-but-missing nullable path behavior consistent with other dependencies.
- [ ] Preserve authored Agent `bindings` in typed returned manifest/result metadata.
- [ ] Do not resolve phase keys against the loaded Loop automatically.
- [ ] Do not resolve Memory spaces/operations against Memory packages automatically.
- [ ] Do not calculate global+phase unions, access constraints, Profile precedence, or MCP effective surfaces.
- [ ] Do not read `consumer_context.file`.
- [ ] Do not invoke `agentpm serve`, `agentpm harness`, model providers, Tools, Memory, or Knowledge retrieval.
- [ ] Export `loadLoop` and all public Loop/binding types.
- [ ] Add tests for direct Loop loading, overrides, minimal/full Loops, missing/malformed/wrong-kind packages, Tool-loader guidance, Agent Loop relationships, missing installed Loops, full bindings metadata, and no cross-package/runtime behavior.

## Milestone 10: Python SDK
> Scope note: add Python SDK parity for installed Loop metadata, Agent singular Loop relationships, and authored binding metadata. `load_loop` and `load_agent` remain structural package loaders only and must not implement any Phase 7B execution semantics.
- [ ] Add `loop` to public package-kind Literals/enums, lockfile package/root TypedDicts, and installed-package handling.
- [ ] Add TypedDicts/models for the complete Loop contract and Agent binding contract.
- [ ] Add typed Agent manifest support for optional top-level singular `loop` and `bindings`.
- [ ] Implement `load_loop` using installed package resolution parallel to `load_profile` / closest metadata-only loader.
- [ ] Support a Loop directory override following existing Python loader conventions.
- [ ] Return package identity/key/integrity/root/manifest path/typed manifest/typed `loop` metadata according to current conventions.
- [ ] Reject missing packages, malformed manifests, wrong kinds, and missing Loop metadata clearly.
- [ ] Update generic Tool `load()` guidance for Loop callers.
- [ ] Update `load_agent` to expose resolved singular Loop relationships with current nullable-path behavior for locked-but-missing packages.
- [ ] Preserve typed authored `bindings` metadata without resolving or executing it.
- [ ] Do not validate phase keys against Loops, Memory selectors against Blueprints, access conflicts, Profile ordering, MCP runtime details, or consumer-context files.
- [ ] Export `load_loop` and public Loop/binding types through `agentpm.__init__`, `__all__`, and relevant type modules.
- [ ] Add tests equivalent to Node and confirm field-name/relationship parity where practical.

## Milestone 11: Documentation, Compatibility Audit, and Final Verification
> Scope note: document the final Loop/Agent binding contracts, audit every package-kind/composition surface for omissions, and run cross-repository verification. This milestone closes gaps against `spec.md`; it must not introduce new terminal targets, expression languages, runtime configuration, binding precedence, cross-package linting, Harness execution, or other scope expansion.
- [ ] Update manifest reference documentation with the complete `kind: "loop"` schema, required/optional fields, stable IDs, implicit outcome behavior, transition rules, terminal targets, checkpoints, limits, access, and error policy.
- [ ] Document Agent singular top-level `loop` and complete `bindings` schema.
- [ ] Document versioned top-level dependencies versus versionless binding package identities.
- [ ] Document global + phase additive binding intent and explicitly state that Phase 7A does not calculate or enforce effective runtime availability.
- [ ] Document Memory binding package/spaces/operations semantics and that operations retain their Blueprint-defined triggers/contracts.
- [ ] Document named MCP bindings and the deliberate absence of host/port/transport/process configuration.
- [ ] Document consumer-context ownership, safe workspace-relative paths, optional file presence, and author-chosen filename.
- [ ] Document the Loop-access-versus-Agent-binding boundary and that lint/publish/install do not reject conflicts.
- [ ] Document that Agent lint validates package membership but does not resolve Loop phases or Memory contents.
- [ ] Document Template `dependencies.loop` and synthesized-root-Agent behavior.
- [ ] Document `loadLoop` / `load_loop` and Agent binding exposure as metadata-only SDK behavior.
- [ ] Document Phase 7B as the separate canonical AgentPM harness implementation and list runtime concerns deliberately absent from 7A.
- [ ] Audit the full codebase for hardcoded `tool|agent|template|skill|knowledge|memory|profile` lists and add `loop` where package kinds are intended.
- [ ] Audit Rust/API client enums, CLI help/error text, OpenAPI/schema definitions, tests, fixtures, package-key parsing, install roots, route helpers, and sample data.
- [ ] Audit lockfile/root models for assumptions that all Agent relationships are vectors; preserve singular Loop semantics.
- [ ] Audit database SQL, migrations, views, triggers, statistics, search, trending, signing, malware/tar validation, and namespace authorization.
- [ ] Audit frontend unions, cards, filters, badges, route maps, landing content, Agent/Template dependency presentation, and Agent manifest rendering.
- [ ] Audit Node/Python exports, package-kind unions, lockfile types, Agent manifest types, and loader guidance.
- [ ] Confirm Loop README/license packaging uses common behavior and README is not interpreted as orchestration guidance.
- [ ] Confirm Loop packages cannot declare dependencies in schema, CLI publish, or backend publish paths.
- [ ] Confirm Agent manifests without Loop/bindings remain compatible and existing required `tools` behavior remains unchanged.
- [ ] Confirm Templates without `loop` remain valid.
- [ ] Confirm lockfile v3 remains compatible unless a reviewed implementation finding justified a change.
- [ ] Bump CLI, Node SDK, and Python SDK release versions in the normal final-release pass.
- [ ] Update web status/version/date surfaces if that remains part of current release practice.
- [ ] Run all verification in `test-plan.md` and report exact commands, failures, skips, migration evidence, and any deviations.

## Milestone 12: Examples
> Scope note: seed realistic public Loop and Agent composition examples only after Milestone 11 verification. Examples should demonstrate the portability of graph-defined orchestration and the complete binding model without implying that Phase 7A executes anything. Use existing AgentPM example stories where possible so Loops compose with real Tool, Skill, Knowledge, Memory, and Profile artifacts.
- [ ] Add a `loop-packages/` directory to `agentpm-examples` following existing package-kind organization.
- [ ] Create 4 production-worthy Loop examples with materially different orchestration structures.
- [ ] Add `@zack/support-escalation-loop` under `loop-packages/` as the support-story Loop example.
- [ ] Model `@zack/support-escalation-loop` around triage, draft-response, and review/escalate control flow.
- [ ] Ensure `@zack/support-escalation-loop` demonstrates explicit outcomes, `$handoff`, an approval checkpoint, and lightweight Memory access intent.
- [ ] Publish `@zack/support-escalation-loop` and wire it into the support template / support app story where the existing package narrative fits cleanly.
- [ ] Add `@zack/incident-response-loop` under `loop-packages/` as the flagship iterative incident/operations Loop example.
- [ ] Model `@zack/incident-response-loop` around assess, execute, and review phases with a reachable cycle back into execution.
- [ ] Ensure `@zack/incident-response-loop` demonstrates explicit outcomes, bounded steps, Tool/Knowledge/Memory access declarations, and retry / fail-phase behavior.
- [ ] Publish `@zack/incident-response-loop` and wire it into the ops agent / app story.
- [ ] Add `@zack/devwork-triage-loop` under `loop-packages/` as the devwork maintainer workflow Loop example.
- [ ] Model `@zack/devwork-triage-loop` around inspect-issue, draft-comment, and resolve-or-handoff control flow.
- [ ] Ensure `@zack/devwork-triage-loop` demonstrates a slower review-oriented orchestration shape distinct from support and incident examples.
- [ ] Publish `@zack/devwork-triage-loop` and wire it into the devwork agent / app story.
- [ ] Add one standalone Loop package not required by an app, such as `@zack/research-review-loop` or `@zack/content-review-loop`, to seed an additional public orchestration pattern.
- [ ] Use the standalone Loop to demonstrate a review/revise pattern with approval semantics without forcing another app integration.
- [ ] Use both implicit `complete` outcomes and explicit outcome objects across the examples.
- [ ] Exercise all three terminal targets across the example set.
- [ ] Use as much of the supported Loop and Agent binding surface as realistically fits the examples, so the public seed set demonstrates the contract broadly rather than only the minimal happy path.
- [ ] Exercise optional archetype, limits, access, checkpoints, and error policy across the example set without forcing every field into every Loop.
- [ ] Ensure optional fields such as `access`, `limits`, checkpoints, error policy, Memory selectors, MCP surfaces, and consumer context appear across the example set wherever they fit naturally.
- [ ] Update at least one existing published Agent example to depend on a Loop and use all major `bindings` surfaces: global, phases, Memory spaces/operations, named MCP surfaces, and consumer context.
- [ ] Ensure bound package identities are versionless and every bound package is declared in the corresponding top-level versioned dependency list.
- [ ] Use real Memory Blueprint space/operation identifiers in Agent examples and preserve snake_case Memory keys.
- [ ] Ensure the MCP example uses at least two named surfaces if the existing Agent story naturally supports them, demonstrating that IDs improve inspectability without adding network configuration.
- [ ] Include a consumer-context filename that is intentionally not `AGENTPM.md` to reinforce author-defined conventions.
- [ ] Wire at least one Loop through a Template `dependencies.loop` flow and verify the synthesized root Agent receives the exact resolved Loop reference.
- [ ] Keep generated local Agents' independently authored Loop dependencies/bindings intact in Template examples.
- [ ] Ensure READMEs explain that Loops/bindings are portable contracts and that actual execution belongs to the consuming runtime / future AgentPM harness.
- [ ] Validate examples through lint, publish dry-run, publish/install where production seeding is intended, registry display, Template `new`, and Node/Python metadata loading.
- [ ] Do not add demo Harness/runtime behavior to examples in Phase 7A.
