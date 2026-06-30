# Tasks

## Milestone 1: Skill Manifest Contract
> Scope note: this milestone establishes Skills as a valid package contract, but does not make them publishable, installable, searchable, or visible in the registry yet. It should update the JSON schema, manifest parsing/types, and validation tests so the rest of the implementation can rely on `kind: "skill"` being a well-defined manifest shape. Packaging, backend persistence, install graph behavior, SDK loading, and frontend UX are covered by later milestones.
- [ ] Add `skill` to the manifest `kind` enum in `schemas/agentpm.manifest.schema.json`.
- [ ] Add `$defs.skillMetadata` with required `entrypoint` and optional `references`, `scripts`, and `compatibility` fields.
- [ ] Add a reusable safe-relative-path definition for Skill file paths, matching existing template path safety rules where possible.
- [ ] Add top-level `skill` property referencing `$defs.skillMetadata`.
- [ ] Add a `oneOf` branch requiring `skill` when `kind` is `skill`.
- [ ] Update dependent schemas so `tools` is allowed for `agent` and `skill`.
- [ ] Update dependent schemas so `skills` is allowed for `agent` as a first-class dependency field.
- [ ] Ensure `skills` is rejected for `kind: "skill"`.
- [ ] Keep `knowledge`, `memory`, and `profiles` agent-only reserved fields.
- [ ] Update template dependency schema to support optional `template.dependencies.skills`.
- [ ] Ensure existing template manifests without `template.dependencies.skills` remain valid.
- [ ] Update CLI manifest structs/types to include `SkillManifest` and `SkillMetadata`.
- [ ] Update `PublishManifest` to include a `Skill` variant.
- [ ] Update manifest parsing to return `PublishManifest::Skill` for `kind: "skill"`.
- [ ] Update manifest kind/name/version helper functions used by publish/signature code to include Skill.
- [ ] Add schema/manifest unit tests for valid procedural Skill with no tools.
- [ ] Add schema/manifest unit tests for valid tool-backed Skill.
- [ ] Add schema/manifest unit tests for valid agent manifest with `skills`.
- [ ] Add schema/manifest unit tests for valid template manifest with `template.dependencies.skills`.
- [ ] Add schema/manifest unit tests proving existing templates without `template.dependencies.skills` remain valid.
- [ ] Add schema/manifest unit tests rejecting Skill-to-Skill dependencies.
- [ ] Add schema/manifest unit tests rejecting unsafe Skill file paths.

## Milestone 2: CLI Init Support
> Scope note: this milestone adds the simplest authoring entry point for new Skills. After this milestone, a user should be able to run `agentpm init --kind skill` and get a valid local Skill package skeleton. This does not publish, install, export, or resolve Skill dependencies yet; it only creates valid starter files that later milestones can package and publish.
- [ ] Add `Skill` to the `InitKind` enum.
- [ ] Update `agentpm init --help` text to mention tool, agent, template, and skill.
- [ ] Add asset template for a minimal Skill `agent.json`.
- [ ] Add asset template for a minimal workflow-oriented `SKILL.md`.
- [ ] Implement `agentpm init --kind skill` to write `agent.json` and `SKILL.md`.
- [ ] Do not create `references/` or `scripts/` from the init path.
- [ ] Ensure generated Skill manifest validates against the updated schema.
- [ ] Add unit test that `init --kind skill` creates both files.
- [ ] Add unit test that generated Skill `agent.json` validates.
- [ ] Add unit test that existing tool/agent/template init behavior still works.


