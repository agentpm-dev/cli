{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "template",
  "name": "{{TEMPLATE_NAME}}",
  "version": "0.1.0",
  "description": "{{TEMPLATE_DESCRIPTION}}",
  "template": {
    "display_name": "{{TEMPLATE_DISPLAY_NAME}}",
    "use_case": "starter",
    "execution_surfaces": ["agentpm-run"],
    "files_root": "template",
    "variables": [
      {
        "name": "project_name",
        "description": "Generated project name. Generation-time only; do not use for API keys, tokens, passwords, or runtime secrets.",
        "required": true,
        "default": "{{PROJECT_NAME_DEFAULT}}"
      }
    ],
    "dependencies": {
      "tools": [],
      "agents": []
    },
    "entrypoints": [
      {
        "label": "Review generated scaffold",
        "command": "cat README.md"
      }
    ]
  }
}
