{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "knowledge",
  "name": "{{KNOWLEDGE_NAME}}",
  "version": "0.1.0",
  "description": "{{KNOWLEDGE_DESCRIPTION}}",
  "knowledge": {
    "mode": "context",
    "content_type": "documentation",
    "documents": [
      {
        "path": "knowledge/docs/context.md",
        "content_type": "text/markdown",
        "role": "context",
        "description": "Starter context document."
      }
    ],
    "retrieval": {
      "strategy": "full_context"
    }
  }
}
