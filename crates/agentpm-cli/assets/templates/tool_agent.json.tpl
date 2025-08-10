{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "kind": "tool",
  "name": "{{TOOL_NAME}}",
  "version": "0.1.0",
  "description": "{{TOOL_DESCRIPTION}}",
  "entrypoint": "{{TOOL_ENTRYPOINT}}",
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