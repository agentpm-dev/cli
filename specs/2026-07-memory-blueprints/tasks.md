# Tasks

## Milestone 1: Memory Manifest Contract
> Scope note: establish kind: "memory" as a valid package contract and make the schema/type layer understand the difference between agent dependency memory arrays and Memory Blueprint memory objects. Define the declarative manifest vocabulary and allow templates to reference Memory packages. This milestone does not validate referenced schema files semantically, generate resolved contracts, publish, install, or execute memory behavior yet.
- [ ] Add `memory` to the shared manifest `kind` enum and final package-kind `oneOf`.
- [ ] Add JSON Schema definitions for Memory Blueprint scopes, record types, spaces, retrieval, capacity, retention, constraints, operations, operation references, and triggers.
- [ ] Overload the top-level `memory` property so agents use a package-reference array and `kind: "memory"` uses Memory Blueprint metadata.
- [ ] Update `dependentSchemas.memory` to allow only agent dependency arrays or Memory Blueprint metadata for the correct kind.
- [ ] Add `memory` to `templateDependencyGroup`.
- [ ] Ensure Memory Blueprints reject top-level dependency arrays and package-specific fields from other kinds.
- [ ] Add Rust manifest structs for Memory Blueprint metadata and a `MemoryManifest`.
- [ ] Add `PublishManifest::Memory` and update publish-kind parsing and error messages.
- [ ] Add manifest schema tests for minimal and advanced valid Memory Blueprints.
- [ ] Add manifest schema tests for invalid models, retrieval modes, retention actions, operation types, trigger types, unsafe schema paths, and forbidden dependencies.
- [ ] Update embedded-schema tests and every hardcoded package-kind assertion.

## Milestone 2: Semantic Validation and Governance
> Scope note: validate the relationships and semantics that JSON Schema alone cannot enforce, including scope and record-type references, model requirements, lifecycle operation contracts, retention durations, source JSON Schemas, and field-level governance annotations. This milestone makes authored blueprints reliably lintable but does not generate build artifacts, persist memory records, evaluate triggers, or execute lifecycle operations.
- [ ] Add a reusable Memory Blueprint semantic validator invoked by lint, build, inspect, and publish readiness checks.
- [ ] Validate top-level scope, record-type, space, and operation key naming.
- [ ] Validate that every space scope reference exists in `memory.scopes`.
- [ ] Validate that every space record-type reference exists in `memory.record_types`.
- [ ] Validate unique values in scope, record-type, retrieval-mode, target, and operation-input arrays.
- [ ] Enforce model-specific rules: document requires `key`, sequence requires `chronological`, and document cannot be append-only.
- [ ] Validate safe package-relative schema paths and ensure referenced files remain inside the package root.
- [ ] Parse referenced files as JSON and compile them as JSON Schema Draft 2020-12.
- [ ] Recursively validate supported `x-agentpm-*` governance annotations and reject unsupported or misspelled AgentPM keywords.
- [ ] Validate data-class, sensitivity, persist, and shareable values.
- [ ] Validate ISO 8601 retention and interval durations using a deterministic parser or strict validator.
- [ ] Validate operation source/output references and ensure record types are permitted by their spaces.
- [ ] Validate consolidate, transform, and delete operation-specific required fields.
- [ ] Validate trigger-specific required fields and cross-references.
- [ ] Validate that capacity triggers reference spaces with `capacity.max_records`.
- [ ] Emit actionable `LintIssue` paths rooted under `/memory` and include source schema file labels where relevant.
- [ ] Remove the old lint warning that treats agent `memory` references as unresolved/reserved.
- [ ] Add unit tests for every semantic validation rule and important error path.

## Milestone 3: Resolved Record Contract Generator
> Scope note: add agentpm memory build and generate one deterministic, complete record schema for every valid space-and-record-type pairing by combining the standard AgentPM memory envelope with the author’s content schema. This milestone creates the consumer-facing contracts and contract index but does not yet add build freshness metadata, publish enforcement, live memory storage, retrieval, or lifecycle execution.
- [ ] Add a dedicated `commands::memory` module and CLI `agentpm memory` command group.
- [ ] Add `agentpm memory build --manifest <path>`.
- [ ] Add internal `MemoryBuildMode::{Check, Write}` behavior parallel to Knowledge.
- [ ] Define the standard AgentPM logical memory-record envelope generator.
- [ ] Generate exact scope properties and required keys for each space.
- [ ] Generate `const` values for space, record type, and schema version.
- [ ] Require `ordinal` in sequence contracts and omit it from document/collection contracts.
- [ ] Add optional `updated_at`, `expires_at`, and provenance properties.
- [ ] Embed or reference the source content schema in a portable installed-package-safe way.
- [ ] Preserve supported governance annotations in generated content contracts.
- [ ] Generate one contract for every declared space-and-record-type pairing.
- [ ] Use deterministic contract filenames and stable `$id` values.
- [ ] Generate `memory/contracts/index.json` sorted by space and record type.
- [ ] Fully replace `memory/contracts/` during successful write builds.
- [ ] Ensure build never modifies `agent.json` or author-owned schema files.
- [ ] Use atomic writes for generated JSON and safe directory replacement behavior.
- [ ] Add build summary output with scope, record-type, space, operation, and contract counts.
- [ ] Add tests for simple document, multi-space, sequence, multiple-record-type, and governance-heavy blueprints.
- [ ] Add tests proving removed pairings remove stale generated files.
- [ ] Add tests proving unchanged builds create byte-identical contracts and index.
- [ ] Add tests proving check mode performs no writes.