## Milestone 3: Skill Publish Packaging
> Scope note: this milestone makes the CLI capable of packaging Skill artifacts correctly, primarily through `agentpm publish --dry-run`. After this milestone, the CLI should be able to validate and build a deterministic Skill tarball containing only the files declared by the Skill manifest. This does not require the backend to accept Skill publishes yet; backend kind support, dependency access validation, registry storage, and install behavior are covered by later milestones.
- [ ] Add `PublishManifest::Skill` branch in CLI publish dispatch.
- [ ] Implement `package_skill(manifest: &SkillManifest, manifest_path: &Path)`.
- [ ] Package `agent.json` as root `agent.json`.
- [ ] Package the declared `skill.entrypoint` file.
- [ ] Package all declared `skill.references[]` files.
- [ ] Package all declared `skill.scripts[]` files.
- [ ] Package top-level `readme` file if it points to a local file and is not already included.
- [ ] Package `license.file` if present and not already included.
- [ ] Preserve relative paths exactly in the tarball.
- [ ] Deduplicate repeated declared paths.
- [ ] Reject missing declared Skill files.
- [ ] Reject unsafe declared paths, including absolute paths and parent traversal.
- [ ] Reuse existing tar safety helpers such as `append_checked`, `ensure_safe_tar_name`, embedded archive blocking, tar entry limits, and artifact byte limits.
- [ ] Ensure executable bits are preserved for scripts on Unix as existing tar append behavior allows.
- [ ] Ensure undeclared files in the Skill directory are not included.
- [ ] Update artifact filename logic if it pattern matches only tool/agent/template.
- [ ] Update publish signing statement helpers to support Skill kind.
- [ ] Add dry-run publish test for minimal Skill.
- [ ] Add dry-run publish test for Skill with references and scripts.
- [ ] Add dry-run publish test for Skill with top-level README and license file.
- [ ] Add dry-run publish test proving undeclared files are omitted.
- [ ] Add dry-run publish test for missing declared file failure.
- [ ] Add dry-run publish test for unsafe Skill path failure.
- [ ] Re-run existing tool, agent, and template publish tests.

## Milestone 4: Backend Package Kind, Publish, and Install Resolution Support
> Scope note: this milestone teaches the registry backend that `skill` is a real package kind. After this milestone, the backend should be able to accept Skill publishes, store Skill packages, return Skill detail URLs, validate Skill dependency access, and resolve install graphs that include agents depending on Skills and Skills depending on tools. This does not require the CLI to extract Skills to `.agentpm/skills`, write lockfile v3, or update frontend search/detail UX yet; those are covered by later milestones.
- [ ] Add database migration updating the `tools.kind` check constraint to allow `skill`.
- [ ] Update SQLAlchemy/domain package kind assumptions if any are hardcoded to `tool|agent|template`.
- [ ] Update backend publish validation to accept `manifest.kind == "skill"`.
- [ ] Update backend publish error text to mention `tool`, `agent`, `template`, or `skill`.
- [ ] Update backend `_normalize_install_item` to accept `skill`.
- [ ] Update backend package detail URL helper to return `/skills/<package_id>/v<version>/overview` for Skill packages.
- [ ] Add backend parser/helper for top-level `skills` requirements, mirroring existing top-level `tools` requirement parsing.
- [ ] Update backend direct dependency extraction so `kind: "skill"` returns top-level tool requirements.
- [ ] Update backend agent dependency extraction so agents validate top-level `tools` and top-level `skills`.
- [ ] Update backend template dependency extraction so templates validate `tools`, `agents`, and `skills`.
- [ ] Update backend install graph expansion so installed agent packages expand declared tools and skills.
- [ ] Update backend install graph expansion so installed Skill packages expand declared tools.
- [ ] Ensure Skill-to-Skill dependency expansion is not supported in Phase 6A.
- [ ] Ensure installed templates do not recursively expand during regular `agentpm install`, preserving existing template behavior.
- [ ] Ensure dependency access validation catches inaccessible private Skill dependencies on publish finalize.
- [ ] Ensure dependency access validation catches inaccessible private Tool dependencies declared by Skills.
- [ ] Ensure registry attestation and package signature statement validation work for Skill kind.
- [ ] Add backend tests for Skill publish init/finalize.
- [ ] Add backend tests for kind conflict between existing tool/agent/template and new Skill of same package name.
- [ ] Add backend tests for installing a Skill directly.
- [ ] Add backend tests for agent dependency graph expansion through Skills to tools.
- [ ] Add backend tests proving Skill-to-Skill dependencies are rejected or ignored according to the manifest/schema contract.
- [ ] Add backend tests for inaccessible Skill dependency failure.
- [ ] Add backend tests for inaccessible Tool dependency declared by a Skill.

