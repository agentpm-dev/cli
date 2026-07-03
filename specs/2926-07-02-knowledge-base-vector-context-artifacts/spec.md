# Feature
Phase 6B: Knowledge Base & Vector Context Artifacts

## Problem / Goal
A large amount of agent setup friction comes from repeatedly doing the same expensive preparation work for retrieval:

- crawl or collect source material
- chunk text
- embed chunks
- build a vector index
- attach the retrieval corpus to an agent

The goal of Phase 6B is to make prepared context packageable, publishable, installable, lockable, inspectable, and attachable to agents and templates as first-class AgentPM artifacts. Knowledge artifacts may be optimized for vector retrieval, or they may be simple context packages intended to be loaded directly into a large-context model without chunking or embeddings.

A `kind: "knowledge"` package represents a passive, dependency-free, portable context artifact. It is not merely “some files” and it is not a managed vector database. It is a versioned artifact containing one of two MVP modes:

- `mode: "context"`: declared documents/files intended for direct context injection, with document metadata and hashes, but no required chunks, embeddings, vectors, or index
- `mode: "vector"`: a prepared retrieval corpus with canonical chunks, source metadata, embedding model metadata, precomputed vector files, an AgentPM-built local retrieval index, retrieval defaults, provenance, license, and rebuild metadata

Strategic POV:

> AgentPM is the package manager and artifact contract for portable agent context. AgentPM does not own how knowledge is created. It owns how prepared knowledge is packaged, validated, installed, locked, inspected, and reused. When the artifact is vector-backed, AgentPM also owns local index generation and query execution.

This lets developers install prepared context instead of rebuilding it locally each time, while preserving interoperability across large-context runtimes, retrieval runtimes, and future vector backends.

## Non-goals
- Do not make AgentPM a crawler.
- Do not make AgentPM a general document chunking framework.
- Do not make AgentPM call embedding providers during `knowledge build`.
- Do not require AgentPM to pay for, proxy, store, or manage provider-owned embedding credentials.
- Do not implement remote vector database export/import in Phase 6B. LanceDB, Chroma, pgvector, Pinecone, Weaviate, and similar integrations are future adapter/exporter work.
- Do not implement neural bridge / vector projection / cross-model vector conversion.
- Do not add Skill-to-Knowledge dependencies in Phase 6B. Skills remain procedural artifacts that may depend on tools only.
- Do not make Knowledge directly executable like a Tool.
- Do not add Knowledge dependencies to Knowledge artifacts.
- Do not implement recurring updates, source recrawling, scheduled rebuilds, or delta updates.
- Do not guarantee that provenance/license metadata is legally correct. AgentPM validates shape and preserves metadata; authors remain responsible for source rights and accuracy.
- Do not solve prompt injection in retrieved context. Treat Knowledge as untrusted context unless curated by the user or organization.
- Do not add a new artifact kind for embedding adapters in Phase 6B. Shell-command adapters are sufficient for the initial query path.

## Constraints / Invariants

### Core artifact model
- Add `knowledge` as a first-class package kind alongside `tool`, `agent`, `template`, and `skill`.
- A Knowledge artifact is passive and dependency-free.
- Knowledge artifacts must not declare `tools`, `skills`, `agents`, `knowledge`, `memory`, or `profiles` dependencies.
- Knowledge artifacts must declare a `knowledge.mode` of either `context` or `vector`.
- `mode: "context"` artifacts package documents for direct context injection and do not require chunks, embeddings, vectors, or indexes.
- `mode: "vector"` artifacts package prepared retrieval corpora and require chunks, sources, embeddings, vectors, and an AgentPM local index.
- Agents may depend on Knowledge using the existing top-level `knowledge` array.
- Templates may depend on Knowledge using `template.dependencies.knowledge`.
- Skills must not depend on Knowledge in Phase 6B.
- Knowledge packages use the existing package tables, publish flow, install flow, namespace rules, malware scan flow, signing flow, search flow, and registry detail concepts where possible.

Recommended dependency graph:

```text
template
  ├── agents
  ├── tools
  ├── skills
  └── knowledge

agent
  ├── tools
  ├── skills
  └── knowledge

skill
  └── tools

knowledge
  └── no dependencies
```

### Manifest naming caveat
The existing `agent.json` schema already uses top-level `knowledge` as an agent dependency array. Phase 6B must support both meanings without ambiguity:

- For `kind: "agent"`, top-level `knowledge` is an array of package references.
- For `kind: "knowledge"`, top-level `knowledge` is the Knowledge artifact contract object.

This requires `dependentSchemas` / `oneOf` updates so the same property name is legal with different shapes depending on `kind`.

### Knowledge manifest contract
A Knowledge package manifest should use `agent.json` with `kind: "knowledge"` and a required top-level `knowledge` object.

