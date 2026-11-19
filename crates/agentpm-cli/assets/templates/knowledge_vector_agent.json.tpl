{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json",
  "standard": { "id": "agentpm", "version": "1.0.0" },
  "kind": "knowledge",
  "name": "{{KNOWLEDGE_NAME}}",
  "version": "0.1.0",
  "description": "{{KNOWLEDGE_DESCRIPTION}}",
  "knowledge": {
    "mode": "vector",
    "content_type": "documentation",
    "corpus": {
      "chunks_path": "knowledge/chunks.jsonl",
      "sources_path": "knowledge/sources.jsonl"
    },
    "embedding": {
      "id": "default",
      "provider": "custom",
      "model": "unknown",
      "dimensions": 1536,
      "metric": "cosine",
      "normalized": true,
      "vectors_path": "knowledge/embeddings/default.f32"
    },
    "retrieval": {
      "strategy": "vector"
    }
  }
}
