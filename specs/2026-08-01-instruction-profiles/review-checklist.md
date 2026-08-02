# Review Checklist

## Contract surfaces

- Confirm the technical kind is exactly `profile` and technical dependency collections are exactly `profiles`.
- Confirm visible product copy uses Instruction Profile / Instruction Profiles.
- Confirm Profile packages do not add `display_name`.
- Confirm common `readme` and existing license object fields are supported without Profile-specific variants.
- Confirm README content is not treated as part of the behavioral contract by the CLI, API, web app, or SDKs.
- Confirm the top-level `profile` object matches `spec.md` exactly, including required/optional fields and `additionalProperties: false` boundaries.
- Confirm tone remains open-ended, verbosity is `concise | balanced | detailed`, and constraint strength is `required | preferred`.
- Confirm no arbitrary `instructions`, `prompt`, `system_prompt`, file-section, parameter, or variable escape hatch was added.
- Confirm Agent top-level `profiles` and Template `template.dependencies.profiles` use existing package reference shapes.
- Confirm Profile packages cannot declare dependencies.
- Confirm API, database, lockfile, route, and SDK shapes changed only where intended.
- Confirm constraint IDs are lowercase kebab-case, no more than 64 characters, with no underscores, uppercase characters, leading/trailing hyphens, or consecutive hyphens.

## Artifact boundary

- Confirm the implementation preserves the distinction between Profiles and Skills.
- Profile fields should describe stable role, objectives, principles, communication posture, audience adaptation, boundaries, and constraints.
- The schema must not add procedures, scripts, references, Tool relationships, workflow steps, output protocols, or task-specific success criteria.
- Do not reject authored text merely because a reviewer believes it sounds procedural; AgentPM should enforce structural boundaries, not subjective content classification.
- Confirm Profiles remain declarations consumed by other systems and are not described as enforcement engines.

## Correctness

- Check minimal and full Profile manifests against every acceptance criterion in `spec.md`.
- Confirm duplicate constraint IDs fail with precise issue paths.
- Confirm normalized vocabulary overlap fails.
- Confirm optional arrays/objects cannot be present in meaningless empty forms where the schema requires content.
- Confirm Profiles with no optional constraints, boundaries, audience, principles, or compatibility remain valid.
- Confirm Profile publish requires no build output or freshness check.
- Confirm init creates only the intended files.
- Confirm direct install routes to `.agentpm/profiles` and does not execute anything.
- Confirm direct Profile installation in an Agent project updates top-level `profiles` with existing range semantics.
- Confirm Agent and Template dependency resolution supports multiple Profiles and treats them as leaves.
- Confirm `agentpm new` writes resolved Profile references instead of an unconditional empty array.
- Confirm private Profile behavior reuses existing namespace authorization and safe error behavior.

## Lockfiles and dependency graphs

- Confirm package keys use `profile:@namespace/name@version`.
- Confirm Agent roots have a first-class `profiles` list.
- Confirm `reserved.profiles` remains readable for backward compatibility.
- Confirm resolvable reserved entries migrate and unresolved entries are retained.
- Confirm lockfile version remains 3 unless an explicit incompatibility justified a change.
- Confirm frozen-install compatibility includes Profile graphs and helpful messaging.
- Confirm reachability traversal, pruning, deduplication, and transitive installed-Agent traversal include Profiles.
- Confirm direct Profile installs follow the established direct Knowledge/Memory root behavior rather than being treated as Agent or Skill roots.
- Check shared Profile dependencies across multiple Agent roots for accidental duplication or pruning.
- Confirm a standalone direct Profile package does not require lockfile v3 solely because its package kind is `profile`; v3 is required when first-class Agent Profile relationships are present.
- Confirm frozen reads do not rewrite or migrate legacy `reserved.profiles`; migration occurs during normal lock regeneration.

## Publish and backend

- Confirm CLI and backend both reject Profile-owned dependencies.
- Confirm backend publish validation enforces the established server-side Profile invariants, including `kind: "profile"`, a structured singular `profile` object, and no Profile-owned dependencies, without unnecessarily duplicating the complete shared JSON Schema validator.
- Confirm common README/license packaging and safe-path checks are reused.
- Confirm no Profile-specific generated metadata columns or build artifacts were introduced.
- Confirm backend Agent graph expansion includes Profiles and Profile packages do not expand dependencies.
- Confirm Template dependency extraction includes Profiles.
- Confirm stored package kind remains authoritative during direct package resolution.
- Confirm Agent and Template `profiles` relationships can only be satisfied by packages whose stored kind is `profile`.
- Confirm PAT behavior remains compatible: `packages:publish` permits Profiles and legacy `tools:publish` remains Tool-only.

## Database migration

