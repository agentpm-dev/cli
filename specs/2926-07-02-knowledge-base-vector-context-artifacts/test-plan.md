# Test Plan

## Required verification
Before Phase 6B is considered done, verify that Knowledge artifacts work end-to-end without regressing existing package kinds.

Required flows:

- Manifest validation accepts valid `kind: "knowledge"` manifests.
- Manifest validation preserves/accepts agent top-level `knowledge` dependency arrays.
- Manifest validation accepts template `template.dependencies.knowledge`.
- Manifest validation rejects Knowledge manifests with dependencies.
- Manifest validation accepts both `mode: "context"` and `mode: "vector"` Knowledge manifests.
- `agentpm init --kind knowledge` creates a valid starter package.
- `agentpm knowledge build` validates context-mode documents and updates `agent.json`.
- `agentpm knowledge build` validates vector-mode chunks, sources, vectors, builds a local index, and updates `agent.json`.
- `agentpm publish --dry-run` packages both context-mode and vector-mode Knowledge artifacts correctly.
- Backend publish accepts Knowledge packages.
- Backend install resolve/init/finalize supports Knowledge packages.
- `agentpm install` installs Knowledge packages into `.agentpm/knowledge/...`.
- Agents and templates that depend on Knowledge resolve/install/lock Knowledge dependencies.
- `agentpm knowledge inspect` displays local and installed Knowledge metadata for both context-mode and vector-mode packages.
- Context-mode Knowledge packages are inspectable/readable and do not require embeddings or indexes.
- `agentpm knowledge query --vector-json` returns ranked chunks with scores and source metadata for vector-mode packages.
- `agentpm knowledge query` fails clearly when it cannot produce a query vector or when vector retrieval is requested for a context-mode package.
- Registry search/detail/list pages show Knowledge packages.
- Private namespace authorization applies to Knowledge packages.
- Existing Tool, Agent, Template, and Skill flows still pass.

## Automated checks

Run the relevant test suites from the repository after implementation. Adjust exact commands to the repo’s existing scripts if names differ.

### CLI / Rust

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Targeted CLI tests should cover:

- schema validation for context-mode and vector-mode Knowledge manifests
- init scaffolding
- context-mode build validation
- vector-mode build validation
- vector count/dimension mismatch
- derived manifest metadata writing for both modes
- publish dry-run packaging for both modes
- install/lockfile Knowledge behavior
- inspect output for both modes
- query with `--vector-json` for vector mode
- clear failure or read/inspect guidance when vector query is attempted on context mode
- query adapter errors if implemented

### Backend / API

Run the backend test suite. Use the repo’s existing command, for example:

- `pytest`
- or the project-specific backend test command if different

Targeted backend tests should cover:

- DB kind constraint / model accepts `knowledge`
- publish validation accepts context-mode and vector-mode Knowledge
- publish validation rejects Knowledge dependencies
- dependency validation for agents/templates that reference Knowledge
- install resolve/init/finalize for direct Knowledge packages
- private Knowledge package access denied/allowed
- search/totals/detail responses include Knowledge

### Frontend / Registry

Run the frontend test suite and lint/build commands. Use the repo’s existing scripts, for example:

- `npm test`
- `npm run lint`
- `npm run build`
- or `pnpm test`, `pnpm lint`, `pnpm build` if the project uses pnpm

Targeted frontend tests should cover:

- Knowledge search result rendering
- Knowledge totals/counts in all-search results
- Knowledge filter/tab behavior
- Knowledge package detail metadata for context-mode and vector-mode packages
- no executable/run treatment for Knowledge packages
- existing Tool/Agent/Template/Skill pages still render

### SDKs

Run Node and Python SDK tests using the repo’s existing commands.

Targeted SDK tests should cover:

- package kind models include `knowledge`
- install/search/detail models accept Knowledge
- loaded agents expose resolved Knowledge metadata if implemented
- `load_knowledge` / `loadKnowledge` exposes context-mode document metadata and vector-mode retrieval metadata if implemented
- generic callable `load(...)` remains tool-only

## Manual checks

### Local context-mode Knowledge authoring flow

Create a minimal context-mode Knowledge package:

1. Run `agentpm init --kind knowledge --name sample-playbook --description "Sample playbook context."`.
2. Configure the manifest with `knowledge.mode: "context"`.
3. Add one or more declared documents such as `knowledge/docs/playbook.md`.
4. Run `agentpm lint`.
5. Run `agentpm knowledge build`.
6. Confirm `agent.json` now includes derived document count, total bytes, and content hash metadata.
7. Run `agentpm knowledge inspect .`.
8. Confirm inspect output shows `mode: context`, declared documents, byte counts, and hashes.
9. Confirm no chunks, sources, embeddings, vectors, or indexes are required.
10. If a read/direct-context command is implemented, run it and confirm it returns the declared document content or paths.

