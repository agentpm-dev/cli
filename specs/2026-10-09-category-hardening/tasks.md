# Tasks

**Stage 2: Category-Enabling Hardening — Make the Agent Package Real**  
**Implementer:** Codex. **Independent reviewer:** Claude Code.  
**Authoritative requirements:** [`spec.md`](spec.md). **Verification:** [`test-plan.md`](test-plan.md). **Review:** [`review-checklist.md`](review-checklist.md).

> This document intentionally uses the **Stage 1 Immediate Hardening** task conventions: numbered **Release Bands**, numbered **Milestones** with A/B/C subtasks when a domain needs smaller slices, explicit **Scope notes**, implementation notes, checkboxes, and a band integration gate. It is intentionally more granular than the first Stage 2 draft. Each milestone tells an implementer what *changes*, what is *not included*, its dependencies, the tests/evidence, and the acceptance conditions.

## How to use this implementation plan

- **Read `spec.md` first.** It is fully self-contained; do not assume access to its planning chat. Especially read the detailed APDS examples and semantic cases before changing schema or validation.
- `REQUIRED / AGREED` is not optional. `PREFERRED` is our strong suggested approach but Codex may improve it with evidence. `INVESTIGATE / EXPERIMENT` requires a documented outcome even when the feature itself is deferred. Claude reviews all normal D01–D20 decisions; escalate only product-semantic or high-impact compatibility conflicts.
- Each `Milestone X[A/B/C]` should normally fit a focused PR or review unit. Split again if necessary rather than merging unrelated backend/UI migrations into a giant change. A **Release Band** is a coherent integration/deployment boundary; parallel development within a band is fine with explicit dependencies.
- **Stage 1 owns** search correctness/facets/ranking/stars/trending, namespace pins/scoped search, shared detail shell, technical SEO, CLI scaffold/lint/output hardening, analytics, Python portability, lockfile v4, multi-artifact publishing, digest/signature/attestation and CI. Review actual landed/in-flight Stage 1 work before any overlapping change; do not implement a competing version.
- **APDS rollout safety:** new-release Registry enforcement and strict CLI validation must ship in a coordinated order. Historical released bytes remain immutable; cutover handling must be decided and verified. A fixture corpus or standard with unresolved semantic contradictions is not ready to freeze.
- **Documentation hard gate:** comprehensive docs and AgentPM-owned README cleanup begins **only in M17A**, after all Stage 1 milestones—including Stage 1 documentation/migration—are done. Normative APDS specification files and small feature-local help may be authored earlier.
- **Visual references:** `assets/landing-structure.png`, `detail-structure.png`, `landing-layered.png`, `detail-layered.png` are layout/interaction inspiration. The **existing website** controls colors, typography, established kind icons and layered/floating aesthetic; image mockups contain fictional data and invalid illustrative JSON.
- **Milestone evidence:** Record exact changed repo paths/DTOs, tests actually run, CLI screenshots/terminal output, browser screenshots, backward-compatibility/migration notes, AC references, open D choices with option/rationale, Claude findings, and the merge/deployment order. Never check off tests not performed.
- **Naming reminder:** Agent Package means `kind: agent` complete authored system (valid even empty/no Loop). Six reusable Components are Tool, Skill, Knowledge, Memory Blueprint, Instruction Profile and Loop. Template is separate. AgentPM CLI = Agent Package Manager. Harness = built-in Agent Package Runner, not the definition standard.

## Milestone and release map

| Release band | What users/implementers get | Milestones |
|---|---|---|
| **1 — APDS definition and enforcement** | Versioned contract; Rust/Python conformance; Agent-first authoring; Registry new-release gate | M1A–M4B |
| **2 — Reproducible install and Runner support** | APDS-aware lock/install/frozen behavior; honest Harness + SDK preflight | M5A–M6B |
| **3 — Inspectable packages and objective Health** | Health evidence; immutable spec links; visual composition/Loop pages; specialized detail pages | M7A–M10B |
| **4 — Product/category website and discovery** | Data-driven featured content/cards; homepage; Explore; namespace; category/pricing/SEO copy | M11A–M14B |
| **5 — CLI/first-use adoption** | Category-first help; beginner example; try/build/template journeys; future Developer handoff | M15A–M16B |
| **6 — Final docs and overall review** | Full docs/READMEs, command verification and independent approval **after Stage 1** | M17A–M17C |

---

## Milestone 1A: Inventory the Existing AgentPM Manifest and Composition Contracts
> **Scope note:** Create a traceable inventory of what AgentPM already declares and interprets across the eight kinds before changing the standard. Outside this milestone: normative schema edits, new validation engine, Runner redesign.
> **Implementation notes:**
> - A normatively frozen version can only represent behavior we have inspected. Include the manifest source, Rust CLI, Python Registry, SDKs, published examples, and Harness interpretations. Treat implementation discrepancies as review findings, not automatic APDS requirements.
> - **Dependencies:** None; consult latest Stage 1 branch and Phase 6/7 source.
> - **Decisions:** D01, D02, D19. **Acceptance:** S2-AC-01, S2-AC-03.

**Implementation checklist**

- [ ] Inventory top-level schema: `$schema`, `$id`, `kind`, local `name`, version, description, and conditional kind requirements.
- [ ] Record the eight kinds and separate Agent Package (`agent`) from six Components and `template`.
- [ ] Map each kind to its actual manifest metadata fields and related CLI commands.
- [ ] Record how dependency declarations are represented (strings versus object selectors) and how Registry `@namespace/name` differs from local manifest `name`.
- [ ] Trace global and phase Agent bindings through actual phase lookup and effective capabilities.
- [ ] Trace Skill Tool inheritance through declared dependencies, bindings and Runner exposure.
- [ ] Trace Memory spaces and operation triggers including `capacity`, `record_count`, `interval`, and `external`.
- [ ] Trace Loop terminal outcomes, default omitted outcomes, and phase restrictions.
- [ ] List published legacy manifests and known common authored examples that will be used for regression.
- [ ] Make an explicit cross-repo contract table and attach file paths/line references to the PR.
- [ ] Have Claude independently review whether the inventory omits a meaningful authoring or execution behavior.

**Required edge cases and integration contracts**

- [ ] Inventory current artifacts and cross-repo contracts before editing: Rust CLI schema/lint/init/install/harness paths, Registry schema/publish/finalize/resolve, Python/Node SDK loads and Runner machine initialization, current `agentpm.manifest.schema.json` and Harness configuration schema. Capture exact schema `$id`, `$defs`, eight kinds, `oneOf` requirements, dependency forms, Loop, bindings, Memory operations, Template execution surfaces. Note fields on which current clients rely.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-03 evidence mapping and the decision record for D01, D02, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 1B: Create the Versioned APDS Schema and Standard README
> **Scope note:** Turn the existing `agentpm.manifest.schema.json` into the canonical immutable v1.0.0 structural contract, with a clear definition README and backward-compatible legacy schema path. Outside this milestone: new alternate manifest format, federation or moving `main` URI authority.
> **Implementation notes:**
> - The existing schema is authoritative starting material. The evolved versioned schema becomes normative; the old path is a derived/aliased compatibility entrypoint. `standard` is a structured object, not a single `apds` string.
> - **Dependencies:** M1A; coordinate any concurrent schema changes in Stage 1.
> - **Decisions:** D02, D19. **Acceptance:** S2-AC-01, S2-AC-03.

**Implementation checklist**

- [ ] Create `standards/agentpm/1.0.0/` (or justified equivalent) and make the intended files independently readable.
- [ ] Add `standard.id` and `standard.version` to the existing common schema with a fixed supported-v1 expectation.
- [ ] Ensure nonblank description rejects whitespace-only content, with equivalent Rust/Python behavior.
- [ ] Remove the incorrect unconditional Agent `tools` requirement while preserving kind-specific Tool requirements.
- [ ] Verify optional `loop`, `profile`/`profiles`, bindings, empty deps and omitted deps on minimal Agent manifests.
- [ ] Preserve the local `name` shape; do not rewrite it as `@namespace/name`.
- [ ] Preserve exact existing eight-kind vocabulary and avoid adding `AgentPackage` as a new machine kind.
- [ ] Give normative schema a version-stable `$id`, distinct from optional editor `$schema`.
- [ ] Select a reproducible build/vendoring mechanism so no two schemas are independently edited.
- [ ] Add a CI or build assertion detecting old-path versus canonical-schema drift.
- [ ] Write README: scope, actors, normative files, declaration, conformance levels, portability and non-goals.
- [ ] Write README standard immutability policy, including changes that require a new version.
- [ ] Demonstrate that offline validation doesn't fetch a mutable repository URL.
- [ ] Record selected path/build relationship and a structural before/after diff.

**Required edge cases and integration contracts**

- [ ] Produce an explicit before/after structural diff of the **existing** `schemas/agentpm.manifest.schema.json`: add `standard:{id,version}`, enforce nonempty trimmed description, remove `tools` as unconditional Agent requirement, retain actual name vs `@namespace/name` reference semantics, keep Loop/Profile/Tools optional for Agent; preserve other kind-specific constraints unless required to correct a conformance defect. **Do not create a greenfield schema.**
- [ ] Establish immutable versioned contract location, **prefer** `cli/standards/agentpm/1.0.0/`, with normative `README.md`, versioned `manifest.schema.json`, `semantics.md`, conformance README/fixtures and a stable semantic contract identifier. Document where the canonical source lives and how the existing schema path remains compatible without independently drifting.
- [ ] Fix normative JSON Schema `$id` to a version-stable URI and document `$schema` editor affordance separate from manifest `standard` identity. Ensure bundled offline validation never requires fetching GitHub's mutable `main` URL.
- [ ] Write `README.md`: what APDS is, scope/roles/portability, exact version/immutability/extension policy, authored package versus runtime, registry independence, eight kinds, terminology, links to schema/semantics/fixtures, how implementers declare support.
- [ ] Verify minimal `kind:agent` no-Tools/no-Loop/no-Profile common-fields manifest is structurally valid; **not** falsely Harness-runnable. Preserve all other kinds' valid field contracts. Decide if unknown properties are rejected according to existing schema, and write explicit extension policy.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-03 evidence mapping and the decision record for D02, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 1C: Write and Review Normative APDS Semantics
> **Scope note:** Specify the meaning of the already implemented artifact/composition system in `semantics.md`, with stable normative rules and examples. Outside this milestone: Do not create an executable semantics DSL or silently standardize a runtime defect.
> **Implementation notes:**
> - This is the most important APDS editorial milestone. Follow the detailed chapter blueprint and behavioral cases in `spec.md`. Every normative assertion is checked against actual sources and mapped to a fixture.
> - **Dependencies:** M1A/M1B; freeze with initial conformance fixture review from M2A.
> - **Decisions:** D01, D19. **Acceptance:** S2-AC-01, S2-AC-03.

**Implementation checklist**

- [ ] Define normative terminology, conformance subjects, validation levels and uncertainty states.
- [ ] Write identity/reference/version/composition rules and their applicability.
- [ ] Write Agent global/phase additive binding semantics using representative valid examples.
- [ ] Write Skill Tool inheritance for global and phase bindings; avoid mandatory duplicate direct Tool binding.
- [ ] Write Loop access policy as a constraint on bound/inherited capabilities.
- [ ] Write exact Loop entry/phase/outcome/transition/terminal/limit/checkpoint/error-policy semantics.
- [ ] Specify Profile composition order and advisory hints; no Profile granting forbidden tools.
- [ ] Write Knowledge context/vector and artifact reference semantics without requiring a storage backend.
- [ ] Write Memory spaces, direct surfaces, operation targets, triggers and global/phase participation; no automatically executed external trigger.
- [ ] Write Template scaffolding and `agentpm-harness` surface interpretation, distinct from runnable Agent.
- [ ] Specify how validators distinguish nonconformant versus incomplete resolved checks and unsupported standards.
- [ ] Separate authored semantics from Runner model/approval/hook/prompt implementation.
- [ ] Give each rule a stable ID, normative MUST/SHOULD/MAY, scope, location, expected diagnostic and fixture mapping.
- [ ] Create a concise rule index so future validation errors and Registry reference pages can deep link correctly.
- [ ] Verify every semantic example uses schema-accurate fields; do not copy mockup-generated `type` or `apds` fields.
- [ ] Produce a semantic mismatch log; escalate only contradictions that change agreed product behavior.
- [ ] Require focused Claude rule-by-rule signoff before calling APDS v1.0.0 frozen.

**Required edge cases and integration contracts**

