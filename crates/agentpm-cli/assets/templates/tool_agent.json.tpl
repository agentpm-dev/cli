{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json",
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