### Local vector-mode Knowledge authoring flow

Create a minimal vector-mode Knowledge package:

1. Run `agentpm init --kind knowledge --name sample-docs --description "Sample docs corpus."`.
2. Configure the manifest with `knowledge.mode: "vector"`.
3. Add a small `knowledge/chunks.jsonl` with at least three chunks.
4. Add a matching `knowledge/sources.jsonl`.
5. Add a raw little-endian float32 `knowledge/embeddings/default.f32` with row count matching chunks and dimensions matching the manifest.
6. Run `agentpm lint`.
7. Run `agentpm knowledge build`.
8. Confirm `agent.json` now includes derived counts/hashes and an `agentpm-local` index entry.
9. Run `agentpm knowledge inspect .`.
10. Run `agentpm knowledge query . --vector-json query.json`.
11. Confirm ranked results include chunk IDs, scores, source IDs, and source metadata.

### Publish/install flow

1. Run `agentpm publish --dry-run` for a context-mode package.
2. Confirm the context-mode tarball includes:
   - `agent.json`
   - declared document files
   - README/license when present
   - no required chunks, sources, vector file, or generated local index files
3. Run `agentpm publish --dry-run` for a vector-mode package.
4. Confirm the vector-mode tarball includes:
   - `agent.json`
   - chunks JSONL
   - sources JSONL
   - vector file
   - generated local index files
   - README/license when present
5. Publish both modes to a test namespace.
6. Install each package into a clean workspace.
7. Confirm `.agentpm/knowledge/<namespace>/<name>/<version>/agent.json` exists for each package.
8. Confirm `agent.lock` contains `knowledge:@namespace/name@version` for each package.

### Agent dependency flow

1. Create an agent manifest with `knowledge: ["@namespace/sample-docs@0.1.0"]`.
2. Run `agentpm install`.
3. Confirm the Knowledge package is resolved, downloaded, installed, and locked.
4. Confirm no old “knowledge is preserved but not resolved” warning appears.

### Template dependency flow

1. Create a template with `template.dependencies.knowledge`.
2. Run `agentpm new`.
3. Confirm the generated root agent manifest includes resolved Knowledge refs.
4. Confirm workspace install includes `.agentpm/knowledge`.
5. Confirm `agent.lock` includes Knowledge packages and root relationships.

### Query and mode failure paths

Verify clear errors for:

- context-mode package with missing declared document
- context-mode package with unsafe document path
- vector query attempted against a context-mode artifact, unless the command intentionally supports a non-vector context-mode behavior
- missing vector JSON file
- vector length mismatch
- vector metadata provider/model mismatch
- missing local index for vector-mode package
- unsupported provider when query text is supplied without vector/adapter for vector mode
- missing built-in provider credentials if a built-in adapter is implemented
- shell adapter non-zero exit if `--embedding-command` is implemented
- shell adapter invalid JSON stdout if `--embedding-command` is implemented

### Registry UI

1. Publish a public Knowledge package.
2. Search for it.
3. Confirm it appears with a Knowledge label/badge.
4. Open detail page.
5. Confirm context-mode detail pages show document/count/hash metadata.
6. Confirm vector-mode detail pages show chunk/source/embedding/retrieval metadata.
7. Confirm no “run” command is shown.
8. Confirm install and inspect/query examples are shown, with query examples only where appropriate for vector-mode packages.

### Private namespace access

1. Publish a private Knowledge package in a private namespace.
2. As a non-member, verify search/detail/install do not expose the package.
3. As a member, verify search/detail/install work.
4. Verify private namespace billing/entitlement rules apply consistently with other package kinds.

## Expected evidence
Report back with:

- exact commands run
- passing test output summaries
- any failing tests and whether they are expected/out of scope
- sample context-mode `agentpm knowledge build` output
- sample vector-mode `agentpm knowledge build` output
- sample context-mode derived `agent.json` snippet showing document counts/hashes
- sample vector-mode derived `agent.json` snippet showing counts/hashes/indexes
- sample `agentpm knowledge inspect` output for both modes
- sample `agentpm knowledge query --vector-json` output for vector mode
- sample `agent.lock` snippet with Knowledge package and root relationship
- screenshots or short descriptions of registry search/detail pages if frontend was changed
- any provider adapter limitations that remain

## Out of scope
The following checks are intentionally out of scope for Phase 6B unless explicitly added later:

- full website crawling
- full document chunking pipelines
- embedding generation for corpus chunks
- scheduled source updates
- delta updates
- remote export/import into LanceDB, Chroma, pgvector, Pinecone, Weaviate, or Milvus
- neural bridge / vector projection correctness
- legal verification of source license/provenance
- robust prompt-injection mitigation for retrieved content
- large-scale retrieval quality benchmarking
- production performance benchmarking across very large corpora
- SDK-managed query embedding providers
- automatic selection of which context-mode documents fit in a model context window