- Confirm `tool_search_index` is dropped before `trending_tools`.
- Confirm `tools_kind_check` allows exactly the intended seven package kinds.
- Confirm `on_install_completed()` includes `profile` and still handles missing kind as Tool for compatibility.
- Confirm `trending_tools` includes Profile rows and partitions ranking by kind.
- Confirm `tool_search_index` is recreated with every existing column and index.
- Confirm nested Profile metadata was not added to the search document.
- Confirm trigger and index recreation is complete and ordered correctly.
- Confirm downgrade behavior follows repository precedent and does not silently discard Profile data.

## Registry web

- Confirm all technical values remain `profile`; visible labels should not expose awkward singular/plural mappings.
- Confirm Profile cards, Explore filters, global search, trending, links, metadata, and private visibility work.
- Confirm Profile Overview renders every optional field defensively.
- Confirm required/preferred constraints are described as declared intent, not guaranteed enforcement.
- Confirm README is a separate documentation tab.
- Confirm Profile pages do not add build, query, lifecycle, contracts, or manual/instruction tabs.
- Confirm Agent and Template dependency pages link to Profile routes.
- Confirm copy positions Profiles as versioned behavioral contracts rather than marketplace personalities.
- Check loading, error, empty, mobile, canonical URL, and metadata states.
- Confirm canonical Profile URLs follow `/profiles/<package-id>/v<version>/overview` and dependency links preserve resolved versions when available.

## SDKs

- Confirm Node exports complete Profile types and `loadProfile`.
- Confirm Python exports complete Profile types and `load_profile`.
- Confirm both loaders return common identity, package root, manifest path, typed manifest, and typed Profile metadata.
- Confirm both reject missing, malformed, and wrong-kind packages clearly.
- Confirm generic Tool loaders guide callers to the Profile loader.
- Confirm Agent loaders expose first-class resolved Profile relationships, including locked-but-missing packages according to existing conventions.
- Confirm field names and relationship semantics are aligned between Node and Python.
- Confirm neither SDK reads README as instructions, compiles prompts, merges Profiles, evaluates compatibility, interpolates values, or enforces constraints.

## Regressions

- Check all hardcoded package-kind lists, not only primary enums.
- Check CLI help and error strings for stale six-kind lists.
- Check API DTOs, serializers, validators, routes, search, stats, and namespace logic.
- Check materialized views, triggers, migrations, and fixtures.
- Check frontend unions, route maps, result dispatch, cards, badges, landing sections, and stats.
- Check Node and Python package-kind unions, lockfile types, exports, and loader guidance.
- Check existing Tool, Agent, Template, Skill, Knowledge, and Memory flows.
- Confirm Profile changes did not alter Knowledge or Memory build/publish requirements.
- Confirm Templates without Profiles and Agents without Profiles remain compatible.
- Confirm older lockfiles remain readable and preserve unrelated reserved data.

## Tests and verification

- Confirm the implementation was verified according to `test-plan.md`.
- Confirm schema tests cover every required field, enum, optional object, and forbidden extra field.
- Confirm install tests cover direct and Agent dependency installs, while Template Profile dependencies are exercised through `agentpm new`, including private, frozen, and legacy lock behavior.
- Confirm backend tests cover publish, resolve, install, search, trending, stats, and authorization.
- Confirm migration behavior was exercised against a real test database, not reviewed only by inspection.
- Confirm web tests cover public/private detail pages and dependency links.
- Confirm Node and Python Profile loaders and Agent relationship loaders have tests.
- Confirm high-risk behavior is not relying only on manual confidence.
- Review the reported evidence for skipped commands or unverified migrations.

## Pattern adherence

- Confirm existing Skill, Knowledge, and Memory package-kind expansion patterns were reused before adding new abstractions.
- Confirm Profile code is simpler than Memory/Knowledge and does not introduce a generalized build subsystem.
- Confirm schema validation handles structural rules and Rust semantic validation is limited to cross-field rules.
- Confirm common README, license, package archive, authorization, search, signing, and security behavior is reused.
- Confirm no new dependency, database subsystem, runtime compiler, prompt assembler, or configuration framework was added.
- Confirm workspace metadata was not expanded unnecessarily when the existing generated-Agent pattern is sufficient.
- Confirm the implementation does not prematurely encode future Harness binding or composition decisions.

## Notes for reviewer

- Pay special attention to the old `reserved.profiles` path. Profiles were already accepted and preserved in Agent manifests before becoming resolvable, so the migration must not lose historical references.
- Search globally for `tool`, `agent`, `template`, `skill`, `knowledge`, and `memory` allowlists. This phase is likely to fail through an overlooked string list rather than through the main schema.
- Inspect the generated root Agent in `agentpm new`; the current code explicitly synthesizes an empty `profiles` array and must be changed to resolved references.
- Inspect install reachability and pruning. A downloaded Profile must be retained whenever it is referenced by a reachable Agent root and removed when it becomes unreachable according to existing lockfile behavior.
- Inspect user-facing constraint language. `required` is not an enforcement guarantee.
- Reject scope expansion into Profile parameters, prompt compilation, progressive disclosure, freeform instruction files, or runtime binding even if those features seem adjacent.
