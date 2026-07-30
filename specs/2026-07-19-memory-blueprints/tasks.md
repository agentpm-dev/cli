# Tasks

## Milestone 1: Memory Manifest Contract
> Scope note: establish `kind: "memory"` as a valid package contract and make the schema and typed-manifest layers distinguish agent dependency `memory` arrays from Memory Blueprint `memory` objects. Define the complete authored declarative vocabulary and allow templates and agents to reference Memory packages. This milestone validates shape only; it does not resolve cross-references, load or compile referenced schema files, generate contracts, publish, install, or execute memory behavior.
- [ ] Add `memory` to the shared manifest `kind` enum and final package-kind `oneOf`.
- [ ] Add JSON Schema definitions for Memory Blueprint scopes, record types, spaces, retrieval, capacity, retention, and constraints using the exact enums and required fields defined in the spec.
- [ ] Add JSON Schema definitions for lifecycle operations, operation references, source handling, provenance expectations, and supported trigger types using the exact MVP contract defined in the spec.
- [ ] Require Memory Blueprint scopes to be declared once under `memory.scopes` and referenced by key from spaces and operations.
- [ ] Require each record type to declare a safe package-relative JSON Schema path and a record schema version.
- [ ] Define the supported field-level governance annotation names and allowed value enums in the shared contract documentation/schema layer: `x-agentpm-data-class`, `x-agentpm-sensitivity`, `x-agentpm-persist`, and `x-agentpm-shareable`.
- [ ] Overload the top-level `memory` property so agents use a package-reference array and `kind: "memory"` uses Memory Blueprint metadata.
- [ ] Update `dependentSchemas.memory` to allow only agent dependency arrays or Memory Blueprint metadata for the correct kind.
- [ ] Add `memory` to `templateDependencyGroup`.
- [ ] Ensure `kind: "memory"` rejects top-level dependency arrays and package-specific fields from other kinds.
- [ ] Add strongly typed Rust structs for scopes, record types, spaces, retrieval, capacity, retention, constraints, operations, triggers, and `MemoryManifest`.
- [ ] Add `memory` to Rust `TemplateDependencies` parsing and preserve it through manifest deserialization.
- [ ] Add `PublishManifest::Memory` and update publish-kind parsing and error messages.
- [ ] Add manifest schema tests for minimal and advanced valid Memory Blueprints.
- [ ] Add tests proving agent `memory` arrays and Memory Blueprint `memory` objects are accepted only for their correct kinds.
- [ ] Add tests proving templates may declare `template.dependencies.memory` while Memory packages cannot declare dependencies.
- [ ] Add manifest schema tests for invalid models, retrieval modes, retention actions, operation types, trigger types, unsafe schema paths, missing required fields, invalid key names, unsupported additional properties, and forbidden dependencies.
- [ ] Update embedded-schema tests and every hardcoded package-kind assertion.

## Milestone 2: Semantic Validation and Governance
> Scope note: validate the relationships and semantics that JSON Schema alone cannot enforce, including reusable scope and record-type references, model requirements, lifecycle operation contracts, supported duration semantics, referenced content schemas, and field-level governance annotations. This milestone makes authored blueprints reliably lintable but remains read-only and does not generate build artifacts, persist memory records, evaluate triggers, enforce retention, or execute lifecycle operations.
- [ ] Add a reusable Memory Blueprint semantic validator invoked by lint, build, inspect, and CLI publish-readiness checks.
- [ ] Separate blueprint-level semantic validation from referenced content-schema validation so errors clearly identify whether they originate in `agent.json` or an external schema file.
- [ ] Validate scope, record-type, space, and operation keys against the shared identifier pattern defined in the manifest contract.
- [ ] Validate that every space scope reference exists in `memory.scopes`.
- [ ] Validate that every space record-type reference exists in `memory.record_types`.
- [ ] Validate unique values in scope, record-type, retrieval-mode, target, and operation-input arrays.
- [ ] Validate that each declared space-and-record-type pairing is unique and produces one logical resolved contract.
- [ ] Enforce model-specific rules: document spaces require `key` retrieval and cannot be append-only; sequence spaces require `chronological` retrieval and deterministic ordering; collection spaces must not declare sequence-only constraints.
- [ ] Validate safe package-relative schema paths, ensure referenced files remain inside the package root, and reject symlink escapes.
- [ ] Parse referenced files as JSON and compile them as JSON Schema Draft 2020-12.
- [ ] Define and enforce the MVP `$ref` policy, including offline behavior and whether safe package-relative multi-file references are supported.
- [ ] Recursively inspect the complete referenced JSON Schema tree for supported and unknown `x-agentpm-*` annotations.
- [ ] Reject unknown `x-agentpm-*` annotations while preserving unrelated third-party extension keywords.
- [ ] Validate `x-agentpm-data-class` and `x-agentpm-sensitivity` against their defined enums.
- [ ] Require `x-agentpm-persist` and `x-agentpm-shareable` to be booleans.
- [ ] Validate retention and trigger durations against the exact supported positive ISO 8601 subset and reject zero, negative, ambiguous, or unsupported values.
- [ ] Validate operation source and output references and ensure record types are permitted by their spaces.
- [ ] Validate `consolidate`, `transform`, and `delete` operation-specific required and forbidden fields.
- [ ] Validate operation source-handling and provenance declarations.
- [ ] Validate trigger-specific required fields, positive thresholds/intervals, and cross-references.
- [ ] Validate that capacity triggers reference spaces with `capacity.max_records`.
- [ ] Validate that external triggers do not introduce arbitrary content-condition expressions.
- [ ] Emit actionable `LintIssue` values with `/memory` manifest paths for blueprint errors and schema filenames plus JSON Pointers for referenced-schema errors.
- [ ] Remove the legacy agent-memory reserved-field warning while preserving warnings for still-reserved fields.
- [ ] Ensure semantic validation is read-only and never rewrites authored schemas.
- [ ] Add unit tests for every semantic validation rule, `$ref` policy, governance traversal path, and important error-reporting case.

