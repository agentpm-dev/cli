# Tasks

## Milestone 1: Instruction Profile Manifest Contract
> Scope note: establish `kind: "profile"` as a valid package contract and define the complete structured Instruction Profile vocabulary in JSON Schema and Rust typed-manifest layers. Distinguish singular Profile package `profile` objects from plural Agent and Template dependency `profiles` collections, and allow Agents and Templates to declare Profile references. This milestone validates manifest shape, typed deserialization, and kind-specific parsing only; it does not perform semantic lint checks, scaffold files, resolve dependencies, write lockfiles, dispatch publish operations, publish, install, render registry UI, load Profiles through SDKs, apply instructions, or enforce behavior.
- [ ] Add `profile` to the manifest schema package-kind enum and add a `kind: "profile"` branch to the top-level `oneOf` requiring the singular `profile` object.
- [ ] Add strict JSON Schema definitions for Profile identity, objectives, principles, audience, communication, vocabulary, boundaries, constraints, compatibility, and capability hints as defined in `spec.md`.
- [ ] Require `profile.identity`, `profile.objectives`, and `profile.communication`.
- [ ] Require `identity.role`, at least one objective, at least one communication tone value, and `communication.verbosity`.
- [ ] Keep tone values open-ended and define `verbosity` as `concise | balanced | detailed`.
- [ ] Define constraint IDs as lowercase kebab-case with a maximum length of 64 characters, rejecting uppercase characters, underscores, leading or trailing hyphens, and consecutive hyphens.
- [ ] Define constraint strength as `required | preferred` and require each constraint to contain a non-empty instruction.
- [ ] Use `additionalProperties: false` on every Profile-specific object and apply appropriate `minItems`, `uniqueItems`, `minLength`, `maxLength`, `minimum`, and `minProperties` rules.
- [ ] Require optional Profile arrays and objects to be non-empty when present rather than accepting fields that contribute no authored metadata.
- [ ] Add the singular top-level `profile` property and restrict it to `kind: "profile"` through `dependentSchemas` or the repository’s equivalent pattern.
- [ ] Preserve top-level `profiles` as the canonical Agent-only Profile dependency collection without renaming it; establish its manifest contract here while resolver and lockfile promotion remain in later milestones.
- [ ] Add optional `profiles` to `templateDependencyGroup`, preserving compatibility with older Template manifests that omit it.
- [ ] Add defaulted `profiles: Vec<PackageReference>` to Rust `TemplateDependencies`.
- [ ] Test that `parse_template_manifest` preserves both string and object-form Profile dependency references.
- [ ] Ensure Templates may declare Profiles only through `template.dependencies.profiles`, not through a top-level `profiles` field.
- [ ] Ensure `kind: "profile"` manifests reject top-level dependency arrays and fields belonging exclusively to Agents, Tools, Templates, Skills, Knowledge packages, or Memory packages.
- [ ] Ensure singular top-level `profile` is rejected for every non-Profile package kind.
- [ ] Ensure plural top-level `profiles` is rejected for every non-Agent package kind.
- [ ] Add Rust Profile manifest structs and enums matching the schema.
- [ ] Add `ProfileManifest` and `parse_profile_manifest`, including a clear wrong-kind error.
- [ ] Add schema tests for a minimal valid Profile containing only the required core.
- [ ] Add schema tests for a full valid Profile using every supported optional field, common `readme` and `license` metadata, and both compatibility capability groups.
- [ ] Confirm `readme` and `license` remain optional common package metadata and that `display_name` is rejected for Profile packages.
- [ ] Add schema failure tests for missing parent and core fields, empty required arrays, and present-but-empty optional arrays or objects.
- [ ] Add schema failure tests for invalid verbosity, invalid constraint strength, malformed constraint IDs, empty constraint instructions, and duplicate array values where schema uniqueness applies.
- [ ] Add schema failure tests for unsupported properties, malformed compatibility metadata, empty compatibility or capability objects, unknown capability names, non-boolean capability values, and zero or negative context-token requirements.
- [ ] Add schema failure tests for singular or plural Profile fields used by incorrect package kinds and for Profile manifests that attempt to declare package dependencies.