Do not include `display_name` in the Knowledge contract. Use `name`, `description`, README, package page, and namespace/package metadata for human-facing naming.

The Knowledge contract supports two MVP modes.

#### Context-mode example

Use `mode: "context"` for small or intentionally whole-document Knowledge packages that are expected to be loaded directly into an agent/runtime context window. Context-mode packages do not require chunking, embeddings, vectors, or indexes.

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "knowledge",
  "name": "engineering-playbook",
  "version": "0.1.0",
  "description": "Engineering playbook intended for direct context loading.",
  "readme": "README.md",
  "license": {
    "spdx": "internal"
  },
  "knowledge": {
    "mode": "context",
    "content_type": "documentation",
    "language": "en",
    "documents": [
      {
        "path": "knowledge/docs/playbook.md",
        "content_type": "text/markdown",
        "role": "context",
        "bytes": 18432,
        "sha256": "sha256:..."
      }
    ],
    "context": {
      "document_count": 1,
      "total_bytes": 18432,
      "content_hash": "sha256:..."
    },
    "provenance": {
      "generated_at": "2026-07-02T00:00:00Z",
      "builder": {
        "name": "custom",
        "version": "unknown"
      }
    }
  }
}
```

#### Vector-mode example

Use `mode: "vector"` for prepared retrieval corpora with chunks, sources, embeddings, vectors, and an AgentPM local index.

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "knowledge",
  "name": "python-docs",
  "version": "0.1.0",
  "description": "Prepared retrieval corpus for Python documentation.",
  "readme": "README.md",
  "license": {
    "spdx": "PSF-2.0"
  },
  "knowledge": {
    "mode": "vector",
    "content_type": "documentation",
    "language": "en",
    "corpus": {
      "chunks_path": "knowledge/chunks.jsonl",
      "sources_path": "knowledge/sources.jsonl",
      "chunk_count": 12482,
      "source_count": 327,
      "content_hash": "sha256:..."
    },
    "chunking": {
      "strategy": "recursive-text-splitter",
      "chunk_size": 512,
      "overlap": 64
    },
    "embedding": {
      "id": "default",
      "provider": "openai",
      "model": "text-embedding-3-small",
      "dimensions": 1536,
      "metric": "cosine",
      "normalized": true,
      "vectors_path": "knowledge/embeddings/default.f32",
      "vector_count": 12482,
      "vectors_hash": "sha256:..."
    },
    "indexes": [
      {
        "id": "default",
        "type": "agentpm-local",
        "path": "knowledge/indexes/default",
        "embedding_id": "default",
        "generated_by": "agentpm knowledge build"
      }
    ],
    "retrieval": {
      "strategy": "vector",
      "default_top_k": 8,
      "return_citations": true
    },
    "provenance": {
      "sources_manifest_path": "knowledge/provenance/sources.jsonl",
      "generated_at": "2026-07-02T00:00:00Z",
      "builder": {
        "name": "custom",
        "version": "unknown"
      }
    }
  }
}
```

### Required Knowledge package files
For Phase 6B, publishable Knowledge artifacts support two modes.

Context-mode packages should include declared documents for direct context loading:

The layouts below are recommended conventions, not fixed path requirements. AgentPM packages and validates the files declared in `agent.json`. Authors may choose different file or directory names as long as all declared paths are safe, package-relative, and, for Knowledge-owned payload files, remain under the `knowledge/` directory.

For context mode, `knowledge.documents[].path` is authoritative.

For vector mode, `knowledge.corpus.chunks_path`, `knowledge.corpus.sources_path`, `knowledge.embedding.vectors_path`, declared `knowledge.indexes[].path`, and optional provenance paths are authoritative.

```text
agent.json
README.md                  # optional but encouraged
knowledge/
  docs/
    playbook.md            # one or more declared context documents
  provenance/
    sources.jsonl          # optional detailed provenance/source manifest
```

Vector-mode packages should include prepared retrieval files:

```text
agent.json
README.md                  # optional but encouraged
knowledge/
  chunks.jsonl             # required for mode=vector
  sources.jsonl            # required for mode=vector
  documents/               # optional source documents
  embeddings/
    default.f32            # required for mode=vector
  indexes/
    default/               # generated by agentpm knowledge build for mode=vector
  provenance/
    sources.jsonl          # optional detailed provenance/source manifest
```

For the MVP, `mode: "context"` artifacts are valid publishable Knowledge packages for direct context injection and must not require chunks, embeddings, vectors, or indexes. `mode: "vector"` artifacts are prepared retrieval corpora and should require chunks, sources, embeddings, and an AgentPM local index produced by `agentpm knowledge build`.

Knowledge packages must be built before publishing. `agentpm publish` must not publish an unbuilt or stale Knowledge package. For Knowledge manifests, publish performs a build-check validation and fails with a clear instruction to run `agentpm knowledge build` if required derived metadata, hashes, or mode-specific build outputs are missing or stale.

