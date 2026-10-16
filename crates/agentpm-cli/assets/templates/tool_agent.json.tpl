{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "tool",
  "name": "{{TOOL_NAME}}",
  "version": "0.1.0",
  "description": "{{TOOL_DESCRIPTION}}",
  "runtime": { "type": "python", "version": "3" },
  "files": ["main.py"],
  "entrypoint": {
    "command": "python3",
    "args": ["main.py"]
  },
  "inputs": {},
  "outputs": {}
}