## Milestone 2: Profile Semantic Linting and Initialization
> Scope note: add the small set of Profile validations that require normalized-string or cross-field comparison and provide a valid `agentpm init --kind profile` starter package. Initialization produces authored source files only, and lint evaluates the structured manifest contract without judging the quality or appropriateness of the authored behavior. This milestone does not generate build artifacts, activate Profile dependency resolution, write lockfile relationships, publish or install packages, interpolate variables, treat README content as instructions, compile prompts, select or combine Profiles, or execute or enforce Profile behavior.
- [ ] Add Profile semantic validation to the shared manifest validation path after schema validation and typed Profile parsing succeed.
- [ ] Reject required Profile text values that are empty after trimming, including `identity.role`, objective entries, communication tone entries, and constraint instructions.
- [ ] Reject optional Profile text fields and array entries that are present but empty after trimming, including identity descriptions and expertise, principles, audience guidance, communication guidance and formatting, vocabulary terms, boundaries, and other authored string values.
- [ ] Report whitespace-only failures at the most precise field or array-entry instance path available.
- [ ] Reject duplicate constraint IDs with precise `/profile/constraints/<index>/id` issue paths, identifying the later duplicate entry rather than reporting only the constraints collection.
- [ ] Reject terms that appear in both `communication.vocabulary.prefer` and `communication.vocabulary.avoid` after trimming and case-folding.
- [ ] Reject normalized duplicate entries within `communication.vocabulary.prefer` or within `communication.vocabulary.avoid` when values differ only by surrounding whitespace or letter case.
- [ ] Keep schema-level `uniqueItems` validation for exact duplicates and use semantic lint only for normalized duplicates and cross-list overlap.
- [ ] Do not add subjective lint warnings about authored tone, the number or absence of optional constraints, behavioral quality, whether guidance is sufficiently detailed, or whether authored text would be better represented as a Skill.
- [ ] Keep the existing warning that Agent `profiles` are preserved but not resolved until Profile dependency resolution is implemented in Milestone 4; remove it in that milestone once the warning is no longer accurate.
- [ ] Add `Profile` to `InitKind` and update `agentpm init` help text to list and describe Instruction Profiles while retaining `profile` as the CLI kind value.
- [ ] Add a Profile `agent.json` asset containing a small valid structured starter with the required identity, objectives, and communication core.
- [ ] Include `readme: "README.md"` in the starter manifest and include no `display_name`, dependency arrays, runtime fields, generated metadata, parameters, or referenced instruction files.
- [ ] Add a Profile README asset explaining the artifact’s purpose, the distinction between Instruction Profiles and Skills, and that declared constraints express author intent rather than runtime enforcement.
- [ ] State in the generated README that `README.md` is package documentation and is not part of the structured behavioral contract consumed by runtimes or SDK loaders.
- [ ] Do not generate a license file by default; preserve common manifest `license` support without introducing Profile-specific license behavior.
- [ ] Make `agentpm init --kind profile` create only `agent.json` and `README.md`.
- [ ] Ensure Profile initialization does not create a `profile/`, `profiles/`, `instructions/`, build-output, schema, script, reference, or generated metadata directory.
- [ ] Keep `--mode` Knowledge-only and test that specifying a non-default mode with `--kind profile` is rejected with the existing non-Knowledge error behavior.
- [ ] Add init tests confirming generated file names, manifest values, README contents, schema validity, absence of unsupported or generated files and directories, and no unresolved template placeholders.
- [ ] Add init regression coverage confirming continued behavior for Tool, Agent, Template, Skill, Knowledge, and Memory initialization.
- [ ] Add lint tests for a semantically valid minimal Profile and a valid full Profile.
- [ ] Add lint failure tests for whitespace-only required and optional text, duplicate constraint IDs, normalized vocabulary duplicates, and terms present in both vocabulary collections.
- [ ] Verify Profile lint failures include precise `instance_path` values and work consistently in pretty, JSON, and NDJSON output where those formats are already covered by shared lint tests.
- [ ] Verify `--strict` behavior remains unchanged: semantic Profile errors always fail lint, while unrelated existing warnings fail only in strict mode.
- [ ] Verify `--fix` retains its existing non-invasive behavior, such as adding `$schema`, and does not rewrite, normalize, remove, or otherwise modify authored Profile content.