## Milestone 4: Build Metadata and Freshness
> Scope note: make Memory builds reproducible and verifiable by generating memory/build.json, hashing all relevant authored inputs and generated outputs, and detecting missing, stale, unsupported, or manually modified build artifacts. This milestone establishes check-mode freshness semantics but does not yet wire those checks into publishing or modify agent.json.
- [ ] Define and serialize `memory/build.json` with type, format version, AgentPM version, build time, source hashes, output hashes, and contract count.
- [ ] Reuse or extract deterministic SHA-256 helpers from Knowledge where appropriate.
- [ ] Compute raw manifest hash.
- [ ] Compute sorted aggregate source-schema hash.
- [ ] Compute canonical contract-input hash from the memory declaration and source schemas.
- [ ] Compute contract-index hash.
- [ ] Compute sorted aggregate generated-contract hash.
- [ ] Ensure informational fields such as `built_at` do not affect contract hashes.
- [ ] Add check-mode comparison structures that report specific missing, stale, unsupported, or modified fields/files.
- [ ] Reject missing indexed contract files.
- [ ] Reject extra unindexed files under `memory/contracts/`.
- [ ] Reject unsupported build metadata type or format version.
- [ ] Add tests for manifest changes, source schema changes, contract edits, index edits, missing files, extra files, count mismatches, and unsupported metadata.

## Milestone 5: Memory Inspect
> Scope note: provide a read-only CLI view of local and installed Memory Blueprint packages, including their authored model, generated contracts, and build freshness. Inspection may report invalid or stale state but must not rebuild, rewrite, repair, persist, query, or execute any memory behavior.
- [ ] Add `agentpm memory inspect <PATH_OR_PACKAGE>` and `--json`.
- [ ] Resolve local directories, manifest files, installed package references, and optional `memory:` prefixes.
- [ ] Add installed Memory package resolution under `.agentpm/memory/<owner>/<name>/<version>`.
- [ ] Render text output for scopes, record types, spaces, retrieval, retention, capacity, constraints, operations, and contracts.
- [ ] Render JSON output with equivalent structured metadata.
- [ ] Report fresh/stale build status and mismatch details without rewriting files.
- [ ] Fail clearly for non-memory packages and invalid source declarations.
- [ ] Add local and installed-package inspect tests.
- [ ] Add stale-build inspect tests.

## Milestone 6: Publish Readiness and Archive Integration
> Scope note: allow Memory packages to enter the existing publish pipeline only when their generated contracts and build metadata are present, current, and unmodified. Ensure all authored schemas and generated files are archived without publish performing a build. This milestone does not yet add registry dependency resolution, installation support, SDK loading, or UI presentation.
- [ ] Add Memory publish preparation beside Knowledge publish preparation.
- [ ] Require current `memory/build.json`, contract index, and generated contracts before publish.
- [ ] Run Memory check mode during publish and reject stale/missing/modified output.
- [ ] Ensure publish never regenerates Memory outputs or modifies local files.
- [ ] Add actionable missing-build and stale-build errors instructing authors to run `agentpm memory build`.
- [ ] Ensure source schemas and generated memory files are included in the package archive under existing archive rules.
- [ ] Ensure Memory packages remain dependency-free at publish time.
- [ ] Add publish tests for fresh packages, never-built packages, stale packages, modified contracts, missing contracts, unsafe schema paths, and forbidden dependencies.
- [ ] Add archive-content assertions for source schemas, contract index, resolved contracts, build metadata, README, and license.