`agentpm publish` should not mutate `agent.json`, compute missing hashes, or generate Knowledge indexes unless a future explicit flag such as `--build` or `--prepare` is added. The normal authoring flow is:

```bash
agentpm knowledge build
agentpm publish
```

For mode: "context", publish verifies declared documents and build-derived document metadata.

For mode: "vector", publish verifies chunks, sources, vectors, derived metadata, and generated local index metadata.

### Context document contract
`mode: "context"` packages use declared document entries instead of chunks, embeddings, vectors, and indexes. Each document entry points to a package-relative file intended for direct context loading by an agent/runtime/harness.

Minimum shape:

```json
{
  "path": "knowledge/docs/playbook.md",
  "content_type": "text/markdown",
  "role": "context",
  "bytes": 18432,
  "sha256": "sha256:..."
}
```

Validation:
- `knowledge.documents` must be a non-empty array for `mode: "context"`
- every document must have a safe package-relative `path`
- every declared document path must exist and be a file
- `content_type`, if present, should be a non-empty string
- `role`, if present, should be a non-empty string and remain open-ended; suggested initial value is `context`
- `bytes` and `sha256` are build-derived fields owned by `agentpm knowledge build`
- context documents should not require chunk/source/vector/index metadata

`mode: "context"` is intended for small or intentionally whole-document corpora. AgentPM should validate/package/install these documents, but it should not attempt semantic retrieval over them unless a future mode or adapter adds that behavior.

### Chunk JSONL contract
`knowledge/chunks.jsonl` is newline-delimited JSON. Each line represents one chunk.

Minimum shape:

```json
{
  "id": "chunk_000001",
  "source_id": "python-docs/contextlib",
  "text": "The contextlib module provides utilities...",
  "metadata": {
    "section": "contextlib",
    "uri": "https://docs.python.org/3/library/contextlib.html"
  }
}
```

Validation:
- every line must be valid JSON
- every chunk must have a non-empty string `id`
- chunk IDs must be unique
- every chunk must have a non-empty string `source_id`
- every chunk must have non-empty string `text`
- `metadata`, if present, must be an object
- every `source_id` must exist in `sources.jsonl`
- row order is meaningful and must remain stable because vectors are row-aligned to this file

### Source JSONL contract
`knowledge/sources.jsonl` is newline-delimited JSON. Each line represents a source document or source unit.

Minimum shape:

```json
{
  "id": "python-docs/contextlib",
  "title": "contextlib — Utilities for with-statement contexts",
  "uri": "https://docs.python.org/3/library/contextlib.html",
  "retrieved_at": "2026-07-02T00:00:00Z",
  "license": "PSF-2.0"
}
```

Validation:
- every line must be valid JSON
- every source must have a non-empty string `id`
- source IDs must be unique
- optional `title`, `uri`, `retrieved_at`, and `license` fields should be preserved if present
- if `retrieved_at` is present, validate it as a string and prefer RFC3339 format, but do not block non-critical metadata unless the implementation already has a lightweight parser available

### Vector file contract
The default vector file format is raw contiguous little-endian `float32`.

Vector invariant:

> Vector row order must exactly match `chunks.jsonl` order.

If `chunks.jsonl` line 1 is `chunk_a`, vector row 1 is the embedding for `chunk_a`.

Validation:
- vector file exists
- file length must be divisible by 4 bytes
- vector count must equal chunk count
- dimensions must equal `knowledge.embedding.dimensions`
- vector bytes must decode as little-endian float32
- reject empty vector files
- reject dimensions of `0`
- for MVP, prefer requiring `metric: "cosine"` and `normalized: true` unless the selected local index implementation supports additional metrics reliably

### Index contract
`agentpm knowledge build` generates the default local index from the declared vectors.

The manifest public index type should be `agentpm-local`. The internal implementation may use any reasonable Rust crate/format, but avoid exposing implementation-specific names in the stable public contract unless required.

`agentpm knowledge query` must use the AgentPM local index and must fail clearly when no generated index exists.

The index is an optimization and local query representation. The canonical interop contract remains chunks + sources + embedding metadata + vector rows.

### Build-derived manifest fields
`agentpm knowledge build` should update `agent.json` with derived metadata according to mode.

For `mode: "context"`:

- `knowledge.documents[].bytes`
- `knowledge.documents[].sha256`
- `knowledge.context.document_count`
- `knowledge.context.total_bytes`
- `knowledge.context.content_hash`

For `mode: "vector"`:

- `knowledge.corpus.chunk_count`
- `knowledge.corpus.source_count`
- `knowledge.corpus.content_hash`
- `knowledge.embedding.vector_count`
- `knowledge.embedding.vectors_hash`
- `knowledge.indexes` entry for generated default `agentpm-local` index
- optional index hash/metadata if convenient