## Milestone 3: Package Kinds, Install Roots, and Resolver Plumbing
> Scope note: make `profile` a recognized package kind across the Rust CLI and shared Rust SDK, preserve backend-authoritative Profile kinds in resolve/install responses, and establish the canonical filesystem location and extraction behavior for installed Profile artifacts. This milestone provides the foundational package-kind and download plumbing needed by later install flows; it does not yet activate Agent `profiles` dependency parsing, mutate local Agent manifests, create first-class Profile lockfile relationships, support frozen or refresh semantics for Profiles, add backend publishing or persistence, process Template Profile dependencies in `agentpm new`, expose registry pages or language SDK loaders, bind Profiles to phases, combine Profiles, or enforce constraints.
- [ ] Add `Profile` to the CLI semver `PackageKind` and the shared Rust SDK install `PackageKind` with `snake_case` serialization as `"profile"`.
- [ ] Add unit tests confirming Profile package-kind serialization and deserialization in both the CLI and shared Rust SDK.
- [ ] Update every exhaustive Rust package-kind match, conversion, display/string helper, parser, supported-kind error message, and test required for the workspace to compile with `PackageKind::Profile`.
- [ ] Ensure package-key construction and parsing support canonical Profile keys such as `profile:@namespace/name@1.0.0`.
- [ ] Update CLI-to-SDK and SDK-to-CLI resolve/install conversions so Profile requirements and resolved Profile packages retain `kind: "profile"`.
- [ ] Preserve the current direct CLI package-spec behavior in which a bare `@namespace/name@version` request may initially use the existing default request kind and the registry response supplies the authoritative stored package kind.
- [ ] Ensure a resolved or install artifact returned as `kind: "profile"` remains a Profile throughout plan conversion, download routing, integrity verification, and extraction rather than being coerced back to Tool.
- [ ] Do not add new user-facing direct-spec syntax solely to encode the Profile kind unless required by an existing generic package-kind pattern.
- [ ] Do not interpret a local `kind: "profile"` manifest as a dependency-bearing manifest; Profile packages contain no package dependencies and must not recursively install artifacts from their structured `profile` metadata.
- [ ] Keep no-spec manifest-driven dependency installation limited to the package kinds and workspace flows that already act as dependency roots; a Profile source package must not install itself or treat its metadata as requirements.
- [ ] Add `.agentpm/profiles` to shared install-directory creation and to `InstallRoots`.
- [ ] Thread the new Profile install root through every existing `InstallRoots` constructor and call site, including workspace and Template code paths, without yet enabling Template Profile dependencies.
- [ ] Route Profile artifacts to `.agentpm/profiles/<namespace>/<name>/<version>` during extraction.
- [ ] Add installed Profile root and manifest-path helpers parallel to the existing Knowledge and Memory path helpers where shared CLI/install code needs them.
- [ ] Ensure Profile extraction uses the same namespace/name/version path sanitization, archive traversal protection, integrity verification, atomic replacement, and cleanup behavior as other package kinds.
- [ ] Include Profile in supported install-response and artifact-kind handling while continuing to reject Template artifacts from generic `agentpm install` according to existing behavior.
- [ ] Ensure unsupported or unknown package-kind values still fail clearly and are not silently interpreted as Profile.
- [ ] Update Template/workspace exhaustive package-kind conversions only as necessary to preserve compilation and pass Profile values through shared structures; defer reading `template.dependencies.profiles`, synthesizing Profile references, and installing Template Profiles to Milestone 7.
- [ ] Keep Agent top-level `profiles` unresolved in this milestone and retain the existing lint warning until Milestone 4 activates first-class Agent Profile dependency and lockfile support.
- [ ] Add unit tests for Profile package-key formatting, enum conversions, SDK request/response conversion, and backend-authoritative response-kind preservation.
- [ ] Add downloader and extraction tests using mocked Profile install artifacts, covering canonical destination paths, namespace and version layout, integrity success and failure, malformed or missing artifacts, archive traversal rejection, atomic replacement, and cleanup after failure.
- [ ] Add a mocked direct Profile install test without a local Agent manifest, limited to resolver response handling and filesystem extraction; defer persistent root, refresh, frozen-lock, and replacement semantics to Milestone 4.
- [ ] Add regression tests confirming Tool, Agent, Skill, Knowledge, and Memory resolver conversion and extraction paths remain unchanged and Template packages remain rejected by generic install.
- [ ] Audit superseded-download cleanup, partial extraction cleanup, Windows and Unix path handling, and all shared install-root construction sites for the new Profile directory.