## Milestone 5: CLI Install, Resolver, Extraction, and Lockfile
> Scope note: this milestone makes Skill packages installable from the CLI and makes Skills first-class in the resolved dependency graph. After this milestone, users should be able to install a Skill directly, install an agent that depends on Skills, run `agentpm install` inside a local Skill package to resolve its tools, and get a `lockfile_version: 3` lockfile with first-class `skills` relationships. This milestone depends on backend resolve support from Milestone 4, but does not add SDK `load_skill`, export manifest generation, registry UI, or documentation polish.
- [ ] Add `Skill` to CLI semver/package kind enum.
- [ ] Update package key formatting/parsing to support `skill:@namespace/name@version`.
- [ ] Update desired-set generation to parse agent top-level `skills` as Skill requirements.
- [ ] Update desired-set generation to parse local skill top-level `tools` as Tool requirements.
- [ ] Update template/workspace desired-set generation to parse template `dependencies.skills`.
- [ ] Update CLI install request/response adapters to support Skill package kind.
- [ ] Update `ensure_supported_install_kinds` so direct Skill installs are allowed while direct Template installs remain rejected.
- [ ] Support manifest-driven `agentpm install` for local `kind: "skill"` manifests.
- [ ] Ensure install creates `.agentpm/skills` alongside `.agentpm/tools` and `.agentpm/agents`.
- [ ] Update download/extract routing so Skill artifacts extract to `.agentpm/skills/<namespace>/<name>/<version>/`.
- [ ] Add helper equivalent to `installed_agent_manifest_path` for installed Skill manifests.
- [ ] Update registry root builders to create Skill roots when direct Skill packages are installed.
- [ ] Update local agent lock root building to put resolved Skill package keys in first-class `skills` field.
- [ ] Update registry agent lock root building to put resolved Skill package keys in first-class `skills` field.
- [ ] Update local Skill lock root building to include resolved tool package keys.
- [ ] Update registry Skill lock root building to include resolved tool package keys.
- [ ] Update `LockedRoot` and `LockRoot` structs to include first-class `skills`.
- [ ] Implement `lockfile_version: 3` for lockfiles containing resolved Skill packages or first-class `skills` root relationships.
- [ ] Continue reading supported v1/v2 lockfiles for existing projects.
- [ ] Remove `skills` from `ReservedReferences` for newly written v3 locks.
- [ ] Keep `knowledge`, `memory`, and `profiles` in `ReservedReferences`.
- [ ] Add compatibility/migration logic for old lockfiles containing `reserved.skills`.
- [ ] During non-frozen install, migrate old `reserved.skills` entries into first-class `skills` relationships when they resolve as Skill packages.
- [ ] Preserve or clearly fail old `reserved.skills` entries that cannot be resolved; do not silently drop them.
- [ ] Update frozen-lock validation so v1/v2 locks fail clearly when the desired graph includes Skill dependencies that require v3.
- [ ] Update workspace lock generation to include Skill dependencies in local agent roots and package roots where relevant.
- [ ] Update direct install manifest mutation logic so direct Skill installs do not incorrectly append to `tools`.
- [ ] Do not auto-append direct Skill installs to local `agent.json` in Phase 6A; require users to explicitly add Skill dependencies to `skills`.
- [ ] Add tests for direct Skill install without a local manifest.
- [ ] Add tests for direct Skill install with a local agent manifest.
- [ ] Add tests for manifest-driven local Skill install resolving declared tools.
- [ ] Add tests for local agent install resolving declared Skills and Skill tool dependencies.
- [ ] Add tests for lockfile v3 shape with first-class `skills`.
- [ ] Add tests for old `reserved.skills` lockfile migration.
- [ ] Add tests for frozen v1/v2 lock failure when Skill dependencies are required.
- [ ] Add tests that direct Template install remains rejected.
- [ ] Re-run existing install workspace tests and update expected lock shape.

