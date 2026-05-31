# Feature
Workflow Templates

## Problem / Goal
AgentPM currently supports publishing and installing individual agent-native packages such as tools and agents. This is useful for reusable building blocks, but users still face blank-page friction when they want to build a complete working agent system. They must decide which tools to install, how to structure the project, how to wire SDK or CLI usage, how to document environment variables, and how to expose tools through MCP.

Workflow Templates solve this by introducing a first-class, publishable `template` artifact kind that scaffolds complete, editable AgentPM workspaces. A template can generate a local project with starter code, documentation, environment guidance, a primary `agent.json`, optional supporting files, and declared AgentPM dependencies.

The goal is to make AgentPM useful not only for installing individual packages, but also for bootstrapping practical starting points such as:

- Python SDK research assistants
- Node SDK triage workers
- CLI/cron-style automation workers
- MCP tool server workspaces
- Multi-agent workspace examples that install multiple agent packages as workspace dependencies without changing the normal agent manifest contract

A successful implementation should let a user run something like:

```bash
agentpm new @agentpm/research-assistant-python my-research-agent
cd my-research-agent
agentpm install --frozen
python main.py "AgentPM"
```

The generated project should be understandable, editable, lockfile-backed, and aligned with the existing AgentPM execution surfaces.

## Non-goals
This phase does not introduce a workflow runtime.

This phase does not add recursive agent dependencies to `kind: "agent"` manifests. Agent manifests must not gain an `agents` array in this phase.

This phase does not make agents orchestrate other agents at runtime.

This phase does not add template hooks such as `prompt.js`, `hooks.py`, `pre_generate`, or `post_generate`.

This phase does not execute template-provided code during scaffolding.

This phase does not run generated source code, `npm install`, `pip install`, shell scripts, package-manager lifecycle commands, or arbitrary commands declared by the template.

This phase does not add slots, adapters, generated glue code, interface mapping, or swappable tool contracts.

This phase does not add a visual template builder.

This phase does not add hosted execution for generated workflows.

This phase does not add template update, migration, diff, or re-apply behavior.

This phase does not make Skills, Knowledge, Memory, or Profiles fully publishable first-class artifact kinds. Existing placeholder fields may continue to be reserved in generated `agent.json` files, but they are not resolved or installed as real package types in this phase.

## Constraints / Invariants

### Manifest and schema
- The manifest file remains `agent.json`.
- Add `template` as a valid top-level `kind` alongside existing `agent` and `tool` kinds.
- Template-specific metadata lives under a top-level `template` object.
- Existing `kind: "agent"` behavior remains compatible:
  - agents continue to require `tools`.
  - agents may keep top-level `skills`, `knowledge`, `memory`, and `profiles` placeholder arrays.
  - agents do not gain `agents` dependencies in this phase.
- Existing `kind: "tool"` behavior remains compatible.
- The schema must continue to reject unexpected fields unless explicitly supported.

### Template artifact model
A template artifact is a package that contains:

- an `agent.json` with `kind: "template"`
- scaffold files under a declared template files root
- optional README/license content using the existing publish flow conventions
- declarative template metadata
- declared AgentPM dependency roots for the generated project

Suggested template manifest shape:

```json
{
  "kind": "template",
  "name": "research-assistant-python",
  "version": "0.1.0",
  "description": "Python SDK starter for a local research assistant.",
  "template": {
    "display_name": "Python Research Assistant",
    "use_case": "research",
    "execution_surfaces": ["python-sdk"],
    "stack": ["python"],
    "files_root": "template",
    "variables": [
      {
        "name": "project_name",
        "description": "Generated project name",
        "required": true,
        "default": "research-assistant"
      }
    ],
    "dependencies": {
      "tools": [
        {
          "name": "@zack/web-page-extract",
          "version": "0.1.2"
        },
        {
          "name": "@zack/summarize-text",
          "version": "0.1.8"
        }
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

Supported `template.execution_surfaces` values for this phase:

- `python-sdk`
- `node-sdk`
- `agentpm-run`
- `agentpm-serve-mcp`
- `multi-agent-workspace`

Supported `template.dependencies` values for this phase:

- `tools`: package refs to install into the generated workspace
- `agents`: package refs to install into the generated workspace as install roots

Do not support `template.dependencies.skills`, `template.dependencies.knowledge`, `template.dependencies.memory`, or `template.dependencies.profiles` as resolved dependency types in this phase.

### Security boundary
`agentpm new` may:

- resolve and download a template artifact
- validate the template manifest
- copy template files into the target directory
- render declared variables into copied text files
- install declared AgentPM tool and agent dependencies using AgentPM’s existing install path
- write the generated project’s `agent.json` and `agent.lock`
- print next-step commands from `template.entrypoints`

`agentpm new` must not:

- execute template-provided code
- execute lifecycle hooks
- run generated application code
- run package-manager install commands
- shell out to arbitrary template-defined commands
- automatically trust or execute scripts included in the template artifact

Templates may contain executable source files, shell scripts, package manifests, and example commands, but those files are only copied/rendered. The user chooses whether to inspect and run them afterward.

### Publish and registry constraints
- Existing package publish flow should support `kind: "template"`.
- `packages:publish` can publish `tool`, `agent`, and `template` packages.
- Legacy `tools:publish` scope remains valid only for `tool` packages.
- Publishing a package name under one kind and later publishing the same namespace/name under a different kind must remain a kind conflict.
- Template artifacts go through the same artifact validation, upload, signing, attestation, and scan pipeline as other package kinds unless explicitly impossible.
- Any tool-specific UI/cache behavior should not break template publishing.

### Install and lockfile constraints
- Existing install endpoints may be reused for downloading template artifacts.
- Install normalization must accept `template` where required for `agentpm new` to resolve/download the template artifact.
- `_resolve_install_graph` must not treat templates like agents.
- Templates must not recursively expand dependencies during normal install graph resolution.
- Agent packages may continue to expand their own tool dependencies through existing agent → tools behavior.
- For `agentpm new`, the generated project’s final `agent.lock` should primarily reflect the generated workspace’s runnable dependencies, not the template artifact itself.
- Template artifact metadata may be recorded in generated README or optional generated metadata, but the generated project should not permanently depend on the template package unless a future update/migration feature is introduced.
- Multi-agent templates install multiple agent packages as workspace/root dependencies. They do not add recursive agents to `kind: "agent"` manifests.

### Generated project constraints
A generated project should generally contain:

```text
project-name/
  agent.json
  agent.lock
  .agentpm/
  README.md
  .env.example
  src/ or scripts/ or other starter files depending on template