## Milestone 3: Resolved Record Contract Generator
> Scope note: add `agentpm memory build` and deterministically compile each valid space-and-record-type pairing into a complete, self-contained JSON Schema combining the standard AgentPM logical record envelope with the authored content schema. This milestone creates the consumer-facing contracts and contract index but does not yet generate or compare build-freshness metadata, enforce publish readiness, persist live records, perform retrieval, or execute lifecycle operations.
- [ ] Add a dedicated `commands::memory` module and CLI `agentpm memory` command group.
- [ ] Add `agentpm memory build --manifest <path>`.
- [ ] Add internal `MemoryBuildMode::{Check, Write}` behavior where both modes generate expected contracts in memory and only `Write` persists them; build-metadata freshness comparison is added in Milestone 4.
- [ ] Define the standard AgentPM logical memory-record envelope generator.
- [ ] Generate required envelope fields for `id`, `record_type`, `space`, `scope`, `schema_version`, `created_at`, and `content`.
- [ ] Generate optional envelope fields for `updated_at`, `expires_at`, and `provenance`.
- [ ] Set `additionalProperties: false` at the envelope level.
- [ ] Generate exact scope properties and required keys for each space.
- [ ] Require each scope value to be a non-empty string and reject undeclared extra scope keys.
- [ ] Generate `const` values for space, record type, and schema version.
- [ ] Require sequence `ordinal` as a non-negative integer and omit it from document and collection contracts.
- [ ] Embed the resolved author content schema under the envelope `content` property so each generated contract is self-contained and installed-package-safe.
- [ ] Preserve the original source-schema path in the contract index.
- [ ] Resolve supported `$ref` values according to the Milestone 2 policy and ensure generated contracts require no network access.
- [ ] Preserve valid JSON Schema keywords and supported governance annotations without changing their semantics.
- [ ] Generate one contract for every declared space-and-record-type pairing.
- [ ] Generate Draft 2020-12 schemas with stable `$id`, `title`, and descriptive metadata.
- [ ] Derive deterministic collision-free filenames from the validated space and record-type keys.
- [ ] Generate `memory/contracts/index.json` with `type`, `format_version`, and entries containing `space`, `record_type`, `schema_version`, `model`, `source_schema`, and generated `path`.
- [ ] Sort index entries by space and record type and reject duplicate identities or paths.
- [ ] Exclude timestamps and environment-dependent values from generated contracts and the contract index.
- [ ] Stage the complete `memory/contracts/` output in a temporary sibling directory and replace the previous directory only after successful generation.
- [ ] Preserve the prior successful generated directory when validation or generation fails.
- [ ] Ensure build never modifies `agent.json` or author-owned schema files.
- [ ] Use atomic file writes within the staged generated directory.
- [ ] Add build summary output with scope, record-type, space, operation, and contract counts plus the generated output path.
- [ ] Add tests for simple document, multi-space, sequence, multiple-record-type, and governance-heavy blueprints.
- [ ] Add tests for required and optional envelope fields, exact scope contracts, sequence ordinal semantics, and model-specific field omission.
- [ ] Add tests proving generated contracts are self-contained and compile without source files or network access.
- [ ] Add tests proving removed pairings remove stale generated files.
- [ ] Add tests proving unchanged builds create byte-identical contracts and index.
- [ ] Add tests proving stable filenames, `$id` values, and ordering.
- [ ] Add tests proving failed generation preserves the previous successful output.
- [ ] Add tests proving check mode performs no writes.

