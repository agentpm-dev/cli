{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "tool",
  "name": "{{TOOL_NAME}}",
  "version": "0.1.0",
  "description": "{{TOOL_DESCRIPTION}}",
  "files": [],
  "entrypoint": {
    "command": "{{TOOL_ENTRYPOINT_COMMAND}}",
    "args": []
  },
  "inputs": {
    "type": "object",
    "properties": {
      "text": {
        "type": "string",
        "description": "Text to process"
      }
    },
    "required": [
      "text"
    ]
  },
  "outputs": {
    "type": "object",
    "properties": {
      "summary": {
        "type": "string",
        "description": "Summarized text"
      }
    },
    "required": [
      "summary"
    ]
  }
}