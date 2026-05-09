# Feature
AgentPM Phase 2: Interoperability / Ecosystem Glue

## Problem / Goal
AgentPM should act as the source of truth for tool packaging, installation, metadata, and runtime execution while making those tools usable from the broader agent ecosystem.

The goal of this phase is to add interoperability surfaces that let installed AgentPM tools be executed or exposed outside of AgentPM-native SDK usage:

- `agentpm run` provides a universal shell-facing execution surface.
- `agentpm serve --mcp` exposes locked AgentPM tools through a local HTTP MCP server.
- `agentpm export --skill` generates a starter Skill scaffold from `agent.json` metadata, including progressive disclosure and execution through `agentpm run`.

The core product principle is:

> Author and package with AgentPM. Expose through ecosystem adapters when needed. Do not require every ecosystem to fully adopt AgentPM's internal model.

## Non-goals
This phase does not attempt to:

- Make Skills fully first-class publishable/installable AgentPM artifacts yet.
- Build a public third-party plugin system.
- Build WASM plugin loading for adapter extensions.
- Build a hosted or remote MCP service.
- Build an AgentPM-native autonomous agent runtime.
- Add workflow templates.
- Add tool input validation inside `agentpm run`.
- Convert AgentPM manifests bidirectionally into every external ecosystem format.
- Change the existing `agent.json` schema unless implementation reveals a necessary compatibility fix.
- Change install, publish, lockfile, or registry semantics beyond what is required to resolve and run installed tools.

## Constraints / Invariants

### Canonical contract
- `agent.json` remains the canonical source of tool metadata, runtime declaration, entrypoint configuration, inputs, outputs, environment requirements, description, and files.
- The tool creator owns the semantic input/output contract through the manifest and tool implementation.
- `agentpm run` must not impose a new logical response envelope unless the tool itself returns one.

### Installed tool layout
Prepared tools are installed under:

```text
.agentpm/tools/<namespace>/<name>/<version>
```

Download cache artifacts live under:

```text
.agentpm/cache
```

The runner should resolve prepared tools from the installed tool layout and should not execute from the download cache.

### Lockfile behavior
`agent.lock` is the default deterministic project source for unversioned tool specs.

Given:

```json
{
  "dependencies": {
    "@zack/slack-post-message": {
      "version": "0.1.1",
      "integrity": "b72d2c239da44b1e1dfd40b8a6a0a53f958d29ba26f55d9625e85e1652426991"
    }
  },
  "lockfile_version": 1
}
```

Then:

```bash
agentpm run @zack/slack-post-message
```

must resolve to the locked `0.1.1` version when available.

### Tool spec resolution
The runner should support:

```bash
agentpm run @namespace/tool-name
agentpm run @namespace/tool-name@0.1.1
agentpm run @namespace/tool-name@latest
agentpm run @namespace/tool-name@^0.1
```

Resolution rules:

1. Unversioned spec: prefer the version recorded in `agent.lock`.
2. Exact version: require that exact prepared version to be installed.
3. `latest`: select the highest installed prepared version.
4. SemVer range: select the highest installed prepared version satisfying the range.
5. If no matching installed prepared version exists, return a clear error.

### Runtime execution model
The Rust CLI runner should mirror the existing SDK execution model:

- Resolve the installed tool.
- Read the installed `agent.json`.
- Reject manifests where `kind != "tool"`.
- Resolve and validate `entrypoint.command`.
- Support `AGENTPM_NODE` and `AGENTPM_PYTHON` interpreter overrides.
- Validate interpreter family against `runtime.type` when `runtime` is declared.
- Merge environment variables in a predictable order.
- Apply `environment.vars` defaults.
- Fail clearly when required environment variables are missing.
- Spawn the tool as a managed subprocess.
- Send JSON input to stdin.
- Parse JSON output from stdout.
- Preserve stderr for diagnostics.
- Enforce a default timeout.
- Respect `entrypoint.timeout_ms` when present.
- Allow a CLI timeout override where specified.
- Enforce an output-size limit.
- Clean up successful run directories.
- Preserve stdout/stderr logs on failure.