## Milestone 4: First-Class Profile Dependencies and Lockfile Relationships
> Scope note: activate Agent `profiles` dependency resolution and promote Profile relationships from `reserved.profiles` into the first-class lockfile graph. Follow the existing Skill, Knowledge, and Memory patterns for local and registry Agents, legacy migration, frozen-lock validation, reachability, pruning, and direct dependency updates. Profiles are leaf packages. This milestone does not add backend relationship expansion, Template `new` support, SDK loaders, Profile binding or layering, prompt compilation, or runtime enforcement.
- [ ] Add first-class `profiles: Vec<String>` relationships to `LockedRoot`, `LockRoot::LocalAgent`, and `LockRoot::RegistryAgent`, preserving compatibility with lockfiles that omit the field.
- [ ] Thread Profile relationships through lock construction, normalization, comparison, serialization, and existing root conversion paths.
- [ ] Keep `ReservedReferences.profiles` for backward-compatible deserialization and unresolved legacy references.
- [ ] Parse Agent top-level `profiles` into Profile package requirements using the existing string and object package-reference forms.
- [ ] Resolve local Agent Profile dependencies and record canonical Profile package keys on the local Agent lock root.
- [ ] Read Profile dependencies from installed registry Agent manifests and record them on registry Agent lock roots.
- [ ] Treat Profile packages as leaves and do not inspect Profile metadata for additional package dependencies.
- [ ] Return clear errors for missing or wrong-kind Profile dependencies.
- [ ] Add Profile handling to direct dependency-kind detection and update a local Agent’s top-level `profiles` collection when a Profile is installed directly.
- [ ] Preserve existing dependency range update behavior, including `--update-range`.
- [ ] Remove the existing warning that Agent Profiles are preserved but not resolved once Profile dependency resolution is active.
- [ ] Add `migrate_reserved_profiles` following the existing reserved Skill, Knowledge, and Memory migration patterns.
- [ ] Move resolvable legacy Profile references into first-class root `profiles` relationships while preserving references that cannot be resolved safely.
- [ ] Keep lockfile version 3 and require it when an Agent root contains first-class Profile relationships.
- [ ] Preserve compatibility with older v3 locks containing `reserved.profiles`, including frozen-lock behavior consistent with the existing reserved relationship migrations.
- [ ] Include Profile relationships in graph reachability, package retention, deduplication, pruning, and deterministic lock serialization.
- [ ] Treat standalone direct Profile installs like direct Knowledge and Memory installs rather than creating Agent- or Skill-style roots.
- [ ] Ensure Profile upgrades and dependency removal prune superseded or unreachable Profile packages according to existing lockfile behavior.
- [ ] Update lockfile fixtures and assertions that currently expect Agent Profiles to remain reserved.
- [ ] Add tests covering local and registry Agent Profiles, multiple and shared Profiles, direct Profile installation, missing and wrong-kind dependencies, frozen locks, legacy `reserved.profiles` migration, reachability, pruning, and regression behavior for existing package kinds.

## Milestone 5: Profile Publishing, Persistence, and Registry Dependency Resolution
> Scope note: make Profile packages publishable and persistable as immutable leaf artifacts, add the backend and database package-kind support required for Profile rows, and recognize Agent and Template Profile relationships. Follow existing package publishing, common-file handling, signing, private namespace, dependency-resolution, and artifact-kind migration patterns. Agent Profile dependencies participate in normal Agent installation; Template Profile dependencies are validated and persisted here but remain deferred to `agentpm new`. This milestone does not add build outputs, Profile-specific storage columns, runtime activation, Profile parameters, prompt compilation, registry UI, or language SDK loaders.
- [ ] Add `PublishManifest::Profile`, Profile parse dispatch, supported-kind messaging, and exhaustive CLI publish handling.
- [ ] Add Profile archive packaging containing `agent.json` and supported manifest-declared README and license files through existing common packaging behavior.
- [ ] Do not package arbitrary undeclared files or add a Profile-specific archive layout, entrypoint, manual payload, generated metadata, or build check.
- [ ] Ensure Profile publishing uses shared manifest validation and does not mutate authored source files.
- [ ] Add CLI publish tests for minimal and full Profiles, common README/license handling, invalid or unsafe declared files, dependency rejection, exact archive contents, and no-build behavior.
- [ ] Add `profile` to backend package-kind types, publish-kind detection, validation, service dispatch, supported-kind errors, and package detail URL generation.
- [ ] Allow Profile publishing through the existing `packages:publish` scope while keeping legacy `tools:publish` Tool-only.
- [ ] Support Profile publish-init and finalize through existing package creation, upload, signing, attestation, malware scanning, private namespace, idempotency, and version persistence flows.
- [ ] Validate that Profile manifests contain the structured singular `profile` object and do not declare package dependencies.
- [ ] Reuse common README and license extraction and persistence.
- [ ] Preserve the complete Profile manifest in existing manifest JSON storage without adding Profile-specific database columns.
- [ ] Add Agent top-level `profiles` to backend dependency extraction and normal Agent install-graph expansion.
- [ ] Require Agent Profile references to resolve to packages whose stored kind is `profile`.
- [ ] Keep Profile packages as dependency leaves.
- [ ] Add Template `dependencies.profiles` to publish-time dependency extraction, validation, and relationship persistence.
- [ ] Require Template Profile references to resolve to packages whose stored kind is `profile`.
- [ ] Do not expand Template Profile dependencies during generic `agentpm install`; defer their installation to Milestone 7.
- [ ] Preserve existing string/object reference forms, version-range behavior, deduplication, private dependency access checks, and authoritative-kind behavior for direct package resolution.
- [ ] Add the Profile artifact-kind database migration using the existing Skill, Knowledge, and Memory migration pattern.
- [ ] Update `tools_kind_check` to allow `profile`.
- [ ] Update the install-completion trigger to count direct and dependency Profile installs.
- [ ] Recreate `trending_tools` with Profile included as its own package-kind partition.
- [ ] Recreate `tool_search_index` and its existing indexes without adding nested Profile metadata to the search document.
- [ ] Preserve existing ranking, visibility, signing, install-count, and search behavior.
- [ ] Provide a downgrade consistent with existing artifact-kind migrations.
- [ ] Add backend tests for Profile publish-init/finalize, public and private namespaces, signing, common README/license metadata, direct Profile resolution, and authoritative stored kind.
- [ ] Add Agent and Template relationship tests covering valid, missing, private, shared, incompatible-version, and wrong-kind Profile dependencies.
- [ ] Add resolve/install tests confirming Agent Profiles expand normally, Profile packages remain leaves, and Template Profiles do not expand during generic install.
- [ ] Add migration verification for Profile row persistence, search indexing, per-kind trending, and install-stat counting.
- [ ] Add regression coverage confirming existing package kinds retain their publish, relationship, install, search, trending, and migration behavior.