Treat these as derived truth owned by `build`. Authors provide paths and operational intent; `build` validates the files and records computed facts.

`agentpm publish` must verify these build-derived fields instead of trusting their presence alone. Publish should recompute enough mode-specific metadata to prove the package is already built and current.

For `mode: "context"`, publish should verify:

- every declared `knowledge.documents[].path` exists and is safe
- every document `bytes` value matches the current file size
- every document `sha256` value matches the current file contents
- `knowledge.context.document_count` matches the declared document count
- `knowledge.context.total_bytes` matches the sum of declared document byte counts
- `knowledge.context.content_hash` matches the recomputed aggregate context content hash

For `mode: "vector"`, publish should verify:

- `knowledge.corpus.chunks_path` exists and validates
- `knowledge.corpus.sources_path` exists and validates
- `knowledge.embedding.vectors_path` exists and validates
- `knowledge.corpus.chunk_count` matches the current chunks file
- `knowledge.corpus.source_count` matches the current sources file
- `knowledge.corpus.content_hash` matches the recomputed corpus hash
- `knowledge.embedding.vector_count` matches the current vector file
- `knowledge.embedding.vectors_hash` matches the current vector file
- vector dimensions still match `knowledge.embedding.dimensions`
- an `agentpm-local` index entry exists
- the declared `agentpm-local` index path exists

For vector mode, `agentpm knowledge build` should also write index metadata that lets publish detect stale indexes without rebuilding them. The index metadata should record the source corpus hash, source vector hash, dimensions, vector count, index type, and AgentPM version used to build the index.

Example index metadata:

```json
{
  "type": "agentpm-local",
  "embedding_id": "default",
  "source_corpus_hash": "sha256:...",
  "source_vectors_hash": "sha256:...",
  "dimensions": 1536,
  "vector_count": 12482,
  "built_at": "2026-07-03T00:00:00Z",
  "agentpm_version": "0.6.0"
}
```

Publish should fail if the index metadata does not match the current manifest-derived corpus/vector hashes, dimensions, or vector count.

### Strict vs loose validation
Strictly validate fields AgentPM uses to build, install, query, lock, or verify:

- `kind`
- package identity
- `knowledge.mode`
- required paths for the selected mode
- safe relative paths
- declared context document paths for `mode: "context"`
- chunks/sources JSONL shape for `mode: "vector"`
- chunk ID uniqueness for `mode: "vector"`
- source ID references for `mode: "vector"`
- vector dimensions/count/format for `mode: "vector"`
- embedding dimensions for `mode: "vector"`
- embedding metric if AgentPM query depends on it
- `normalized` boolean for `mode: "vector"`
- index type/path if AgentPM must query it
- retrieval `default_top_k` for vector retrieval

Loosely validate informational/provenance fields:

- `chunking.strategy`
- `chunking.tool`
- `chunking.tool_version`
- `content_type`
- `language`
- `license`
- `provenance.builder`
- `provenance.generated_at`

For example, `chunking.strategy` should be a non-empty string if present, but should not be restricted to a fixed enum. AgentPM does not prove the chunks were actually produced by the declared strategy in Phase 6B.

Useful rule:

> If AgentPM uses the field to build, install, query, lock, or verify the artifact, validate strictly. If the field explains how the artifact was produced, validate lightly and preserve it.

### CLI commands
Add a `knowledge` CLI command group:

```bash
agentpm knowledge build
agentpm knowledge inspect [path-or-package]
agentpm knowledge query <path-or-package> [query text]
```

Also update:

```bash
agentpm init --kind knowledge
agentpm lint
agentpm publish
agentpm install
agentpm new
```

### `agentpm init --kind knowledge`
Creates a starter Knowledge package. The default starter can be either `mode: "context"` for the lowest-friction starting point, or it can accept a flag/template variant for `mode: "vector"` if the implementation wants both scaffolds immediately. Prefer making context mode the default because it does not require embeddings.

Context starter layout:

```text
agent.json
README.md
knowledge/
  docs/
    context.md
```

Vector starter layout:

```text
agent.json
README.md
knowledge/
  chunks.jsonl
  sources.jsonl
  embeddings/
```

The starter manifest should validate. Placeholder data should be minimal but realistic enough to show expected shapes. Do not generate `indexes/default` in `init`; that belongs to `build` for vector-mode packages.

### `agentpm knowledge build`
Definition:

> `agentpm knowledge build` prepares a local Knowledge package for publishing and local use. It reads `agent.json`, validates the selected Knowledge mode, computes derived metadata, and updates `agent.json`. For `mode: "context"`, it validates declared documents and computes document hashes/byte counts. For `mode: "vector"`, it validates the declared corpus, source, embedding, and index paths, verifies chunk/source/vector consistency, computes derived counts and content hashes, and builds or refreshes AgentPM’s default local retrieval index. It does not crawl source documents or call embedding providers in Phase 6B.