## Milestone 6: Export Skill Alignment
> Scope note: this milestone aligns the existing `agentpm export --skill` scaffold path with first-class Skill packages. After this milestone, export should still work as a backward-compatible scaffold generator, and users can optionally ask it to generate a starter `kind: "skill"` manifest. Export remains an authoring helper only: it must not install packages, update lockfiles, mutate workspace manifests, or publish anything.
- [ ] Add `--manifest` flag to `agentpm export --skill`.
- [ ] Keep default export behavior backward compatible when `--manifest` is omitted.
- [ ] When `--manifest` is passed, generate a valid `kind: "skill"` `agent.json` in the output directory.
- [ ] Generated export manifest should use `<tool-leaf-name>-skill` as the Skill package name by default to avoid package-name conflicts with the source tool.
- [ ] Generated export manifest should include top-level `tools` with the source tool pinned to the resolved version.
- [ ] Generated export manifest should include `skill.entrypoint`, `skill.references`, and `skill.scripts` pointing to generated files.
- [ ] Generated export manifest should include reasonable compatibility metadata, at minimum `runtimes: ["agentpm-run", "shell"]` if using the current run script.
- [ ] Update export resolver flow to prefer installed/locked tool metadata when present.
- [ ] Add remote registry resolution fallback when the tool is not installed.
- [ ] Remote fallback must fetch enough manifest metadata to render `SKILL.md`, `references/tool-contract.md`, `references/examples.md`, and `scripts/run.sh`.
- [ ] Remote fallback must not install the tool, write `.agentpm`, update `agent.lock`, or mutate workspace files.
- [ ] Remote fallback must send credentials when available so private accessible tools can be exported.
- [ ] Ensure `--force` controls overwriting generated `agent.json` as part of the output directory, matching existing overwrite behavior.
- [ ] Add tests for existing installed-tool export behavior.
- [ ] Add tests for installed-tool export with `--manifest`.
- [ ] Add tests that generated export manifest validates against the Skill schema.
- [ ] Add tests for remote export with `--manifest`.
- [ ] Add tests proving remote export does not mutate lockfile, workspace manifests, or install directories.
- [ ] Add tests for output overwrite behavior with `--force` still working.
- [ ] Add docs examples for `agentpm export --skill <tool>` and `agentpm export --skill <tool> --manifest`.

## Milestone 7: Search, Trending, and Registry API
> Scope note: this milestone makes Skills discoverable through backend search and registry API responses. After this milestone, API consumers should be able to request `skills` search results, see Skills in `all` package streams, and receive correct counts, cursors, DTOs, and package URLs. This does not implement the frontend Skill detail page, visual search tabs, SDK loading helpers, or docs; those are covered by later milestones.
- [ ] Update search API type enum to include `skills`.
- [ ] Update search service type routing so `type_=skills` filters `package_kind="skill"`.
- [ ] Update search service `package_kind` type annotations to include `skill`.
- [ ] Update result item type handling to include `skill`.
- [ ] Add `SkillDto` to common DTO types or generalize package DTOs if appropriate.
- [ ] Update `_tool_row_to_item` to return `{"itemType": "skill", "skill": ...}` for Skill rows.
- [ ] Update `_package_item_payload`, `_merge_results`, `_seek_from_tool_item`, and all package item checks to include `skill`.
- [ ] Ensure `all` search merging treats Skills as package items for relevance, newest, trending, most-downloaded, cursor generation, and pagination.
- [ ] Update `totals_by_type` to include `skills`.
- [ ] Update kind totals SQL aggregation mapping to include `skill`.
- [ ] Ensure `Trending`, `Most downloaded`, `Newest`, and `Relevance` include Skill packages in package streams.
- [ ] Update OpenAPI/API schema or TypeScript generated types if present.
- [ ] Update materialized view definitions only if they filter package kinds elsewhere; the shown `tool_search_index` already exposes `kind` generically.
- [ ] Update trending materialized view if it filters or display logic assumes only tools/agents/templates.
- [ ] Ensure materialized view refresh jobs continue to work after adding Skill kind.
- [ ] Add backend search tests for skills-only search.
- [ ] Add backend search tests for all-search totals including skills.
- [ ] Add backend search tests for all-search pagination/cursors with mixed package kinds including skills.
- [ ] Add backend search tests for trending/newest including skills.
- [ ] Add backend search tests for private Skill visibility.