## Milestone 4: Build Metadata and Freshness
> Scope note: make Memory builds reproducible and independently verifiable by generating `memory/build.json`, recording deterministic hashes for authored inputs and generated outputs, and classifying missing, stale, unsupported, inconsistent, or manually modified build state. Check mode regenerates expected contracts in memory but performs no writes. This milestone does not yet wire freshness checks into publish and never modifies `agent.json` or author-owned schemas.
- [ ] Define and serialize `memory/build.json` using portable JSON primitives with an explicit `type`, `format_version`, AgentPM version, build time, authored-input metadata, generated-output metadata, and contract count.
- [ ] Reuse or extract deterministic SHA-256 helpers from Knowledge where appropriate.
- [ ] Compute the SHA-256 hash of the exact `agent.json` bytes and define that any manifest byte change requires a rebuild.
- [ ] Record every referenced source-schema path and SHA-256 hash in deterministic path order.
- [ ] Compute a sorted aggregate source-schema hash using unambiguous path-and-content framing.
- [ ] Compute a canonical semantic contract-input hash from the `memory` declaration and resolved source-schema contents.
- [ ] Define canonical JSON serialization using recursively sorted object keys, preserved array order, normalized UTF-8 output, and no insignificant whitespace.
- [ ] Compute the exact contract-index file hash.
- [ ] Add each generated contract’s SHA-256 hash to its contract-index entry.
- [ ] Compute a sorted aggregate generated-contract hash using unambiguous path-and-content framing.
- [ ] Ensure `built_at`, AgentPM version, absolute paths, filesystem metadata, and temporary paths do not affect semantic input or generated-output hashes.
- [ ] Require `contract_count` to match the number of index entries and indexed contract files.
- [ ] In check mode, regenerate expected contracts and the expected index in memory from current authored inputs.
- [ ] Compare expected generated bytes, current source hashes, persisted output files, index entries, and build metadata.
- [ ] Add comparison structures that classify missing build, unsupported format, stale source input, missing output, modified output, unexpected output, and inconsistent metadata.
- [ ] Report the specific changed source-schema, contract, index, or metadata field where possible.
- [ ] Reject missing indexed contract files.
- [ ] Treat `memory/contracts/` as fully build-managed and reject extra files or directories not present in the expected generated layout.
- [ ] Reject unsupported build metadata type or format version.
- [ ] Reject unsupported contract-index type or format version.
- [ ] Write `memory/build.json` only after successful generation and replacement of `memory/contracts/`.
- [ ] Ensure a missing or failed build-metadata write cannot be considered a fresh build.
- [ ] Keep `memory/build.json` and `memory/contracts/index.json` composed of documented portable JSON primitives so the backend and SDKs can validate their structure without reproducing Rust-only generation logic.
- [ ] Add tests for manifest changes, formatting-only manifest changes, source-schema changes, contract edits, per-contract hash edits, aggregate hash edits, index edits, missing files, extra files/directories, count mismatches, unsupported metadata, and failed metadata writes.
- [ ] Add tests proving diagnostics identify the affected source schema or generated contract where possible.
- [ ] Add tests proving informational metadata does not affect contract hashes.
- [ ] Add tests proving check mode regenerates expected output without performing writes.