## Milestone 7: Init and Authoring Experience
> Scope note:  add a practical starting workflow for Memory Blueprint authors through agentpm init --kind memory, including a valid starter manifest, content schema, README, and directory layout. Initialization scaffolds authored source files only; it does not run agentpm memory build, create generated contracts, configure a backend, or create live memory data.
- [ ] Add `memory` to CLI init kind parsing and help text.
- [ ] Add a Memory Blueprint `agent.json` template.
- [ ] Add a starter `schemas/user-preference.schema.json`.
- [ ] Add a Memory Blueprint README template explaining build and publish steps.
- [ ] Scaffold directories without generating build output.
- [ ] Ensure rendered starter files pass lint.
- [ ] Add init tests and snapshots for default and named Memory Blueprint packages.

## Milestone 8: Package Kind, Dependency Resolution, and Lockfiles
> Scope note: make Memory a first-class installable package kind across CLI resolution, installation paths, agent dependencies, template dependencies, registry relationships, and lockfiles. This milestone resolves and installs blueprint artifacts only; it does not bind them to stores, map runtime scope values, load live records, or execute the declared policies and operations.
- [ ] Add `PackageKind::Memory` and update parsing, display, package keys, API kind conversion, and hardcoded match statements.
- [ ] Add Memory installation layout under `.agentpm/memory/...`.
- [ ] Resolve top-level agent `memory` dependencies as first-class packages.
- [ ] Add `memory` to template dependency parsing and generated-project dependency resolution.
- [ ] Update registry dependency collection for agents and templates.
- [ ] Update lockfile roots/relationships to represent Memory packages using existing conventions.
- [ ] Convert any reserved Memory relationship handling to first-class resolution without duplicate entries.
- [ ] Preserve compatibility with existing agents containing empty memory arrays.
- [ ] Preserve compatibility with existing readable lockfile versions.
- [ ] Add install tests for direct Memory packages, agent Memory dependencies, and template Memory dependencies.
- [ ] Add version-range, duplicate, conflict, and missing-package tests.
- [ ] Add lockfile snapshot tests for Memory package keys and relationships.

## Milestone 9: Backend and Registry APIs
> Scope note: make Memory packages publishable, discoverable, downloadable, permission-aware, and usable as agent or template dependencies through the existing registry API architecture. Expose enough manifest and packaged contract data for clients and the frontend without introducing a hosted memory store, runtime adapter system, lifecycle scheduler, or new database-backed memory service.
- [ ] Add `memory` to backend package-kind enums, validators, serializers, and API schemas.
- [ ] Support Memory packages in publish init/finalize and package detail flows.
- [ ] Support Memory dependencies for agents and templates in registry relationship validation and persistence.
- [ ] Apply existing public/private namespace authorization consistently to Memory search, detail, download, install, and dependency resolution.
- [ ] Exclude inaccessible private Memory packages from global search and discovery.
- [ ] Expose Memory Blueprint manifest metadata to the frontend.
- [ ] Expose generated contract index and contract schema content through the least invasive existing package-file/detail mechanism.
- [ ] Avoid unnecessary database columns if the manifest/archive can remain the source of truth.
- [ ] Add API tests for public/private Memory publish, detail, search, download, and dependency relationships.
- [ ] Add malformed or missing generated-contract rejection tests where backend validation currently inspects archives.

## Milestone 10: Registry UI
> Scope note: give Memory Blueprints a dedicated package-detail presentation that makes scopes, record types, spaces, governance, lifecycle declarations, and resolved record contracts understandable while preserving the existing registry’s visual and authorization patterns. The UI is descriptive and inspectable only; it does not edit blueprints, create records, configure storage, execute operations, or simulate the future harness.
- [ ] Add Memory Blueprint labels, badges, icons, filters, and package-card handling using existing package-kind patterns.
- [ ] Add a Memory-specific detail presentation that matches the current package-detail visual language.
- [ ] Display scopes and descriptions.
- [ ] Display record types, versions, and schema paths.
- [ ] Display spaces, models, scopes, accepted record types, retrieval modes, capacity, retention, and constraints.
- [ ] Display lifecycle operations, trigger types, source/output contracts, and source handling.
- [ ] Parse and display field-level governance annotations in a readable format.
- [ ] Add a resolved record-contract viewer that distinguishes envelope fields from blueprint content fields.
- [ ] Display build metadata/freshness only where it improves author/consumer understanding.
- [ ] Preserve README, install instructions, namespace metadata, private-package behavior, loading states, and responsive behavior.
- [ ] Add frontend tests for simple, advanced, empty-optional-section, private, and malformed-contract states.

