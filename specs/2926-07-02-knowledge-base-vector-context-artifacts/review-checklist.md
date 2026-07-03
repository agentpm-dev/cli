# Review Checklist

## Contract surfaces
- Confirm `schemas/agentpm.manifest.schema.json` intentionally adds `kind: "knowledge"`.
- Confirm the top-level `knowledge` field is correctly kind-dependent:
  - agent: dependency array
  - knowledge: contract object
- Confirm `template.dependencies.knowledge` is supported.
- Confirm Knowledge manifests cannot declare dependencies.
- Confirm Skills still cannot depend on Knowledge.
- Confirm Knowledge supports both `mode: "context"` and `mode: "vector"` in the manifest contract.
- Confirm context-mode Knowledge declares documents for direct context loading and does not require chunks, embeddings, vectors, or indexes.
- Confirm vector-mode Knowledge declares chunks, sources, embeddings, vectors, and an `agentpm-local` index.
- Confirm package kind enums/models were updated in CLI, backend, frontend, and SDKs.
- Confirm DB constraints allow `knowledge`.
- Confirm lockfile keys and relationships use the existing v2 lockfile pattern.
- Confirm install layout adds `.agentpm/knowledge/...` without disrupting `.agentpm/tools`, `.agentpm/agents`, or `.agentpm/skills`.
- Confirm public docs explain what AgentPM does and does not do for Knowledge artifacts.
- Confirm public docs explain that Knowledge artifacts package portable context, and that vector retrieval is one supported mode, not the only valid mode.

## Correctness
- Check the main happy path against `spec.md`:
  - init
  - build
  - publish
  - install
  - inspect
  - context-mode build/inspect
  - vector-mode query with vector JSON
  - agent dependency resolution
  - template dependency resolution
  - registry display
- Confirm `agentpm knowledge build` does not crawl, chunk, embed, or call embedding providers.
- Confirm context-mode build validates declared document paths and computes document count/byte count/hash metadata.
- Confirm context-mode build does not require chunks JSONL, sources JSONL, embeddings, vectors, or indexes.
- Confirm vector-mode build validates chunks JSONL with line-numbered errors.
- Confirm vector-mode build validates sources JSONL with line-numbered errors.
- Confirm vector-mode chunk IDs and source IDs are unique.
- Confirm every vector-mode chunk `source_id` resolves to `sources.jsonl`.
- Confirm vector row order is documented and build validates all enforceable count/dimension invariants.
- Confirm vector file parsing uses raw little-endian float32.
- Confirm vector count equals chunk count.
- Confirm manifest dimensions equal vector dimensions.
- Confirm build writes derived counts/hashes/index metadata atomically for vector mode.
- Confirm build writes derived document metadata atomically for context mode.
- Confirm the generated local index is usable by `agentpm knowledge query` for vector mode.
- Confirm context-mode artifacts are inspectable/readable but do not pretend to support vector retrieval.
- Confirm `agentpm knowledge query --vector-json` does not call providers.
- Confirm shell adapter execution, if implemented, validates stdout and fails safely.
- Confirm built-in provider adapter, if implemented, is BYO-token only and limited to query text embedding.
- Confirm query returns source metadata/citations where available for vector mode.
- Confirm query fails clearly instead of silently returning invalid results when vector/provider/model/dimensions mismatch.
- Confirm vector query against a context-mode artifact fails clearly or directs the user to inspect/read/direct-context usage.
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
- Confirm failure paths are tested, especially vector mismatch, missing index for vector mode, unsafe paths, unsupported provider query, and invalid context document paths.
- Confirm tests do not rely only on happy-path fixtures.
- Confirm any skipped tests or unverified manual checks are documented.

## Pattern adherence
- Confirm implementation reused existing manifest loading, validation, and atomic write helpers.
- Confirm implementation reused existing package publish/upload/signing/malware scan patterns.
- Confirm implementation reused existing install/download/extract/cache patterns.
- Confirm implementation reused existing lockfile/root relationship patterns.
- Confirm implementation avoided broad renames from `tools` table/model names unless necessary.
- Confirm new abstractions are justified:
  - context document validator/reader
  - local index builder/query layer
  - query vector resolver
  - shell embedding adapter runner
- Confirm provider-specific code is isolated behind an adapter boundary.
- Confirm retrieval logic is separate from query-vector production.
- Confirm context loading logic is separate from vector retrieval logic.
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
- Confirm the artifact contract preserves declared documents for direct context loading.
- Confirm the artifact contract preserves canonical chunks/sources/vectors for vector-mode artifacts so future backend exporters can be implemented.
- Confirm the implementation does not hard-code a single vector database backend into the manifest contract.
- Confirm the implementation can later add `knowledge export --format pgvector|chroma|lancedb|pinecone` without changing the core manifest shape.
- Confirm unsupported embedding providers are still valid for vector-mode publish/install/inspect.
- Confirm users can query unsupported vector-mode providers by supplying a compatible vector or command adapter.
- Confirm context-mode artifacts remain valid even when no embedding provider is declared.
- Confirm Skill-to-Knowledge dependencies remain intentionally unsupported, not accidentally half-supported.

## Notes for reviewer
- The most important design boundary is: AgentPM packages portable context artifacts; it does not generate the corpus or own embedding provider billing.
- Knowledge has two valid modes: `context` for direct document context and `vector` for prepared retrieval.
- The highest-risk schema detail is the overloaded `knowledge` field plus mode-specific validation. Review this carefully.
- The highest-risk runtime detail for vector mode is vector-to-chunk alignment. Row order must match `chunks.jsonl`.
- The highest-risk product detail is making vector retrieval or built-in provider adapters feel mandatory. Context mode must remain first-class, and provider adapters must remain optional DX only.
- The highest-risk implementation churn is hardcoded package kind lists across CLI/backend/frontend/SDKs.