## Milestone 5: Memory Inspect
> Scope note: provide a read-only CLI view of local and installed Memory Blueprint packages, including authored structure, generated contract inventory, and normalized build status. Inspection is diagnostic: it may report a package as not built, stale, malformed, or unsupported, but it must never rebuild, rewrite, repair, persist, query, or execute memory behavior.
- [ ] Add `agentpm memory inspect <PATH_OR_PACKAGE>` and `--json`.
- [ ] Resolve targets in this order: existing local path, explicit manifest file, then installed Memory package reference.
- [ ] Accept `@namespace/name`, `@namespace/name@range`, and optional `memory:` prefixes using existing package-spec and lockfile conventions.
- [ ] Resolve installed packages only as `PackageKind::Memory`.
- [ ] Add installed Memory package resolution under `.agentpm/memory/<owner>/<name>/<version>`.
- [ ] Keep installed-package inspection confined to the installed package root.
- [ ] Render package identity, target, manifest path, package root, and generated metadata paths.
- [ ] Render text output for scopes, record types, spaces, retrieval, retention, capacity, constraints, lifecycle operations, triggers, and generated contract inventory.
- [ ] Label lifecycle operations and triggers as declarative blueprint behavior rather than executed runtime state.
- [ ] List contract identities, source schema paths, generated paths, hashes, and availability without embedding every full contract schema by default.
- [ ] Render `--json` output containing target resolution, package identity, authored Memory metadata, source schemas, contract-index entries, build metadata, normalized build status, and structured mismatch details.
- [ ] Report normalized build status values such as `not_built`, `fresh`, `stale`, `invalid`, and `unsupported`.
- [ ] Return successful diagnostic output for valid authored blueprints with missing or stale generated output.
- [ ] Report never-built packages with guidance to run `agentpm memory build`.
- [ ] Report missing, modified, unexpected, or unsupported generated output without rewriting files.
- [ ] Fail clearly for non-Memory packages, invalid authored declarations, unsafe paths, unreadable source schemas, and unsupported schema-reference behavior.
- [ ] Distinguish missing target, package not installed, unmatched version, and wrong-kind errors.
- [ ] Add tests for local directories, direct manifest paths, installed package references, version ranges, and `memory:` prefixes.
- [ ] Add tests for `not_built`, fresh, stale-source, modified-contract, missing-contract, invalid-metadata, and unsupported-format statuses.
- [ ] Add tests proving inspect performs no writes and leaves file contents and modification times unchanged.
- [ ] Add tests proving installed inspection cannot resolve files outside the installed package root.
- [ ] Add stable text and JSON output tests, including structured mismatch categories and affected paths.

## Milestone 6: Publish Readiness and Archive Integration
> Scope note: make the CLI the authoritative Memory build-readiness gate before archive creation. A Memory package may be packaged for upload only when its generated contracts and build metadata are present, supported, current, internally consistent, and unmodified. Publish preparation must verify existing output and archive it unchanged; it must never rebuild, repair, or rewrite Memory files. Backend structural validation is implemented separately in the registry milestone.
- [ ] Add CLI-side Memory publish preparation beside Knowledge publish preparation.
- [ ] Run `MemoryBuildMode::Check` before archive creation and reject missing, stale, unsupported, inconsistent, modified, or unexpected build output, including build metadata, source-schema hashes, contract-index contents, indexed contract hashes, and extra or missing generated files.
- [ ] Reuse the shared Memory check result and diagnostics rather than reimplementing freshness or hash-comparison logic inside the publish command.
- [ ] Require current `memory/build.json`, `memory/contracts/index.json`, and every indexed resolved contract before publish.
- [ ] Ensure publish preparation never rebuilds, repairs, normalizes, or rewrites Memory output.
- [ ] Ensure publish preparation preserves the contents and modification times of `agent.json`, source schemas, generated contracts, the contract index, and `memory/build.json`.
- [ ] Map Memory check failures to actionable publish errors for never-built, stale-source, modified-output, unsupported-format, inconsistent-metadata, and invalid-source states.
- [ ] When stale authored input is the root cause, prioritize stale-source publish errors over downstream generated-output inconsistency signals so authors are told to rebuild rather than being overwhelmed by secondary hash/index mismatches.
- [ ] Instruct authors to run `agentpm memory build` only where rebuilding is the appropriate recovery.
- [ ] Include every declared source schema, `memory/build.json`, `memory/contracts/index.json`, and every indexed resolved contract in the package archive.
- [ ] Fail archive preparation if required Memory files are excluded by archive filters, ignore rules, or path handling.
- [ ] Ensure no unindexed file under `memory/contracts/` is included in a publishable archive.
- [ ] Reject Memory publish preparation when the manifest declares any top-level package dependency array or package-kind-specific dependency group.
- [ ] Add publish tests for fresh, never-built, stale-source, modified-contract, missing-contract, extra-contract, malformed-index, unsupported-format, inconsistent-hash/count, unsafe-schema-path, and forbidden-dependency cases.
- [ ] Add tests proving publish preparation performs no writes and reuses shared check-mode diagnostics.
- [ ] Add archive-content assertions for all declared source schemas, build metadata, contract index, every indexed contract, README, and license.
- [ ] Add archive tests proving unindexed generated files are rejected and required Memory files cannot be silently omitted.

