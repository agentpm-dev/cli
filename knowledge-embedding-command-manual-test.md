# Knowledge Embedding Command Manual Test

This is an internal manual test flow for `agentpm knowledge query --embedding-command`.

It rebuilds a tiny vector-mode Knowledge package from scratch, uses the existing OpenAI helper script as the embedding adapter, and verifies that text queries return real ranked results.

## Prerequisites

- `OPENAI_API_KEY` set in your shell
- `python3` available locally
- the local CLI runnable from this repo

Quick sanity checks:

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm

cargo run -p agentpm-cli -- --help
python3 scripts/knowledge_openai_embeddings.py --help
python3 scripts/knowledge_openai_embeddings.py adapter --help
```

## 1. Create a fresh scratch package

```bash
mkdir -p /tmp/agentpm-knowledge-embedding-command
cd /tmp/agentpm-knowledge-embedding-command
rm -rf docs-search

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  init --kind knowledge --mode vector --name docs-search --description "Embedding command manual test package"

cd docs-search
```

## 2. Create a tiny real-text corpus

Replace `knowledge/chunks.jsonl` with:

```jsonl
{"id":"chunk_quickstart","source_id":"src_readme","text":"AgentPM CLI supports init, install, publish, and lint flows for packages. The quick start shows how to initialize tool, skill, agent, template, and knowledge packages from the command line."}
{"id":"chunk_templates","source_id":"src_readme","text":"Workflow templates generate editable AgentPM workspaces. agentpm new copies scaffold files, resolves declared dependencies, writes agent.json, agentpm.workspace.json, and agent.lock, and does not execute generated code during scaffolding."}
{"id":"chunk_knowledge_cli","source_id":"src_knowledge_docs","text":"The knowledge command supports build, inspect, and query flows. Vector-mode packages declare chunks, sources, embeddings, and retrieval metadata, then agentpm knowledge build validates the corpus and writes local index metadata."}
```

Replace `knowledge/sources.jsonl` with:

```jsonl
{"id":"src_readme","title":"AgentPM CLI README","uri":"file:///Users/zackhine/projects/agentpm-project/agentpm/README.md"}
{"id":"src_knowledge_docs","title":"Knowledge CLI Docs","uri":"file:///Users/zackhine/projects/agentpm-project/agentpm-api/docs/v0.1/cli/knowledge.mdx"}
```

Replace `agent.json` with:

```json
{
  "kind": "knowledge",
  "name": "docs-search",
  "version": "0.1.0",
  "description": "Embedding command manual test package",
  "knowledge": {
    "mode": "vector",
    "corpus": {
      "chunks_path": "knowledge/chunks.jsonl",
      "sources_path": "knowledge/sources.jsonl"
    },
    "embedding": {
      "id": "default",
      "provider": "openai",
      "model": "text-embedding-3-small",
      "dimensions": 1536,
      "metric": "cosine",
      "normalized": true,
      "vectors_path": "knowledge/embeddings/default.f32"
    },
    "retrieval": {
      "strategy": "vector",
      "default_top_k": 3
    }
  }
}
```

## 3. Generate the corpus vectors

Use the helper script to embed each chunk’s text and write the raw float32 vector file:

```bash
python3 /Users/zackhine/projects/agentpm-project/agentpm/scripts/knowledge_openai_embeddings.py chunks-to-f32 \
  --chunks knowledge/chunks.jsonl \
  --output knowledge/embeddings/default.f32
```

Expected result:

- the command succeeds
- `knowledge/embeddings/default.f32` is created
- stderr prints something like `Wrote 3 vectors x 1536 dimensions`

## 4. Build and inspect the package

```bash
cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- knowledge build

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- knowledge inspect .

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- knowledge inspect . --json
```

Expected checks:

- inspect reports `Mode: vector`
- inspect shows chunk count `3`
- inspect shows the OpenAI model and `1536` dimensions
- inspect reports a fresh local index

## 5. Query with the embedding adapter

Run a templates-oriented query:

```bash
cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  knowledge query . "How does agentpm new handle templates and generated workspaces?" \
  --embedding-command "python3 /Users/zackhine/projects/agentpm-project/agentpm/scripts/knowledge_openai_embeddings.py adapter" \
  --top-k 2 \
  --include-text \
  --include-metadata
```

Expected result:

- the query succeeds
- `chunk_templates` ranks first or near first
- output includes score, row, chunk ID, source ID, title, URI, and text

Good sample shape:

```text
Knowledge query: docs-search@0.1.0
Target: .
Search: agentpm-local local exact search (metric=cosine, normalized=true)
Results: 2
1. score=... row=1 chunk=chunk_templates source=src_readme
```

Run a second query focused on the knowledge command itself:

```bash
cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  knowledge query . "What does agentpm knowledge build validate for vector packages?" \
  --embedding-command "python3 /Users/zackhine/projects/agentpm-project/agentpm/scripts/knowledge_openai_embeddings.py adapter" \
  --top-k 2 \
  --include-text \
  --include-metadata \
  --json
```

Expected result:

- the query succeeds
- `chunk_knowledge_cli` ranks first or near first
- JSON output includes `results[]` rows with `row`, `score`, `chunk_id`, `source_id`, `source_title`, `source_uri`, and `text`

## 6. Failure-path spot checks

### Missing query text

```bash
cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  knowledge query . \
  --embedding-command "python3 /Users/zackhine/projects/agentpm-project/agentpm/scripts/knowledge_openai_embeddings.py adapter"
```

Expected result:

- fails clearly
- message mentions that `--embedding-command` requires query text input

### Invalid adapter command

```bash
cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  knowledge query . "test query" \
  --embedding-command "python3 /no/such/script.py"
```

Expected result:

- fails clearly
- message indicates adapter execution failed

### Context-mode still fails before adapter execution

From a separate scratch directory:

```bash
cd /tmp/agentpm-knowledge-embedding-command
rm -rf context-playbook

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  init --kind knowledge --mode context --name context-playbook --description "Context package"

cd context-playbook

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- knowledge build

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  knowledge query . "How does handoff work?" \
  --embedding-command "python3 /Users/zackhine/projects/agentpm-project/agentpm/scripts/knowledge_openai_embeddings.py adapter"
```

Expected result:

- fails clearly because the package is `mode="context"`
- message says it is intended for direct context loading

## 7. Optional installed-package check

If you publish this package to your test namespace, you can verify the installed-package path too:

```bash
mkdir -p /tmp/agentpm-knowledge-embedding-install
cd /tmp/agentpm-knowledge-embedding-install

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  install @zack/docs-search@0.1.0

cargo run --manifest-path /Users/zackhine/projects/agentpm-project/agentpm/Cargo.toml -p agentpm-cli -- \
  knowledge query @zack/docs-search "How does agentpm new handle templates?" \
  --embedding-command "python3 /Users/zackhine/projects/agentpm-project/agentpm/scripts/knowledge_openai_embeddings.py adapter" \
  --top-k 2 \
  --include-text
```

Expected result:

- the package ref resolves through `.agentpm/knowledge/...`
- results are consistent with the local-package query

## Notes

- This uses the same helper script both for corpus vector generation and for query-time adapter execution.
- The helper script is only for local manual testing and examples. It is not part of the CLI contract.
- `--embedding-command` executes argv directly. Keep commands simple and explicit rather than relying on shell features.