- [ ] Write *detailed normative* `semantics.md` aligned to the APDS design and semantic chapter blueprint in `spec.md`. At minimum cover:
  - [ ] conformance levels and certainty (`conformant`/`nonconformant`/`incomplete`/`unsupported`) and independent validator vs Runner vs Registry roles;
  - [ ] identity, scoped/unscoped names, versions, dependency refs, kind constraints, graph resolution and transitivity;
  - [ ] **global + phase bindings additive**, phase-named access, versionless binding resolution to exact locked dependencies, binding availability vs Loop prohibition;
  - [ ] Skill-inherited Tool bindings in **global and phase** scope, with no redundant direct Tool binding;
  - [ ] Loop phase/entry/outcome/transition/terminal/limit/access/checkpoint semantics, including omitted-outcome `complete` behavior;
  - [ ] Profiles global-to-phase composition, identity/objective/communication/boundaries/compatibility hints, and advisory hints never granting capabilities;
  - [ ] Knowledge context/vector authoring and retrieval-interface meaning without mandating runtime storage/index;
  - [ ] Memory spaces, record types, operation triggers (`external`, `interval`, `record_count`, `capacity` where actual schema supports), global vs phase participation, operation targets beyond directly bound spaces, non-autonomous declarative behavior;
  - [ ] Template scaffold semantics versus Agent Package runtime and `agentpm-harness` execution surface when Stage 1 has landed;
  - [ ] explicit Legacy inference semantics, conformance vs security and compatibility vs environment-ready distinctions;
  - [ ] Runner boundaries: provider, approvals, hooks, prompt assembly, I/O, traces, runtime implementation are not APDS normative requirements.
- [ ] Give each **normative** rule a stable ID, MUST/SHOULD/MAY statement, applicability, rationale, diagnostic level/target, representative example and fixture reference. Treat sample IDs `APDS-ID-001`, `APDS-AGENT-001`, `APDS-BIND-001/002`, `APDS-SKILL-001`, `APDS-LOOP-001/002`, `APDS-PROFILE-001`, `APDS-MEM-001/002`, `APDS-TEMPLATE-001` in spec as suggested, not finished rules; reconcile with actual implementation. Do not simply paraphrase entire current docs into ambiguous prose.
- [ ] Audit all Phase 6/7 semantics against current authored manifests and Harness behavior; if contradiction is material, record it and request product decision **before** publishing normative v1.0.0. Do not standardize accidental bugs.
- [ ] Record **D01**, **D02**, **D19** decisions and evidence; include canonical schema synchronization check strategy and candidate semantic rule table. Request focused Claude APDS contract review before committing the version as immutable.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-03 evidence mapping and the decision record for D01, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 2A: Create a Shared, Executable APDS Conformance Fixture Corpus
> **Scope note:** Convert APDS structure and semantics into deterministic positive, negative, and unresolved-graph test cases. Outside this milestone: assumption that a standalone manifest can validate its entire graph; no fake retrospective verification.
> **Implementation notes:**
> - Both language implementations will read exactly the same fixture content and expected status/rule IDs. Each major semantic rule needs a proving positive and a rejecting negative case.
> - **Dependencies:** M1B/M1C normative draft.
> - **Decisions:** D03, D19. **Acceptance:** S2-AC-01, S2-AC-03, S2-AC-04.

**Implementation checklist**

- [ ] Define fixture index format with case ID, root manifest, optional resolved graph, validation level and expected results.
- [ ] Add one valid authored manifest for each of the eight kinds.
- [ ] Add the dependency-free valid Agent case with no `tools`, `loop` or Profiles.
- [ ] Add a multi-component valid Agent case with real schema-shaped references.
- [ ] Add paired global Skill inherited Tool and phase Skill inherited Tool cases.
- [ ] Add additive global+phase bindings with Loop allowing and denying tools in different phases.
- [ ] Add wrong phase and wrong-kind reference failures with stable rule IDs.
- [ ] Add Loop missing entry, unknown transition, invalid target and omitted-outcome valid cases.
- [ ] Add Profile hint versus Loop prohibition case.
- [ ] Add Memory operation target-not-directly-bound success plus trigger eligibility cases.
- [ ] Add Knowledge context/vector and Template scaffold positives/negatives.
- [ ] Add selector missing, unsupported, malformed and whitespace description failures.
- [ ] Add incomplete resolved-graph cases and clearly mark what could not be evaluated.
- [ ] Add legacy inferred declaration fixtures without upgrading historical bytes to explicit APDS.
- [ ] Pin fixture bytes/version for v1.0.0 and document controlled fixture updates.
- [ ] Review examples against actual parser before declaring case validity.

**Required edge cases and integration contracts**

- [ ] Design conformance harness/fixture manifest, **prefer** `conformance/fixtures/{valid,invalid,resolved}` plus expected-results index. Each test declares validation level, status and stable rule IDs (plus JSON paths and severity where meaningful). Retain exact bytes/checksums once APDS v1.0.0 freezes.
- [ ] Build positive fixtures for every kind plus minimal Agent, empty arrays/omitted dependencies, legitimate complex composition, Skill Tool-inheritance global/phase, additive scopes restricted by Loop, Loop implicit outcome, Knowledge modes, Profile hints, Memory Blueprint operation scopes/trigger semantics, Template scaffold, and a resolved graph with multiple exact dependency versions.
- [ ] Build negative fixtures for missing/unsupported standard, whitespace description, malformed field/name/reference, wrong-kind dependency, unknown bound Loop phase, invalid transitions/outcomes, invalid Memory spaces/operations, invalid Profile/Template/Tool structures, and direct/transitive integrity/identity mismatch where validation level permits.
- [ ] Include explicit **incomplete/not fully evaluated** fixtures when a valid standalone manifest lacks needed external dependency context; separate unsupported-standard from invalid structure. Include legacy inferred behavior fixtures but do not pretend those old bytes are explicitly conformant.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-03, S2-AC-04 evidence mapping and the decision record for D03, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 2B: Make Rust CLI Validate APDS v1.0.0
> **Scope note:** Implement independent Rust APDS schema/semantics validation with stable diagnostics and offline standard selection. Outside this milestone: Registry dependency and automatic fallback to whatever version is newest.
> **Implementation notes:**
> - Start from existing CLI lint infrastructure and reuse current validation shape. The new contract adds a version selector and graph-aware semantic checks; it does not require rewriting the entire CLI.
> - **Dependencies:** M1B/M1C, M2A fixture corpus.
> - **Decisions:** D03, D19. **Acceptance:** S2-AC-01, S2-AC-04.

**Implementation checklist**

- [ ] Map manifest `standard` into explicit supported-contract selection.
- [ ] Bundle authoritative v1.0.0 schema; validate offline with no GitHub fetch.
- [ ] Implement intrinsic checks and rule-ID diagnostics with affected JSON paths.
- [ ] Implement resolved-graph checks when dependency definitions are available.
- [ ] Return incomplete/not evaluated rather than false failure when graph context is absent.
- [ ] Return unsupported when declaration is unknown rather than silently using v1.
- [ ] Preserve human-readable CLI lint formatting and compatible structured output behavior.
- [ ] Run entire fixture corpus against Rust expected results, not selective handpicked cases.
- [ ] Test invocation in a disconnected local environment.
- [ ] Test deterministic diagnostic ordering and duplicate-rule behavior.
- [ ] Record scope of evaluation in output used by downstream CLI publish/inspect code.
- [ ] Document any parser behavior that must be fixed to preserve APDS optional dependencies.

**Required edge cases and integration contracts**

- [ ] Update Rust CLI validation engine to select bundled supported version, run schema and semantic checks with stable diagnostics, distinguish standalone and resolved validation, reject unknown selector rather than silently falling back.
- [ ] Create a simple contract capability matrix: which checks can run intrinsically, which require the resolved graph, which depend on runtime compatibility; report certainty and unresolved checks to callers without false success.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-04 evidence mapping and the decision record for D03, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 2C: Make Python Registry Independently Validate the Same APDS
> **Scope note:** Add a Registry-side APDS implementation that independently evaluates the same pinned fixture corpus and agrees with Rust. Outside this milestone: Do not shell out to Rust CLI or accept a client-authored `valid=true` assertion as verification.
> **Implementation notes:**
> - Stage 2 needs two implementations with one normative contract. Python JSON Schema library behavior/regex matching may diverge from Rust; explicitly test and reconcile instead of masking differences.
> - **Dependencies:** M2A and M2B expected outcome model.
> - **Decisions:** D03, D19. **Acceptance:** S2-AC-01, S2-AC-04.

**Implementation checklist**

- [ ] Choose existing Registry Python test/runtime conventions and pin the v1.0.0 schema bundle.
- [ ] Implement standard ID/version lookup with explicit unsupported result.
- [ ] Implement schema checks using compatible JSON Schema draft semantics.
- [ ] Implement semantic rules independently in Python, respecting intrinsic/resolved distinction.
- [ ] Produce stable rule IDs, paths and validation-level/result states.
- [ ] Run same fixture corpus against Python validator and compare case-by-case to Rust outcomes.
- [ ] Test regex, optional arrays and malformed/unknown property handling for parity.
- [ ] Add CI parity/checksum job or equivalent cross-repo fixture synchronization gate.
- [ ] Test scanner/signing/ACL failures remain separate from APDS results.
- [ ] Document how canonical fixtures reach both repos without relying on mutable production network.
- [ ] Have Claude exercise representative same-manifest Rust/Python results independently.
- [ ] Freeze v1.0.0 contract only after discrepancies and semantic mismatch log are resolved.

**Required edge cases and integration contracts**

- [ ] Implement independent Python Registry validation engine that uses the **same pinned standard files/fixtures** but independently evaluates schema/semantic rules. Confirm appropriate JSON Schema draft support and regex/semantic equivalence; avoid simply shelling out to the CLI or accepting client assertion.
- [ ] Integrate conformance fixture suite in both CI pipelines, with a single documented fixture update process; fail any change that causes divergence or a schema path/version mismatch. Include a hash/check that packaged canonical files match expected versioned bundle.
- [ ] Record **D03** and final **D19** shape, and have Claude independently exercise representative fixtures against both validators and the written semantics.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-04 evidence mapping and the decision record for D03, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 3A: Default CLI `init` to an APDS Agent Package
> **Scope note:** Change new authoring to start with a valid Agent Package and keep explicit scaffolds for seven other kinds. Outside this milestone: mandatory Loop/Tools/Profile; preserve Stage 1 Tool scaffold repair.
> **Implementation notes:**
> - This is an intentional early adopter compatibility change. Existing scripts that relied on `init` producing a Tool should select `--kind tool` explicitly; explain the migration in help.
> - **Dependencies:** M1B/M2B; Stage 1 M8 CLI authoring improvements.
> - **Decisions:** D04. **Acceptance:** S2-AC-02, S2-AC-03.

**Implementation checklist**

- [ ] Change clap default kind from Tool to Agent.
- [ ] Set default project/artifact name appropriate for an Agent Package (not `my-tool`).
- [ ] Write a useful nonblank description and APDS `standard` in default scaffold.
- [ ] Keep optional component and Loop fields absent or empty without violating APDS.
- [ ] Generate `standard` for Tool, Skill, Knowledge, Memory, Profile, Loop and Template explicit scaffolds.
- [ ] Preserve Stage 1's runtime-valid Tool Python/Node scaffold files and defaults.
- [ ] Use proper JSON encoding for quotes, slashes, Unicode and newlines in generated descriptions.
- [ ] Validate invalid local names before writing files; prevent partial/broken files on error.
- [ ] Ensure `init --kind agent` equals default behavior.
- [ ] Snapshot no-flags and all eight explicit kind scaffolds and lint them with v1.0.0.
- [ ] Check templates, docs snippets and help that previously assumed default Tool.
- [ ] Explain valid-without-Loop but not-yet-Harness-runnable next steps in immediate CLI output.

**Required edge cases and integration contracts**

- [ ] Change `agentpm init` default `--kind` to `agent` and default name to an Agent Package-appropriate value (not `my-tool`); generate a valid package with `kind`, name, version, meaningful description, supported `standard`; no forced Tools/Loop/Profile.
- [ ] Update all seven **explicit** non-Agent kinds to emit standard declaration and valid existing kind-specific scaffold, including Stage 1's repaired Python/Node Tool scaffolding. Verify each with lint/build where relevant.
- [ ] Serialize user-provided name/description/inputs using a safe JSON writer, never string interpolation. Validate malformed inputs and duplicates without emitting broken JSON or mutating preexisting files unexpectedly.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-02, S2-AC-03 evidence mapping and the decision record for D04 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 3B: Make CLI Lint and Authoring Preflight APDS-aware
> **Scope note:** Use the standard selector and diagnostic model in normal authoring commands with clear strict defaults. Outside this milestone: broad CLI messaging rewrite (Stage 1 M9) and premature full docs rewrite (Stage 2 final milestone).
> **Implementation notes:**
> - Local historical definitions may benefit from explicit migration mode, but strict new authored lint must never silently infer APDS and call it verified.
> - **Dependencies:** M2B, M3A, Stage 1 M8/M9 coordination.
> - **Decisions:** D04. **Acceptance:** S2-AC-02, S2-AC-03, S2-AC-04.

**Implementation checklist**

- [ ] Make normal `lint` validate explicitly declared supported standard.
- [ ] Distinguish missing standard, unknown version, schema defect and resolved-graph issue in output.
- [ ] Provide targeted remediation including version selector and source path.
- [ ] Preserve exit statuses, stderr/stdout/machine output guarantees from Stage 1.
- [ ] Evaluate need for optional opt-in legacy migration/lint mode and record D04 outcome.
- [ ] Prevent permissive migration mode from becoming default publish authorization.
- [ ] Ensure build/inspect/publish preflight reuse same canonical validation outcomes.
- [ ] Regression-check `agentpm new`, `agentpm run`, Tool exports and the existing publish link.
- [ ] Snapshot interactive and noninteractive diagnostics.
- [ ] Test whitespace descriptions, malformed names, unknown kinds and incorrect paths.
- [ ] Have Claude verify users don't confuse APDS validation with Runner readiness.