## Milestone 6: Backend Read APIs, Search, Trending, and Statistics
> Scope note: complete first-class backend read and discovery support for persisted Profile packages. Add `profile` to generic package models, response contracts, filters, namespace listings, search, trending, statistics, authorization, and common metadata surfaces. Return Profile metadata through the stored manifest and reuse existing package APIs rather than introducing Profile-specific persistence or endpoint families. This milestone does not modify the database migration from Milestone 5, publish or resolve packages, process Template workspaces, index nested Profile fields, add runtime or build APIs, render frontend pages, or add language SDK loaders.
- [ ] Add `profile` to remaining backend package-kind enums, Literals, DTOs, serializers, application-service types, route validators, and hardcoded read-side allowlists.
- [ ] Ensure generic package and version detail APIs support public and authorized private Profiles and preserve `kind: "profile"` in responses.
- [ ] Return the complete structured Profile contract through the stored manifest without adding Profile-specific database columns or duplicated response fields.
- [ ] Support common README, license, signing, security, version-listing, latest-version behavior for Profiles.
- [ ] Add Profile support to namespace package listings and apply existing namespace visibility, membership, and authorization behavior.
- [ ] Add `profile` to search filters and results, including existing query, sort, pagination, visibility, and private-access behavior.
- [ ] Keep search indexing limited to the existing package name, namespace, and description fields; do not index nested Profile metadata.
- [ ] Add `profile` to trending filters and responses while preserving existing public-only, per-kind ranking behavior.
- [ ] Add Profile support to generic package statistics and ensure Profile installs recorded by the Milestone 5 trigger appear in existing totals and buckets.
- [ ] Preserve historical install-plan compatibility and existing aggregate-statistics behavior.
- [ ] Ensure canonical Profile frontend links use `/profiles/<package-id>/v<version>/overview` where backend URL helpers produce package links.
- [ ] Do not add a Profile-specific metadata API; consumers should read the singular `profile` object from the generic manifest response.
- [ ] Ensure existing artifact-specific APIs reject Profiles according to established wrong-kind behavior, and do not add Profile-specific build, inspect, query, manual, contract, execution, prompt-compilation, compatibility-evaluation, or enforcement endpoints.
- [ ] Add backend tests for Profile package/version details, README and security metadata, namespace visibility, authorization, yanking, and response serialization.
- [ ] Add search, trending, and statistics tests covering Profile filtering, public/private visibility, pagination or ranking behavior, and install counts.
- [ ] Add negative tests confirming artifact-specific endpoints reject Profiles.
- [ ] Add regression coverage confirming existing package kinds retain their read, discovery, authorization, search, trending, and statistics behavior.

