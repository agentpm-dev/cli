{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "memory",
  "name": "{{MEMORY_NAME}}",
  "version": "0.1.0",
  "description": "{{MEMORY_DESCRIPTION}}",
  "readme": "README.md",
  "memory": {
    "scopes": {
      "user": {
        "description": "The user whose memory is being retained."
      }
    },
    "record_types": {
      "user_preference": {
        "version": "1.0.0",
        "description": "Durable structured preferences for one user.",
        "schema": "schemas/user-preference.schema.json"
      }
    },
    "spaces": {
      "profile": {
        "description": "The current durable profile for one user.",
        "model": "document",
        "record_types": ["user_preference"],
        "scope": ["user"],
        "retrieval": {
          "modes": ["key"]
        }
      }
    }
  }
}