```

The generated root `agent.json` must be valid under the normal `kind: "agent"` schema. It should use `tools`, `skills`, `knowledge`, `memory`, and `profiles` fields as supported by the current manifest schema. It must not include an `agents` array.

Example generated root manifest:

```json
{
  "kind": "agent",
  "name": "my-research-assistant",
  "version": "0.1.0",
  "description": "Generated from @agentpm/research-assistant-python.",
  "tools": [
    {
      "name": "@zack/web-page-extract",
      "version": "0.1.2"
    },
    {
      "name": "@zack/summarize-text",
      "version": "0.1.8"
    }
  ],
  "skills": [],
  "knowledge": [],
  "memory": [],
  "profiles": []
}
```

### CLI constraints
Add a new command:

```bash
agentpm new <template-ref> [target-dir]
```

Expected behavior:

1. Resolve the template ref.
2. Download the template artifact using the existing artifact download/session flow if practical.
3. Validate that the downloaded package manifest has `kind: "template"`.
4. Validate the `template` object.
5. Determine target directory:
   - use `[target-dir]` when provided.
   - otherwise use `template.variables.project_name.default` or the template package name.
6. Refuse to write into a non-empty directory unless an explicit force flag is implemented.
7. Prompt for required variables if interactive prompting exists in the CLI.
8. Support non-interactive variable passing if practical, for example:

```bash
agentpm new @agentpm/research-assistant-python my-agent --var project_name=my-agent
```

9. Copy/render files from `template.files_root` into the target directory.
10. Install declared `template.dependencies.tools` and `template.dependencies.agents` into the generated project.
11. Write/update `agent.lock` for generated runnable dependencies.
12. Print next steps and entrypoints.

The exact flag names may follow existing CLI style, but the user-facing flow should remain centered on `agentpm new`.

### Registry constraints
Templates should be searchable and browsable in the registry/site.

Template pages should have slightly different treatment than tools because users are browsing outcomes, not just callable packages.

A template detail page should show:

- template name
- description
- use case
- execution surfaces
- stack
- included tool dependencies
- included agent dependencies
- bootstrap command
- next-step commands / entrypoints
- README content
- version and publish metadata
- scan/signing status where already supported

Search/filtering should support templates as a distinct artifact type.

### Examples constraints
The examples repo should include official workflow templates covering all current execution surfaces:

1. Python Research Assistant
2. Node Triage Worker
3. CLI Automation Worker
4. MCP Tool Server
5. Multi-Agent Support Workspace

These examples should be real templates using `kind: "template"`, not just documentation pages.

## Acceptance criteria
- The manifest schema accepts `kind: "template"` with a valid `template` object.
- The manifest schema continues to accept existing valid `kind: "agent"` and `kind: "tool"` manifests unchanged.
- The manifest schema continues to reject agents that try to add unsupported recursive agent dependency fields.
- Publishing a template package succeeds with `packages:publish` scope.
- Publishing a template package fails with legacy `tools:publish` scope only.
- Publishing a template with a namespace/name that already exists as a tool or agent returns a kind conflict.
- Install/download support can retrieve a template artifact for `agentpm new`.
- Normal install graph resolution does not recursively expand template dependencies.
- Agent package install graph resolution still expands tool dependencies from agent manifests.
- `agentpm new <template-ref> [target-dir]` creates a local project from the template artifact.
- `agentpm new` refuses to scaffold from a package whose manifest kind is not `template`.
- `agentpm new` copies/renders files from the declared `template.files_root`.
- `agentpm new` does not execute template-provided code or scripts.
- `agentpm new` installs declared template tool dependencies into the generated project.
- `agentpm new` installs declared template agent dependencies as workspace/root dependencies where supported.
- The generated project receives a runnable-dependency `agent.lock` that does not treat the template artifact as a permanent dependency.
- The generated root `agent.json` is schema-valid as `kind: "agent"` and does not include `agents` dependencies.
- Registry search supports template artifacts.
- Registry template detail pages show use case, stack, execution surfaces, dependencies, and bootstrap command.
- Official examples exist for the Python SDK, Node SDK, `agentpm run`, `agentpm serve --mcp`, and a multi-agent workspace scaffold.
- Each official example must be runnable or manually verifiable using the commands documented in its README.
- The multi-agent example must use template-level agent dependencies only; `kind: "agent"` manifests must not gain recursive `agents[]` dependencies.
- At least one generated example project can be smoke-tested through each relevant execution surface.

## Risks / edge cases
- Template artifacts could contain malicious code. Mitigation: `agentpm new` must copy/render only and never execute template-provided code. Security scans that are called on tools and agents must also continue to be called on templates.
- Users may assume entrypoint commands are executed automatically. Mitigation: output and docs must clearly show them as next steps.
- Template dependency installation may accidentally treat templates like agents and expand dependencies unexpectedly. Mitigation: explicitly branch template handling in install graph logic.
- Multi-agent templates may imply recursive agent orchestration. Mitigation: docs and schema must make clear that multiple agents are workspace dependencies, not nested `agent` dependencies.
- Generated projects could overwrite user files. Mitigation: fail on existing non-empty target directories unless force behavior is deliberately added.
- Variable rendering could corrupt binary files. Mitigation: render only text files or define a safe text-file detection/allowlist.
- Template files may include path traversal entries. Mitigation: reuse or add safe extraction/copy path validation.
- Existing UI assumptions may route unknown non-tool kinds to tool pages. Mitigation: add template-specific routes and fallback handling.
- Tool-specific cache invalidation may not refresh template views. Mitigation: update package-aware cache/indexing paths.
- Scan pipeline may have tool-specific labels or assumptions. Mitigation: make scan display package-kind-aware.
- Template dependencies may reference unavailable/yanked package versions. Mitigation: fail with normal install resolution errors and do not leave a partially initialized project without clear messaging.
- Generated lockfiles may differ from existing lockfile version handling. Mitigation: reuse the current lockfile writer where possible.
- Examples may depend on packages that are not seeded/published. Mitigation: either publish required official packages first or use existing known packages.

## Open questions
- Should `agentpm new` support interactive prompts in the first implementation, or should it start with defaults plus `--var key=value` for non-interactive generation? Yes it should allow either way.
- Should `agentpm new` have a `--force` flag for non-empty directories in this phase, or should that be deferred? It should be deferred.
- Should generated projects include a `generatedFrom` metadata file, for example `.agentpm/template.json`, or should the template origin only appear in README/description? Yes, a template.json. Should not be included in agent.lock though. Variables should only be recorded if they are explicitly non-secret. Since we’re saying template variables are generation-time scaffold values, not runtime secrets, that is okay. But the spec should still say: do not put API keys/tokens/passwords in template.variables; use .env.example and runtime environment variables instead
- Should the template artifact itself ever appear in a transient lockfile during generation, or should it only be downloaded as an implementation detail? It shuold not.
- Should template registry pages live under `/templates/...`, or should the existing package routing become kind-aware with separate tabs/routes? Should live under /templates/
- Should `template.execution_surfaces` be strict enum-validated now, or flexible strings to avoid schema churn? They should be enums for now, maybe change it later.
- Should `template.stack` be strict enum-validated now, or flexible strings for registry filtering? Enums for now.
- Should the initial implementation render variables in all text files, or only files with explicit placeholder syntax/extension allowlist? Any files provided by the template creator.

## References
- Existing AgentPM manifest schema / `agent.json` contract.
- Existing install and lockfile behavior.
- Existing publish/install provenance and integrity verification.

## Related Specs