## Milestone 8: Registry Frontend UX
> Scope note: this milestone makes Skills understandable and discoverable in the web registry. After this milestone, users should be able to find Skills, distinguish them from tools/agents/templates, open a Skill detail page, inspect declared dependencies and metadata, and copy an install command. This milestone consumes the search/API support from Milestone 7; it does not add SDK loading helpers, CLI install behavior, new backend dependency-resolution semantics, or the dedicated backend/frontend work needed to surface `SKILL.md` as first-class manual content.
- [ ] Add Skill kind badge/icon/display label.
- [ ] Add `/skills/<package_id>/v<version>/overview` route.
- [ ] Add Skill package detail page or adapt shared package detail page with Skill-specific sections.
- [ ] Display declared tool dependencies.
- [ ] Display declared `skill.references` and `skill.scripts`.
- [ ] Display compatibility metadata if present.
- [ ] Display install command for Skill packages.
- [ ] Ensure Skill pages do not show tool-only runtime/input/output sections as if the Skill itself is executable.
- [ ] Add Skills search tab/filter.
- [ ] Add Skills count to search totals UI.
- [ ] Ensure all-search result cards render Skills distinctly from tools, agents, and templates.
- [ ] Include Skills in homepage/trending package lists where package kinds are mixed.
- [ ] Update package detail URL generation and links anywhere kind is switched.
- [ ] Add frontend tests or manual verification notes for Skill search/detail rendering.

## Milestone 8a: Skill Manual Content (`SKILL.md`)
> Scope note: this milestone adds the missing backend and frontend support needed to display authored Skill manual content from `skill.entrypoint` / `SKILL.md` in the registry. After this milestone, a published Skill with an entrypoint file should expose that content distinctly from the normal README path, and the web Skill detail page should render it in a dedicated Manual tab. This milestone should not broaden SDK loading behavior or change the existing README contract beyond what is necessary to distinguish manual content from README content.
- [ ] Implement Skill manual ingestion/storage using the same overall publish pipeline shape as README: CLI publish payload -> backend validation -> persisted package-version fields -> detail API exposure.
- [ ] Do not overload existing README storage fields for Skill manuals; add parallel Skill-manual storage/exposure fields instead.
- [ ] Define the backend detail/readme contract for Skill entrypoint content.
- [ ] Decide whether to extend the existing tool readme DTO/route or add a dedicated Skill-manual field/route.
- [ ] Ensure Skill publish/finalize stores `skill.entrypoint` content and path separately from top-level README content when present.
- [ ] Ensure the stored Skill manual content preserves the declared `skill.entrypoint` path.
- [ ] Expose Skill manual content through the registry detail API for authorized viewers of public/private packages.
- [ ] Keep existing README behavior intact for tools, agents, and templates.
- [ ] Ensure Skill detail API responses can distinguish:
  - no manual content
  - manual content from `skill.entrypoint`
  - separate README content from top-level `readme`
- [ ] Update frontend Skill detail pages to render the Manual tab from the dedicated Skill manual payload.
- [ ] Keep the README tab separate and only render README content there.
- [ ] Add backend tests for Skill manual storage/exposure.
- [ ] Add frontend tests for:
  - Skill with manual only
  - Skill with manual plus separate README
  - Skill with no exposed manual content
- [ ] Add manual verification notes for published Skills where `SKILL.md` exists.

## Milestone 9: Node and Python SDK Skill Support
> Scope note: this milestone makes Skills usable from SDK-based agent runtimes as inspectable artifacts. After this milestone, SDK users should be able to load an agent, inspect its resolved Skill dependencies, load a Skill, read its entrypoint content, inspect its declared tool dependencies, and then load/run those tools through the existing tool `load(...)` API. Skills are not executable SDK objects in Phase 6A; `load(...)` remains tool-only and no `run_skill`/`skill.run` API should be introduced.
- [ ] Add `skill` to package kind enums/types in both Node and Python SDKs.
- [ ] Update SDK resolve/install request and response models to support Skill package refs and resolved Skill packages.
- [ ] Update SDK search/package DTO models to support Skill results.
- [ ] Add `SkillDto` / `LoadedSkill` types or equivalent SDK-native structures.
- [ ] Add manifest/type support for `kind: "skill"` and the `skill` metadata block where SDK manifest types exist.
- [ ] Keep existing generic `load(...)` tool-only in both SDKs.
- [ ] Ensure `load(...)` continues returning a callable tool function and is not overloaded to return Skill objects.
- [ ] If `load(...)` is called with a Skill package ref, fail clearly with guidance to use `load_skill` / `loadSkill`.
- [ ] Add Python `load_skill(...)`, modeled after existing `load_agent(...)`.
- [ ] Add Node `loadSkill(...)`, modeled after existing `loadAgent(...)`.
- [ ] Loaded Skill objects should include package name, version, description, manifest metadata, and the `skill` metadata block.
- [ ] Loaded Skill objects should include `entrypoint_path` and eager-loaded `entrypoint_content`.
- [ ] Loaded Skill objects should expose declared `references` and `scripts` paths without eagerly loading their contents in Phase 6A.
- [ ] Loaded Skill objects should include `resolved_tools` / `resolvedTools`, matching each SDK's existing naming convention.
- [ ] Add `resolved_skills` / `resolvedSkills` to loaded Agent objects.
- [ ] Ensure `resolved_skills` contains concrete Skill package refs with name and version.
- [ ] Ensure SDK Skill loading reads from the installed Skill artifact under `.agentpm/skills/<namespace>/<name>/<version>/`.
- [ ] Ensure SDK Skill loading resolves the Skill's declared top-level `tools` using the installed lock/package metadata rather than re-resolving from the registry when possible.
- [ ] Add SDK tests for loading a procedural Skill with no tools.
- [ ] Add SDK tests for loading a tool-backed Skill and reading `resolved_tools`.
- [ ] Add SDK tests for loading an Agent with `resolved_skills`.
- [ ] Add SDK tests proving `load(...)` still works for tools.
- [ ] Add SDK tests proving `load(...)` rejects Skill refs with a clear error.
- [ ] Add SDK README/docs examples for `load_skill` / `loadSkill`.

