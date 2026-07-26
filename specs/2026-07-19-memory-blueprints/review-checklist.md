# Review Checklist

## Contract surfaces
- Confirm `kind: "memory"` was added everywhere package kinds are public or persisted.
- Confirm the overloaded top-level `memory` property is unambiguous: dependency array for agents, metadata object for Memory Blueprints.
- Confirm templates support `template.dependencies.memory` without changing existing required dependency fields.
- Confirm Memory Blueprints cannot declare package dependencies.
- Confirm the manifest schema, Rust models, backend models, SDK types, lockfiles, API responses, and frontend models agree on field names and allowed values.
- Confirm docs match the actual schema exactly.

## Declarative boundary
- Confirm no Phase 6C code persists or queries live memory records.
- Confirm no Memory Blueprint operation references tools, skills, prompts, commands, models, providers, or storage backends.
- Confirm retrieval modes describe capabilities rather than infrastructure.
- Confirm semantic retrieval does not require embedding configuration.
- Confirm runtime behavior is described as future consumer/harness responsibility.

## Memory model correctness
- Confirm scopes are declared once at top level and reused by spaces.
- Confirm spaces cannot reference undeclared scopes or record types.
- Confirm record schemas validate only `content`, while AgentPM owns the envelope.
- Confirm document identity is one logical record per complete ordered scope tuple.
- Confirm collections allow multiple unordered records per scope tuple.
- Confirm sequences require `ordinal` and document/collection contracts do not.
- Confirm model-specific retrieval rules are enforced.
- Confirm graph semantics were not quietly introduced.

## Governance
- Confirm data class and sensitivity are separate axes.
- Confirm all supported governance enums match the spec.
- Confirm persist/shareable are boolean.
- Confirm unsupported or misspelled `x-agentpm-*` keywords fail.
- Confirm recursive governance validation covers nested schemas and `$defs`.
- Confirm generated contracts preserve governance annotations.
- Confirm UI wording does not claim that Phase 6C enforces governance against live records.

## Operations and triggers
- Confirm only consolidate, transform, and delete operations are supported.
- Confirm only external, record_count, capacity, and interval triggers are supported.
- Confirm arbitrary condition languages or business-event matchers were not added.
- Confirm operation inputs/outputs/targets are validated against spaces and accepted record types.
- Confirm transform requires exactly one input.
- Confirm capacity triggers require configured space capacity.
- Confirm operation execution is not implemented.

## Build output
- Confirm `agentpm memory build` never modifies `agent.json` or author schemas.
- Confirm generated paths exactly match the spec.
- Confirm all generated JSON is deterministic and uses safe package-relative references.
- Confirm contract filenames and `$id` values are stable.
- Confirm the index is sorted and complete.
- Confirm rebuild fully removes stale generated contracts.
- Confirm local `$ref` handling remains valid after installation and registry display.
- Confirm remote/circular schema-reference behavior is explicit and safe.

## Freshness and publish
- Confirm build metadata hashes cover the manifest, source schemas, index, and contracts as specified.
- Confirm hashes are computed with stable path ordering and canonical inputs.
- Confirm `built_at` does not affect output hashes.
- Confirm publish uses check mode and never writes.
- Confirm publish rejects never-built, stale, modified, missing, extra, and unsupported output.
- Confirm errors tell the author to run `agentpm memory build`.
- Confirm no generated output is silently repaired during publish.

## CLI behavior
- Confirm `agentpm init --kind memory` creates a valid starter package but does not build it.
- Confirm lint validates authored declarations without requiring generated output.
- Confirm inspect supports local and installed targets.
- Confirm inspect reports stale state without rebuilding.
- Confirm no `agentpm memory query` command was added.
- Confirm help text and supported-kind errors include memory consistently.