Expected behavior:
- default manifest path: `agent.json`
- optional `--manifest <path>`
- optional `--check` or `--dry-run` mode may validate without writing; include only if easy and consistent with repo patterns
- write derived metadata atomically using existing manifest write helpers
- for `mode: "context"`, validate declared documents and compute document count, byte count, per-document hashes, and aggregate content hash
- for `mode: "vector"`, rebuild `knowledge/indexes/default` from vectors
- fail if the manifest is not `kind: "knowledge"`
- fail if required files for the selected mode are missing or invalid
- fail if vector count/dimensions do not match chunks/manifest for `mode: "vector"`

### `agentpm publish` build-check behavior

For `kind: "knowledge"` packages, `agentpm publish` must verify that `agentpm knowledge build` has already been run and that the build outputs are current.

Publish should reuse the same validation and metadata computation logic as `agentpm knowledge build`, but in check-only mode:

agentpm knowledge build
  validates inputs
  computes derived metadata
  writes derived metadata
  writes vector index metadata for mode="vector"

agentpm publish
  validates inputs
  recomputes derived metadata
  compares recomputed metadata to manifest fields
  verifies vector index metadata for mode="vector"
  packages files only if the build state is current
  does not mutate files

If build metadata is missing, publish should fail with:

Knowledge package is not built.

Run:
agentpm knowledge build

Then publish again.

If build metadata is stale, publish should fail with a specific mismatch and the same recovery instruction:

Knowledge package build metadata is stale.

knowledge.documents[0].sha256 does not match knowledge/docs/playbook.md.

Run:
agentpm knowledge build

Then publish again.

For stale vector indexes, publish should fail with a specific index mismatch:

Knowledge index is stale.

knowledge/indexes/default was built for vectors hash sha256:old...
Current vectors hash is sha256:new...

Run:
agentpm knowledge build

Then publish again.

Publish must not silently rebuild indexes or rewrite agent.json in Phase 6B.

### `agentpm knowledge inspect`
Reads a local or installed Knowledge package and prints metadata.

Suggested human output for vector mode:

```text
Knowledge: @zack/python-docs@0.1.0
Mode: vector
Description: Prepared retrieval corpus for Python documentation.
Chunks: 12,482
Sources: 327
Embedding: openai/text-embedding-3-small, 1536 dims, cosine, normalized
Vectors: knowledge/embeddings/default.f32, sha256:...
Indexes:
  - default agentpm-local knowledge/indexes/default
Retrieval defaults:
  topK: 8
  citations: true
```

Suggested human output for context mode:

```text
Knowledge: @zack/engineering-playbook@0.1.0
Mode: context
Description: Engineering playbook intended for direct context loading.
Documents: 1
Total bytes: 18,432
Content hash: sha256:...
Files:
  - knowledge/docs/playbook.md text/markdown sha256:...
```

Suggested JSON output with `--json` should include the resolved manifest metadata and useful installed path information.

### `agentpm knowledge query`
`knowledge query` is the retrieval equivalent of `agentpm run` for tools. It should let developers verify that a vector-mode Knowledge artifact is actually usable.

For `mode: "context"`, `knowledge query` should not attempt semantic retrieval because there is no vector index. It should fail clearly and direct users to `agentpm knowledge inspect` and future context-loading/runtime behavior, or to rebuild/publish the artifact as `mode: "vector"` if semantic retrieval is desired. A future `agentpm knowledge read` command could expose context documents directly, but that command is not required in Phase 6B.

Core command shape:

```bash
agentpm knowledge query <knowledge-ref-or-path> "How does auth work?"
agentpm knowledge query <knowledge-ref-or-path> --vector-json query.json
agentpm knowledge query <knowledge-ref-or-path> --embedding-command ./embed-query "How does auth work?"
```

Query must be provider-neutral at the contract level. The retrieval engine consumes a vector. Producing the query vector is a separate adapter concern.

Vector resolution precedence:
1. `--vector-json` or `--vector`: use the supplied query vector. Do not call any provider or adapter.
2. `--embedding-command`: shell out to a user-provided command adapter.
3. Built-in provider adapter: use only when the artifact provider/model is supported and user credentials are available.
4. Fail clearly.

Required in Phase 6B:
- support `--vector-json <file|->`
- support `--embedding-command <cmd>` if implementation cost is reasonable
- include at most one built-in query adapter for DX if desired, likely OpenAI, using BYO credentials from environment/config
- do not require built-in provider support for valid Knowledge artifacts

Strong product stance:

> Built-in adapters are conveniences. They are not required by the Knowledge artifact contract. AgentPM must not own provider billing or credentials.

### Query vector JSON format
Suggested input:

```json
{
  "vector": [0.012, -0.083, 0.44],
  "embedding": {
    "provider": "openai",
    "model": "text-embedding-3-small",
    "dimensions": 1536
  }
}
```

Validation:
- `vector` must be an array of numbers
- vector length must equal manifest `knowledge.embedding.dimensions`
- if `embedding.provider`, `embedding.model`, or `embedding.dimensions` are present, validate against the artifact manifest and fail or warn on mismatch; prefer fail for provider/model/dimension mismatch because bad query vectors silently degrade retrieval
- if `--vector` raw format is implemented, use raw little-endian float32 to match artifact vector format

### Shell embedding adapter contract
For `--embedding-command`, AgentPM should execute a command without shell interpolation by default.

Input to adapter stdin:

```json
{
  "text": "How does auth work?",
  "embedding": {
    "provider": "openai",
    "model": "text-embedding-3-small",
    "dimensions": 1536,
    "metric": "cosine",
    "normalized": true
  }
}
```

Output from adapter stdout:

```json
{
  "vector": [0.012, -0.083, 0.44],
  "dimensions": 1536
}
```

Execution constraints:
- execute argv directly; avoid shell interpolation
- timeout
- max stdout size
- non-zero exit fails query
- stderr shown only on failure
- do not pass AgentPM registry tokens to the adapter unless the user’s environment does so explicitly
- validate stdout JSON before retrieval
- validate vector length equals manifest dimensions

### Built-in provider adapter
If implemented in Phase 6B:
- implement as an internal adapter boundary, not hardwired inside retrieval
- use user-provided credentials only, such as `OPENAI_API_KEY`
- query only embeds the query text; build/publish/install/inspect must not call providers
- unsupported providers remain valid for publish/install/inspect, but `knowledge query "text"` fails unless vector or adapter path is provided

Suggested unsupported provider error:

```text
This artifact uses voyage/voyage-code-3.
agentpm knowledge query cannot embed text for provider "voyage".
Provide --vector-json or --embedding-command, or use a runtime that supports this provider.
```

### Install and local layout
Existing install layout has package kind directories such as `.agentpm/tools`, `.agentpm/agents`, and `.agentpm/skills`. Add:

```text
.agentpm/knowledge/<namespace>/<name>/<version>/
```

Update cache/download/extract helpers, install command, workspace generation, and SDK path resolution where package kinds are enumerated.

### Lockfile behavior
Knowledge packages must be pinned in `agent.lock` like other package kinds.

Package keys:

```text
knowledge:@zack/python-docs@0.1.0
```

Agents/templates that reference Knowledge should include resolved Knowledge dependencies in root relationship data.

Example shape, adapting to existing lockfile v3 conventions:

```json
{
  "lockfile_version": 3,
  "packages": {
    "knowledge:@zack/python-docs@0.1.0": {
      "kind": "knowledge",
      "name": "@zack/python-docs",
      "version": "0.1.0",
      "integrity": "..."
    }
  },
  "roots": {
    "local:agent": {
      "tools": [],
      "skills": [],
      "knowledge": [
        "knowledge:@zack/python-docs@0.1.0"
      ],
      "reserved": {
        "memory": [],
        "profiles": []
      }
    }
  }
}
```

Use the existing lockfile shape/pattern actually present in the repo. Do not introduce a new relationship model if the v3 lockfile already uses `roots`.

### Backend / registry constraints
- The physical `tools` table is used for all package kinds. Add `knowledge` anywhere database constraints, Python models, route validation, service code, DTOs, search indexes, and frontend types currently enumerate package kinds.
- Use the existing `tool_versions` / package versions table and S3 layout unless the current implementation strongly requires a new path.
- Existing private namespace access rules apply to Knowledge.
- Publish-time dependency access validation must validate:
  - for `kind: "agent"`: top-level `tools`, `skills`, and `knowledge`
  - for `kind: "template"`: `template.dependencies.tools`, `.agents`, `.skills`, and `.knowledge`
  - for `kind: "skill"`: top-level `tools`
  - for `kind: "knowledge"`: no dependencies
- Publish-time validation for `kind: "knowledge"` must include a Knowledge build-check. The registry should reject unbuilt or stale Knowledge packages even if the CLI fails to catch them. Backend validation should verify required mode-specific derived metadata is present in the manifest and that required packaged files exist in the uploaded artifact. The backend does not need to rebuild indexes, but it should reject obviously incomplete Knowledge artifacts.
- Malware scanning, yanking, signing, namespace signer policy, and entitlement enforcement apply to Knowledge packages the same way as other package kinds.
- Consider artifact size. Knowledge packages may be significantly larger than tools/agents/skills/templates. The existing artifact max is large, but tar entry caps, blocked embedded archive rules, readme/license caps, and upload/download UX should be reviewed.

### Search, trending, and registry UX
Knowledge packages must appear in registry search, package detail pages, namespace package lists, profile package lists, activity, and any package-kind filters.

