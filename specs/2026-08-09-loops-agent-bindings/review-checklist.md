# Review Checklist

## Contract surfaces

- Confirm the technical package kind is exactly `loop`.
- Confirm Loop packages use a required structured top-level `loop` object and Agents use singular top-level `loop` as a package reference.
- Confirm other package kinds reject top-level `loop`.
- Confirm Loop packages do not add `display_name` and continue to use common README/license metadata only.
- Confirm README is documentation and is not implicitly treated as Loop phase guidance by CLI/API/web/SDK code.
- Confirm Loop-specific objects use `additionalProperties: false` and match `spec.md` exactly.
- Confirm stable Loop/checkpoint/MCP IDs use the specified lowercase kebab-case contract.
- Confirm Memory space/operation binding names continue to use the existing Memory Blueprint snake_case key contract rather than Loop ID syntax.
- Confirm `archetype` remains optional/open-ended and is not an enum or runtime dispatch key.
- Confirm phase arrays are not interpreted as implicit execution order.
- Confirm omitted outcomes mean exactly one implicit `complete` outcome.
- Confirm present outcomes are objects with `id` + `description` and do not retain implicit `complete` unless authored.
- Confirm transition objects contain only `from`, `on`, and `to`.
- Confirm standardized terminal targets are exactly `$end`, `$abort`, and `$handoff` in Phase 7A.
- Confirm `max_steps` is the only initial Loop limit and the implementation does not reintroduce ambiguous `max_turns`.
- Confirm approval checkpoints use the exact v1 shape and no approver/provider/UI fields were added.
- Confirm Tool/phase error policy actions match the closed v1 vocabulary and cross-field rules.
- Confirm Agent `bindings` is Agent-only and contains only the intended `global`, `phases`, `mcp`, and `consumer_context` surfaces.
- Confirm binding package identities are versionless and cannot carry ranges or object-form versions.
- Confirm Template `dependencies.loop` is optional and singular.
- Confirm existing Agent top-level `tools` requirement was not changed by this phase.

## Loop artifact boundary

- Confirm the Loop remains a portable declarative orchestration contract rather than a workflow programming language.
- Reject additions of expressions, JSONPath, callbacks, scripts, arbitrary conditions, provider logic, state transformations, or embedded executable code.
- Confirm graph behavior derives from entry phase + phase outcomes + transitions, not from archetype names.
- Confirm phase `objective` supplies semantic meaning and runtime code does not infer behavior from IDs such as `plan` or `review`.
- Confirm access metadata names only Tool/Knowledge/Memory activity classes and does not name concrete packages/providers/stores.
- Confirm Skills and Profiles were not added to Loop access permissions without a new product decision.
- Confirm `knowledge: true` does not mean automatic retrieval and `memory.write: true` does not mean automatic phase-end writes.
- Confirm `$handoff` yields control generically and does not create Agent-to-Agent dependencies.
- Confirm approval is metadata only and the phase does not add run suspension/resume infrastructure.
- Confirm error policy is metadata only and no retry engine/backoff scheduler was added.

## Loop semantic correctness

- Check minimal and full Loop manifests against every acceptance criterion in `spec.md`.
- Confirm whitespace-only required authored text fails with precise paths.
- Confirm phase IDs are unique.
- Confirm explicit outcome IDs are unique within each phase.
- Confirm `entry_phase` exists.
- Confirm every transition source and non-terminal destination exists.
- Confirm transition `on` values are validated against the source phase's exact valid outcome set.
- Confirm explicit outcomes remove implicit `complete` unless `complete` is explicitly authored.
- Confirm every valid phase/outcome pair has exactly one transition.
- Confirm ambiguous duplicate transitions fail.
- Confirm every phase is reachable from the entry phase.
- Confirm at least one terminal target is reachable.
- Confirm cycles remain valid and do not require `max_steps`.
- Confirm checkpoint IDs/targets are valid and only one approval checkpoint targets a given phase.
- Confirm error-policy retry/non-retry fields are mutually consistent.
- Confirm no semantic validator attempts to judge whether the orchestration design is subjectively good.

## Agent binding boundary

