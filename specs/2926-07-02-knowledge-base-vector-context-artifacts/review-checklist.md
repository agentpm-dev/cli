# Review Checklist

## Contract surfaces
- Confirm `schemas/agentpm.manifest.schema.json` intentionally adds `kind: "knowledge"`.
- Confirm the top-level `knowledge` field is correctly kind-dependent:
  - agent: dependency array
  - knowledge: contract object
- Confirm `template.dependencies.knowledge` is supported.
- Confirm Knowledge manifests cannot declare dependencies.
- Confirm Skills still cannot depend on Knowledge.
- Confirm package kind enums/models were updated in CLI, backend, frontend, and SDKs.
- Confirm DB constraints allow `knowledge`.
- Confirm lockfile keys and relationships use the existing v2 lockfile pattern.
- Confirm install layout adds `.agentpm/knowledge/...` without disrupting `.agentpm/tools`, `.agentpm/agents`, or `.agentpm/skills`.
- Confirm public docs explain what AgentPM does and does not do for Knowledge artifacts.

## Correctness
- Check the main happy path against `spec.md`:
  - init
  - build
  - publish
  - install
  - inspect
  - query with vector JSON
  - agent dependency resolution
  - template dependency resolution
  - registry display
- Confirm `agentpm knowledge build` does not crawl, chunk, or call embedding providers.
- Confirm `agentpm knowledge build` validates chunks JSONL with line-numbered errors.
- Confirm `agentpm knowledge build` validates sources JSONL with line-numbered errors.
- Confirm chunk IDs and source IDs are unique.
- Confirm every chunk `source_id` resolves to `sources.jsonl`.
- Confirm vector row order is documented and build validates all enforceable count/dimension invariants.
- Confirm vector file parsing uses raw little-endian float32.
- Confirm vector count equals chunk count.
- Confirm manifest dimensions equal vector dimensions.
- Confirm build writes derived counts/hashes/index metadata atomically.
- Confirm the generated local index is usable by `agentpm knowledge query`.
- Confirm `agentpm knowledge query --vector-json` does not call providers.
- Confirm shell adapter execution, if implemented, validates stdout and fails safely.
- Confirm built-in provider adapter, if implemented, is BYO-token only and limited to query text embedding.
- Confirm query returns source metadata/citations where available.
- Confirm query fails clearly instead of silently returning invalid results when vector/provider/model/dimensions mismatch.
- Confirm Knowledge detail UI does not present Knowledge as executable.

## Regressions
- Confirm existing tool manifests still validate.
- Confirm existing agent manifests still validate.
- Confirm existing template manifests still validate.
- Confirm existing skill manifests still validate.
- Confirm existing `agentpm init` behavior for tool/agent/template/skill still works.
- Confirm existing `agentpm publish --dry-run` behavior for tool/agent/template/skill still works.
- Confirm existing install/lockfile behavior for tool/agent/template/skill still works.
- Confirm existing `agentpm new` template flows still work without `template.dependencies.knowledge`.
- Confirm existing Skill support from Phase 6A still works.
- Confirm existing registry search/detail pages for other package kinds still render.
- Confirm private namespace enforcement did not regress for existing package kinds.
- Confirm SDK callable `load(...)` remains tool-only.

## Tests and verification
- Confirm verification followed `test-plan.md`.
- Confirm new schema tests cover valid and invalid Knowledge manifests.
- Confirm new CLI tests cover init/build/publish/install/inspect/query behavior.
- Confirm backend tests cover publish/install/search/private access for Knowledge.
- Confirm frontend tests cover Knowledge search/detail rendering.
- Confirm SDK tests cover Knowledge package kind support if SDK changes were made.
- Confirm failure paths are tested, especially vector mismatch, missing index, unsafe paths, and unsupported provider query.
- Confirm tests do not rely only on happy-path fixtures.
- Confirm any skipped tests or unverified manual checks are documented.

## Pattern adherence
- Confirm implementation reused existing manifest loading, validation, and atomic write helpers.
- Confirm implementation reused existing package publish/upload/signing/malware scan patterns.
- Confirm implementation reused existing install/download/extract/cache patterns.
- Confirm implementation reused existing lockfile/root relationship patterns.
- Confirm implementation avoided broad renames from `tools` table/model names unless necessary.
- Confirm new abstractions are justified:
  - local index builder/query layer
  - query vector resolver
  - shell embedding adapter runner
- Confirm provider-specific code is isolated behind an adapter boundary.
- Confirm retrieval logic is separate from query-vector production.
- Confirm no provider credentials are stored by AgentPM.
- Confirm adapter command execution avoids shell interpolation by default and has timeout/stdout limits.
- Confirm public manifest uses `agentpm-local`, not an unstable internal index implementation name, unless the spec was intentionally changed.

## Security and safety
- Confirm path validation prevents absolute paths and `..` traversal.
- Confirm tar packaging keeps safe path and embedded archive protections.
- Confirm adapter commands are not given registry tokens by AgentPM.
- Confirm stderr/stdout handling for adapters does not accidentally leak secrets in normal success output.
- Confirm private Knowledge package access is enforced consistently across search/detail/install.
- Confirm prompt-injection risk is documented.
- Confirm license/provenance metadata is displayed/preserved without claiming AgentPM legally verifies it.

## Future compatibility
- Confirm the artifact contract preserves canonical chunks/sources/vectors so future backend exporters can be implemented.
- Confirm the implementation does not hard-code a single vector database backend into the manifest contract.
- Confirm the implementation can later add `knowledge export --format pgvector|chroma|lancedb|pinecone` without changing the core manifest shape.
- Confirm unsupported embedding providers are still valid for publish/install/inspect.
- Confirm users can query unsupported providers by supplying a compatible vector or command adapter.
- Confirm Skill-to-Knowledge dependencies remain intentionally unsupported, not accidentally half-supported.

## Notes for reviewer
- The most important design boundary is: AgentPM packages prepared retrieval artifacts; it does not generate the corpus or own embedding provider billing.
- The highest-risk schema detail is the overloaded `knowledge` field. Review this carefully.
- The highest-risk runtime detail is vector-to-chunk alignment. Row order must match `chunks.jsonl`.
- The highest-risk product detail is making built-in provider adapters feel mandatory. They must remain optional DX only.
- The highest-risk implementation churn is hardcoded package kind lists across CLI/backend/frontend/SDKs.