**Required edge cases and integration contracts**

- [ ] Integrate APDS selector/rule diagnostics into `agentpm lint` with actionable CLI messages, correct exit codes, and structured/machine output compatibility where present. Add a direct path/command for validating published/installed artifacts if existing inspect flow permits without inventing an unrelated command family.
- [ ] Handle unsupported/missing standard in newly authored files strictly. Investigate explicit opt-in local migration/legacy lint helper; **D04** decision must explain migration UX and why it does not make silent bypass the default.
- [ ] Preserve Tool execution `agentpm run`, Template `agentpm new` and `agentpm publish` existing behavior at this milestone; top-level category help/copy polishing is later M15.
- [ ] Tests: no-flags init, all eight explicit kinds, dangerous escape characters/Unicode, no-dependency Agent lint, old manifests, unknown standard, direct semantic rule failures; use Stage 1 CLI golden-output conventions.
- [ ] Document any authored artifact migration notes needed **now** to avoid developer dead ends; defer broad docs rewrite to M17A–M17C.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-02, S2-AC-03, S2-AC-04 evidence mapping and the decision record for D04 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 4A: Enforce APDS in Registry Publish Finalization
> **Scope note:** Make the Python Registry reject invalid newly uploaded releases by inspecting the staged artifact, independent of CLI validation. Outside this milestone: rewriting existing artifacts; no replacing Stage 1 release integrity or malware/ACL gates.
> **Implementation notes:**
> - Treat the Registry as the authoritative new-release gate even for custom HTTP clients. Preserve atomic finalize and accurate errors.
> - **Dependencies:** M2C; Stage 1 M13A/B release/finalize changes if deployed.
> - **Decisions:** D05, D19. **Acceptance:** S2-AC-05.

**Implementation checklist**

- [ ] Trace init/finalize/upload/scan flow and identify the last safe precommit enforcement point.
- [ ] Add version-scoped standard ID/origin metadata with backward-compatible migrations/serializers.
- [ ] Load actual embedded `agent.json` from staged artifact using existing secure tar validation.
- [ ] Run Python APDS structural checks and all resolvable semantic checks.
- [ ] Compare artifact local identity, Registry namespace mapping, kind and version against init metadata.
- [ ] Compare embedded declared standard against request metadata; reject mismatches.
- [ ] Ensure multi-artifact Tool publish applies correct release/artifact association checks.
- [ ] Keep malware scan, signer verification, Registry attestation, ACL and APDS statuses separate.
- [ ] Reject before discoverability and preserve atomic rollback/idempotent retry behavior.
- [ ] Add forged request tests that bypass CLI lint and still fail server.
- [ ] Add malformed/missing/unsupported schema and mismatched tar negative tests.
- [ ] Expose precise diagnostics and retain existing successful Registry package link.
- [ ] Prove unsigned/invalid signature cannot be bypassed merely by APDS conformance.

**Required edge cases and integration contracts**

- [ ] Inventory published version rows, version identity, init/finalize endpoints, staged tar/manifest bytes, scan/attestation and publish metadata. Record snapshot of legacy cases and queued/pending publish sessions.
- [ ] Add `standard` ID/version and explicit/inferred provenance to Registry schema/response DTOs where needed; version rollouts/backfills distinguish inferred from declared and never rewrite immutable tar, digest, signature, release version or malware evidence.
- [ ] Validate staged `agent.json` actual bytes, not merely init request payload; validate requested kind/name/version/standard match embedded manifest and resolved package/release metadata, including Stage 1 multi-artifact target release paths. Enforce **every new version**, including a release under legacy package identity.
- [ ] Apply independent Python APDS validator; reject structural/semantic violations prior to final commit/discoverability and without bypass via custom client or old CLI. Properly separate APDS errors from ACL/namespace, scan, integrity and signing failures.
- [ ] Decide reliable resolution context needed for composition validation: prove what server can validate at publish, report incomplete checks rather than claiming full conformance, and decide hard publishing requirements without blocking legitimate packages that reference unavailable/private graph context unintentionally. Preserve normal dependency-visibility/authorization behavior.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-05 evidence mapping and the decision record for D05, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 4B: Roll Out New-Publish Enforcement Without Breaking Legacy Releases
> **Scope note:** Define safe transition policy for legacy published bytes, new versions of old identities, and sessions in flight at cutover. Outside this milestone: grandfathering of new versions and false retroactive declaration.
> **Implementation notes:**
> - This milestone owns the compatibility switch: old immutable releases stay installable, but all new finalized versions need declared v1.0.0 or other explicitly supported APDS.
> - **Dependencies:** M4A, Stage 1 multi-artifact rollout status.
> - **Decisions:** D05, D19. **Acceptance:** S2-AC-05.

**Implementation checklist**

- [ ] Inventory real pre-APDS published versions and immutable digest/signature references.
- [ ] Define server-controlled enforcement boundary for each new finalized release.
- [ ] Choose and record in-flight session restart/grace policy; do not trust client timestamp.
- [ ] Make outdated CLI error actionable with minimum upgrade or migration instructions.
- [ ] Preserve already published manifest bytes and stored integrity values exactly.
- [ ] Represent effective legacy `agentpm/1.0.0` interpretation as **inferred**, not verified.
- [ ] Require explicit standard for new versions of historically legacy package identities.
- [ ] Test sessions created before deployment and finalized afterward.
- [ ] Test old released version install/lookup and its new APDS-compliant successor side by side.
- [ ] Exercise rollback on rejected finalize and ensure no phantom discoverable package exists.
- [ ] Add migration/verification script if backend metadata needs backfill; never re-sign old tar.
- [ ] Document deployment order of CLI, Registry and public UI to avoid temporary false success.
- [ ] Have Claude approve migration/security matrix before server enforcement is enabled.

**Required edge cases and integration contracts**

- [ ] Define and test **rollout cutoff and in-flight session policy** on the server. Prepare compatible CLI release + Registry accepted DTOs before the strict gate activates; explicit upgrades/errors for older publishing clients, failed finalize cleanup and safe rollback.
- [ ] Preserve old published versions as installable and their original bytes/signatures; report an effective legacy interpretation only with **legacy-inferred** status, never `verified` unless separately supported by evidence.
- [ ] Integration/security tests: direct forged init, request/manifest mismatch, altered staged tar, invalid selector/rule, mixed release artifact targets, private deps, pending session boundary, legacy old version, new version of legacy package, stable server error classification and atomic finalization.
- [ ] Produce an end-to-end publication compatibility/migration brief and D05/D19 decision record; require Claude review before enabling strict enforcement in production.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-05 evidence mapping and the decision record for D05, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Release Band 1: APDS Definition, Authoring and New-release Enforcement
Covered milestones: 1A–4B.

This gives AgentPM one pinned APDS v1.0.0 definition derived from its current schema, well-specified Phase 6/7 semantics, independent Rust/Python conformance, Agent Package-first new authoring, and a server-side new-release gate. Shipping the strict gate must be coordinated with the CLI and legacy session policy. An existing immutable release remains installable, but a new version cannot bypass APDS by using an older client.

- [ ] Confirm one canonical immutable schema, normative rules and shared fixture corpus, with offline parity across Rust and Python.
- [ ] Confirm all eight newly authored kinds lint, default init is Agent, explicit Tool scaffold remains valid.
- [ ] Run custom-client server bypass attempts and in-flight/legacy/new-version migration matrix.
- [ ] Obtain Claude review of APDS rule coverage, schema/version immutability, security and rollout order.

---

## Milestone 5A: Carry APDS Identity Through Resolve, Install and agent.lock
> **Scope note:** Make resolved and installed packages retain their effective APDS standard ID/version and the distinction between authored declaration and historical inference. Outside this milestone: Do not introduce host-target metadata into the portable lock or duplicate Stage 1 lock v4.
> **Implementation notes:**
> - The authoritative bytes are the installed artifact manifest; versioned Registry records, resolve DTOs and lock metadata are cached/derived views that must be reconciled rather than blindly trusted.
> - **Dependencies:** M4B; Stage 1 M11B lock v4, M14B release-digest contract.
> - **Decisions:** D06, D19. **Acceptance:** S2-AC-06.

**Implementation checklist**

- [ ] Read Stage 1's final `LockedPackage`, resolve plan and forward-version checks as implemented.
- [ ] Choose standard/origin lock shape while preferring lockfile version 4.
- [ ] Add explicit versus legacy inferred origin to Registry resolve responses.
- [ ] Map APDS metadata through CLI resolve DTO, in-memory plan and lock serialization.
- [ ] Retain new metadata through ordinary install lock regeneration.
- [ ] Reconcile the locked version/kind/name with the actual downloaded embedded manifest.
- [ ] Verify integrity using Stage 1's logical release digest rules, not new ad hoc checksum logic.
- [ ] Do not write chosen platform target or local Python interpreter into portable lock entries.
- [ ] Test v2/v3 reads, new v4 writes, future lock version rejection and unknown-field safety.
- [ ] Test multiple root kinds and exact version identity collisions.
- [ ] Document any justified v5 proposal before coding it and require Claude compatibility review.
- [ ] Check legacy inferred status is not promoted to declared/verified by serialization.

**Required edge cases and integration contracts**

- [ ] Read Stage 1's finalized `agent.lock` v4 contract and implement APDS standard+declared/inferred metadata without introducing an extra file format when possible. Verify no existing v4 reader silently drops it or rewrites future unknown fields.
- [ ] Carry effective standard and provenance through Registry resolve DTO, CLI resolved package plan, installed state and lock records; compare against actual embedded manifest + pinned release kind/name/version/digest.
- [ ] Validate unsupported standard and malformed/mismatched identity in install path with clear remediation; preserve verified release integrity and target selection from Stage 1.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-06 evidence mapping and the decision record for D06, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 5B: Fix Frozen Installs, Dependency Closure and Empty-Dependency Agents
> **Scope note:** Make a clean workspace reproducibly install a locked Agent/Skill/Component graph and gracefully handle empty dependency sets. Outside this milestone: broad resolver rearchitecture or redo of Stage 1 Python target-aware install.
> **Implementation notes:**
> - `--frozen` is meaningful only when it reproduces exact root and transitive closure from the lock. Current Tool assumptions in direct kinds and checksum/version satisfaction must be audited explicitly.
> - **Dependencies:** M5A; Stage 1 M15 target-aware installer.
> - **Decisions:** D06. **Acceptance:** S2-AC-07.

**Implementation checklist**

- [ ] Reproduce frozen direct Agent root, not just frozen Tool root.
- [ ] Reproduce frozen direct Skill root and its Tool dependencies.
- [ ] Reconstruct full transitive closure in an initially empty `.agentpm` workspace.
- [ ] Validate every locked dependency satisfies authored version range.
- [ ] Validate exact kind identity and selected version for every edge.
- [ ] Fail on missing lock entry rather than filling it from latest Registry version.
- [ ] Fail on Registry returned digest mismatch before modifying installed state.
- [ ] Install valid Agent with no deps without sending erroneous empty resolve request.
- [ ] Test normal install lock regeneration preserves every APDS origin/metadata field.
- [ ] Test duplicate same-named package references and multi-Agent/multi-version coexistence.
- [ ] Test legacy artifact install/read still succeeds with accurate inferred metadata.
- [ ] Test offline/frozen behavior separately from normal online resolve.
- [ ] Add integration tests for repeated clean installs producing same pinned closure.
- [ ] Document failures with actionable command/error output (not fabricated package version).

**Required edge cases and integration contracts**

- [ ] Fix `install --frozen` direct Agent and Skill cases that currently assume a Tool; verify exact kind and selected version for roots and dependencies and reconstruct **entire transitive closure** on a fresh workspace.
- [ ] Check authored semver ranges vs frozen locked versions and artifact integrity across Registry responses, downloaded tar/sidecars, lock and local installation; mismatch must fail without corrupting working state.
- [ ] Treat an Agent Package with no dependencies as a successful no-op dependency resolution/installation, including frozen and nonfrozen modes; do not submit an empty resolver list that fails.
- [ ] Regression test normal/frozen reinstall, clean workspace recreation, duplicated/transitive refs, kind-specific roots, legacy packages, mismatched digests, unsupported standard, v4 reader/writer roundtrip and older locks allowed by migration policy.
- [ ] Record **D06** decision if any format bump is even considered; must show why v4 cannot safely work and how all consumers migrate.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-07 evidence mapping and the decision record for D06 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 6A: Validate Harness Standard Support and Real Preflight Readiness
> **Scope note:** Teach Harness which APDS versions it understands and distinguish conformant definition, compatible Runner, and configured runtime. Outside this milestone: new Runner implementation or change to phase/approval engine design.
> **Implementation notes:**
> - An Agent Package with no Loop is valid APDS but the current Harness requires a Loop to run. Model/provider readiness must not be assumed when no terminal is available.
> - **Dependencies:** M5B and M2B APDS; CLI/Runner machine protocol current source.
> - **Decisions:** D20. **Acceptance:** S2-AC-08.

**Implementation checklist**