### `agentpm run` input validation
For this phase, `agentpm run` only validates that the provided input is valid JSON.

It does not validate the input against the manifest `inputs` schema. Tool implementations remain responsible for semantic validation and error reporting.

### Adapter architecture
MCP should be implemented as AgentPM's first built-in ecosystem adapter, but the implementation should avoid hardcoding MCP behavior directly into unrelated CLI logic.

This phase should introduce a small internal adapter boundary that can support future adapters, but it should not expose a public plugin API yet.

Third-party plugin loading, including WASM plugins, is intentionally future work.

### MCP server
`agentpm serve --mcp` starts a local HTTP MCP-compatible server.

Default bind behavior:

```text
127.0.0.1:7331
```

By default, the MCP server exposes all tools listed in `agent.lock`.

The server should route MCP tool calls through the same shared Rust runner used by `agentpm run`.

### Skill export
Skills are expected to become first-class AgentPM artifacts in the future. In this phase, `agentpm export --skill` is an ecosystem scaffold/export surface, not a generic plugin adapter.

Generated Skills should:

- Be created from installed tool metadata in `agent.json`.
- Include progressive disclosure.
- Keep `SKILL.md` concise and workflow-oriented.
- Move deeper generated contract details into reference files.
- Delegate execution to `agentpm run` by default.
- Include TODO placeholders where human workflow authorship is expected.

Default output structure:

```text
skills/<tool-name>/
  SKILL.md
  references/
    tool-contract.md
    examples.md
  scripts/
    run.sh
```

The implementation should support an output override and safe overwrite behavior.

## Acceptance criteria

### Shared runner
- A reusable Rust runner module can resolve and execute installed AgentPM tools from `.agentpm/tools/<namespace>/<name>/<version>`.
- Unversioned specs resolve through `agent.lock` when possible.
- Exact versions, `latest`, and SemVer ranges resolve against installed prepared versions.
- Non-tool manifests are rejected for execution.
- Required environment variables are enforced.
- Environment defaults are applied.
- Interpreter overrides via `AGENTPM_NODE` and `AGENTPM_PYTHON` are honored.
- Interpreter/runtime mismatches fail before execution.
- Tool subprocesses receive JSON on stdin and return JSON from stdout.
- Timeout, output limit, malformed JSON, missing interpreter, missing tool, and non-zero child exit failures are surfaced clearly.

### `agentpm run`
- `agentpm run @namespace/tool-name` executes the locked installed version when available.
- `agentpm run @namespace/tool-name@version` executes the exact installed version.
- `agentpm run @namespace/tool-name@latest` executes the highest installed prepared version.
- `agentpm run @namespace/tool-name@range` executes the highest installed prepared version satisfying the range.
- JSON can be supplied through stdin.
- JSON can be supplied through `--input`.
- JSON can be supplied through `--input-file`.
- Valid tool JSON output is printed to stdout.
- Runner diagnostics and errors are printed to stderr.
- The command exits non-zero on failure.

### Internal adapter boundary
- MCP uses an internal adapter-facing description/invocation layer rather than directly duplicating tool resolution and execution logic.
- The adapter boundary remains internal and is not documented as a public plugin API.
- Code comments or internal docs make clear that future plugin/WASM support is out of scope for this phase.

### `agentpm serve --mcp`
- `agentpm serve --mcp` starts a local HTTP MCP-compatible server on `127.0.0.1:7331` by default.
- The server exposes all tools listed in `agent.lock` by default.
- Exposed MCP tool metadata is derived from `agent.json`, including name, description, and input schema when available.
- MCP tool calls execute through the shared Rust runner.
- Tool results are returned through MCP without inventing a new AgentPM-specific result envelope.
- Missing tool, bad arguments, runner failure, timeout, malformed output, and subprocess failure map to useful MCP errors.
- At least one MCP-compatible client flow can connect and call an AgentPM-installed tool.