## Milestone 7: Init and Authoring Experience
> Scope note: add a practical starting workflow for Memory Blueprint authors through `agentpm init --kind memory`, producing a small valid authored blueprint, content schema, README, and source directory layout. Initialization creates authored source files only; it does not run `agentpm memory build`, create generated directories or contracts, configure a storage backend, or create live memory records.
- [ ] Add `memory` to CLI init kind parsing and help text.
- [ ] Add a minimal but representative Memory Blueprint `agent.json` template with one scope, one record type, one document space, key retrieval, and no lifecycle operations.
- [ ] Add a starter `schemas/user-preference.schema.json` using ordinary Draft 2020-12 JSON Schema and a minimal set of supported governance annotations.
- [ ] Add a Memory Blueprint README template explaining authored files, generated files, `agentpm memory build`, `agentpm memory inspect`, publish readiness, and the fact that the blueprint does not provide live memory storage.
- [ ] Keep starter JSON concise and place explanatory guidance in the README rather than relying on unsupported JSON comments.
- [ ] Scaffold only `agent.json`, `README.md`, and authored schema directories/files.
- [ ] Do not create `memory/`, `memory/build.json`, `memory/contracts/`, or other generated output during initialization.
- [ ] Preserve the existing `agentpm init` path-collision and overwrite behavior.
- [ ] Ensure rendered starter files pass lint.
- [ ] Ensure the starter package successfully builds when `agentpm memory build` is run explicitly.
- [ ] Add init tests and snapshots for default and named Memory Blueprint packages.
- [ ] Add tests proving package-name substitution preserves valid schema paths, record-type references, and README commands.
- [ ] Add tests proving initialization creates no generated Memory files or directories.

## Milestone 8: Package Kind, Dependency Resolution, and Lockfiles
> Scope note: make Memory a first-class package kind in CLI package resolution, installation, agent and template dependency expansion, generated workspaces, and lockfiles. Memory packages install as immutable blueprint artifacts and act as dependency-graph leaves. This milestone does not bind blueprints to stores, resolve runtime scope values, load or mutate records, evaluate triggers, enforce retention, or execute lifecycle operations. Backend dependency authorization and relationship persistence are completed in the registry milestone.
- [ ] Add `PackageKind::Memory` and update parsing, display, package keys, API kind conversion, command help, and hardcoded match statements.
- [ ] Install Memory packages under `.agentpm/memory/<namespace>/<name>/<version>/` using the existing archive-integrity, replacement, and installed-version conventions.
- [ ] Preserve authored source schemas, `memory/build.json`, `memory/contracts/index.json`, and generated contracts exactly as packaged during installation.
- [ ] Support direct installation of Memory packages using existing typed and untyped package-reference conventions.
- [ ] Expand top-level agent `memory` dependencies alongside tools, skills, and knowledge using existing version-range resolution rules.
- [ ] Add `memory` to template dependency parsing and resolve `template.dependencies.memory` during `agentpm new`.
- [ ] Ensure standalone template installation remains non-recursive according to existing template semantics.
- [ ] Include agent and template Memory dependencies in CLI registry-resolution requests.
- [ ] Enforce dependency-field kind compatibility so agent `memory` and template Memory references resolve only to stored Memory packages.
- [ ] Treat Memory packages as dependency-graph leaves and do not enqueue dependencies from a Memory manifest.
- [ ] Ensure generated workspaces install and record template Memory dependencies using existing workspace conventions.
- [ ] Represent Memory package entries with `memory:@namespace/name@version` package keys and the existing lockfile package-entry structure.
- [ ] Record agent-to-Memory and generated-workspace-to-Memory relationships using existing first-class relationship conventions.
- [ ] Remove Memory from reserved relationship metadata where it becomes first-class while preserving still-reserved fields.
- [ ] Deduplicate Memory package entries when the same exact package is referenced by multiple roots or paths.
- [ ] Preserve compatibility with existing agents containing empty `memory` arrays.
- [ ] Continue reading supported lockfile versions and legacy empty reserved-memory structures.
- [ ] Do not increment the lockfile version solely because `memory` becomes a first-class package kind unless the current format cannot represent it.
- [ ] Preserve unrelated lockfile packages and relationships when adding, updating, or removing Memory dependencies.
- [ ] Add install tests for direct Memory packages, agent Memory dependencies, and template Memory dependencies through `agentpm new`.
- [ ] Add tests proving standalone template installation does not expand Memory dependencies.
- [ ] Add tests proving Memory packages are dependency leaves.
- [ ] Add version-range, exact-version, wrong-kind, duplicate, conflict, inaccessible, and missing-package tests.
- [ ] Add deduplication tests where multiple roots reference the same Memory package.
- [ ] Add installed-layout assertions for authored schemas and all generated Memory build artifacts.
- [ ] Add lockfile snapshots for Memory package keys, agent relationships, generated-workspace relationships, and legacy reserved-memory compatibility.

