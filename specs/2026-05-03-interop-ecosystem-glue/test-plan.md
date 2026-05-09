# Test Plan

## Required verification
- Verify `agentpm run` can execute an installed Node tool.
- Verify `agentpm run` can execute an installed Python tool.
- Verify unversioned `agentpm run @namespace/tool-name` uses `agent.lock` when available.
- Verify exact version resolution works.
- Verify `latest` resolution works against installed prepared versions.
- Verify SemVer range resolution works against installed prepared versions if range support is implemented.
- Verify JSON input works through stdin.
- Verify JSON input works through `--input`.
- Verify JSON input works through `--input-file`.
- Verify invalid JSON input fails clearly.
- Verify missing installed tools fail clearly.
- Verify non-tool manifests fail clearly.
- Verify missing required environment variables fail clearly.
- Verify manifest environment defaults are applied.
- Verify interpreter/runtime mismatch fails before execution.
- Verify timeout failures are handled and return non-zero.
- Verify malformed tool stdout fails clearly.
- Verify excessive output fails clearly.
- Verify successful runs print JSON to stdout and diagnostics do not pollute stdout.
- Verify failed runs print useful diagnostics to stderr.
- Verify `agentpm serve --mcp` starts an HTTP MCP server on `127.0.0.1:7331` by default.
- Verify `agentpm serve --mcp` exposes all tools listed in `agent.lock` by default.
- Verify an MCP-compatible client can list and call at least one exposed AgentPM tool.
- Verify MCP tool calls route through the same runner behavior as `agentpm run`.
- Verify MCP runner failures map to useful MCP errors.
- Verify `agentpm export --skill @namespace/tool-name` generates the expected Skill scaffold under `skills/<tool-name>/`.
- Verify generated Skills include progressive disclosure through `SKILL.md` and reference files.
- Verify generated execution instructions delegate to `agentpm run`.
- Verify existing Skill output directories are not overwritten unless `--force` is used.

## Automated checks
Run the repo's standard formatting, linting, and test commands. Exact command names should be adjusted to match the CLI repo's existing conventions.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

If the repo has integration tests or snapshot tests, run them as well:

```bash
cargo test --test '*'
```

If the repo has pre-commit configured, run:

```bash
pre-commit run --all-files
```

## Automated test cases

### Runner resolution
- Fixture: `.agentpm/tools/zack/echo-json/0.1.0/agent.json` exists and `agent.lock` points to `0.1.0`.
- Command: `agentpm run @zack/echo-json --input '{"message":"hi"}'`.
- Expected: resolves `0.1.0`, exits 0, prints valid JSON.

### Exact version
- Fixture: `.agentpm/tools/zack/echo-json/0.1.0` and `.agentpm/tools/zack/echo-json/0.2.0` both exist.
- Command: `agentpm run @zack/echo-json@0.1.0 --input '{"message":"hi"}'`.
- Expected: executes `0.1.0`.

### Latest version
- Fixture: `.agentpm/tools/zack/echo-json/0.1.0` and `.agentpm/tools/zack/echo-json/0.2.0` both exist.
- Command: `agentpm run @zack/echo-json@latest --input '{"message":"hi"}'`.
- Expected: executes `0.2.0`.

### SemVer range
- Fixture: `.agentpm/tools/zack/echo-json/0.1.0`, `.agentpm/tools/zack/echo-json/0.1.5`, and `.agentpm/tools/zack/echo-json/0.2.0` exist.
- Command: `agentpm run @zack/echo-json@^0.1 --input '{"message":"hi"}'`.
- Expected: executes the highest installed version satisfying `^0.1` according to the selected SemVer behavior.

### Stdin input
```bash
echo '{"message":"hi"}' | agentpm run @zack/echo-json
```

Expected: exits 0 and prints valid JSON.

### Inline input
```bash
agentpm run @zack/echo-json --input '{"message":"hi"}'
```

Expected: exits 0 and prints valid JSON.