- [ ] Define supported standards capability list with exact version IDs.
- [ ] Check selected Agent and relevant resolved Component standards before run.
- [ ] Preserve legacy interpretation as qualified status.
- [ ] Return unsupported-standard result for unknown version, without fallback.
- [ ] Return valid-but-not-runnable reason for absent Loop.
- [ ] Keep readiness errors separate from conformance violations.
- [ ] Test missing model/provider in headless preflight returns not-ready.
- [ ] Retain interactive bootstrap prompt where a TTY is actually available.
- [ ] Expose same supported standards through existing machine initialization/preflight.
- [ ] Add clear CLI/TUI status text without claiming one Runner is mandatory for APDS.
- [ ] Test unsupported Memory/Knowledge backend as Runner limitation, not APDS invalidity.
- [ ] Prove no accidental change in loop stop/transition/approval behavior.

**Required edge cases and integration contracts**

- [ ] Define Harness supported-standard capabilities (`agentpm` v1.0.0) and integrate into current preflight/machine initialization/CLI/TUI/headless/SDK report surfaces, keeping Harness protocol versions distinct. Preserve existing APIs when possible.
- [ ] Use pinned Agent graph and standard provenance to validate selected Agent + relevant Components before execution; error explicitly on unsupported declared standards, structural conformance failures or unresolved composition.
- [ ] Display distinct readiness outcome: **APDS conformance** vs **Harness support** vs **local runtime ready**. APDS-valid without Loop is a **not runnable** explanation, not invalid APDS. Don't block external Runner use.
- [ ] Ensure headless/noninteractive preflight fails truthfully when required model/provider data is missing; prevent TUI bootstrap assumptions from leaking into machine mode.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-08 evidence mapping and the decision record for D20 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 6B: Fix Exact-version Harness Bindings and SDK Visibility
> **Scope note:** Resolve actual pinned Components in Harness and make standard support visible consistently to headless, TUI and SDK clients. Outside this milestone: rewrite of Node/Python SDK package models or new machine protocol.
> **Implementation notes:**
> - Versionless authored binding IDs must resolve **via the selected Agent graph**. Name-only workspace search can select a different compatible-looking version and violates reproducibility.
> - **Dependencies:** M6A; M5A/B locked identity; Stage 1 runtime changes.
> - **Decisions:** D13, D20. **Acceptance:** S2-AC-08, S2-AC-09.

**Implementation checklist**

- [ ] Trace current Harness Component lookup that only uses a name.
- [ ] Use selected Agent's resolved graph/lock to determine exact Component version and kind.
- [ ] Test workspace with two Agents binding the same named Tool at different versions.
- [ ] Verify Skill-inherited Tools are exposed only in valid global/phase scope and Loop policy.
- [ ] Verify Memory operation target semantics remain as in Phase 7A.
- [ ] Expose standard support list through Node SDK bridge/host APIs.
- [ ] Expose equivalent support list through Python SDK bridge/host APIs.
- [ ] Confirm TUI, headless and machine preflight outputs agree semantically.
- [ ] Set up A/B test fixture for optional Agent description in prompt context.
- [ ] Compare outputs, duplication, token footprint and traceability; record D13 decision.
- [ ] Keep existing prompt behavior if experiment doesn't show material benefit.
- [ ] Record D20 interface choice and Claude signoff on exact-version resolution.

**Required edge cases and integration contracts**

- [ ] Fix binding-to-installed-Component resolution where name-only lookup could select the wrong version in multi-Agent/multi-version workspaces. Use exact lock identity/version and test phase/global plus Skill-inherited Tool scopes and Loop access restrictions.
- [ ] Validate TUI, headless, machine initialization/controls, and Node/Python SDK parity for compatibility/status output, including back-compat for consumers of existing payload fields.
- [ ] **D13 experiment:** select a small repeatable loop-based Agent; compare prompts/outputs with and without injecting top-level Agent `description`; measure fidelity, duplication, token/cost effects, and phase output quality; record keep/defer/reject and tests for chosen behavior. Do **not** amend APDS semantics based solely on preference.
- [ ] **D20 decision:** describe why the selected capabilities advertisement belongs in existing Harness interfaces rather than a new standard protocol. Require Claude to review no Loop/provider model errors and version correctness.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-08, S2-AC-09 evidence mapping and the decision record for D13, D20 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Release Band 2: Pinned Install and Harness Runner Compatibility
Covered milestones: 5A–6B.

This gives users exact APDS metadata through resolve/install/lock v4, reproducible direct/transitive frozen installs, no-dependency Agent install behavior, and Harness capability/preflight reporting that differentiates a conformant Agent Package from a supported Runner and a ready local environment. It retains the existing Harness engine rather than introducing a new framework.

- [ ] Reproduce frozen multi-kind dependency closure from clean workspace; test wrong digest/range/kind and legacy versions.
- [ ] Verify minimal Agent is conformant but not Harness runnable without Loop.
- [ ] Verify supported versions through TUI/headless/machine/SDK and missing-model headless failure.
- [ ] Verify multi-version workspaces use exactly locked Component identity and Skill/Memory/Loop semantics.

---

## Milestone 7A: Inventory Universal Package Health Evidence
> **Scope note:** Define which release facts are actually verifiable and what each can legitimately claim. Outside this milestone: quality score, reviews, ranking changes or retrospective fabricated proof.
> **Implementation notes:**
> - Evidence can refer to package identity, selected version, release artifact, or local Runner state. The Health API/public page must never confuse those scopes.
> - **Dependencies:** APDS status from M2/M4; Stage 1 M12–M16 provenance and portability evidence.
> - **Decisions:** D12, D19. **Acceptance:** S2-AC-10.

**Implementation checklist**

- [ ] List all proposed universal signals with exact originating DB fields/services.
- [ ] Classify each as identity, release, artifact or local-environment scoped.
- [ ] Define declared standard separate from independently verified APDS conformance result.
- [ ] Define explicit APDS versus legacy inferred origin in Health terms.
- [ ] Define verified signature versus merely present author signature.
- [ ] Define Registry attestation versus unsupported/unverified attestation.
- [ ] Define malware scan status and unavailable/unknown scan distinction.
- [ ] Define logical release/artifact digest and integrity evidence from Stage 1.
- [ ] Define maintenance recency and deprecation only when actual data exists.
- [ ] Separate star/installs as identity-level popularity, not Health.
- [ ] Specify status vocabulary and evidence freshness/report link semantics.
- [ ] Write a signal-to-source matrix as a review artifact before building UI.
- [ ] Have Claude challenge every trust-related label for overstated claims.

**Required edge cases and integration contracts**

- [ ] Inventory authoritative source for every proposed Health signal: APDS declaration/conformance level, release/artifact digests, author signer verification, Registry attestation, scan, license, release date, dependency integrity/resolution, deprecation, Tool targets/platform/runtime/compatibility. Mark unavailable data plainly.
- [ ] Define evidence origin and freshness/verification semantics for each field; signatures can exist without independent verification; clean malware scan ≠ safe; declared platform compatibility ≠ local enforcement; APDS inference ≠ verified conformance.
- [ ] Separate identity-level stars/installs/publisher from selected-version integrity/conformance/release metadata; preserve Stage 1 metrics calculations rather than duplicating scores.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-10 evidence mapping and the decision record for D12, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 7B: Implement Kind-specific Health Extensions and Registry Responses
> **Scope note:** Expose a coherent Health evidence envelope with optional kind-specific facts, only where data is supported. Outside this milestone: OS compatibility badge on Profile/Loop/Template unless meaningful and sourced.
> **Implementation notes:**
> - Prefer one shared evidence representation with per-kind providers/adapters, not eight diverging APIs or eight copies of security math.
> - **Dependencies:** M7A; Stage 1 final release artifact model.
> - **Decisions:** D12, D17, D19. **Acceptance:** S2-AC-10.

**Implementation checklist**

- [ ] Implement universal signal aggregation in Registry with version scope.
- [ ] Add Tool target, runtime, dependency portability evidence only from real Stage 1 outputs.
- [ ] Add Agent graph/Runner support evidence without pretending server knows local readiness.
- [ ] Add Skill declared Tool/reference/script facts with no auto-execute claim.
- [ ] Add Knowledge mode/corpus/index evidence where metadata/hash is trustworthy.
- [ ] Add Memory spaces/record type/operation/trigger facts without backend-execution claim.
- [ ] Add Profile guidance/hints and Loop graph validity as appropriate advisory facts.
- [ ] Add Template scaffold stack/execution surface compatibility only when declared.
- [ ] Support `not applicable` and absent evidence explicitly, not green success defaults.
- [ ] Prevent leakage of private dependency metadata through public Health aggregation.
- [ ] Test one recent and one legacy release for every kind.
- [ ] Test forged report/status fields cannot override authoritative source.
- [ ] Test version switching changes release Health but not identity-level star state.
- [ ] Record D12/D17 representation decision and review evidence language.

**Required edge cases and integration contracts**

- [ ] Propose **universal per-version evidence envelope** plus **kind-specific extensions/adapters**; define status vocabulary verified/passing, failed, unavailable/unknown, incomplete, advisory, not-applicable. Avoid implying all kinds have Tool-style OS support.
- [ ] Build or extend Registry DTO(s)/server aggregation using existing services. Avoid inventing per-kind database tables or duplicating Stage 1 integrity computation without justification. Handle old releases, incomplete evidence and offline/failed retrieval safely.
- [ ] Test representative release of **each of eight kinds** (including Template): universal signal applicability, kind extensions, nonexistent data, ambiguous verification, adversarial forged signals, version switch, public/private visibility, no false safety claims.
- [ ] Record D12/D17/D19 selected shape and Claude review of evidence/wording before UI badges ship.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-10 evidence mapping and the decision record for D12, D17, D19 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 8A: Publish the Versioned APDS Website Reference
> **Scope note:** Give developers a stable explanation and exact authoritative source for APDS 1.0.0, linked from package pages. Outside this milestone: user-supplied runtime schema fetching and confusing it with a general category landing page.
> **Implementation notes:**
> - Preferred route `/standards/agentpm/1.0.0`; immutable schema and semantics are bundled/build-pinned. Deep links must lead to the exact version.
> - **Dependencies:** M1B/C frozen contract; Stage 1 M7 technical SEO foundation when available.
> - **Decisions:** D07. **Acceptance:** S2-AC-12, S2-AC-17.

**Implementation checklist**

- [ ] Choose trusted version catalog representation and deployment integration.
- [ ] Publish human-readable standard overview for `agentpm / 1.0.0`.
- [ ] Link to exact JSON Schema (canonical `$id`) and normative `semantics.md`.
- [ ] Link to fixture/conformance instructions and authoritative GitHub source.
- [ ] Make raw manifest `View agent.json` links separately point to selected artifact version.
- [ ] Make APDS declaration distinct from verification status in UI label.
- [ ] Use inferred legacy label where appropriate, not verified conformance.
- [ ] Test unknown standard version route yields honest not-supported response.
- [ ] Ensure no untrusted package URL is fetched for standards content.
- [ ] Coordinate canonical/robots/sitemap decisions with Stage 1 SEO.
- [ ] Test stable links, keyboard navigation, accessible code/link typography.
- [ ] Record D07 trusted-source/build-versus-runtime choice.

**Required edge cases and integration contracts**

- [ ] Add versioned standard reference route, prefer `/standards/agentpm/1.0.0`: human-readable intro, normative `semantics.md` and schema links, stable canonical source and version, supporting fixture docs. Use **trusted pinned** content; no arbitrary standard URL fetch from authors or moving `main` for immutable normative bytes.
- [ ] Expose separate links/components for declared standard vs conformance evidence, actual version-specific raw `agent.json`, and verified/legacy inference states. Ensure links work with real current detail routes and new/old releases.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-12, S2-AC-17 evidence mapping and the decision record for D07 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 8B: Create Reusable Component, Composition and Package Card UI Primitives
> **Scope note:** Build shared website building blocks for real Agent Package composition, kind labels, standard status and Package Health without rebranding the site. Outside this milestone: Do not copy mockup metrics, invented `apds` manifest fields or fake integrations.
> **Implementation notes:**
> - Reuse current AgentPM theme, gradients, elevation, rounded floating surfaces and kind icons. The first mockups teach hierarchy; second mockups demonstrate depth; production design tokens remain authoritative.
> - **Dependencies:** M7B Health, Stage 1 M6 shell, M8A standard references.
> - **Decisions:** D09. **Acceptance:** S2-AC-11, S2-AC-12, S2-AC-16.

**Implementation checklist**

- [ ] Inventory existing design tokens, reusable cards and Stage 1 shared detail shell.
- [ ] Document colors/elevation/typography to preserve in before/after reference captures.
- [ ] Build a compact Agent Package identity/summary card from real DTOs.
- [ ] Build reusable kind icon and category label primitives for six Components/Template.
- [ ] Build exact-version link and direct/transitive dependency chip primitives.
- [ ] Build truth-state badges that distinguish declaration, verification, inference and unknown.
- [ ] Build a graph-to-UI mapping for resolved Agent composition without relying on raw JSON on screen.
- [ ] Build Loop phase/transition/capability model that handles branches and cycles.
- [ ] Provide keyboard/screen-reader text fallback for large or interactive graphs.
- [ ] Support empty deps, missing resolved context, private dependencies and unsupported Loop.
- [ ] Test cards with long names, narrow viewport, theme support if existing site has it.
- [ ] Keep presentation architecture flexible across detail, featured, Explore and OG usage.
- [ ] Record D09 variant approach and why it fits existing React/Next patterns.