Knowledge detail page should emphasize:
- description and README
- mode: `context` or `vector`
- for context mode: document count, total bytes, declared document list/content types where appropriate
- for vector mode: chunk count and source count
- for vector mode: embedding provider/model
- for vector mode: dimensions
- for vector mode: metric
- for vector mode: normalized flag
- for vector mode: retrieval defaults
- for vector mode: index status/type
- license/provenance
- install command
- inspect command
- example query command only for vector-mode packages
- private/public visibility and signatures consistent with other package pages

Do not present Knowledge as executable.

### SDK constraints
Phase 6B SDK support may be metadata/path-first rather than full retrieval.

Minimum:
- package-kind models recognize `knowledge`
- installed agent loaders include resolved Knowledge metadata/paths alongside resolved tools and skills if such agent loaders exist
- SDKs expose a way to locate/read installed Knowledge artifact metadata if consistent with existing `load_agent`/`loadSkill` patterns
- do not add SDK-managed provider billing or provider keys
- do not overload callable `load(...)` tool APIs to return Knowledge objects

Optional but useful:
- Python `load_knowledge(...)`
- Node `loadKnowledge(...)`
- loaded Knowledge object includes manifest, package path, mode, context document metadata when `mode: "context"`, and chunks/sources paths, embedding metadata, index metadata, and retrieval defaults when `mode: "vector"`
- direct SDK query support can be deferred unless straightforward

## Acceptance criteria
- `kind: "knowledge"` is valid in `agent.json` with a required Knowledge contract object.
- Existing `agent`, `tool`, `template`, and `skill` manifests continue to validate.
- Agent manifests can reference Knowledge packages using top-level `knowledge`.
- Template manifests can reference Knowledge packages using `template.dependencies.knowledge`.
- Skill manifests cannot reference Knowledge packages.
- Knowledge manifests cannot declare dependencies.
- `agentpm init --kind knowledge` creates a valid starter manifest and directory layout.
- `agentpm knowledge build` supports `mode: "context"` by validating declared documents and computing document metadata/hashes without requiring chunks, embeddings, vectors, or indexes.
- `agentpm knowledge build` supports `mode: "vector"` by validating chunks, sources, embeddings, vector dimensions/counts, safe paths, and required files.
- `agentpm knowledge build` generates the default AgentPM local index from vectors for `mode: "vector"`.
- `agentpm knowledge build` updates derived metadata in `agent.json` for the selected mode.
- Knowledge packages must be built before publishing.
- `agentpm publish` performs a build-check for Knowledge packages and fails if required build-derived metadata is missing or stale.
- `agentpm publish` does not mutate `agent.json`, compute missing build metadata, or generate Knowledge indexes by default.
- For context-mode packages, publish verifies declared document hashes, byte counts, document count, total bytes, and aggregate content hash.
- For vector-mode packages, publish verifies chunk/source/vector counts and hashes, vector dimensions, generated `agentpm-local` index presence, and index metadata freshness.
- `agentpm publish --dry-run` succeeds for valid built context-mode and vector-mode Knowledge packages and fails for unbuilt or stale Knowledge packages.
- `agentpm publish` supports Knowledge packages through the registry publish flow.
- Backend publish validation, DB constraints, and package kind normalization accept Knowledge.
- Install resolve/init/finalize support Knowledge.
- Direct `agentpm install @ns/knowledge-pkg` installs to `.agentpm/knowledge/...`.
- Installing an agent that references Knowledge resolves, installs, and locks Knowledge dependencies.
- Installing/generating a template that references Knowledge resolves, installs, and locks Knowledge dependencies.
- `agent.lock` includes Knowledge packages and root relationship entries.
- `agentpm knowledge inspect` works for local and installed context-mode and vector-mode Knowledge packages.
- `agentpm knowledge query <ref> --vector-json <file>` returns ranked chunks with scores and source metadata for vector-mode packages.
- `agentpm knowledge query` fails clearly for context-mode packages because they do not include vector indexes.
- `agentpm knowledge query <ref> --embedding-command <cmd> "query text"` works if adapter support is implemented in the milestone.
- `agentpm knowledge query <ref> "query text"` uses a built-in adapter only when supported and configured, and otherwise fails with a clear error telling the user to pass a vector or adapter.
- Search, trending/package lists, package detail pages, namespace pages, and profile pages recognize Knowledge.
- Private namespace access rules apply consistently to private Knowledge packages.
- Existing tool, agent, template, and skill publish/install/search flows continue to pass.
- Documentation and examples explain what AgentPM does and does not do for Knowledge artifacts.