## Dependencies and lockfiles
- Confirm Memory installs under `.agentpm/memory/...`.
- Confirm lockfile package keys use `memory:`.
- Confirm agents resolve top-level Memory dependencies.
- Confirm templates resolve Memory dependencies during generation.
- Confirm reserved memory relationship handling was converted cleanly rather than duplicated.
- Confirm Memory packages themselves remain dependency-free.
- Confirm old lockfiles and existing empty `memory` arrays remain compatible.

## SDKs
- Confirm `loadMemory` and `load_memory` mirror Knowledge package loading rather than live record loading.
- Confirm SDK methods validate package kind.
- Confirm SDKs expose generated contract enumeration clearly.
- Confirm no store adapter or live CRUD surface was introduced.
- Confirm public exports include all intended Memory types.
- Confirm Node and Python field naming follows each SDK’s established conventions without changing wire-format names.

## Backend and authorization
- Confirm `memory` is handled in publish, detail, search, download, install, dependency, and namespace flows.
- Confirm private Memory packages follow exactly the existing private namespace rules.
- Confirm unauthorized users cannot infer private manifest or contract details.
- Confirm agent/template dependency validation enforces access to private Memory packages.
- Confirm the backend does not add unnecessary duplicated metadata storage without justification.

## Registry UI
- Confirm the Memory page reuses existing package-detail patterns.
- Confirm scopes, record types, spaces, operations, governance, and resolved contracts are readable.
- Confirm envelope fields are visually distinguished from content fields.
- Confirm absent optional sections render cleanly.
- Confirm the UI does not imply live persistence, enforcement, or a specific vector provider.
- Confirm private, loading, error, and responsive states match adjacent package kinds.

## Correctness
- Check the main author flow against every acceptance criterion in `spec.md`.
- Check stale-build and missing-build failure paths carefully.
- Check generated contract semantics for all three models.
- Check governance preservation using nested source schemas.
- Check multiple record types in one space and one record type in multiple spaces.
- Check operation cross-reference validation.
- Check source schema path and `$ref` safety.

## Regressions
- Check existing Knowledge build/publish behavior was not generalized in a way that changes its outputs.
- Check existing tool, agent, template, skill, and knowledge manifests still validate.
- Check existing install layouts and lockfile package kinds remain stable.
- Check agent manifests still require tools unless another spec intentionally changes that.
- Check template generation without Memory dependencies is unchanged.
- Check package search/filter UI still handles every existing kind.
- Check private namespace behavior for existing kinds is unchanged.

## Tests and verification
- Confirm the work was verified according to `test-plan.md`.
- Confirm focused tests cover semantic validation, deterministic generation, freshness, publish, install, lockfiles, SDKs, backend, and UI.
- Confirm high-risk build and publish behavior is not relying only on manual verification.
- Confirm examples are validated and built in automated tests where practical.
- Confirm full repository test suites were run or any omissions are documented.

## Pattern adherence
- Confirm existing Knowledge command, path-safety, atomic-write, hashing, target-resolution, and publish-check patterns were reused where appropriate.
- Confirm Memory-specific code was not forced into Knowledge abstractions when the semantics differ.
- Confirm new shared helpers are justified and do not make Knowledge behavior less clear.
- Confirm no new dependency was added without a concrete need, especially for duration or JSON Schema handling.
- Confirm generated-file ownership and cleanup follow existing repository conventions.

## Notes for reviewer
- Pay particular attention to portable `$ref` resolution in generated contracts; a contract that only validates from the author’s source tree is a release blocker.
- Pay particular attention to stale-build verification. It must detect both changed source inputs and manually changed generated outputs.
- Pay particular attention to recursive governance-key validation because ordinary JSON Schema compilation may ignore unknown extension keywords.
- Pay particular attention to first-class lockfile resolution so existing reserved `memory` fields do not create duplicate roots or relationships.
- Pay particular attention to UI/API privacy so private contract schemas are never exposed through generic file or metadata endpoints.
- Reject scope expansion into live memory runtime behavior, storage adapters, bindings, or harness orchestration unless a separate approved spec covers it.