**Required edge cases and integration contracts**

- [ ] Audit current website tokens/layout components and Stage 1 detail shell/OG implementation; document which existing cards, icons, layered shadows, typography and gradients must be preserved. Shared primitives should make pages clearer, not flatten current look.
- [ ] Build reusable kind icon/label, version/tag/source, truth-state, concise composition-chip and responsive Agent Package identity/card primitives. Avoid coding mockup placeholder values into production.
- [ ] Implement a reusable graph-to-view model for direct/transitive resolved Components and Loop phase/scoped binding summaries, with honest missing/invalid/nonlinear cases and accessible text fallback. Prove readability in light/dark/mobile if current site supports both.
- [ ] Include links to standard/raw manifest, Health, and kind-specific detail surfaces in component design examples. Record D07/D09 implementation decisions and screenshot deviations from mockup with rationale.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-11, S2-AC-12, S2-AC-16 evidence mapping and the decision record for D09 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 9A: Replace Agent Overview JSON-first Layout with Composition
> **Scope note:** Make the selected Agent Package version understandable as a whole system before visitors need to inspect raw manifest JSON. Outside this milestone: loss of Bindings, Examples, Readme, Security or actual manifest link.
> **Implementation notes:**
> - The diagram is a representation of real authored/resolved relationships. Direct dependencies are not equivalent to Skill-inherited Tools; each link must resolve to an exact version where known.
> - **Dependencies:** M5B resolved graph, M8B shared components, Stage 1 M6 shared shell.
> - **Decisions:** D09. **Acceptance:** S2-AC-11.

**Implementation checklist**

- [ ] Define new Overview reading order: identity/purpose/actions, composition, execution, standard/Health, examples, raw source.
- [ ] Show exact kind/name/version for each direct declared Component.
- [ ] Show transitive Skill-declared Tools with inherited relationship and source Skill.
- [ ] Link to real Component detail routes for selected versions.
- [ ] Distinguish authored direct reference from resolution result when unavailable.
- [ ] Provide no-dependencies state that says valid Agent Package, not invalid/broken.
- [ ] Support graph fan-out, multiple Skills and repeated dependencies without double-counting.
- [ ] Handle packages with private dependencies using appropriate authorization/limited state.
- [ ] Keep actual `agent.json` accessible as a full source view.
- [ ] Maintain all existing tab links and author-owned README rendering.
- [ ] Build accessible text/table fallback for composition graph.
- [ ] Test long names, mobile overflow, unknown component, legacy version, and selection changes.
- [ ] Capture screenshots and ask Claude to inspect accuracy versus manifest/lock.

**Required edge cases and integration contracts**

- [ ] Restructure Agent Overview information hierarchy: purpose/publisher/version, direct/transitive Component graph, phase/Loop architecture, APDS standard/source, Health summary, Examples, secondary raw `agent.json`; retain existing Bindings/Examples/Readme/Security tabs.
- [ ] Render exact versioned links for direct Tools/Skills/Knowledge/Memory/Profiles/Loop and transitive Skill-inherited Tools; distinguish authored declarations vs resolved references where needed. Never rely only on name in multi-version graph.
- [ ] Test minimal no-Tools/no-Loop Agent; complex multi-Component Agent; missing/unresolved dependency; legacy package; private Component access; nonlinear Loop; out-of-date/unsupported standard; long names/mobile/keyboard.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-11 evidence mapping and the decision record for D09 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 9B: Show Actual Loop Architecture, Standard, Health and Run Actions
> **Scope note:** Expose how a compatible Agent Package is structured for execution and how to use it, while separating APDS conformance from Harness availability. Outside this milestone: fake linear phase flow and hidden model/credential prerequisites.
> **Implementation notes:**
> - The primary detail page should teach installed/runnable distinctions. Exact command syntax must be checked in the current CLI and SDK.
> - **Dependencies:** M6A/B Harness compatibility, M7B Health, M8A standard reference, M9A composition.
> - **Decisions:** D09. **Acceptance:** S2-AC-11, S2-AC-12.

**Implementation checklist**

- [ ] Show Loop presence and actual phase IDs/objectives/access policies when authored.
- [ ] Represent transitions and outcomes including branch/cycle/terminal targets correctly.
- [ ] Show global and phase bindings and relevant inherited Tools, without inventing permissions.
- [ ] Make Install primary available action with selected version.
- [ ] Show Run with Harness only when supported, or visible explanation when unavailable.
- [ ] Show Load via Node/Python SDK with actual current supported package/version syntax.
- [ ] Link to raw selected-version `agent.json` and exact versioned APDS specification.
- [ ] Place standard identity and conformance evidence as separate labels.
- [ ] Include selected-version Health summary with link to deeper evidence.
- [ ] Render Examples with actual authored prompts or honest empty state.
- [ ] Provide no-Loop Agent explanatory section, not a broken Run button.
- [ ] Test unknown standard, legacy inferred, backend unavailable and no-model-ready examples.
- [ ] Verify mobile/code copy/keyboard behavior and screen reader reading order.
- [ ] Claude reviews content truthfulness and compatibility messaging.

**Required edge cases and integration contracts**

- [ ] Depict effective global and per-phase bindings without inventing permissions, Memory operation execution, approvals or arbitrary linear transition order. Show nonlinear graph branches/terminal outcomes correctly or provide accessible fallback.
- [ ] Header actions: Install, conditional **Run with Harness** (available only when meaningful, otherwise explanatory) and Load via Node/Python SDK. Copy code matches actual CLI/SDK syntax, current version and package kind; avoid promising one-click execution without provider setup.
- [ ] Show standard id/version as fact, conformance as separately evidenced status; link **View agent.json** and **View APDS specification** to correct version; preserve security/provenance inspection and selected-version identity distinction.
- [ ] Preserve author-managed README and existing Examples detail; optional inline copyable starter prompt section uses authored examples only and correctly handles none.
- [ ] Capture before/after screenshots and Claude design/content review against current site and both mockup families (hierarchy yes, fictional facts and palette no).

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-11, S2-AC-12 evidence mapping and the decision record for D09 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 10A: Preserve All Specialized Detail Pages Under the New Hierarchy
> **Scope note:** Retain each Component/Template kind’s technical inspection features while consistently exposing standard, identity and version. Outside this milestone: Do not turn all eight kinds into generic Agent Package marketing views.
> **Implementation notes:**
> - Reuse common shell and source/standard/Health primitives but keep kind-specific data and actions. README prose belongs to authors.
> - **Dependencies:** M8B primitives, Stage 1 M6 detail shell.
> - **Decisions:** D17. **Acceptance:** S2-AC-11, S2-AC-12.

**Implementation checklist**

- [ ] Audit existing Tool runtime/input/output/execution detail against proposed shell.
- [ ] Preserve Skill entrypoint, scripts, references, compatible environments and Tool dependencies.
- [ ] Preserve Knowledge mode, corpus/docs, embedding/index/provenance, query inspection.
- [ ] Preserve Memory scope, record contracts, spaces, lifecycle and governance tabs.
- [ ] Preserve Instruction Profile identity/objectives/audience/communication/boundaries.
- [ ] Preserve Loop phase graph, outcomes/transitions/limits/error policy.
- [ ] Preserve Template use case, variables, stack, dependencies, entrypoints, bootstrap.
- [ ] Add category-correct breadcrumbs/labels without changing stable route IDs.
- [ ] Add standard manifest/spec links to every applicable kind.
- [ ] Keep author READMEs verbatim and technically relevant tab names accessible.
- [ ] Verify each kind on legacy, new and empty-content examples.
- [ ] Check shared shell behavior on responsive layouts and version switching.
- [ ] Claude flags any missing specialized inspection as a regression.

**Required edge cases and integration contracts**

- [ ] Preserve/augment Tool runtime and I/O, Skill entrypoints/reference/scripts, Knowledge mode/corpus/embedding and query flows, Memory Blueprint spaces/record/lifecycle/governance, Profile identity/communication/constraints, Loop graph/phases/outcomes/limits, Template variables/stack/dependencies/entrypoints/surfaces.
- [ ] Add declared APDS and exact `agent.json`/spec links on all kinds without calling every kind an Agent Package; align breadcrumbs, shell, identity/version facts and signing-policy wording.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-11, S2-AC-12 evidence mapping and the decision record for D17 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 10B: Render Honest Package Health and Security Across All Eight Kinds
> **Scope note:** Present universal and kind-specific objective evidence with correct applicability, source and version scope. Outside this milestone: universal quality score or all-green default on missing data.
> **Implementation notes:**
> - Preferred short version summary near identity plus detailed Health/Security view. Codex may choose tab structure after reviewing existing Security; never remove SHA/signature/scan drill-down.
> - **Dependencies:** M7A/B Health data, M10A preserved specialized shell, Stage 1 M6 security panels.
> - **Decisions:** D12, D17. **Acceptance:** S2-AC-10, S2-AC-12.

**Implementation checklist**

- [ ] Implement concise release-scoped Health summary with clear status labels.
- [ ] Implement detailed evidence inspection and source/freshness explanation.
- [ ] Distinguish signed, verified signature, Registry attestation and scanned facts.
- [ ] Distinguish declared versus inferred APDS, and validation level completeness.
- [ ] Render not-applicable state for irrelevant kind-specific signals.
- [ ] Render unknown/unavailable/not evaluated without green checks.
- [ ] Display Tool target/platform only from available release artifact evidence.
- [ ] Display Agent Runner compatibility without claiming visitor local readiness.
- [ ] Preserve existing Security SHA-256/license/signature/attestation/scan data.
- [ ] Verify identity-level stars/install counts do not change with version selector.
- [ ] Test private/missing/failing evidence, old version and each of eight kinds.
- [ ] Review warnings and UI copy for certification/safety overclaims.
- [ ] Capture screenshots and Claude audit signal-by-signal against backend evidence.

**Required edge cases and integration contracts**

- [ ] Implement concise per-version Health summaries and deeper inspection in dedicated Health tab or reworked Security section; choose tab structure via D12, preserving existing Security evidence (signatures, attestations, scans, SHA/license).
- [ ] Render semantic empty/unknown/not-applicable/advisory states appropriately and accessible, never universal green success check by default; provide source/freshness and version switching.
- [ ] Test all eight kinds, each with health evidence present and absent, legacy inferred version, withheld private evidence, accurate compatibility/advisory copy, regression of specialized tabs/actions, responsive behavior.
- [ ] Record D12/D17 tradeoffs and Claude review for consistent architecture and non-misleading health.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-10, S2-AC-12 evidence mapping and the decision record for D12, D17 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Release Band 3: Inspectability, Versioned Standard Reference and Package Health
Covered milestones: 7A–10B.

This gives each published Agent Package an intelligible composition and execution Overview, deep access to its immutable manifest and corresponding versioned APDS standard, version-scoped objective Package Health, and preserved specialized detail views for all eight kinds. A Health display is evidence-backed rather than a quality score or blanket safety claim.

- [ ] Verify full standard/raw manifest links and direct/transitive Component links to exact versions.
- [ ] Verify Loop phase scopes/branches/cycles are rendered without fabricated execution behavior.
- [ ] Audit universal and kind-specific Health signals for every kind, status, source, visibility and version scope.
- [ ] Have Claude inspect all eight kind pages and current-design-token fidelity versus mockups.

---

## Milestone 11A: Implement Data-driven Featured Package Selection
> **Scope note:** Provide one configurable source for choosing and ordering real featured artifacts on homepage/Explore without changing presentation JSX. Outside this milestone: CMS, AI recommendation engine, invented metrics, or final featured example selection.
> **Implementation notes:**
> - Prefer lightweight config or Registry-backed editorial records. Namespace pins already exist under Stage 1, but they are publisher-local and may not be the right contract for global placement.
> - **Dependencies:** Stage 1 M4/M5 popularity/pins; new shared cards M8B.
> - **Decisions:** D10. **Acceptance:** S2-AC-15.

**Implementation checklist**

- [ ] Inspect Stage 1 namespace pins and existing featured/trending data sources.
- [ ] Compare static checked-in config and small Registry-backed curation; record D10.
- [ ] Define placements (hero, Explore featured, other optional areas) and explicit ordering.
- [ ] Represent package kind/identity and version policy without hardcoding IDs in components.
- [ ] Add server authorization/visibility filtering for private/unpublished/yanked/deleted items.
- [ ] Avoid leaking even the title of an inaccessible curated artifact to anonymous users.
- [ ] Handle missing selections, no eligible items and partial lists gracefully.
- [ ] Keep Featured visually/semantically separate from Stage 1 measured Trending.
- [ ] Resolve correct version and real metrics for selected public releases.
- [ ] Seed data-driven test fixtures without deciding permanent highlighted packages.
- [ ] Test stable ordering after suppressed/ineligible items.
- [ ] Test internal publisher edit workflow or config procedure without frontend changes.
- [ ] Document D10 selected source and how curator updates it safely.