## Milestone 7: Template `new` and Workspace Integration
> Scope note: add Profile dependency support to `agentpm new` using the existing Template, Agent, install, and workspace-lock patterns. Resolve and install direct Template Profiles, write exact resolved references into the synthesized root Agent, and preserve Profiles declared by generated local Agents. Profiles remain immutable leaf packages represented through Agent relationships. This milestone does not redefine the Template schema, add workspace-level Profile roots, introduce Profile parameters or prompts, mutate installed Profiles, define binding or layering behavior, assemble prompts, or execute runtime behavior.
- [ ] Include `template.dependencies.profiles` in `agentpm new` dependency requests using `PackageKind::Profile`.
- [ ] Include Profile requirements declared by rendered local Agent manifests through the normal Agent dependency parser.
- [ ] Apply existing version resolution, deduplication, wrong-kind validation, private dependency access, and conflict behavior to Profile requirements.
- [ ] Install resolved Profiles into the existing `.agentpm/profiles/<namespace>/<name>/<version>` layout.
- [ ] Keep Profile packages as dependency leaves and do not inspect their metadata for additional requirements.
- [ ] Add `resolved_profile_manifest_refs` following the existing Skill, Knowledge, and Memory helper patterns.
- [ ] Materialize direct Template Profiles as exact-version references in the synthesized root Agent’s top-level `profiles` array.
- [ ] Keep Profiles explicitly declared by generated local Agents attached to those Agents rather than copying direct Template Profiles into every Agent.
- [ ] Include root and local Agent Profile relationships in workspace lock generation using the first-class lockfile support from Milestone 4.
- [ ] Deduplicate shared Profile packages across workspace Agent roots according to existing lock graph behavior.
- [ ] Do not add Profiles to `WorkspacePackageRoots` or `agentpm.workspace.json`; represent direct Template Profiles through the synthesized root Agent.
- [ ] Do not prompt for Profile values, reinterpret Template variables as Profile parameters, or substitute variables into installed Profile package contents.
- [ ] Ensure subsequent workspace install and frozen-lock flows handle Profile dependencies through the same Agent manifest and lockfile behavior as other first-class relationships.
- [ ] Add tests for direct Template Profiles, Profiles declared by generated Agents, shared Profiles, missing or wrong-kind dependencies, private Profiles, synthesized Agent output, workspace lock output, reinstall, and frozen behavior.
- [ ] Add regression coverage confirming existing Tool, Agent, Skill, Knowledge, and Memory behavior in `agentpm new` remains unchanged.

## Milestone 8: Registry Web Experience
> Scope note: expose Instruction Profiles as a first-class user-facing registry package type while retaining `profile` as the technical kind. Add discovery, cards, dependency links, versioned detail pages, and structured manifest presentation through existing package UI patterns. This milestone renders authored metadata only; it does not add Profile-specific backend APIs, treat README content as instructions, edit or configure Profiles, calculate compatibility, enforce constraints, introduce personality-marketplace framing, or alter resolution and installation behavior.
- [ ] Add `profile` to search, trending, statistics, dependency, package-detail, namespace-listing, and route TypeScript unions.
- [ ] Add Profile manifest and API types matching the structured contract and existing generic package/version response shapes.
- [ ] Add a thin Profile fetch helper using the existing generic package, version, README, and Security APIs, including private namespace access behavior.
- [ ] Do not add or expect a Profile-specific metadata backend endpoint.
- [ ] Add Profile cards using the visible label **Instruction Profile**, while preserving `"profile"` in technical API values and route dispatch.
- [ ] Include correct detail links, install snippets, version metadata, signing state, namespace visibility, and existing package-card actions.
- [ ] Add Profile support to Explore filters, search results, global search, trending sections, package-kind badges, namespace package lists, and route generation.
- [ ] Add Profile dependency presentation to Agent and Template detail surfaces, linking to the resolved version when relationship data includes one.
- [ ] Add versioned Profile pages following existing package route conventions, with canonical URLs such as `/profiles/<profileId>/v<version>/overview`.
- [ ] Add Overview, README, and Security tabs only; do not add build, query, lifecycle, contract, execution, manual, or instruction-file tabs.
- [ ] Render identity, expertise, objectives, principles, audience, communication, vocabulary, boundaries, constraints, and compatibility sections when present.
- [ ] Render optional sections gracefully without empty cards or placeholder content.
- [ ] Label constraint strength accurately and state concisely that Profile constraints express declared behavior rather than runtime enforcement.
- [ ] Present minimum context and required/recommended capabilities as authored advisory metadata without calculating compatibility status.
- [ ] Keep README presentation separate from the structured behavioral contract.
- [ ] Add Instruction Profile copy anywhere the registry enumerates or promotes first-class package kinds, following existing visual and editorial patterns.
- [ ] Add component and route tests for Profile cards, filters, namespace listings, public/private detail loading, version routing, dependency links, optional fields, wrong-kind or missing packages, and responsive layouts.
- [ ] Audit page metadata, canonical URLs, breadcrumbs, loading and empty states, version-not-found behavior, and 404 handling for Profile routes.