### File input
```bash
printf '{"message":"hi"}' > payload.json
agentpm run @zack/echo-json --input-file payload.json
```

Expected: exits 0 and prints valid JSON.

### Invalid JSON
```bash
agentpm run @zack/echo-json --input '{bad json}'
```

Expected: exits non-zero and prints a clear invalid JSON error to stderr.

### Missing required env var
- Fixture manifest declares a required environment variable with no default.
- Command: `agentpm run @zack/requires-env --input '{}'`.
- Expected: exits non-zero with a missing environment variable error.

### Environment default
- Fixture manifest declares an environment variable default.
- Command: `agentpm run @zack/uses-env-default --input '{}'`.
- Expected: tool receives the default and exits 0.

### Timeout
- Fixture tool sleeps longer than configured timeout.
- Command: `agentpm run @zack/slow-tool --timeout-ms 100 --input '{}'`.
- Expected: exits non-zero with timeout error; process is terminated.

### Malformed output
- Fixture tool exits 0 but prints non-JSON stdout.
- Command: `agentpm run @zack/bad-output --input '{}'`.
- Expected: exits non-zero with malformed JSON output error.

### MCP tool listing
- Fixture lockfile contains at least one installed tool.
- Command: `agentpm serve --mcp`.
- Test client: request MCP tool list.
- Expected: locked tool appears with manifest-derived description and input schema.

### MCP tool call
- Fixture lockfile contains `@zack/echo-json`.
- Command: `agentpm serve --mcp`.
- Test client: call exposed echo tool with JSON args.
- Expected: response contains the tool's JSON output.

### Skill export structure
```bash
agentpm export --skill @zack/echo-json
```

Expected generated files:

```text
skills/echo-json/
  SKILL.md
  references/
    tool-contract.md
    examples.md
  scripts/
    run.sh
```

### Skill export overwrite protection
```bash
agentpm export --skill @zack/echo-json
agentpm export --skill @zack/echo-json
```

Expected: second command fails unless `--force` is supplied.

## Manual checks

### Primary user flow
From a clean test project with installed tools:

```bash
agentpm install @zack/echo-json
agentpm run @zack/echo-json --input '{"message":"hello"}'
agentpm serve --mcp
agentpm export --skill @zack/echo-json
```

Verify:

- `agentpm run` returns the expected JSON.
- MCP server starts on `127.0.0.1:7331`.
- An MCP-compatible client can connect, list tools, and call the installed tool.
- Skill files are generated under `skills/echo-json/`.
- The generated Skill points back to `agentpm run`.

### MCP client compatibility
Manually configure at least one target client such as Claude, Cursor, or Codex to connect to the local AgentPM MCP server.

Verify:

- The client can discover tools.
- The client can invoke at least one tool.
- Tool errors are visible and understandable.

### Skill scaffold review
Open the generated Skill and verify:

- `SKILL.md` is concise.
- Progressive disclosure is present.
- Reference files contain generated manifest details.
- TODO sections make it clear the Skill is a scaffold.
- Execution delegates to `agentpm run`.

### Adjacent regression checks
Verify existing commands still work:

```bash
agentpm init --help
agentpm lint --help
agentpm install --help
agentpm publish --help
```

If credentials are available, verify install still writes expected prepared tool files and lockfile entries.

## Expected evidence
Report back with:

- Commands run.
- Passing automated check output.
- A sample `agentpm run` command and JSON output.
- A sample MCP tool list or call output.
- A sample generated Skill file tree.
- Any platform-specific behavior observed, especially on Windows if tested.
- Any checks that could not be completed and why.

## Out of scope
- Full public third-party plugin API verification.
- WASM plugin loading verification.
- Hosted or remote MCP server verification.
- First-class Skill publish/install verification.
- Agent artifact execution verification.
- Runtime validation of input against manifest `inputs` schema.
- Comprehensive MCP client matrix testing across every supported client.