**Required edge cases and integration contracts**

- [ ] Select lightweight feature/curation source (**prefer** data configuration or Registry-backed curation reusing Stage 1), define fields for ordered placement(s), kind, artifact identity, selected version/latest behavior and optional editorial label. Do not embed fixed IDs inside components or invent a CMS.
- [ ] Implement server-validated eligibility and visibility/authorization; gracefully skip unavailable, private, unpublished, deleted, unindexed or malformed entries. A bad featured entry must not crash homepage/Explore or leak private metadata.
- [ ] Support changing featured package selection and order **without editing presentation JSX**; test empty, partial, full and stale lists. Explicitly distinguish **Featured (curated)** from **Trending (Stage 1 measured)** and from namespace pins.
- [ ] Produce data fixture(s) for later golden starter examples, but **do not require choosing actual featured packages now**.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-15 evidence mapping and the decision record for D10 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 11B: Build Reusable Agent Package Cards and Investigate README Embedding
> **Scope note:** Create grounded card variants and produce an explicit technical decision/prototype for Markdown-embeddable GitHub README Agent Package Cards. Outside this milestone: Full dynamic public image service is not automatically authorized by the investigation.
> **Implementation notes:**
> - Keep homepage, Explore, social preview and detail visual identity consistent but allow different information density. A card is not an A2A Agent Card protocol document.
> - **Dependencies:** M8B shared primitives; M11A featured data; Stage 1 M7 OG infrastructure.
> - **Decisions:** D09, D16. **Acceptance:** S2-AC-16.

**Implementation checklist**

- [ ] Define compact listing versus featured/hero Agent Package card variant requirements.
- [ ] Show real purpose, selected version, six Component categories where present and install action.
- [ ] Avoid implying everything is universally runnable or explicitly APDS verified.
- [ ] Reuse Stage 1 social/OG preview generation where practical.
- [ ] Prototype Markdown image/link embedding with correct Registry destination.
- [ ] Compare static snapshot vs dynamically rendered card, cache/invalidations and abuse limits.
- [ ] Compare pinned package version versus latest-version image behavior.
- [ ] Decide which trust/Health signals can safely appear on public embeddable images.
- [ ] Check private namespace access and accidental exposure through image caching or card URLs.
- [ ] Check GitHub README image rendering and reasonable image dimensions.
- [ ] Record D16 analysis, prototype screenshot, decision to implement or defer, and rough scope.
- [ ] Keep any deferred actual embedding implementation tracked as later work.
- [ ] Coordinate output with future AgentPM Developer sharing handoff.

**Required edge cases and integration contracts**

- [ ] Build Agent Package Card variants as appropriate for homepage hero, featured listing, Explore card and OG/share preview; show actual identity/purpose/composition/release standard, with controlled truncation/responsiveness and absent metadata fallbacks. Preserve visual consistency with current site rather than following synthetic mockup colors.
- [ ] **Mandatory D16 investigation/prototype:** GitHub README Markdown embed: image URL/static vs dynamic card, pinned/latest semantics, change invalidation, cache, auth/private redaction, proper alt/accessibility, source provenance, share link, costs/abuse, OG reuse. Write decision **implement v1 now** or **defer with a concrete later task/brief**, and ask Claude to review tradeoff. Do not silently label full embedding “done” based on OG alone.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-16 evidence mapping and the decision record for D09, D16 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 12A: Redesign Homepage Hero Around the Complete Agent Package
> **Scope note:** Replace Tool/Template-first hero hierarchy with developer problem → AgentPM Manager → real composed Agent Package. Outside this milestone: Not a total color/brand redesign; not a standards-body homepage.
> **Implementation notes:**
> - Preserve current layered/floating visual language and colors. Generated mockups illustrate structure but may include fictional names, counts, false verification and other inaccurate strings.
> - **Dependencies:** M11A/B curation/card, M6 supported run path, Stage 1 M7 SEO primitives.
> - **Decisions:** D08. **Acceptance:** S2-AC-13.

**Implementation checklist**

- [ ] Inventory current `website/src/app/page.tsx`, Hero, Trending, Templates and codebox sections.
- [ ] Identify which existing sections should move to Explore or be compacted.
- [ ] Write hero copy centered on reusable complete Agent systems and AgentPM as Manager.
- [ ] Explicitly distinguish Agent Package, Components, APDS and built-in Harness Runner.
- [ ] Render a real curated Agent Package hero card with exact version and composition.
- [ ] Link through to current detail route and use real install/Harness command where applicable.
- [ ] Show sensible fallback if no eligible featured Agent Package exists.
- [ ] Preserve install-CLI affordance and honest SDK/framework portability statement.
- [ ] Present Templates as a separate scaffold route rather than the category primitive.
- [ ] Ensure main CTA leads to populated Agent Package discovery and next actions.
- [ ] Avoid hero stat panels emphasizing zero recent installs or fabricated metrics.
- [ ] Capture before/after desktop views and category comprehension check.

**Required edge cases and integration contracts**

- [ ] Audit existing `website/src/app/page.tsx`, Hero, Trending, Templates and kind-specific showcase components and current design tokens. Draft content hierarchy and before/after section map; retire rather than replicate redundant stacked component-specific trending blocks.
- [ ] Rewrite hero around developer problem and missing reusable artifact, with explicit **AgentPM — Agent Package Manager**, meaningful primary CTA, simple Agent Package example and correct Install/Run command hierarchy; copy avoids overselling ecosystem adoption.
- [ ] Present a real, data-driven featured Agent Package composition card with versioned link, component summary, standard, installation/Harness snippet when applicable and graceful no-featured fallback; never display fictitious numbers or incorrect package facts.
- [ ] Include concise product/standard/Registry/Harness reference Runner model, reusable Component/Template pathways secondary to Agent Package, reduced low-volume metric emphasis and three onboarding paths.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-13 evidence mapping and the decision record for D08 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 12B: Complete Responsive Homepage Story, Component Paths and Starter Paths
> **Scope note:** Make the full homepage a coherent category-first product journey that works on desktop/mobile and connects to real destinations. Outside this milestone: repetitive six-kind feature inventory and fictional ready-to-run promise.
> **Implementation notes:**
> - Broader discovery belongs to Explore. Homepage should be deliberate and comparatively concise, with Components as secondary reusable parts and Templates clearly distinct.
> - **Dependencies:** M12A hero; M11A featured; M15B starter later—use safe interim fallback.
> - **Decisions:** D08. **Acceptance:** S2-AC-13, S2-AC-19.

**Implementation checklist**

- [ ] Build compact visual explanation of Agent Package and how Components compose into it.
- [ ] Explain Manager/Registry/APDS/Harness roles without implying all Runners support APDS now.
- [ ] Add small featured Agent Package row with accurate data-driven cards.
- [ ] Add compact navigation for all six Component kinds with one-line meaning.
- [ ] Add distinct Template pathway with real browsing link.
- [ ] Add three onboarding paths (Try, Build, Template) and current real commands/routes.
- [ ] Reuse kind icons and current brand palette/elevation tokens.
- [ ] Check meaningful responsive layout and code-box overflow on small screens.
- [ ] Test keyboard focus, readable diagram alt/text fallback and contrast.
- [ ] Test anonymous, authenticated and absent-featured-data rendering.
- [ ] Test link destinations and query state without introducing dead routes.
- [ ] Verify homepage copy against actual standard version and available Runner support.
- [ ] Capture desktop/tablet/mobile screenshots; Claude review layout and category meaning.

**Required edge cases and integration contracts**

- [ ] Preserve existing AgentPM brand palette, typography, floating panels/gradients and responsive UX; borrow diagrams/information density from `assets/*-structure.png` and layered feel from `assets/*-layered.png` without copying their styling or invented integrations.
- [ ] Wire real Explore/Agent/package-doc destinations; confirm no disabled signup/navigation regression or reliance on a currently nonexistent AgentPM Developer Agent.
- [ ] Verify responsive/mobile, keyboard, accessibility, signed-in/out, no featured data and realistic long package descriptions; create side-by-side screenshots and initial developer comprehension walkthrough.
- [ ] Record D08 final design decisions; Claude reviews category clarity, copy accuracy, mockup consistency and real-data behavior.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-13, S2-AC-19 evidence mapping and the decision record for D08 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 13A: Reorganize Explore and Enrich Every Kind of Search Card
> **Scope note:** Make Explore a category-aware discovery experience while leaving Stage 1 search correctness and deterministic ranking intact. Outside this milestone: new relevance algorithm, recursive popularity scoring or AI search.
> **Implementation notes:**
> - No-query `/explore` is curated discovery. A text query remains Stage 1 result ranking. Grouping kinds in UI must not alter persisted machine kind or break URLs/cursors.
> - **Dependencies:** Stage 1 M1–M4 discovery; M11A featured; M8B cards.
> - **Decisions:** D11. **Acceptance:** S2-AC-14, S2-AC-15.

**Implementation checklist**

- [ ] Group Kind facet into Agent Packages, six Components, Templates and Namespaces.
- [ ] Preserve each underlying kind/filter value and URL query parameter interpretation.
- [ ] Show accurate grouped result counts and labels for selected filters.
- [ ] Build no-query Explore featured, measured Trending, Component and Template sections.
- [ ] Keep curated Featured separate from measured Trending and publisher pins.
- [ ] Enrich Agent Package result card with real composition/Loop summary.
- [ ] Enrich Tool/Skill/Knowledge/Memory/Profile/Loop/Template cards with kind-specific metadata.
- [ ] Use absent/unknown field fallbacks instead of invented attributes.
- [ ] Preserve relevance sorting for text searches; do not force Agent-first ranking.
- [ ] Preserve Stage 1 pagination/cursor/history/filter URL state.
- [ ] Improve context-specific no-results CTA without hiding clear filters.
- [ ] Test public/private visibility, no-query, query, kinds, empty/error/loading and long names.
- [ ] Test tiny catalogs and a mix of exact and fuzzy matches.
- [ ] Claude independently compares results/order with Stage 1 known-good behavior.

**Required edge cases and integration contracts**

- [ ] Group kind filters/facets/UI counts as Agent Packages / Components (six kinds) / Templates / Namespaces while preserving actual backend kind constants, existing deep link query params, URL history and search semantics.
- [ ] Make no-query `/explore` a useful data-driven discovery surface with Featured (curated) and Trending (measured) sections, Component and Template pathways; handle no curated data with safe fallback. Query-bearing Explore must keep Stage 1's deterministic relevance, pagination, facet combination and visibility.
- [ ] Expand kind-specific result summaries using existing trustworthy metadata: Agent composition; Tool runtime/target; Skill capabilities/dependencies; Knowledge mode/corpus; Memory spaces/retrieval; Profile identity; Loop phase/archetype; Template stack/use case/surfaces; anonymous users should see only public data.
- [ ] Align package-detail breadcrumbs, search labels/status, catalog taxonomy, loading/no-result/error states, filters and footer copy with new category. Stage 2 may add copyable “build something like this” idea prompts, but only active AgentPM Developer destinations when Agent is actually available.
- [ ] Regression suite for no-query Explore; text relevancy and stable sort with/without filters; public/private mixed; missing curated items; namespace-specific card/pin result order; back/forward and cursor navigation; long labels and responsive layout.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-14, S2-AC-15 evidence mapping and the decision record for D11 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 13B: Update Namespace Publisher Catalogs to Match Category Hierarchy
> **Scope note:** Organize each publisher page as a clear catalog of Agent Packages, Components and Templates while preserving Stage 1 namespace features. Outside this milestone: new namespace search implementation and Agent-only pinned policy.
> **Implementation notes:**
> - Namespace Owner/Admin pins can highlight any supported artifact kind. Namespace-scoped searches share the same service as Explore, with hard server constraint.
> - **Dependencies:** Stage 1 M5 scoped discovery/pins; M13A category cards.
> - **Decisions:** D11. **Acceptance:** S2-AC-14.

**Implementation checklist**

- [ ] Show Agent Packages and Components/Templates as understandable catalog groups.
- [ ] Preserve Stage 1 scoped search, facets, sort, URL state and pagination.
- [ ] Preserve Owner/Admin pin management for any eligible kind.
- [ ] Render pins with shared enriched card primitives where practical.
- [ ] Render accurate type summary/counts rather than generated peer-kind word salad.
- [ ] Ensure recent-activity entries lead to useful artifact/version destinations.
- [ ] Use correct signing/attestation wording for namespace security policy.
- [ ] Keep publisher identity/permission controls unchanged.
- [ ] Test no pins, some pins, invalid/yanked pins and mixed visibility.
- [ ] Test anonymous pages versus Owner/Admin management states.
- [ ] Compare equivalent namespace query results to filtered Explore for semantic consistency.
- [ ] Claude checks there is no private data leakage or type-priority distortion.

**Required edge cases and integration contracts**