## Milestone 9: Backend and Registry APIs
> Scope note: make Memory packages publishable, discoverable, downloadable, permission-aware, and usable as agent or template dependencies through the existing registry API architecture. During publish finalization, perform server-side structural defense-in-depth validation of the uploaded Memory archive, parallel to the existing Knowledge flow. The backend must inspect packaged metadata and files but must not regenerate resolved contracts or reproduce the CLI’s full freshness calculation. Expose enough manifest and packaged contract data for clients and the frontend without introducing a hosted memory store, runtime adapter system, lifecycle scheduler, or new database-backed memory service.
- [ ] Add `memory` to backend package-kind enums, validators, serializers, and API schemas.
- [ ] Support Memory packages in publish init/finalize and package detail flows.
- [ ] During publish finalization, enumerate the uploaded archive and invoke Memory-specific structural validation after generic artifact validation.
- [ ] Reject Memory packages that declare package dependencies.
- [ ] Require `memory/build.json` and `memory/contracts/index.json` in the uploaded archive.
- [ ] Safely read and parse `memory/build.json` and `memory/contracts/index.json` from the uploaded tarball with bounded file sizes.
- [ ] Validate the required build-metadata shape, including supported type, format version, source/output hashes, and contract count.
- [ ] Validate the contract-index shape, unique space-and-record-type entries, safe relative paths, and consistency with the declared contract count.
- [ ] Require every contract path listed by the contract index to exist in the uploaded archive.
- [ ] Require every source schema path declared by `memory.record_types` to exist in the uploaded archive.
- [ ] Reject duplicate contract-index entries, unsafe paths, missing indexed contracts, missing source schemas, and obvious build/index count inconsistencies.
- [ ] Keep backend validation structural: do not regenerate resolved contracts, recompute the complete Memory build, or duplicate `MemoryBuildMode::Check` in Python.
- [ ] Support Memory dependencies for agents and templates in registry relationship validation and persistence.
- [ ] Add or reuse a safe streaming archive helper that can collect member names and read selected small JSON members without extracting the archive to disk, while enforcing path safety and per-file size limits.
- [ ] Apply existing public/private namespace authorization consistently to Memory search, detail, download, install, and dependency resolution.
- [ ] Exclude inaccessible private Memory packages from global search and discovery.
- [ ] Expose Memory Blueprint manifest metadata to the frontend.
- [ ] Persist the generated contract index and resolved contract schemas at publish time so registry clients do not need to reopen package archives.
- [ ] Keep the uploaded archive as the canonical package artifact while treating the extracted database representation as registry presentation and API data.
- [ ] Add package-version persistence for extracted Memory build metadata, contract index metadata, and resolved contract schemas.
- [ ] Add database migration and ORM fields for `memory_build_metadata`, `memory_contract_index`, and `memory_contracts`, or an equivalent bounded JSONB representation.
- [ ] During Memory publish finalization, safely read and parse `memory/build.json`, `memory/contracts/index.json`, and every indexed resolved contract from the uploaded archive.
- [ ] Enforce per-file, contract-count, and aggregate extracted-content limits before persisting Memory metadata.
- [ ] Validate that contract index entries and parsed contracts agree on space, record type, schema version, and path.
- [ ] Persist extracted Memory metadata only after all structural validation succeeds.
- [ ] Expose Memory build metadata and contract index through package detail responses.
- [ ] Add an API mechanism to retrieve an individual resolved contract without including every contract in the base package-detail payload.
- [ ] Apply package visibility and private-namespace authorization to all Memory contract endpoints.
- [ ] Add publish tests proving extracted Memory metadata and contracts are persisted correctly.
- [ ] Add rejection tests for oversized metadata, oversized contracts, excessive contract counts, invalid JSON, duplicate contract paths, mismatched contract identities, and aggregate size violations.
- [ ] Add a Memory-specific forbidden-dependency check covering top-level package-reference arrays and template dependency groups rather than relying only on the Knowledge dependency-field list.
- [ ] Add API tests for public/private Memory publish, detail, search, download, and dependency relationships.
- [ ] Add malformed or missing generated-contract rejection tests where backend validation currently inspects archives.
- [ ] Add backend publish-finalize tests for missing or malformed build metadata, missing or malformed contract indexes, unsafe indexed paths, duplicate entries, missing indexed contracts, missing source schemas, contract-count mismatches, forbidden dependencies, and a structurally valid Memory archive.

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
- [ ] Load contract navigation from the persisted contract index and fetch individual resolved contract schemas through the Memory contract API.
- [ ] Avoid including all resolved contract schemas in the initial package-detail response when they can be loaded on demand.
- [ ] Preserve README, install instructions, namespace metadata, private-package behavior, loading states, and responsive behavior.
- [ ] Add frontend tests for simple, advanced, empty-optional-section, private, and malformed-contract states.