### `agentpm export --skill`
- `agentpm export --skill @namespace/tool-name` resolves the installed tool and generates `skills/<tool-name>/` by default.
- The generated Skill includes `SKILL.md`, `references/tool-contract.md`, `references/examples.md`, and `scripts/run.sh`.
- `SKILL.md` uses progressive disclosure and stays concise.
- Deeper generated schema/runtime/environment details are placed in reference files.
- Generated execution instructions delegate to `agentpm run @namespace/tool-name` by default.
- Generated files include TODO placeholders for human-authored workflow guidance.
- Existing output directories are not overwritten unless an explicit force option is used.

### Documentation and examples
- CLI help is added for `run`, `serve --mcp`, and `export --skill`.
- Docs show the full interoperability story:

```bash
agentpm install @namespace/tool-name
agentpm run @namespace/tool-name --input '{"example":true}'
agentpm serve --mcp
agentpm export --skill @namespace/tool-name
```

- Docs explain that MCP is the first built-in ecosystem adapter.
- Docs explain that public third-party plugins are future work.
- Docs explain that Skill export is a scaffold and that Skills may become first-class AgentPM artifacts later.

## Risks / edge cases

### Resolution ambiguity
- Multiple installed versions may exist for the same tool.
- Unversioned specs could be ambiguous without `agent.lock`.
- `latest` and SemVer range behavior must not accidentally ignore prerelease semantics unless intentionally supported.
- Namespace/name parsing must handle scoped specs like `@namespace/tool-name@version` correctly.

### Runtime compatibility
- Interpreter names differ across platforms (`python`, `python3`, `node`, `nodejs`).
- Windows process termination and PATH lookup behavior may differ from macOS/Linux.
- Absolute interpreter paths may need careful validation.
- Interpreter override variables could point to missing or incompatible binaries.

### Subprocess behavior
- Tools may write logs to stdout before emitting JSON.
- Tools may emit invalid JSON or multiple JSON objects.
- Tools may hang after producing JSON.
- Tools may produce too much output.
- Tools may exit non-zero after partially writing output.
- Killing process trees differs between Windows and POSIX systems.

### Environment handling
- Required variables may be missing.
- Manifest defaults could mask missing environment configuration if implemented incorrectly.
- Tool entrypoint env and caller env merge order must be predictable.

### MCP compatibility
- MCP transport expectations may vary by client.
- Claude, Cursor, and Codex may require slightly different configuration shapes even if the server is protocol-compatible.
- Tool names derived from AgentPM refs may need normalization for MCP.
- Large schemas or complex JSON Schema features may not map perfectly to MCP metadata.

### Skill scaffold quality
- Generated Skills may look more complete than they actually are.
- Skill instructions could overpromise if TODO sections are not clear.
- Two tools with the same name but different namespaces may collide under `skills/<tool-name>/`.
- Shell script permissions may differ across platforms.

### Scope creep
- Adapter boundaries could turn into an unplanned public plugin system.
- MCP support could expand into hosted/server management concerns.
- Skill export could expand into first-class Skill publishing before the phase is ready.

## Open questions

- Should `agentpm serve --mcp` support both HTTP and stdio in this phase, or should stdio remain future work?
- Should MCP tool names preserve AgentPM refs exactly, or normalize them into MCP-safe names with a mapping layer?
- Should `agentpm serve --mcp --tool` and `--tools` be included in the first implementation or deferred? Answer: Yes
- Should `agentpm run --env KEY=value` be included now, or should users rely on shell environment variables for the first pass?
- Should generated `scripts/run.sh` be accompanied by a Windows `.ps1` wrapper?
- Should Skill export use `skills/<tool-name>/` only, or support `skills/<namespace>/<tool-name>/` as an option for collision avoidance?
- Should `agentpm run` support a `--raw` or `--pretty` output mode, or always print compact JSON for scripting?

## References
- Existing AgentPM manifest schema / `agent.json` contract.
- Existing install and lockfile behavior.
- Existing publish/install provenance and integrity verification.
- Existing Node SDK managed subprocess runner behavior.
- Existing Python SDK managed subprocess runner behavior.

## Related Specs