## Milestone 11: Node SDK
> Scope note: add typed Node SDK support for locating and loading installed Memory Blueprint metadata, generated build metadata, and resolved record contracts in the same general manner as Knowledge packages. loadMemory is a package metadata loader in this phase and must not introduce persistence adapters, record CRUD, retrieval APIs, trigger evaluation, or lifecycle execution.
- [ ] Add Memory package kind/types to Node SDK public models.
- [ ] Add typed Memory Blueprint metadata interfaces for scopes, record types, spaces, retrieval, retention, constraints, operations, triggers, build metadata, and contract index entries.
- [ ] Add `loadMemory` mirroring `loadKnowledge` package-resolution behavior.
- [ ] Return package root, manifest metadata, build metadata, contract index, and resolved contract locations or parsed schemas according to existing SDK conventions.
- [ ] Reject missing packages and wrong package kinds clearly.
- [ ] Do not add live record persistence or CRUD APIs.
- [ ] Optionally export a canonical `MemoryRecord<T>` type if it fits existing SDK type patterns.
- [ ] Update Node SDK public exports.
- [ ] Add Node SDK unit/integration tests for local installed Memory packages and stale/missing generated metadata handling.

## Milestone 12: Python SDK
> Scope note: add typed Python SDK support for locating and loading installed Memory Blueprint metadata, generated build metadata, and resolved record contracts in parity with the Node SDK and existing Knowledge behavior. load_memory remains a blueprint metadata loader only and does not implement live record persistence, querying, mutation, retention enforcement, or operation execution.
- [ ] Add Memory package kind/models to Python SDK public types.
- [ ] Add typed Memory Blueprint models matching the manifest and generated metadata contract.
- [ ] Add `load_memory` mirroring `load_knowledge` package-resolution behavior.
- [ ] Return package root, manifest metadata, build metadata, contract index, and resolved contract locations or parsed schemas according to existing SDK conventions.
- [ ] Reject missing packages and wrong package kinds clearly.
- [ ] Do not add live record persistence or CRUD APIs.
- [ ] Optionally export a generic canonical `MemoryRecord` model if it fits existing SDK patterns.
- [ ] Update Python package exports.
- [ ] Add Python SDK tests for installed Memory packages and generated contract loading.

## Milestone 13: Documentation and Examples
> Scope note: document the complete Memory Blueprint authoring and consumption contract, generated record-envelope model, build/publish workflow, dependency behavior, SDK metadata loading, and Phase 7 boundary. Provide representative packages that demonstrate the supported MVP vocabulary without implying that AgentPM currently stores memory, enforces retention, evaluates triggers, or runs lifecycle operations.
- [ ] Add a complete Memory Blueprint manifest reference.
- [ ] Document scopes, record types, spaces, model semantics, retrieval modes, retention, capacity, constraints, operations, and triggers.
- [ ] Document all governance annotations and precise semantics.
- [ ] Document the canonical logical memory-record envelope and resolved contracts.
- [ ] Document `agentpm memory build` and generated paths.
- [ ] Document publish freshness requirements and stale-build recovery.
- [ ] Document `agentpm memory inspect`.
- [ ] Document `loadMemory` and `load_memory` as blueprint metadata loaders only.
- [ ] Update agent dependency docs for top-level `memory`.
- [ ] Update template dependency docs for `template.dependencies.memory`.
- [ ] Add at least one simple user-profile example package.
- [ ] Add one conversational-continuity reference example exercising sequence, document, collection, retention, semantic retrieval, governance, and consolidation.
- [ ] Clearly document Phase 6C non-goals and the future Phase 7 binding/harness boundary.

## Milestone 14: Compatibility and Cleanup
> Scope note: complete the cross-repository package-kind audit, remove obsolete reserved-memory assumptions, verify existing artifact workflows remain unchanged, and run the full test matrix. This milestone should close omissions and regressions only; it must not expand Memory Blueprints into bindings, storage adapters, live record APIs, harness execution, or other Phase 7 functionality.
- [ ] Audit the CLI for hardcoded `tool|agent|template|skill|knowledge` lists and add `memory` where intended.
- [ ] Audit backend models, migrations, API clients, and serializers for package-kind lists.
- [ ] Audit Node SDK public exports and package-kind unions.
- [ ] Audit Python SDK exports and package-kind literals/enums.
- [ ] Audit frontend package-kind filters, labels, icons, routes, and cards.
- [ ] Audit docs and examples for package-kind lists.
- [ ] Remove outdated warnings or comments that describe memory as reserved/unresolved.
- [ ] Ensure Memory Blueprints do not accidentally gain tool, skill, knowledge, agent, template, loop, or profile dependencies.
- [ ] Ensure existing tool, agent, template, skill, and knowledge publishing/install flows remain unchanged.
- [ ] Ensure existing templates without `memory` remain valid.
- [ ] Ensure existing agents with empty `memory` arrays remain valid.
- [ ] Run full CLI, backend, frontend, Node SDK, and Python SDK test suites.