## Milestone 11: Node SDK
> Scope note: add typed Node SDK support for locating and loading already-installed Memory Blueprint packages, including authored manifest metadata, generated build metadata, the contract index, and safe references to resolved record contracts. `loadMemory` is a read-only package metadata loader; it does not install packages, verify authoritative build freshness, persist or query records, evaluate triggers, enforce retention, or execute lifecycle operations.
- [ ] Add `memory` to Node SDK package-kind unions and public package models.
- [ ] Add typed interfaces for Memory Blueprint scopes, record types, spaces, retrieval, retention, capacity, constraints, operations, triggers, build metadata, contract indexes, and contract-index entries.
- [ ] Add `loadMemory` mirroring `loadKnowledge` installed-package resolution behavior.
- [ ] Resolve only already-installed packages using existing package-root and version-selection conventions.
- [ ] Return package root, manifest path, parsed manifest metadata, parsed Memory Blueprint metadata, build path and metadata, contract-index path and metadata, and indexed contract references with absolute local paths.
- [ ] Resolve contracts from contract-index identities rather than independently constructing filenames.
- [ ] Add a typed helper for loading and parsing one indexed resolved contract on demand.
- [ ] Resolve all indexed paths relative to the installed package root and reject absolute paths, traversal, symlink escapes, duplicate identities, duplicate paths, and files outside `memory/contracts/`.
- [ ] Validate required generated files, metadata shape, contract count consistency, supported type/format versions, and indexed contract existence.
- [ ] Perform structural validation only; do not recompute source hashes, regenerate contracts, or reproduce CLI freshness checking.
- [ ] Reject missing packages, wrong package kinds, malformed metadata, unsupported formats, missing indexed contracts, and unknown contract identities with clear typed errors.
- [ ] Do not download, install, rebuild, repair, persist, query, or mutate Memory data.
- [ ] Consider exporting a generic `MemoryRecordEnvelope<TContent, TScope>` containing only fields common to all generated contracts; omit it if it obscures model-specific differences.
- [ ] Update Node SDK public exports.
- [ ] Add tests for installed package resolution, parsed metadata, on-demand contract loading, safe path handling, duplicate index entries, count mismatches, missing generated metadata, malformed indexes, missing contracts, wrong kinds, and unsupported formats.
- [ ] Use shared Memory fixtures with the Python SDK and verify equivalent artifact interpretation.

## Milestone 12: Python SDK
> Scope note: add typed Python SDK support for locating and loading already-installed Memory Blueprint packages in parity with the Node SDK, including authored manifest metadata, generated build metadata, the contract index, and safe references to resolved record contracts. `load_memory` is a read-only package metadata loader; it does not install packages, verify authoritative build freshness, persist or query records, evaluate triggers, enforce retention, or execute lifecycle operations.
- [ ] Add `memory` to Python SDK package-kind literals, enums, and public package models.
- [ ] Add typed models for Memory Blueprint scopes, record types, spaces, retrieval, retention, capacity, constraints, operations, triggers, build metadata, contract indexes, and contract-index entries.
- [ ] Add `load_memory` mirroring `load_knowledge` installed-package resolution behavior.
- [ ] Resolve only already-installed packages using existing package-root and version-selection conventions.
- [ ] Return package root, manifest path, parsed manifest metadata, parsed Memory Blueprint metadata, build path and metadata, contract-index path and metadata, and indexed contract references with absolute local paths.
- [ ] Resolve contracts from contract-index identities rather than independently constructing filenames.
- [ ] Add a typed helper for loading and parsing one indexed resolved contract on demand.
- [ ] Resolve indexed paths relative to the installed package root and reject absolute paths, traversal, symlink escapes, duplicate identities, duplicate paths, and files outside `memory/contracts/`.
- [ ] Validate required generated files, metadata shape, contract count consistency, supported type/format versions, and indexed contract existence.
- [ ] Perform structural validation only; do not recompute source hashes, regenerate contracts, or reproduce CLI freshness checking.
- [ ] Reject missing packages, wrong package kinds, malformed metadata, unsupported formats, missing indexed contracts, and unknown contract identities with clear exceptions.
- [ ] Do not download, install, rebuild, repair, persist, query, or mutate Memory data.
- [ ] Consider exporting a generic `MemoryRecordEnvelope[ContentT, ScopeT]` containing only fields common to all generated contracts; omit it if it obscures model-specific differences.
- [ ] Update Python package exports.
- [ ] Add tests for installed package resolution, parsed metadata, on-demand contract loading, safe path handling, duplicate index entries, count mismatches, missing generated metadata, malformed indexes, missing contracts, wrong kinds, and unsupported formats.
- [ ] Use shared Memory fixtures with the Node SDK and verify equivalent artifact interpretation.