## Milestone 10: Documentation
> Scope note: this milestone updates user-facing documentation after the core Skill lifecycle has been implemented and should be completed before deploying the phase. After this milestone, users should understand what Skills are, when to use Skills versus tools/agents/templates, how to author Skills from scratch or from an exported tool scaffold, how to publish/install/discover Skills, and how to load Skills through the SDKs. This milestone should not introduce new behavior; it should document behavior implemented in earlier milestones.
- [ ] Update manifest reference docs with `kind: "skill"`.
- [ ] Document Skill semantics: reusable operational know-how, not necessarily executable code.
- [ ] Document when to use a Skill versus a Tool, Agent, or Template.
- [ ] Document top-level `tools` for Skill tool dependencies.
- [ ] Document that Skills cannot depend on Skills in Phase 6A.
- [ ] Document `skill.entrypoint`, `skill.references`, `skill.scripts`, and `skill.compatibility`.
- [ ] Document compatibility metadata as descriptive/discoverable only, not runtime-enforced.
- [ ] Document Skill packaging rules and declared-file behavior.
- [ ] Document that Skill packaging does not crawl Markdown links or include undeclared files.
- [ ] Update `agentpm init` docs to include `--kind skill`.
- [ ] Update `agentpm export --skill` docs to include `--manifest` and remote resolve fallback.
- [ ] Clarify that remote export does not install tools or mutate the workspace.
- [ ] Update install docs to include direct Skill install, local Skill manifest install, and agent Skill dependencies.
- [ ] Update lockfile docs to explain `lockfile_version: 3`, first-class `skills` root fields, and migration from old `reserved.skills`.
- [ ] Update publish docs to include Skill publish flow.
- [ ] Update search/registry docs to mention Skill discovery.
- [ ] Document SDK Skill loading with `load_skill` / `loadSkill`.
- [ ] Document that SDK `load(...)` remains tool-only and Skills are inspectable, not runnable, in Phase 6A.
- [ ] Update any roadmap/docs text that says Skills are not first-class artifacts yet.

