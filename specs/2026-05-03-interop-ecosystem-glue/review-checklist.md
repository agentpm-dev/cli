# Review Checklist

## Contract surfaces
- [ ] Confirm `agent.json` schema was not changed unless the spec explicitly required it.
- [ ] Confirm `agent.lock` shape was not changed unless the spec explicitly required it.
- [ ] Confirm install layout assumptions remain `.agentpm/tools/<namespace>/<name>/<version>`.
- [ ] Confirm cache behavior under `.agentpm/cache` was not changed by runner work.
- [ ] Confirm existing commands still parse and route correctly.
- [ ] Confirm new command names and flags match the spec.
- [ ] Confirm `agentpm run` does not introduce a new output envelope around tool results.
- [ ] Confirm `agentpm run` does not validate input against manifest `inputs` in this phase.
- [ ] Confirm MCP output does not invent unnecessary AgentPM-specific wrapping around tool results.
- [ ] Confirm generated Skill structure matches `skills/<tool-name>/` by default.
- [ ] Confirm docs were updated for user-facing behavior changes.

## Correctness
- [ ] Check `agentpm run @namespace/tool-name` resolves through `agent.lock` when available.
- [ ] Check exact version resolution works.
- [ ] Check `latest` resolution works against installed prepared versions.
- [ ] Check SemVer range resolution works if implemented.
- [ ] Check missing prepared tools fail clearly.
- [ ] Check missing lockfile or missing locked dependency failures are understandable.
- [ ] Check non-tool manifests are rejected for execution.
- [ ] Check required environment variables are enforced.
- [ ] Check environment defaults are applied.
- [ ] Check interpreter overrides with `AGENTPM_NODE` and `AGENTPM_PYTHON` work.
- [ ] Check interpreter/runtime mismatches fail before execution.
- [ ] Check subprocess execution sends JSON on stdin.
- [ ] Check subprocess execution parses JSON from stdout.
- [ ] Check stderr is preserved for diagnostics and does not pollute successful stdout.
- [ ] Check timeouts terminate the tool process.
- [ ] Check output-size limits are enforced.
- [ ] Check failure logs are preserved.
- [ ] Check success temp directories are cleaned up.

## `agentpm run`
- [ ] Check stdin input works.
- [ ] Check `--input` works.
- [ ] Check `--input-file` works.
- [ ] Check invalid JSON input exits non-zero.
- [ ] Check successful tool output is printed as valid JSON.
- [ ] Check errors print to stderr.
- [ ] Check the command exits non-zero on resolution, validation, timeout, interpreter, spawn, malformed output, and child failure errors.
- [ ] Check `--timeout-ms` overrides manifest/default timeout if implemented.
- [ ] Check `--env KEY=value` behavior if implemented.

## Internal adapter boundary
- [ ] Confirm MCP uses shared runner/resolver behavior rather than duplicating execution logic.
- [ ] Confirm adapter-facing descriptors are internal and not documented as a public plugin API.
- [ ] Confirm the implementation is structured so future adapters can be added without rewriting the runner.
- [ ] Confirm there is no accidental public WASM/plugin API surface in this phase.
- [ ] Confirm comments/docs accurately describe public plugin support as future work.

## MCP server
- [ ] Check `agentpm serve --mcp` starts an HTTP server on `127.0.0.1:7331` by default.
- [ ] Check host/port flags if implemented.
- [ ] Check all tools listed in `agent.lock` are exposed by default.
- [ ] Check `--tool` / `--tools` filtering if implemented.
- [ ] Check MCP tool descriptions come from manifest metadata.
- [ ] Check MCP input metadata comes from manifest `inputs` where available.
- [ ] Check AgentPM refs are mapped to MCP-safe names without losing the ability to route calls back to the original tool.
- [ ] Check MCP tool calls route through the shared runner.
- [ ] Check MCP errors are useful for missing tools, invalid args, runner failures, timeouts, malformed output, and subprocess failures.
- [ ] Check at least one MCP-compatible client can discover and invoke a tool.

## Skill export
- [ ] Check `agentpm export --skill @namespace/tool-name` resolves the installed tool correctly.
- [ ] Check default output path is `skills/<tool-name>/`.
- [ ] Check `--output` works if implemented.
- [ ] Check existing output directories are protected unless `--force` is supplied.
- [ ] Check generated file tree includes `SKILL.md`, `references/tool-contract.md`, `references/examples.md`, and `scripts/run.sh`.
- [ ] Check `SKILL.md` is concise and workflow-oriented.
- [ ] Check progressive disclosure is present.
- [ ] Check deeper schema/runtime/environment details are in reference files.
- [ ] Check generated execution instructions delegate to `agentpm run @namespace/tool-name`.
- [ ] Check TODO placeholders make human-authored workflow work clear.
- [ ] Check generated Skill wording does not imply it is a polished final Skill.
- [ ] Check script permissions on `scripts/run.sh` where relevant.

## Regressions
- [ ] Check existing `init` behavior still works.
- [ ] Check existing `lint` behavior still works.
- [ ] Check existing `publish` behavior still works or is not impacted by unrelated changes.
- [ ] Check existing `install` behavior still writes the expected prepared tools and lockfile.
- [ ] Check existing auth-related commands still compile and route.
- [ ] Check existing manifest validation behavior has not changed unexpectedly.
- [ ] Check existing SDK assumptions are not broken by CLI changes.
- [ ] Check platform-specific process behavior, especially Windows process termination and PATH lookup, if touched.

## Tests and verification
- [ ] Confirm `cargo fmt --check` passed.
- [ ] Confirm `cargo clippy --all-targets --all-features -- -D warnings` passed, or record the repo's accepted equivalent.
- [ ] Confirm `cargo test --all` passed.
- [ ] Confirm integration tests or fixture tests were run.
- [ ] Confirm new runner behavior has unit or integration coverage.
- [ ] Confirm `agentpm run` has CLI-level coverage.
- [ ] Confirm MCP behavior has automated coverage where practical.
- [ ] Confirm Skill export has generated file structure/content coverage.
- [ ] Confirm manual MCP client verification was performed or clearly documented as not performed.
- [ ] Confirm expected evidence from `test-plan.md` was reported.

## Pattern adherence
- [ ] Check existing CLI command/module patterns were reused.
- [ ] Check existing manifest handling was reused instead of creating duplicate parsing logic.
- [ ] Check existing install/lockfile helpers were reused where available.
- [ ] Check new dependencies are justified.
- [ ] Check dependency choices fit the Rust CLI's existing style and maintenance expectations.
- [ ] Check errors are clear and consistent with existing command behavior.
- [ ] Check user-facing output separates stdout and stderr appropriately.
- [ ] Check no large abstraction was introduced only for speculative future plugin support.
- [ ] Check MCP and Skill work remain scoped to this phase.

## Notes for reviewer
- Pay special attention to whether the runner is genuinely reusable by `run`, MCP, and future adapters.
- Pay special attention to stdout cleanliness for `agentpm run`; shell consumers need machine-readable output.
- Pay special attention to lockfile-aware resolution; unversioned specs should be deterministic when `agent.lock` is present.
- Pay special attention to process cleanup and timeout behavior.
- Pay special attention to MCP client compatibility because automated tests may not fully prove real-client behavior.
- Pay special attention to Skill export wording; it should generate a scaffold with progressive disclosure, not overpromise polished Skill generation.
- Avoid approving changes that quietly turn the internal adapter seam into a public plugin API before that API has its own spec.
