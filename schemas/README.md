# AgentPM Manifest Schema

This folder contains the JSON Schema that defines `agent.json` for AgentPM.

- **File:** `agentpm.manifest.schema.json`
- **Kinds:** `tool` (single tool package), `skill` (procedural package), `agent` (composed package), and `template` (workflow starter package)
- **Strictness:** `additionalProperties: false` (unknown fields are rejected)

---

## Quick start

### Generate from the CLI

You can scaffold a valid manifest with the CLI:

```bash
agentpm init --kind tool --name summarize --description "Summarize input text"
```

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/main/schemas/agentpm.manifest.schema.json",
  "kind": "tool",
  "name": "summarize",
  "version": "0.1.0",
  "description": "Summarize input text",
  "entrypoint": {
    "command": "node",
    "args": ["dist/summarize.js"],
    "cwd": ".",
    "timeout_ms": 60000,
    "env": { }
  },
  "inputs": { "type": "object" },
  "outputs": { "type": "object" },
  "files": ["dist/"],
  "runtime": { "type": "node", "version": "20.10" }
}
```

```bash
agentpm init --kind agent --name research-assistant --description "Assistant composed of multiple tools"
```

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "agent",
  "name": "research-assistant",
  "version": "0.1.0",
  "description": "Assistant composed of multiple tools",
  "tools": [],
  "skills": [],
  "knowledge": [],
  "memory": [],
  "profiles": [],
  "examples": [
    {
      "title": "Example prompt",
      "prompt": "Describe the user request this agent should handle."
    }
  ]
}
```

```bash
agentpm init --kind skill --name triage-playbook --description "Procedural guidance for incident triage"
```

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "skill",
  "name": "triage-playbook",
  "version": "0.1.0",
  "description": "Procedural guidance for incident triage",
  "tools": [],
  "skill": {
    "entrypoint": "SKILL.md"
  }
}
```

```bash
agentpm init --kind template --name research-assistant-python --description "Python SDK starter for a local research assistant"
```

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "template",
  "name": "research-assistant-python",
  "version": "0.1.0",
  "description": "Python SDK starter for a local research assistant",
  "template": {
    "display_name": "Research Assistant Python",
    "use_case": "starter",
    "execution_surfaces": ["agentpm-run"],
    "files_root": "template",
    "variables": [
      {
        "name": "project_name",
        "description": "Generated project name. Generation-time only; do not use for secrets.",
        "required": true,
        "default": "research-assistant-python"
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
```

> For fully reproducible validation, **pin to a tag or commit**:
>
> ```
> https://raw.githubusercontent.com/agentpm-dev/cli/v0.1.0/schemas/agentpm.manifest.schema.json
> ```
> or
> ```
> https://raw.githubusercontent.com/agentpm-dev/cli/<COMMIT_SHA>/schemas/agentpm.manifest.schema.json
> ```

---

## Validate locally

Use the built-in linter:
```bash
# In the directory containing agent.json
agentpm lint
```

Or specify a path:
```bash
agentpm lint path/to/agent.json
```

> Editor tips: most IDEs (VS Code, JetBrains) will validate automatically when `$schema` is present.

---

## Field reference (overview)

| Field         | Type     | Required | Allowed on | Notes                                                                               |
|---------------|----------|----------|-----------|-------------------------------------------------------------------------------------|
| `$schema`     | string   | no       | all       | URI to this schema (optional but recommended)                                       |
| `kind`        | enum     | **yes**  | all       | `"tool"`, `"skill"`, `"agent"`, or `"template"` (discriminator)                     |
| `name`        | string   | **yes**  | all       | `^[a-z][a-z0-9-]{0,63}$`                                                            |
| `version`     | semver   | **yes**  | all       | SemVer string (supports pre/metadata)                                               |
| `description` | string   | **yes**  | all       | Free text                                                                           |
| `tools`       | array    | **yes**¹ | **agent**, **skill** | Array of tool refs: string or `{name, version}`                           |
| `skills`      | array    | no       | **agent** | Array of skill refs: string or `{name, version}`                                    |
| `knowledge`   | array    | no       | **agent** | Reserved future refs. Validated and preserved, but not resolved today.              |
| `memory`      | array    | no       | **agent** | Reserved future refs. Validated and preserved, but not resolved today.              |
| `profiles`    | array    | no       | **agent** | Reserved future refs. Validated and preserved, but not resolved today.              |
| `examples`    | array    | no       | **agent** | Inline prompt examples `{ title, prompt }`.                                         |
| `skill`       | object   | **yes**⁴ | **skill** | Skill metadata including `entrypoint`, optional references, scripts, and compatibility |
| `template`    | object   | **yes**³ | **template** | Template metadata including files root, dependencies, variables, and entrypoints |
| `entrypoint`  | string   | **yes**² | **tool**  | Execution ref `{command, args, cwd, timeout_ms, env}`                               |
| `inputs`      | object   | **yes**² | **tool**  | JSON Schema (or shape) for inputs                                                   |
| `outputs`     | object   | **yes**² | **tool**  | JSON Schema (or shape) for outputs                                                  |
| `files`       | string[] | **yes**² | **tool**  | Non-empty list of paths/globs to package                                            |
| `runtime`     | object   | no       | **tool**  | `{ type: "python or node", version: "MAJOR[.MINOR[.PATCH]]" }`                      | 
| `environment` | object   | no       | all       | Dictionary of required or optional env variables `{required, description, default}` | 
| `readme`      | string   | no       | all       | Path to README file. Will automatically look for README.md if not specified.         | 
| `license`     | object   | no       | all       | `{ spdx: "license spdx", file: "Path to LICENSE file" }`                            | 