- Confirm top-level dependency declarations remain the sole source of package versions/ranges.
- Confirm bindings identify only already-declared dependency package identities.
- Confirm every bound Tool/Skill/Knowledge/Memory/Profile package is checked against the corresponding top-level Agent dependency collection.
- Confirm MCP Tools are checked against top-level `tools`.
- Confirm wrong-collection membership does not satisfy the semantic check.
- Confirm duplicate canonical identities within one binding scope are handled deterministically and rejected where specified.
- Confirm duplicate Memory package entries within the same binding scope are rejected.
- Confirm `bindings.phases` requires an Agent Loop.
- Confirm phase bindings are optional and the implementation does not require an empty object for every Loop phase.
- Confirm global + phase binding intent is additive and no generic `inherit`, `replace`, `exclude`, `override`, or precedence framework was introduced.
- Confirm same package identity may be associated globally and within a phase without creating duplicate package dependencies.
- Confirm Memory selectors can bind spaces, operations, or both and do not redefine Blueprint operation semantics/triggers.
- Confirm record-type-level Memory bindings were not added.
- Confirm MCP bindings contain IDs + Tool identities only and no host/port/transport/process/auth configuration.
- Confirm MCP binding does not implicitly create global/phase Tool bindings and Tool bindings do not implicitly create MCP surfaces.
- Confirm consumer context is Agent-global only, author-named, safe-relative, optional, consumer-owned, and not packaged/read in 7A.
- Confirm no magic `AGENTPM.md` convention was introduced.

## Deliberately non-resolving validation

- Confirm Agent lint does not fetch/resolve the Loop solely to validate `bindings.phases` keys.
- Confirm Agent lint does not fetch/resolve Memory Blueprints solely to validate bound spaces or operations.
- Confirm Agent lint does not inspect Tool/Skill/Knowledge/Profile package contents for binding validation.
- Confirm Agent lint/publish/install do not compare Loop `access` with Agent bindings.
- Confirm a Tool bound to a phase whose Loop says `tools: false` remains a valid package composition in 7A.
- Confirm no warning/error says such a binding will be inactive during install/publish/lint; that opinion belongs to the future harness/runtime.
- Confirm no `override_loop_policy` or similar schema field was added.
- Confirm backend publish/resolve code likewise avoids cross-package runtime-policy validation.

## Init and publish

- Confirm `agentpm init --kind loop` creates only `agent.json` and `README.md`.
- Confirm starter Loop is semantically valid and demonstrates representative generic control flow without runtime-specific configuration.
- Confirm init creates no build/generated/script/runtime directories.
- Confirm Loop publish requires no build output or freshness check.
- Confirm Loop publish runs normal schema + semantic validation before archive creation.
- Confirm CLI and backend reject Loop-owned package dependencies.
- Confirm common README/license packaging and path safety are reused.
- Confirm no Loop-specific generated metadata/database columns were introduced.
- Confirm Agent publish accepts `loop` + `bindings` using only local binding membership validation.

## Package kind, resolver, and install roots

- Confirm `loop` was added to every intended package-kind enum/conversion/message.
- Confirm direct resolver responses preserve backend-authoritative `kind: "loop"`.
- Confirm Loop package keys use `loop:@namespace/name@version`.
- Confirm Loops install under `.agentpm/loops/<namespace>/<name>/<version>`.
- Confirm extraction uses the same integrity/traversal/atomic replacement behavior as other kinds.
- Confirm Loop packages are dependency leaves.
- Confirm direct Loop install in a local Agent updates singular top-level `loop` and does not create bindings.
- Confirm direct install of a different Loop replaces the prior singular reference rather than creating multiple Loops.
- Confirm no hidden Loop execution occurs during install.

## Lockfiles and dependency graphs

- Confirm Agent roots use a singular optional Loop relationship rather than a plural vector merely for implementation convenience.
- Confirm lockfile version remains 3 unless an explicitly reviewed incompatibility required a change.
- Confirm old v3 locks omitting Loop relationship fields remain readable.
- Confirm no legacy `reserved.loop` migration was invented unless such data already existed in the repository.
- Confirm local and registry Agent Loop dependencies resolve to Loop package kind only.
- Confirm reachability, retention, pruning, deduplication, refresh, root replacement, and transitive installed-Agent traversal include Loop relationships.
- Confirm shared exact Loop packages across reachable Agent roots are deduplicated.
- Confirm frozen install validates required Loop relationships and produces actionable wrong-kind/missing errors.
- Confirm Agent `bindings` remain manifest metadata and are not duplicated into lockfile relationships.