## Milestone 9: Node SDK
> Scope note: add typed Node SDK support for locating and loading installed Profile package metadata and exposing resolved Agent Profile dependencies through existing package and lockfile conventions. `loadProfile` is a metadata loader only; it does not read README content as instructions, load generated outputs, interpolate values, compile prompts, select or combine Profiles, evaluate compatibility, enforce constraints, invoke a harness, or execute agent behavior.
- [ ] Add `profile` to public package-kind unions and lockfile package/root types.
- [ ] Add typed interfaces for the complete Profile manifest contract, including identity, objectives, principles, audience, communication, vocabulary, boundaries, constraints, and compatibility.
- [ ] Add typed Profile manifest and loaded-result interfaces consistent with existing SDK loader models.
- [ ] Add `LoadProfileOptions` using the same package resolution and directory-override conventions as existing loaders.
- [ ] Implement `loadProfile` using installed package resolution parallel to `loadKnowledge`, but without generated metadata, contract, index, or auxiliary-file loading.
- [ ] Return package identity, package key, integrity, root, manifest path, parsed manifest, and typed `profile` metadata according to existing SDK conventions.
- [ ] Reject missing packages, missing or malformed manifests, wrong package kinds, and missing structured Profile metadata with clear errors.
- [ ] Do not require README or license files to exist in order to load valid Profile metadata.
- [ ] Update generic Tool `load()` detection and guidance to direct Profile callers to `loadProfile`.
- [ ] Update `loadAgent` to expose resolved Profile dependencies from first-class root `profiles` relationships using existing missing-on-disk and nullable-path conventions.
- [ ] Preserve legacy `reserved.profiles` data only to the extent existing lockfile models expose reserved references; do not convert it into active Profile relationships in the SDK.
- [ ] Export `loadProfile` and all public Profile types from the SDK entrypoint.
- [ ] Add tests for direct loading, directory overrides, minimal and full Profiles, missing or malformed manifests, wrong kinds, Tool-loader guidance, Agent Profile relationships, missing installed dependencies, multiple Profiles, and shared Profiles.
- [ ] Confirm Profile loading introduces no prompt compilation, README interpretation, Profile composition, compatibility evaluation, or constraint enforcement.

## Milestone 10: Python SDK
> Scope note: add typed Python SDK support equivalent to the Node SDK for locating and loading installed Profile metadata and exposing resolved Agent Profile relationships. `load_profile` is a package metadata loader only; it does not read README content as instructions, load generated outputs, interpolate configuration, compile prompts, bind or combine Profiles, evaluate compatibility, enforce constraints, invoke a runtime, or execute agent behavior.
- [ ] Add `profile` to public package-kind Literals, lockfile package/root TypedDicts, and internal package-resolution handling.
- [ ] Add TypedDicts for the complete Profile manifest contract, including identity, objectives, principles, audience, communication, vocabulary, boundaries, constraints, and compatibility.
- [ ] Add typed Profile manifest and loaded-result models consistent with existing Python SDK loaders.
- [ ] Implement `load_profile` using installed package resolution parallel to `load_knowledge`, but without generated metadata, contract, index, or auxiliary-file loading.
- [ ] Support a Profile directory override using the same conventions as existing Python loaders.
- [ ] Return package identity, package key, integrity, root, manifest path, parsed manifest, and typed `profile` metadata according to existing SDK conventions.
- [ ] Reject missing packages, missing or malformed manifests, wrong package kinds, and missing structured Profile metadata with clear errors.
- [ ] Do not require README or license files to exist in order to load valid Profile metadata.
- [ ] Update generic Tool `load()` guidance to direct Profile callers to `load_profile`.
- [ ] Update `load_agent` to expose resolved first-class Profile dependencies using the same missing-on-disk and nullable-path conventions as Node.
- [ ] Preserve legacy `reserved.profiles` data only to the extent existing lockfile models expose reserved references; do not convert it into active Profile relationships in the SDK.
- [ ] Export `load_profile` and public Profile types through `agentpm.__init__`, `__all__`, and relevant type modules.
- [ ] Add tests for direct loading, directory overrides, minimal and full Profiles, missing or malformed manifests, wrong kinds, Tool-loader guidance, Agent Profile relationships, missing installed dependencies, multiple Profiles, and shared Profiles.
- [ ] Confirm Python and Node behavior align on field names, return metadata, nullability, error behavior where practical, and all non-enforcement boundaries.