## Milestone 13: Documentation and Cleanup
> Scope note: complete every documentation, compatibility, packaging, release-version, and deployment task required before creating real published Memory Blueprint example packages. This milestone closes the feature out across repos, updates public-facing product/docs surfaces, removes stale reserved-memory assumptions, verifies existing workflows remain unchanged, updates released version markers, and deploys the production surfaces that the examples will rely on. Example package creation is intentionally deferred to Milestone 14.
- [ ] Update repo READMEs and public documentation surfaces across repos where the Memory Blueprint workflow changes what users read or run.
- [ ] Update the `agentpm-api` MDX documentation set for Memory Blueprints, including both edits to existing pages and brand-new MDX files wherever Memory needs its own dedicated public doc surface.
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
- [ ] Clearly document Phase 6C non-goals and the future Phase 7 binding/harness boundary.
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
- [ ] Update the CLI version for the Memory Blueprint release.
- [ ] Update the Node SDK and Python SDK versions for the Memory Blueprint release.
- [ ] Update the web status page version/date values so the public UI reflects the Memory Blueprint release.
- [ ] Deploy the required production surfaces after docs, cleanup, and release-version updates are complete, including the registry/backend, web UI, CLI release, and both SDKs.
- [ ] Run full CLI, backend, frontend, Node SDK, and Python SDK test suites.

## Milestone 14: Examples
> Scope note: create and publish representative Memory Blueprint packages only after Milestone 13 is complete and production has been updated. These examples are seeded public artifacts, so they should rely on the finalized docs, package-kind support, release versions, and deployed production surfaces from the prior milestone. Examples must demonstrate the supported MVP vocabulary without implying that AgentPM currently stores memory, enforces retention, evaluates triggers, or executes lifecycle operations.
- [ ] Add `@zack/support-customer-state` as the seeded simple document-style Memory Blueprint package.
  - [ ] Place it under `agentpm-examples/memory-packages/support-customer-state`.
  - [ ] Keep it intentionally narrow and approachable as the first public Memory example.
  - [ ] Model a durable customer or user profile with document storage, key retrieval, and light lifecycle policy.
  - [ ] Include more than one authored field in the source schema so the generated contract is visibly useful in the UI and SDKs.
- [ ] Add `@zack/conversation-continuity` as the seeded flagship multi-space Memory Blueprint package.
  - [ ] Place it under `agentpm-examples/memory-packages/conversation-continuity`.
  - [ ] Exercise sequence, document, and collection spaces together in one package.
  - [ ] Exercise retrieval semantics, retention, capacity, governance annotations, and declarative lifecycle operations.
  - [ ] Include at least one consolidate operation so the example demonstrates the richer lifecycle vocabulary.
- [ ] Add `@zack/devwork-maintainer-state` as the seeded workflow-oriented Memory Blueprint package.
  - [ ] Place it under `agentpm-examples/memory-packages/devwork-maintainer-state`.
  - [ ] Keep it aligned with the existing devwork package story rather than introducing a disconnected domain.
  - [ ] Model maintainer continuity such as durable preferences, active work threads, or follow-up notes.
  - [ ] Include at least one lifecycle operation that fits the devwork workflow story.
- [ ] Wire example integrations into existing published package flows.
  - [ ] Add `@zack/support-customer-state` to an existing template package so generated workspaces demonstrate template-driven Memory usage.
  - [ ] Add `@zack/conversation-continuity` to at least one existing agent package so a published agent demonstrates agent-level Memory usage.
  - [ ] Add `@zack/devwork-maintainer-state` to the devwork example flow if it fits cleanly without expanding scope beyond the existing story.
- [ ] Keep the three examples intentionally distinct rather than repeating the same memory model with different nouns.
  - [ ] `support-customer-state` should represent a simple durable support-state contract.
  - [ ] `conversation-continuity` should represent the main multi-space continuity example.
  - [ ] `devwork-maintainer-state` should represent workflow or maintainer continuity.
- [ ] Verify the seeded examples end to end.
  - [ ] Run `agentpm lint` and `agentpm memory build` on all three example packages.
  - [ ] Verify inspect output for the simple document-style package.
  - [ ] Verify inspect output for the workflow-oriented package.
  - [ ] Verify inspect output for the flagship multi-space package.
  - [ ] Verify template-driven install flow for the template example.
  - [ ] Verify agent-level install/load flow for the agent example.