## Template/new

- Confirm Template `dependencies.loop` accepts exactly one normal package reference when present.
- Confirm `agentpm new` resolves and installs the direct Template Loop.
- Confirm synthesized root Agent receives the exact resolved Loop reference.
- Confirm generated local Agent manifests retain their own independently authored Loop dependencies and bindings.
- Confirm direct Template Loop is not copied into every local Agent.
- Confirm workspace locks contain singular Loop relationships through Agent roots.
- Confirm Template variables are not treated as Loop/binding runtime parameters.
- Confirm no Loop-specific prompting is introduced.

## Publish and backend

- Confirm backend package-kind allowlists include `loop` everywhere intended.
- Confirm complete Loop manifests persist in existing manifest JSON rather than new generated-metadata columns.
- Confirm Agent singular Loop relationships are extracted/persisted/expanded normally.
- Confirm only stored kind `loop` can satisfy Agent/Template Loop dependency fields.
- Confirm Template Loop relationships are validated/persisted but generic Template install semantics are unchanged.
- Confirm backend does not evaluate Agent bindings against Loop/Memory package contents.
- Confirm existing signing, namespace visibility, private dependency access, malware scan, and immutable publish behavior are reused.
- Confirm legacy Tool-only publish authorization behavior remains unchanged where applicable while generic package publish permits Loops.

## Database migration

- Confirm `tool_search_index` / `trending_tools` drop order follows current repository dependency requirements.
- Confirm `tools_kind_check` allows exactly the intended eight package kinds.
- Confirm install-completion statistics include Loop installs and preserve compatibility behavior.
- Confirm `trending_tools` includes Loop rows as their own `kind` partition.
- Confirm `tool_search_index` is recreated with every existing required column/index.
- Confirm nested Loop fields and Agent bindings were not added to full-text search.
- Confirm downgrade behavior follows repository precedent and does not silently discard Loop rows/data.

## Registry web

- Confirm Loop cards, filters, search, trending, namespace lists, badges, links, and private visibility work.
- Confirm canonical URLs follow `/loops/<package-id>/v<version>/overview`.
- Confirm Loop Overview renders every supported optional field defensively.
- Confirm implicit `complete` outcomes are represented accurately if the UI materializes them for readability.
- Confirm the UI does not imply phase arrays define order independent of transitions.
- Confirm README remains separate documentation.
- Confirm Agent detail surfaces link to resolved Loop dependency.
- Confirm Agent binding sections expose authored global/phase/Memory/MCP/consumer-context metadata without inventing runtime state.
- Confirm Memory binding display does not claim that spaces/operations were resolved/validated if they were not.
- Confirm access conflicts are not displayed as package validation failures.
- Confirm no Harness/run/approval/model/provider/MCP-network controls were introduced.
- Check loading, empty, mobile, canonical metadata, wrong-kind, and 404 states.

## SDKs

- Confirm Node exports complete Loop types and `loadLoop`.
- Confirm Python exports equivalent Loop types and `load_loop`.
- Confirm both loaders return common identity/root/manifest + typed Loop metadata using established conventions.
- Confirm both reject missing/malformed/wrong-kind packages clearly.
- Confirm generic Tool loaders guide Loop callers to the Loop-specific loader.
- Confirm Agent loaders expose singular resolved Loop relationships, including locked-but-missing paths according to current conventions.
- Confirm Agent loaders expose typed authored bindings.
- Confirm neither SDK validates phase IDs against Loops, Memory selectors against Blueprints, access conflicts, effective global+phase bindings, Profile precedence, MCP runtime configuration, or consumer-context file contents.
- Confirm neither SDK compiles prompts, executes a Loop, starts MCP, invokes Tools/Knowledge/Memory, or invokes the future Harness.
- Confirm Node/Python field names and relationship semantics align.

## Regressions