## Milestone 11: Documentation, Compatibility Audit, and Final Verification
> Scope note: finish the implemented product surface by documenting the final contracts and boundaries, auditing every existing package-kind surface for omissions, and running the required cross-repository verification before seeding real published examples. This milestone should close gaps and regressions against `spec.md`; it must not introduce new schema concepts, Profile parameters, build/runtime behavior, binding or layering rules, enforcement claims, provider-specific prompt formats, or other feature expansion that was not implemented and tested in earlier milestones.
- [ ] Update manifest reference documentation with the complete Profile schema, required/optional fields, enums, examples, and common README/license behavior.
- [ ] Document `kind: "profile"`, Agent top-level `profiles`, and `template.dependencies.profiles`.
- [ ] Document the Profile-versus-Skill boundary with concrete examples.
- [ ] Document that required constraints are author intent and not enforcement.
- [ ] Document that Profiles have no build command, generated output, runtime execution, parameters, variables, or install-time prompts.
- [ ] Document direct install, Agent dependency install, Template `new`, registry discovery, `loadProfile`, and `load_profile` flows.
- [ ] Audit the full codebase for hardcoded `tool|agent|template|skill|knowledge|memory` strings and add `profile` where package kinds are intended.
- [ ] Audit Rust and API client enums, CLI help text, error messages, OpenAPI/schema definitions, tests, fixtures, and sample data.
- [ ] Audit database SQL, materialized views, triggers, statistics, search, trending, namespace access, signing, malware/tar validation, and canonical URL helpers.
- [ ] Audit frontend route maps, type unions, cards, filters, badges, stats, global search, metadata, landing content, and dependency rendering.
- [ ] Audit Node and Python public exports and package documentation.
- [ ] Bump the released version numbers for the CLI, the Node SDK, and the Python SDK as part of the final pre-example release pass.
- [ ] Update the web status page version and date values so they reflect the released Instruction Profile-capable CLI and SDK builds.
- [ ] Confirm Profile README and license files are packaged and displayed through common behavior.
- [ ] Confirm Profiles cannot declare dependencies in schema, CLI publish, or backend publish paths.
- [ ] Confirm old v3 locks with `reserved.profiles` remain readable and regenerate safely.
- [ ] Run all verification in `test-plan.md` and report evidence, failures, skipped checks, and migration verification.

## Milestone 12: Examples
> Scope note: seed real published Instruction Profile examples only after Milestone 11 documentation, compatibility audit, and verification are complete. These examples should reinforce the established product story, exercise different optional Profile fields, and be suitable to publish to AgentPM production without expanding the phase beyond the implemented Profile contract.
- [ ] Add a new `profile-packages/` directory in `agentpm-examples` for published Instruction Profile examples, following the same organizational pattern used for other package kinds.
- [ ] Build **3 to 4** complete Profile package examples rather than the previous minimum of two.
- [ ] Make the example set production-worthy and realistic enough to publish to AgentPM production as seeded content, while still exercising as many optional Profile fields as fit naturally.
- [ ] Prefer concrete examples that extend the existing AgentPM story instead of isolated toy artifacts.
- [ ] Add `@zack/support-response-style` as a customer-support communication Profile with strong use of communication, vocabulary, boundaries, constraints, and compatibility metadata.
- [ ] Publish `@zack/support-response-style`, then add it to the support-oriented template package flow where it fits cleanly, and finally update the corresponding `agent-app-*` example so the installed Profile is visible through the SDK/example app path.
- [ ] Add `@zack/incident-operator-style` as an operations-facing Profile for incident updates, risk communication, and escalation language, using a different communication pattern from the support example.
- [ ] Publish `@zack/incident-operator-style`, then add it to the relevant existing operations agent package and update the matching `agent-app-*` example so resolved Profile dependencies are exercised end to end.
- [ ] Add `@zack/devwork-maintainer-style` as a maintainer-facing Profile that fits the devwork example story and emphasizes concise operational judgment, review posture, and handoff expectations.
- [ ] Publish `@zack/devwork-maintainer-style`, then add it to the relevant existing devwork agent package and update the matching `agent-app-*` example if it fits cleanly without expanding scope beyond the current story.
- [ ] Optionally add a fourth standalone Profile example only if it adds clear seeded value and distinct optional-field coverage without forcing a contrived integration path.
- [ ] Ensure at least one example is wired through a Template, at least one is wired through an Agent package, and the resulting installed Profiles are surfaced in at least one updated `agent-app-*` directory.
- [ ] Ensure the example set explicitly covers both template-consumed Profiles and agent-consumed Profiles rather than only one integration style.
- [ ] Ensure at least one updated `agent-app-*` example surfaces loaded Profile metadata in a human-visible way so the end-to-end SDK loading path is demonstrated, not just the manifest and lockfile dependency edges.
- [ ] Use a broad mix of implemented optional fields across the example set, including realistic use of principles, audience, expertise, communication guidelines, formatting preferences, vocabulary prefer/avoid lists, boundaries, constraints, and compatibility hints where appropriate.
- [ ] Keep examples within the implemented Profile contract only; do not imply runtime enforcement, prompt compilation, profile layering, profile composition, variable interpolation, or other unimplemented behavior.
- [ ] Use only implemented Profile contract fields and current product behavior; do not introduce speculative binding, layering, enforcement, or prompt-compilation concepts through the examples.
- [ ] Ensure each example’s README explains what the Profile is for, how it differs from a Skill in the same story, and that the structured `profile` object is the portable contract while the README remains package documentation.
- [ ] Validate each example with the normal lint, publish, install, registry-display, and SDK loading flows before treating the phase as complete.