¹ required only when `kind = "agent"` or `kind = "skill"`  
² required only when `kind = "tool"`
³ required only when `kind = "template"`
⁴ required only when `kind = "skill"`

**Kind-specific rules** are enforced via schema constraints:
- `entrypoint`, `inputs`, `outputs`, `files`, `runtime` ⇒ **only valid on tools**
- `tools` ⇒ **valid on agents and skills**
- `skill` ⇒ **only valid on skills**
- `skills`, `knowledge`, `memory`, `profiles`, `examples` ⇒ **only valid on agents**
- `template` ⇒ **only valid on templates**

---

## Examples

### Tool (single-tool package)

```json
{
  "kind": "tool",
  "name": "summarize",
  "version": "0.1.0",
  "description": "Summarize input text",
  "entrypoint": "dist/summarize.js",
  "inputs": { "type": "object" },
  "outputs": { "type": "object" },
  "files": ["dist/"],
  "runtime": { "type": "node", "version": "20.10" }
}
```

### Agent (composed)

```json
{
  "kind": "agent",
  "name": "research-assistant",
  "version": "0.1.0",
  "description": "Assistant composed of multiple tools",
  "tools": [
    "@acme/summarize@1.2.0",
    { "name": "@acme/translate", "version": "1.0.0" }
  ],
  "skills": [],
  "knowledge": [],
  "memory": [],
  "profiles": [],
  "examples": [
    {
      "title": "Research a topic",
      "prompt": "Read these sources and summarize the main tradeoffs."
    }
  ]
}
```

### Skill (procedural package)

```json
{
  "kind": "skill",
  "name": "triage-playbook",
  "version": "0.1.0",
  "description": "Procedural guidance for incident triage",
  "tools": [
    "@zack/incident-severity@1.0.0"
  ],
  "skill": {
    "entrypoint": "SKILL.md",
    "references": ["references/checklist.md"],
    "scripts": ["scripts/run.sh"]
  }
}
```

### Template (workflow starter package)

```json
{
  "kind": "template",
  "name": "research-assistant-python",
  "version": "0.1.0",
  "description": "Python SDK starter for a local research assistant",
  "template": {
    "display_name": "Python Research Assistant",
    "use_case": "research",
    "execution_surfaces": ["python-sdk"],
    "files_root": "template",
    "variables": [
      {
        "name": "project_name",
        "description": "Generated project name. Generation-time only; do not use for secrets.",
        "required": true,
        "default": "research-assistant"
      }
    ],
    "dependencies": {
      "tools": [
        { "name": "@zack/web-page-extract", "version": "0.1.2" }
      ],
      "agents": []
    },
    "entrypoints": [
      {
        "label": "Run locally",
        "command": "python main.py \"AgentPM\""
      }
    ]
  }
}
```

## Template fields

Template manifests use a top-level `template` object. It defines:

- `display_name`
- `use_case`
- `execution_surfaces`
- `stack`
- `files_root`
- `variables`
- `dependencies.tools`
- `dependencies.agents`
- `entrypoints`

`template.variables` are generation-time scaffold values only. Use them for things like project names and starter copy. Do **not** use them for API keys, tokens, passwords, or runtime secrets. Runtime configuration belongs in `.env.example` and manifest `environment.vars` where applicable.

`template.stack` is enum-constrained in the MMP to: `python`, `node`, `shell`, `mcp`, and `workspace`.

`template.files_root` must be a relative path inside the template artifact. Absolute paths, empty paths, and `..` traversal are rejected.

## Defining `inputs` and `outputs`

Both `inputs` and `outputs` are **JSON Schema (Draft 2020-12)** objects that describe the shape your tool expects/returns. The most common pattern is:

- `type: "object"`
- a `properties` map
- a `required` list
- (optional but recommended) `additionalProperties: false`

```json
"inputs": {
  "type": "object",
  "properties": {
    "text": {
      "type": "string",
      "description": "Text to process"
    }
  },
  "required": ["text"]
},
"outputs": {
  "type": "object",
  "properties": {
    "summary": {
      "type": "string",
      "description": "Summarized text"
    }
  },
  "required": ["summary"]
}
```

## Defining `entrypoint`

```json
 "entrypoint": {
    "command": "node", // Bare interpreter (node|nodejs|python|python3) name or absolute path to it 
    "args": ["dist/summarize.js"], // Path to runnable entry
    "cwd": ".", // optional
    "timeout_ms": 60000, // optional
    "env": { } // optional
  },
```

## Defining `environment`

```json
 "environment": {
    "vars": {
      "OPENAI_API_KEY": {
        "required": true,
        "description": "API key for OpenAI"
      },
      "OPENAI_BASE_URL": {
        "required": false,
        "description": "Custom API endpoint; defaults to https://api.openai.com/v1",
        "default": "https://api.openai.com/v1"
      }
    }
  }
```

If a tool is used without passing a required env variable to the load function, it will fail.

---

## Versioning & stability

- The schema file is versioned in this repo. When making **breaking** changes, copy to a new filename (e.g., `agentpm.manifest.schema.v2.json`) and update docs.
- Generators should point `$schema` to a **tag** (e.g., `v0.1.0`) or **commit** for stability.
- The CLI will evolve to validate manifests against the bundled schema version it supports.

---

## Design notes

- **Strict by default:** `additionalProperties: false` rejects unknown fields.
- **SemVer check:** `version` uses a SemVer-compatible regex.
- **Paths:** `files` entries are strings; treat them as repo-relative paths or globs. Future work may add `exclude`.

---

## Contributing

PRs welcome! If you add fields:
1. Update the schema with constraints.
2. Add examples under `schemas/examples/`.
3. Note whether they’re `agent`-only or `tool`-only.
4. Update this README and the main CLI README as needed.