- [ ] Adjust namespace pages to showcase Agent Packages, Components and Templates while respecting owner/admin pinned **any-kind** selections and Stage 1 scoped search, sorting, pagination and namespace-signing semantics. Improve Recent Activity click-through only if trivial and correctly grounded.
- [ ] Record D11 and any DTO change against Stage 1 shared result API; Claude checks no arbitrary Agent sort boost and no private data leakage.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-14 evidence mapping and the decision record for D11 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 14A: Publish a Deeper Agent Package Management Explainer and Shared Terminology
> **Scope note:** Give the category its own focused explanation while making all shared website chrome identify AgentPM and artifact kinds correctly. Outside this milestone: misleading industry-standard claim and generic global rename of machine fields.
> **Implementation notes:**
> - Prefer `/agent-package-management` for why/the mental model and `/standards/agentpm/1.0.0` for normative technical rules. The homepage remains AgentPM-first.
> - **Dependencies:** M12 homepage, M8A normative APDS reference; Stage 1 M7 SEO.
> - **Decisions:** D18. **Acceptance:** S2-AC-17.

**Implementation checklist**

- [ ] Write product-focused explanation of developer reuse problem and common alternatives.
- [ ] Define Agent Package versus six Component kinds versus Template.
- [ ] Explain APDS semantics, Manager, Registry and compatible Runner without overpromising support.
- [ ] Explain why this differs from copying Git repos or packaging individual Tools.
- [ ] Link to real install/run/build flows and versioned APDS technical page.
- [ ] Audit top nav, footer, breadcrumbs, category naming and search prompt language.
- [ ] Replace product-controlled Agents labels with Agent Packages when referring to artifacts.
- [ ] Retain Agent where referring to acting runtime behavior or machine `kind: agent`.
- [ ] Avoid inflating unverified claims such as `works in every framework`.
- [ ] Check meta title, H1/H2 and internal-link labels for category terms.
- [ ] Capture anonymous reader comprehension feedback and necessary copy changes.
- [ ] Record D18 route/design/claim rationale and Claude editorial review.

**Required edge cases and integration contracts**

- [ ] Add a concise deeper category explainer, **prefer** `/agent-package-management`, linking to AgentPM Manager/Registry/Harness product actions, APDS normative route, portable Agent composition and the main competitor/status-quo problem; AgentPM remains hero product, not generic standards site.
- [ ] Audit shared navigation, mobile chrome, breadcrumbs, footer, pricing, metadata titles/descriptions, H1/H2, internal links, OG previews, and social snippets for Agent Package vs Component vs Agent acting-entity usage. Avoid blind global renames and route breaks.
- [ ] Review security/legal honesty of “standard”, “open”, “framework-independent”, “works everywhere”, “verified”, runtime provider and category adoption claims; no unproven universal compatibility or fake third-party endorsement.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-17 evidence mapping and the decision record for D18 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 14B: Align Pricing Copy, SEO Semantics and Shared Navigation
> **Scope note:** Carry consistent product/category terminology into pricing, social/search metadata and reusable site navigation without changing the billing model. Outside this milestone: checkout or plan pricing changes; no new SEO engine.
> **Implementation notes:**
> - Keep existing Free/Pro/Team behavior. Fix only copy and public access explanation; preserve Stage 1 canonical/sitemap/robots behavior.
> - **Dependencies:** M14A category story; Stage 1 M7 SEO and M10 analytics.
> - **Decisions:** D18. **Acceptance:** S2-AC-17.

**Implementation checklist**

- [ ] State public Agent Package and Component publishing/installation is free according to existing plan rules.
- [ ] Describe paid private namespaces and shared internal Team work accurately.
- [ ] Do not claim dedicated Registry hosting is included in Team if not true.
- [ ] Add clear anonymous Explore entry where appropriate.
- [ ] Audit homepage, explore, category page, namespace, package details and pricing metadata.
- [ ] Use unique descriptive titles/H1/H2 and category-related anchor text.
- [ ] Check canonical behavior for new routes and parameterized Explore URLs.
- [ ] Validate OG/social images have truthful public selected-version data.
- [ ] Keep new route indexing intentional; no crawl explosion from filters.
- [ ] Review claim language for signing, scans, conformity and Runner support.
- [ ] Test pricing navigation and existing checkout/signup regressions.
- [ ] Capture SEO/meta screenshots/test results and Claude copy review.

**Required edge cases and integration contracts**

- [ ] Pricing pass: Free/public packages and Components, private namespaces on paid plans, Team as shared internal publisher workspace, accurate access for public Explore. **No plan/checkout changes.**
- [ ] Reuse Stage 1 SSR SEO, canonical, sitemap and robots implementation; ensure new standard/category routes are intentionally crawlable and arbitrary Explore query/filter states aren't accidentally indexed; preserve old package detail canonical/OG paths.
- [ ] Test links/canonicals/share previews, anonymous browsing, SEO snapshots, pricing consistency, page navigation and comprehension; record D18 copy/IA rationale.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-17 evidence mapping and the decision record for D18 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Release Band 4: Category-first Website, Curated Discovery and Sharing Foundation
Covered milestones: 11A–14B.

This gives visitors a product-first homepage centered on complete Agent Packages, an intentional no-query Explore, richer cards and namespace catalog hierarchy, a deeper category page, accurate pricing/SEO meaning, and lightweight data-driven featured package selection. It also includes an explicit investigation into GitHub README-embeddable cards without requiring a CMS or a new recommendation engine.

- [ ] Change featured package choices/order using data/config without editing React presentation code.
- [ ] Verify Featured and Trending are separately labeled and private/ineligible curated items never leak.
- [ ] Verify query relevance, filters/cursors and namespace pins still behave exactly as Stage 1 promises.
- [ ] Verify website preserves existing palette/elevation while adopting composition-centric hierarchy; review SEO and pricing truthfulness.

---

## Milestone 15A: Make CLI Help Identify AgentPM as an Agent Package Manager
> **Scope note:** Turn CLI discovery into an accurate guide to Agent Package Management and the Tool-vs-Harness distinction. Outside this milestone: breaking command renames, argument redesign or Stage 1 error/TTY rewrite.
> **Implementation notes:**
> - The literal terms Agent Package Manager and Agent Package Management must appear in top-level help. Keep help compact, current and accurate.
> - **Dependencies:** M3A init, M5 install, M6 Harness; Stage 1 M8/M9 changes.
> - **Decisions:** D14. **Acceptance:** S2-AC-18.

**Implementation checklist**

- [ ] Set top-level clap help heading to explicitly describe AgentPM as an Agent Package Manager.
- [ ] Explain Agent Package Management in one concise lifecycle sentence.
- [ ] Show working Get Started command examples, tested against current parser.
- [ ] Explain default `agentpm init` now generates an Agent Package.
- [ ] Explain `--kind tool`/other Components for explicit initialization.
- [ ] Explain `agentpm lint` as APDS-aware definition validation.
- [ ] Explain `agentpm install` as dependency/lock resolution and local install.
- [ ] Explain `agentpm new` as Template workspace scaffolding.
- [ ] Explain `agentpm run` as Tool execution, not full Agent Package run.
- [ ] Explain `agentpm harness` as reference Runner for compatible Agent Packages.
- [ ] Preserve publish Registry link copy and SDK loading explanation.
- [ ] Snapshot `--help` top-level and relevant subcommands under noninteractive output.
- [ ] Test code examples rather than copying mockup-generated pseudo-CLI syntax.

**Required edge cases and integration contracts**

- [ ] Rewrite top-level `agentpm --help`: explicitly **“AgentPM — Agent Package Manager”** and **“Agent Package Management”**, summarize package lifecycle, show short working first-use commands. Keep commands/options backward-compatible; no verbose marketing essay in clap help.
- [ ] Update relevant help for `init` (new Agent default, Component kinds), `lint` (APDS), `publish` (Registry and link), `install` (dependencies/lock), `new` (Template scaffold), `run` (individual Tool), `harness` (Run an Agent Package with built-in reference Runner), and SDK-facing context. Verify CLI output snapshots/TTY/non-TTY and machine-readable contracts.
- [ ] Provide honest next steps for a default `agentpm init` scaffold that is **valid but not necessarily runnable** until a compatible Loop/model setup is present. Preserve existing CLI success publish Registry link; no duplicate dashboard requirement.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-18 evidence mapping and the decision record for D14 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 15B: Select or Publish a Reliable Low-cost First-run Agent Package
> **Scope note:** Provide one real, small, useful, tested Agent Package for the first-run try path. Outside this milestone: Not AgentPM Developer, not a generic misleading Tool-only demo and not a hardcoded featured registry ID.
> **Implementation notes:**
> - Candidate selection can occur during implementation. Prefer existing genuine examples when reliable, otherwise author one with small composition and low external prerequisites.
> - **Dependencies:** M6 Harness valid path; M11A featured selection; Stage 1 compatibility as deployed.
> - **Decisions:** D14. **Acceptance:** S2-AC-19.

**Implementation checklist**

- [ ] Inventory example repo and Registry candidates against first-run requirements.
- [ ] Choose or create a useful Agent Package with small understandable composition.
- [ ] Ensure Loop is compatible with currently shipped Harness if demonstrating run.
- [ ] Avoid mandatory external SaaS credentials besides clearly explained model provider where feasible.
- [ ] Document required model selection and estimated short-run cost/latency.
- [ ] Write a short starting prompt and representative expected output shape.
- [ ] Install fresh workspace from the public/authorized Registry and verify exact closure.
- [ ] Run through supported Harness TUI or headless path and capture evidence.
- [ ] Test missing provider/credential case and actionable remediation.
- [ ] Pin working package version in tutorial/demo codeboxes.
- [ ] Keep featured selection data-driven and swap example via curation, not JSX rewrite.
- [ ] Record why the example is useful and what it demonstrates about Components.
- [ ] Claude independently follows try flow without unstated setup assumptions.

**Required edge cases and integration contracts**

- [ ] Select **or create** real beginner-friendly published Agent Package, not the complex flagship AgentPM Developer. Candidate must require few external credentials, have predictable behavior, small genuine composition, short run, low model cost, readable example prompts/outcomes and clear provider/model setup.
- [ ] Verify exact dependency closure and Harness command on clean compatible systems (or capture documented environment constraints); keep starter version pinned in tutorial/example instructions to avoid drift; avoid a hidden paid service dependency.
- [ ] Test complete beginner try flow end-to-end, fallback for unavailable credentials or provider, CLI init/lint/publish help and all kind-specific init paths.
- [ ] Document D14 starter choice/cost/limitations; Claude reviews that a fresh developer can actually follow examples without unlisted prerequisites.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-19 evidence mapping and the decision record for D14 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 16A: Connect Try, Build and Template Journeys Across the Product
> **Scope note:** Turn homepage, Explore, package/Template details and CLI into three explicit paths with meaningful completion. Outside this milestone: forced account wizard, mandatory Developer assistant and dead destination.
> **Implementation notes:**
> - Prefer contextual onboarding plus perhaps a concise Get Started hub if useful. User chooses path; default experience encourages actually running an Agent Package.
> - **Dependencies:** M12–M15; Stage 1 analytics if deployed.
> - **Decisions:** D15. **Acceptance:** S2-AC-19.

**Implementation checklist**

- [ ] Map Try journey: discover → install → provider/configure → Harness run.
- [ ] Map Build journey: default init → compose → lint/install → test → publish.
- [ ] Map Template journey: choose → `agentpm new` → customize → run via supported surface.
- [ ] Identify exact current routes/commands and first meaningful success for each path.
- [ ] Link homepage CTAs, Explore, package detail and Template detail contextually.
- [ ] Make valid-but-not-runnable default init's next step explicit.
- [ ] Ensure successful CLI publish still provides Registry URL and page loads.
- [ ] Give creators a next step to author/share after successfully running existing package.
- [ ] Use copyable AgentPM Developer prompts only where tool is actually available.
- [ ] Provide manual/Template alternative where Developer isn't ready.
- [ ] Improve no-result and empty states without hiding clear/search controls.
- [ ] Decide and document whether small Get Started hub is worth introducing (D15).
- [ ] Test mobile/accessibility, signed-out and no-permission states and all CTAs.
- [ ] Use Stage 1 metrics only within privacy-approved instrumentation scope.

**Required edge cases and integration contracts**

- [ ] Map three complete journeys with actual routes and commands: **Try** discover/install/configure/Harness; **Build** init/compose/lint/test/publish; **Template** choose/new/customize/run. Capture first successful outcome and correct next action for each. No forced signup wizard.
- [ ] Add contextual entry points/CTAs and small helpful empty states across homepage, Explore, Agent detail, Template detail and relevant completed paths. Choose whether a concise `Get Started` hub adds value (**D15**) while avoiding duplicated docs or long funnel.
- [ ] Ensure installing/running naturally leads to how to author or publish one's own package; publishing returns the existing Registry link and, where Card foundation is available, offers appropriate sharing affordance. Never require Developer for authoring.
- [ ] AgentPM Developer integration **only when present**: copyable scoped prompts on package pages or no-result searches; hide/replace with direct manual/Template guidance if absent. No imaginary launch CTA.
- [ ] Conduct small technical-developer comprehension walk-through: category definition, Agent vs Agent Package, Components, APDS vs Runner readiness, first successful run, build/publish entry. Use Stage 1 analytics if available, with appropriate privacy and no new covert data collection. Record misunderstanding and fix high-severity copy blockers.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-19 evidence mapping and the decision record for D15 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 16B: Validate Comprehension and Write the Future AgentPM Developer Handoff
> **Scope note:** Check basic category comprehension and preserve the future user-owned creation/publishing adoption loop in a standalone brief. Outside this milestone: Do not implement the full AgentPM Developer wizard or assert category-market fit was proven.
> **Implementation notes:**
> - The user is learning Deploy Empathy and intends to discuss customer conversations separately. This milestone asks for lightweight comprehension evidence, not a full research program.
> - **Dependencies:** M16A journeys; M11B card investigation.
> - **Decisions:** D15, D16. **Acceptance:** S2-AC-20, S2-AC-21.