## Milestone 11: Compatibility and Cleanup
> Scope note: this milestone is the final regression, consistency, and terminology pass after the main Skill lifecycle is implemented. It should catch hardcoded package-kind lists, stale `reserved.skills` assumptions, broken exports, docs/UI wording drift, private namespace regressions, and SDK export gaps. This milestone should not introduce major new Skill behavior; it should stabilize the completed implementation and verify existing tools, agents, and templates still work.
- [ ] Audit codebase for hardcoded `tool|agent|template` string lists and add `skill` where applicable.
- [ ] Audit API clients/SDK models for package kind enums.
- [ ] Audit generated API/OpenAPI clients for package kind enums and Skill DTO coverage.
- [ ] Audit CLI help text for package kind lists.
- [ ] Audit backend receipt/detail/package URL generation for Skill-safe routing.
- [ ] Audit frontend package URL generation and route links for Skill-safe routing.
- [ ] Audit tests that assert old lock root `reserved.skills` shape and update them.
- [ ] Audit lockfile v3 read/write paths for consistent `skills` and `reserved` handling.
- [ ] Audit docs and UI copy that use “tools” when they mean “packages,” but avoid broad table/model renames.
- [ ] Ensure old `agentpm export --skill` behavior still works for installed tools.
- [ ] Ensure existing published tool, agent, and template packages remain publishable and installable.
- [ ] Ensure existing template `dependencies` without `skills` remains valid.
- [ ] Ensure private namespace access applies consistently to private Skill publish/install/search/detail flows.
- [ ] Ensure registry/search/trending behavior for existing tools, agents, and templates is unchanged except for added Skill support.
- [ ] Audit Node SDK public exports to ensure Skill helpers/types are exported consistently.
- [ ] Audit Python SDK package exports to ensure Skill helpers/types are exported consistently.
- [ ] Run full CLI test suite.
- [ ] Run full backend test suite.
- [ ] Run full frontend test suite.
- [ ] Run full Node SDK test suite.
- [ ] Run full Python SDK test suite.

## Milestone 12: Examples Repo Update
> Scope note: this milestone updates `agentpm-examples` after the Skill docs are complete and the latest CLI, backend, frontend, and SDK behavior has been deployed. The goal is to verify real example flows against the deployed stack and leave the example repo aligned with the final public interfaces rather than local pre-deploy behavior.
- [ ] Add a new `skill-packages/*` example group in `agentpm-examples` for publishable Skill source packages.
- [ ] Create exactly four first-wave Skill packages that are strong enough to seed the public registry:
  - [ ] `incident-handoff-checklist` as a minimal procedural Skill with no tools.
  - [ ] `slack-status-update` as a tool-backed Skill using `@zack/slack-post-message`.
  - [ ] `issue-triage-playbook` as a tool-backed Skill using `@zack/github-issues`.
  - [ ] `research-brief-playbook` as a multi-tool Skill for the research stack.
- [ ] Make each Skill package look publishable and real:
  - [ ] valid `agent.json`
  - [ ] authored `SKILL.md`
  - [ ] useful top-level `README.md`
  - [ ] `references/` and `scripts/` only where they help the example rather than as filler
- [ ] Use `scripts/` intentionally rather than uniformly:
  - [ ] for procedural-only Skills, prefer no scripts unless one genuinely improves the example
  - [ ] for single-tool Skills, prefer a thin `scripts/run.sh`
  - [ ] for multi-tool Skills, allow `scripts/run.sh` plus a small number of thin named helper scripts when they clarify the workflow
- [ ] Keep the script boundary honest:
  - [ ] helper scripts should delegate to `agentpm run ...` rather than re-implementing tool logic
  - [ ] do not turn Skill `scripts/` into mini apps with heavy orchestration logic
  - [ ] preserve the repo story that the Skill holds procedure while AgentPM still owns execution
- [ ] Plan per-Skill script/reference shape explicitly:
  - [ ] `incident-handoff-checklist`: likely `SKILL.md` plus optional reference material, but no required `scripts/`
  - [ ] `slack-status-update`: include `scripts/run.sh` delegating to `agentpm run @zack/slack-post-message`
  - [ ] `issue-triage-playbook`: include `scripts/run.sh` delegating to `agentpm run @zack/github-issues`
  - [ ] `research-brief-playbook`: include either one canonical `scripts/run.sh` or a few thin named helper scripts if that better expresses the multi-tool workflow
- [ ] Use `slack-status-update` as the concrete export-based example:
  - [ ] start from `agentpm export --skill @zack/slack-post-message`
  - [ ] promote the generated scaffold into `skill-packages/slack-status-update`
  - [ ] edit it into a publishable Skill package rather than leaving it as a local generated walkthrough
- [ ] Decide the tool ownership boundary intentionally:
  - [ ] when a tool is part of a reusable procedure, move it under the Skill package that owns that procedure
  - [ ] avoid leaving the same tool duplicated at both the agent root and the Skill unless there is a deliberate reason
  - [ ] keep direct agent-level tools only when they remain truly agent-level capabilities outside the Skill story