## Risks / edge cases
- Top-level `knowledge` means two different things depending on `kind`; schema changes can easily make agent dependency arrays and Knowledge contract objects conflict.
- Existing semantic warnings say agent `knowledge` is preserved but not resolved. Those warnings must be removed or changed when Phase 6B resolves Knowledge.
- Missing enum updates across CLI, SDKs, backend, DB check constraints, frontend filters, and tests can cause Knowledge packages to disappear or render as tools.
- Knowledge artifacts may be much larger than prior package kinds. Upload/download time, S3 size, tar entry limits, malware scan time, and progress output need review.
- Context-mode artifacts make it easier to package whole documents, which can increase package size and downstream context-window/token usage even without embeddings.
- Raw float32 vector parsing is easy to get wrong across endianness and dimensionality.
- If vector row order drifts from chunk row order, retrieval returns wrong chunks. Build must make the invariant explicit and validate all count/dimension conditions it can.
- AgentPM cannot prove that vectors came from the declared model. It can only validate structural compatibility and preserve author-provided metadata.
- Querying with a vector from the wrong model may return low-quality results even if dimensions match. Fail on explicit provider/model metadata mismatches when provided.
- Shell embedding adapters can leak environment variables if users configure them that way. AgentPM should not inject registry tokens into adapter execution.
- Built-in provider adapters can make AgentPM feel vendor-specific. Keep them optional and BYO-token only.
- Prompt injection in retrieved chunks is possible. Docs should warn that retrieved Knowledge is context, not trusted instruction.
- Source/license/provenance metadata may be inaccurate. AgentPM should preserve and display it but not claim legal verification.
- Backend export/import is deferred, but vector-mode manifests must preserve enough canonical data for future exporters. Context-mode manifests must preserve enough document metadata for future context-loading runtimes.
- Local index implementation might evolve. Publicly expose `agentpm-local`, not an implementation-specific format, unless necessary.
- Existing tar packaging blocks embedded archives. Optional `knowledge/documents/` may contain archives; keep the existing safety rule unless there is a deliberate exception.
- Checking only for the presence of build-derived fields is not enough. Publish must recompute and compare enough metadata to detect stale documents, stale vectors, and stale indexes.
- Vector index freshness is hard to prove from the index directory alone. `agentpm knowledge build` should write index metadata that records source corpus/vector hashes, dimensions, and vector count so publish can detect stale indexes without rebuilding them.
- If publish silently mutates Knowledge packages, authors may accidentally publish generated files they did not review. Phase 6B should keep build explicit and publish check-only.

## Open questions
- Should `agentpm init --kind knowledge` default to `mode: "context"`, or should it require/accept a mode flag? Recommendation: default to context mode for lowest-friction authoring and optionally add a vector starter flag/template.
- Should Phase 6B add `agentpm knowledge read` for context-mode packages, or is `inspect` plus runtime attachment enough for the MVP?
- Which local index implementation should be used internally for `agentpm-local`?
- Should Phase 6B require `metric: "cosine"` and `normalized: true`, or allow additional metrics if the chosen index supports them?
- Should `--embedding-command` be implemented in the first milestone with `query`, or should Phase 6B first ship `--vector-json` and add command adapters in a follow-up milestone?
- Should a built-in OpenAI adapter ship in Phase 6B, or should the first release stay fully adapter/vector-only?
- Should raw `--vector <file.f32>` be included in Phase 6B or deferred in favor of easier-to-debug `--vector-json`?
- Should `knowledge build` support `--check` to validate without writing derived manifest/index files?
- Should Phase 6B add an explicit `agentpm publish --build` or `--prepare` flag later? Recommendation: not in the MVP. Publish should be check-only by default and fail with instructions to run `agentpm knowledge build`.
- Should Knowledge detail pages show retrieved sample chunks or only metadata? Recommendation: metadata only in Phase 6B.
- Should SDKs include `load_knowledge` / `loadKnowledge` in Phase 6B or defer SDK-specific loading until after CLI/registry flows are stable?
- Should package refs allow same name across different kinds in the same namespace if existing DB constraints do not already support it?
- Should `knowledge.provenance.generated_at` be required after build, or optional?
- Should `content_type`, `language`, and `license` remain free-form strings/objects for interoperability, or should recommended values be documented?

## Related Specs
- Existing manifest schema: `schemas/agentpm.manifest.schema.json`
- Existing CLI manifest parsing and lint implementation
- Existing CLI init implementation
- Existing CLI publish implementation
- Existing CLI install/lockfile v2 implementation
- Existing CLI new/template workspace implementation
- Existing CLI run command for tool execution, as analogy for Knowledge query
- Existing backend publish/install service
- Existing shared package data model using `tools` and `tool_versions` tables
- Existing registry search service and `tool_search_index` materialized view
- Existing registry package detail/search/frontend pages
- Phase 4 workflow templates
- Phase 5 private namespaces and package access rules
- Phase 6A Skills as First-Class Artifacts
- Future Phase 7 loop/harness work, where installed Knowledge can be attached to agents at runtime