- Search globally for seven-kind hardcoded lists and verify `loop` was added only where package-kind expansion is intended.
- Check CLI help/error strings for stale seven-kind lists.
- Check API DTOs/serializers/validators/routes/search/stats/namespace logic.
- Check database constraints/views/triggers/migrations/fixtures.
- Check frontend unions/routes/cards/badges/filters/stats/landing surfaces.
- Check Node/Python package-kind unions, lockfile types, Agent manifest types, exports, and loader guidance.
- Confirm existing Tool, Agent, Template, Skill, Knowledge, Memory, and Profile flows remain unchanged.
- Confirm Knowledge/Memory build/publish requirements are not affected.
- Confirm Agents without Loop/bindings remain compatible.
- Confirm Templates without a direct Template `loop` remain compatible.
- Confirm existing required Agent `tools` behavior remains intact.
- Confirm older lockfiles remain readable.

## Tests and verification

- Confirm the implementation was verified according to `test-plan.md`.
- Confirm schema tests cover every Loop/binding object, enum, optional field, and forbidden property.
- Confirm graph semantic tests cover implicit/explicit outcomes, determinism, reachability, cycles, checkpoints, and error policy.
- Confirm Agent binding lint tests cover every package kind plus intentionally non-resolving cases.
- Confirm install tests cover direct Loop installs, Agent dependency installs, frozen/refresh/pruning, and Template `new`.
- Confirm backend tests cover publish, resolve, install, search, trending, stats, auth, and relationship kinds.
- Confirm migration behavior was exercised against a real test database rather than inspection only.
- Confirm web tests cover public/private Loop pages and Agent binding presentation.
- Confirm Node/Python Loop loaders and Agent relationship/binding loaders have tests.
- Review evidence for skipped commands, environmental blockers, or unverified migrations.

## Pattern adherence

- Confirm existing Profile package-kind expansion patterns were reused for Loop lifecycle plumbing before adding new abstractions.
- Confirm Memory informed binding vocabulary only; no Memory build/runtime subsystem was duplicated into 7A.
- Confirm schema validation handles structural rules and Rust semantic validation handles only local cross-field/graph rules.
- Confirm common README/license/archive/install/auth/search/signing/security behavior is reused.
- Confirm no new workflow engine, expression evaluator, runtime compiler, prompt assembler, provider abstraction, Memory store, MCP server manager, or approval subsystem was added.
- Confirm lockfile changes preserve singular Agent Loop semantics rather than generalizing Agents to multiple Loops.
- Confirm Template integration follows existing synthesized-root-Agent patterns instead of unnecessary workspace metadata expansion.
- Confirm implementation does not prematurely encode Phase 7B Harness execution decisions beyond the portable metadata contract explicitly defined in `spec.md`.

## Notes for reviewer

- This phase intentionally gives Agent bindings more weight than a typical dependency list. Inspect their schema carefully, but reject attempts to make them executable.
- The most important validation boundary is local Agent lint: bound packages must be declared top-level, but external Loop/Memory contents must not be fetched to validate phase/space/operation names.
- Pay special attention to versionless binding identities. Reusing normal version-capable `packageRef` would create two version sources and is a contract bug.
- Pay special attention to implicit outcome semantics. A phase with authored outcomes must not accidentally retain implicit `complete`.
- Inspect graph validation for exactly-one transition per valid outcome. A runtime should not have to invent missing control flow.
- Inspect `archetype` handling for hidden runtime switches. New third-party archetype strings must not require code changes.
- Inspect MCP fields for host/port/transport leakage. The portable Agent declares logical MCP Tool surfaces only.
- Inspect consumer context to ensure it is workspace-relative, optional, consumer-owned, not packaged, and not standardized to a magic filename.
- Inspect Memory binding examples/tests for snake_case operation names and current Memory lifecycle semantics; do not treat operations as generic writes.
- Search globally for package-kind allowlists: as the eighth kind, Loop support is likely to fail through an overlooked string list rather than the primary schema.
- Reject scope expansion into `agentpm harness`, model/provider settings, prompt composition, live execution, binding overrides, Agent-to-Agent handoff, or expression languages even if they appear adjacent and useful.