- [ ] Update `agent-packages/ops-python` to depend on:
  - [ ] `incident-handoff-checklist`
  - [ ] `slack-status-update`
  - [ ] `issue-triage-playbook`
  - [ ] remove direct agent-level duplicates for tools now owned by those Skills
  - [ ] keep `csv-query` / `json-transform` agent-level unless a new Skill explicitly absorbs them
- [ ] Update `agent-packages/devwork-python` to depend on:
  - [ ] `issue-triage-playbook`
  - [ ] remove direct agent-level duplicates if that Skill now owns the same tool
- [ ] Update the research story to include `research-brief-playbook`:
  - [ ] either in `agent-packages/research-node` or, if the local app story is clearer, in the checked-in `agent-app-research-node` root manifest
  - [ ] ensure the example remains a clean local-app story rather than forcing it into the published-agent pattern unnecessarily
- [ ] Update the template packages that pin published agents so they follow the new Skill-aware agent versions:
  - [ ] bump `template-packages/triage-worker-python` to the new published `@zack/ops-console` version
  - [ ] bump `template-packages/support-assistant-workspace` to the new published `@zack/ops-console` version
  - [ ] update any generated template files, READMEs, tests, and hardcoded `AGENT_SPEC` strings that reference the old version
- [ ] Decide whether `template-packages/research-assistant-node` should also move to a Skill-aware story:
  - [ ] if yes, update its manifest, generated files, and README to reflect local `skills[]` plus `loadSkill(...)`
  - [ ] if no, leave it as the direct-tools local-app template intentionally and keep that distinction explicit
- [ ] Update `agent-app-ops-python` to become the canonical Python Skill-loading example:
  - [ ] keep `load_agent(...)`
  - [ ] add `resolvedSkills`
  - [ ] call `load_skill(...)` for each resolved Skill
  - [ ] use Skill manuals / metadata in the app wiring or prompt setup
  - [ ] flatten Skill `resolvedTools` into the final loaded tool set
  - [ ] keep the existing extra direct local tool path where it still adds value
- [ ] Update `agent-app-devwork-python` to show Skill reuse rather than introducing a separate parallel workflow:
  - [ ] keep `load_agent(...)`
  - [ ] load `issue-triage-playbook` through `resolvedSkills`
  - [ ] use `load_skill(...)` plus Skill `resolvedTools` before `load(...)`
  - [ ] preserve the LangGraph / approval-gated app story
- [ ] Update `agent-app-research-node` to show Node Skill usage without destroying its value as a local-app example:
  - [ ] keep the local root `agent.json` story
  - [ ] add local `skills[]`
  - [ ] use `loadSkill(...)` directly in the app
  - [ ] use Skill manuals and Skill `resolvedTools` in the app wiring
  - [ ] do not force this app into a `loadAgent(...)`-driven published-agent flow unless the example clearly improves
- [ ] Ensure the examples repo demonstrates both of the intended SDK stories:
  - [ ] Python: published agent package -> `load_agent(...)` -> `resolvedSkills` -> `load_skill(...)` -> `resolvedTools` -> `load(...)`
  - [ ] Node: local app root -> local `skills[]` -> `loadSkill(...)` -> `resolvedTools` -> `load(...)`
- [ ] Update the existing `skill-workflow-slack-post-message` example if it still adds value after `skill-packages/slack-status-update` exists:
  - [ ] either repoint it to the new publishable Skill package story
  - [ ] or narrow it explicitly to the “export a starter Skill scaffold” workflow and avoid overlapping too much with the publishable source package example
- [ ] Update `agentpm-examples/README.md` so the repo story now includes:
  - [ ] `skill-packages/*`
  - [ ] first-class Skill seeds
  - [ ] which agent apps demonstrate Skill loading
  - [ ] which agent packages depend on published Skills
- [ ] Update affected example READMEs so the documented setup, install, publish, and run flows stay truthful.
- [ ] Run `agentpm lint` in every changed Skill and Agent manifest root.
- [ ] Verify the example publish/install/load flows against the deployed stack:
  - [ ] publish the four Skill packages to the real registry
  - [ ] install the updated published agent packages and/or Skills in the example apps
  - [ ] verify the Python examples against the published agent + Skill dependency chain
  - [ ] verify the Node example against installed local Skill dependencies
- [ ] Run the relevant repo-native tests / checks for every changed example surface before closing the milestone.
