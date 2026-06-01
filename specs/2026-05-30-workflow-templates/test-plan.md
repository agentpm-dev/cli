# Test Plan

## Required verification
Before this work is considered done, verify the feature across schema, backend publish/install APIs, CLI generation, registry UX, and official examples.

The most important end-to-end verification is:

1. Publish or seed a valid `kind: "template"` package.
2. Run `agentpm new <template-ref> <target-dir>`.
3. Confirm the generated project contains expected files.
4. Confirm the generated root `agent.json` is valid and does not include recursive `agents` dependencies.
5. Confirm the generated `agent.lock` contains runnable tool/agent dependencies but does not make the template artifact a permanent dependency.
6. Confirm the generated project can use the intended execution surface.
7. Confirm no template-provided code executes during scaffolding.

## Automated checks
Run the existing automated checks for each touched repo. Use exact repo commands where they exist. Suggested commands:

### CLI / Rust
```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Add or update Rust tests for:

- manifest schema validation for `kind: "template"`
- deserializing template manifests
- `agentpm lint` with valid and invalid templates
- `agentpm new` argument parsing
- `agentpm new --var key=value` parsing if implemented
- target directory safety
- refusing non-template packages
- refusing non-empty target directories
- safe template extraction/copy behavior
- no hook/script execution during scaffolding
- generated project lockfile behavior

### Registry / Flask API
```bash
pytest
ruff check .
black --check .
mypy .
```

Use the actual project commands if they differ.

Add or update backend tests for:

- publishing a template with `packages:publish`
- rejecting template publishing with legacy `tools:publish`
- preserving tool publishing with `tools:publish`
- kind conflict across tool/agent/template packages
- publish finalize receipt for templates
- install resolve accepting template roots where needed
- install resolve not expanding template dependencies like agent tools
- install resolve still expanding agent tool dependencies
- install init returning downloadable template artifacts
- search/index API returning templates as template packages

### Registry / Next.js
```bash
pnpm lint
pnpm test
pnpm build
```

Use the actual project commands if they differ.

Add or update frontend checks for:

- template search result cards
- template detail page rendering
- bootstrap command display
- execution surface display
- use case and stack display
- dependency list display
- no regression in tool detail pages
- no regression in agent detail pages

### Examples repo
Run whatever validation exists for examples. At minimum:

```bash
find . -name agent.json -print
agentpm lint path/to/template/agent.json
```

If the CLI does not support linting arbitrary paths, run validation through the repo’s existing schema validation command or add one.

For Node examples:

```bash
pnpm install --frozen-lockfile
pnpm build
pnpm test
```

For Python examples:

```bash
python -m compileall .
python -m pytest
```

Use per-template commands documented in each example README.

## Manual checks

### Template publishing
- Publish a valid template package in a local/dev environment.
- Confirm the publish receipt reports `kind: "template"`.
- Confirm the template appears in the registry as a template, not as a tool.
- Confirm the template detail page has the correct bootstrap command.

### Template generation
- Run:

```bash
agentpm new @namespace/template-name my-generated-project
```

- Confirm `my-generated-project/` is created.
- Confirm expected scaffold files are present.
- Confirm `agent.json` is generated or copied correctly.
- Confirm `agentpm.workspace.json` is present for generated workspace-style templates and is treated as committed workspace topology.
- Confirm `agent.lock` is written after dependency installation.
- Confirm `.agentpm/` contains installed runnable dependencies.
- Confirm template origin is documented in README or optional generated metadata if implemented.
- Confirm the target directory is not overwritten if it already contains files.

### Security boundary
- Add a harmless script to a test template that would create a sentinel file if executed.
- Run `agentpm new`.
- Confirm the sentinel file is not created.
- Confirm `agentpm new` did not run package-manager install commands.
- Confirm `agentpm new` did not run generated app code.

### Python SDK template
- Bootstrap the Python Research Assistant template.
- Install any required Python dependencies manually according to README.
- Run the documented command.
- Confirm it calls installed AgentPM tools through the Python SDK.
- Confirm output is written or printed as documented.

### Node SDK template
- Bootstrap the Node Triage Worker template.
- Install Node dependencies manually according to README.
- Run the documented command.
- Confirm it calls installed AgentPM tools through the Node SDK.
- Confirm output is written or printed as documented.

### CLI automation template
- Bootstrap the CLI Automation Worker template.
- Run the documented script.
- Confirm the script uses `agentpm run --input-file` or stdin.
- Confirm output is produced as documented.

### MCP server template
- Bootstrap the MCP Tool Server template.
- Start the server:

```bash
agentpm serve --mcp --host 127.0.0.1 --port 7331
```

- Verify health:

```bash
curl http://127.0.0.1:7331/
```

- Verify initialize:

```bash
curl -X POST http://127.0.0.1:7331/ \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
```

- Verify tools list:

```bash
curl -X POST http://127.0.0.1:7331/mcp \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

- Confirm the listed tools match the generated project lockfile.

### Multi-agent workspace template
- Bootstrap the Multi-Agent Support Workspace template.
- Confirm the template declares multiple agent dependencies under `template.dependencies.agents`.
- Confirm generated root `agent.json` does not include `agents`.
- Confirm `agent.lock` includes installed agent package roots and any transitive tools from those agents.
- Confirm docs explain that this is workspace scaffolding, not recursive agent orchestration.

### Regression checks
- Publish a normal tool package.
- Install a normal tool package.
- Run a normal installed tool with `agentpm run`.
- Publish a normal agent package.
- Install a normal agent package and confirm its tools are still resolved.
- Open an existing tool detail page in the registry.
- Open an existing agent detail page in the registry.

## Expected evidence
Report back with:

- Commands run and whether they passed.
- Schema validation output for valid and invalid templates.
- Backend test output for publish/install behavior.
- CLI test output for `agentpm new`.
- Screenshots or local URLs for template search/detail pages if UI changed.
- Example output from a generated Python SDK project.
- Example output from a generated Node SDK project.
- Example output from the CLI automation template.
- Curl output for MCP `initialize` and `tools/list`.
- Confirmation that a no-execution sentinel script was not run during `agentpm new`.
- Any commands that could not be run, with a reason.

## Out of scope
Do not test:

- Runtime orchestration between multiple agents.
- Recursive `agents` dependencies inside `kind: "agent"` manifests.
- Template hooks or lifecycle scripts.
- Generated glue code or slot/adaptor mappings.
- Hosted workflow execution.
- Template update/migration/re-apply behavior.
- First-class Skills, Knowledge, Memory, or Profiles publishing/installing.
- MCP stdio transport.