**Implementation checklist**

- [ ] Prepare a short newcomer comprehension script for Agent Package versus Component/Template.
- [ ] Include APDS versus Harness compatibility versus local runtime readiness questions.
- [ ] Observe ability to find/inspect the full `agent.json` and immutable standard.
- [ ] Observe ability to install/run first package with disclosed model requirements.
- [ ] Observe ability to find Build and Publish paths without founder guidance.
- [ ] Record concrete misunderstandings and fix material UX/copy blockers.
- [ ] Document participation and evidence honestly; if no user sessions occur, mark unverified.
- [ ] Write self-contained AgentPM Developer stage brief with cold-start supply rationale.
- [ ] Describe bring-your-own-idea assisted authoring and optional publication.
- [ ] Describe user-owned ideas rather than tutorial clone generation.
- [ ] Include Registry link + future README Agent Package Card as optional sharing outcome.
- [ ] State AgentPM Developer scope: help build AgentPM artifacts, not general coding.
- [ ] Define meaningful reuse/runs as success signals, not raw publication count.
- [ ] Link D16 README card investigation and outstanding future decisions.
- [ ] Claude reviews handoff for portability to a new chat without this conversation.

**Required edge cases and integration contracts**

- [ ] Produce a **self-contained future AgentPM Developer-stage handoff brief**: developer brings personal idea, Developer helps assemble APDS artifacts, lint/test/run, optional publish, Registry link and eventual README Card; user-owned ideas, no repetitive tutorial clones, no mandatory publish, meaningful reusability and outcome tracking, Developer restricted to AgentPM-building domain. Explicitly mark as **deferred implementation**.
- [ ] Cross-check README Card D16 outcome and document follow-on implementation location (later Stage 2 if approved or subsequent stage) in Developer handoff.
- [ ] Claude reviews three journeys, accessibility, meaningful completion, no false promises/dead CTAs, and the preserved future-stage brief.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-20, S2-AC-21 evidence mapping and the decision record for D15, D16 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Release Band 5: First-run Agent Package Adoption and Developer-stage Handoff
Covered milestones: 15A–16B.

This gives the CLI explicit Agent Package Manager/Management identity, one real economical starter Agent Package, and three actionable Try/Build/Template journeys across web and CLI. The guided user-owned idea→AgentPM Developer→optional publish→share journey is handed to its future stage, not implemented prematurely.

- [ ] Follow Try from anonymous Explore/detail to real install, configuration and Harness run.
- [ ] Follow Build from default Agent init to APDS lint and publish, preserving existing Registry link.
- [ ] Follow Template from scaffold to advertised supported execution surface.
- [ ] Review beginner comprehension and deliver a self-contained future AgentPM Developer handoff including README card follow-up.

---

## Milestone 17A: Inventory and Reorganize Docs After Stage 1 Is Fully Complete
> **Scope note:** After the hard Stage 1 completion gate, define a single category-consistent documentation architecture across AgentPM-owned repos. Outside this milestone: Do not begin while any Stage 1 milestone remains incomplete; no author README rewriting.
> **Implementation notes:**
> - This is the final Stage 2 milestone family. Earlier normative APDS files and feature-local notes are allowed, but broad docs/README cleanup waits for the final Stage 1 contract.
> - **Dependencies:** ALL Stage 1 work complete; all prior Stage 2 feature contracts stabilized.
> - **Decisions:** D18. **Acceptance:** S2-AC-22.

**Implementation checklist**

- [ ] Verify Stage 1 complete including its own M17 documentation and migration.
- [ ] Record exact released CLI, lockfile, publish, Python portability, signing and search contracts.
- [ ] Inventory CLI, website/Registry, Node/Python SDK, examples and owned READMEs.
- [ ] Map docs Introduction/Quickstart/navigation and current Tool-first dead ends.
- [ ] Identify user-authored Registry READMEs and explicitly exclude modification.
- [ ] Propose category-first docs IA with Try, Build, Templates, Components and reference.
- [ ] Ensure APDS immutable spec remains canonical rather than paraphrased alternate source.
- [ ] Plan route changes/redirects and protect existing docs deep links.
- [ ] List all command snippets needing executable proof and package version pinning.
- [ ] Review draft IA with Claude before cross-repo editing begins.

**Required edge cases and integration contracts**

- [ ] Verify Stage 1 fully complete and record which final CLI/Registry/lock/provenance/search/SEO contracts actually shipped. If not fully done, **do not start** comprehensive rewrite; keep normative APDS docs and functionality notes already written.
- [ ] Inventory all AgentPM-controlled user-facing docs and README sources, including CLI repository, Registry/website, SDKs, examples, homepage onboarding, docs Introduction/Quickstart/IA and in-repo READMEs. Do not rewrite package-author READMEs in the hosted Registry.
- [ ] Restructure docs navigation around: what is an Agent Package and why it matters; install/run first; build/publish; Components (six kinds); Templates; APDS schema/semantics and conformance; Registry/versions; Harness Runner and model prerequisites; Node/Python SDK integration; compatibility/Health/security and truthful limitation statements.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-22 evidence mapping and the decision record for D18 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 17B: Rewrite Introduction, Quickstart, Guides and AgentPM-owned READMEs
> **Scope note:** Publish one consistent Agent Package-first explanation across docs, help, website and repository-owned READMEs. Outside this milestone: Do not replace technical depth with marketing blurbs or claim universal Runner support.
> **Implementation notes:**
> - Quickstart should show a real first Agent Package installation and Harness run before making users learn all six Components. Preserve alternate authoring/Template paths.
> - **Dependencies:** M17A approved IA and completed Stage 1 prerequisite.
> - **Decisions:** D18. **Acceptance:** S2-AC-22.

**Implementation checklist**

- [ ] Rewrite Introduction around problem, Agent Package definition, Manager/Registry/APDS/Runner roles.
- [ ] Rewrite Quickstart with working starter install/configure/Harness commands.
- [ ] Provide manual Build path with `agentpm init` new Agent default and optional Loop.
- [ ] Provide Template starter with `agentpm new` semantics, not Agent install conflation.
- [ ] Update Component guides for Tool, Skill, Knowledge, Memory, Profile, Loop roles.
- [ ] Update APDS authoring/schema/semantics links and version support info.
- [ ] Update publish, install/lock, legacy migration, frozen and release provenance guidance.
- [ ] Update Harness Runner and Node/Python SDK integration help with realistic prerequisites.
- [ ] Update CLI, Registry, SDK, examples and website owned README positioning.
- [ ] Use Agent Package for artifact, Agent for runtime entity; no blind global replace.
- [ ] Preserve existing technical references and backwards-compatible docs redirects.
- [ ] Cross-check every reference link and current CLI syntax with actual shipped code.
- [ ] Separate objective Package Health facts from trust/quality/compatibility promises.

**Required edge cases and integration contracts**

- [ ] Rewrite Introduction and Quickstart to show a real Agent Package-first path instead of leading with Tool creation/use. Provide alternate manual build and Template routes, clear `init` default Agent Package, package-vs-component taxonomy and exact CLI commands.
- [ ] Unify all AgentPM-controlled README positioning with current website and CLI help: Agent Package Manager; Agent Package Management; Agent Packages; Components; Templates; APDS; Registry; Harness reference Runner; portability without universal interoperability claim.
- [ ] Cross-link immutable versioned APDS schema+semantics/fixtures and Registry `/standards/...` page. Do not make README/docs another moving authority for normative semantics; normative contract remains version-pinned.
- [ ] Update guides for publication enforcement, explicit/legacy standards, lockfile v4 + frozen behavior, Python target compatibility, provenance verification, Package Health signals, and common errors based on actual Stage 1+2 shipped behavior.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-22 evidence mapping and the decision record for D18 (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Milestone 17C: Run Final End-to-end Documentation and Cross-product Verification
> **Scope note:** Prove every public first-use example actually works and all Stage 2 contracts agree after the complete docs rewrite. Outside this milestone: unexecuted tests claimed as passed; no changes to settled product semantics just for wording.
> **Implementation notes:**
> - The release gate spans Rust CLI, Python Registry, Node/Python SDKs, Next website and real starter. Claude reviews before marking Stage 2 complete.
> - **Dependencies:** M17A/B; entire Stage 1 and Stage 2 completion.
> - **Decisions:** D01–D20 closure. **Acceptance:** S2-AC-01, S2-AC-02, S2-AC-03, S2-AC-04, S2-AC-05, S2-AC-06, S2-AC-07, S2-AC-08, S2-AC-09, S2-AC-10, S2-AC-11, S2-AC-12, S2-AC-13, S2-AC-14, S2-AC-15, S2-AC-16, S2-AC-17, S2-AC-18, S2-AC-19, S2-AC-20, S2-AC-21, S2-AC-22.

**Implementation checklist**

- [ ] Run actual docs commands for install/try, build/lint/publish and Template flows.
- [ ] Check missing credential and no-Loop valid-but-not-runnable documentation.
- [ ] Cross-verify docs and homepage code boxes reference the same selected version.
- [ ] Verify all APDS standard/manifest/fixture URLs refer to immutable v1.0.0.
- [ ] Recheck old/new legacy publication and install behavior after all migrations.
- [ ] Verify eight kind detail views, Health states and anonymous/private visibility.
- [ ] Verify Stage 1 search/SEO/lock/provenance regressions remain green.
- [ ] Capture final screenshots and ensure current brand palette/elevation preserved.
- [ ] Map all S2-AC criteria to tests/screenshots/report links.
- [ ] Close all D01–D20 with written reviewed decisions, not ambiguous unresolved choices.
- [ ] Provide Claude independent review package and resolve blocking findings.
- [ ] Record legitimate scope deferrals, future Developer handoff and open category-learning questions.
- [ ] Do not declare Stage 2 complete if Stage 1 hard gate or integration tests are outstanding.

**Required edge cases and integration contracts**

- [ ] Ensure agentpm-examples golden starter, README snippets, homepage codeboxes, docs and CLI help all pass exact-command smoke tests (with appropriate sandbox/test credentials); distinguish `agentpm run` Tool from Harness Agent.
- [ ] Verify all public links/canonicals, README badges/Card status, docs version consistency, searchability, accessibility and no regressions from final naming pass. No blind filewide Agent→Agent Package replacements.
- [ ] Have Claude independently review docs from a developer's perspective, verify tests/evidence and record unresolved follow-up topics instead of implying they're shipped.

**Verification and handoff**

- [ ] Record automated tests and exact commands actually run for this milestone; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state none).
- [ ] Update the S2-AC-01, S2-AC-02, S2-AC-03, S2-AC-04, S2-AC-05, S2-AC-06, S2-AC-07, S2-AC-08, S2-AC-09, S2-AC-10, S2-AC-11, S2-AC-12, S2-AC-13, S2-AC-14, S2-AC-15, S2-AC-16, S2-AC-17, S2-AC-18, S2-AC-19, S2-AC-20, S2-AC-21, S2-AC-22 evidence mapping and the decision record for D01–D20 closure (if applicable).
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

**Exit gate:** The scoped deliverable works against real source contracts; listed edge cases, verification and Claude review have passed.

## Release Band 6: Final Documentation, Full Integration and Independent Approval
Covered milestones: 17A–17C.

This gives the finished AgentPM ecosystem one accurate Agent Package-first documentation story across docs, CLI, Registry, SDKs and owned READMEs, based on stabilized Stage 1 and Stage 2 contracts. The whole stage is only complete after the high-risk lifecycle and UX matrices pass and Claude has reviewed the evidence.

- [ ] Document that Stage 1, including its M17 docs work, was fully complete before M17A began.
- [ ] Execute actual install/run/build/Template documentation commands, not just link checks.
- [ ] Map S2-AC-01–22 and D01–D20 to concrete evidence and reviewed decisions.
- [ ] Resolve blocking Claude findings and record deferred category-market questions and future-stage work.

---

## End-of-stage signoff and follow-on coordination

- [ ] Ensure all six numbered release bands have complete evidence and no unresolved blocking security/reproducibility failure.
- [ ] Validate the immutable APDS source files, schema compatibility path, Rust/Python fixture parity, Registry enforcement and accurate legacy status.
- [ ] Validate the full user mental model: Agent Package, Component, Template, APDS, Manager, Registry, Runner.
- [ ] Hand off future **reverse dependency discovery** as a deferred Registry idea, not an incomplete Stage 2 task.
- [ ] Hand off the **AgentPM Developer guided creator journey** separately, with optional publish/share and README card investigation notes.
- [ ] Retain first-run and customer-comprehension questions for Stage 5 user learning; no claim that category-market fit is established.
- [ ] Deliver Codex implementation records and Claude independent review findings with evidence links and unresolved issues clearly marked.
