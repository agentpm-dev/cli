# Review Checklist

## Contract surfaces
- Confirm `agent.json` schema changes are intentional and match `spec.md`.
- Confirm `kind: "tool"` manifests remain compatible.
- Confirm `kind: "agent"` manifests do not require executable tool fields.
- Confirm `compatibility` was not added in this phase.
- Confirm `instructions` was not added in this phase.
- Confirm `skills`, `knowledge`, `memory`, and `profiles` are validated and preserved but not resolved.
- Confirm examples are inline prompt examples in `agent.json`, not separate files.
- Confirm package names are unique within namespace regardless of kind.
- Confirm public API responses include package kind where needed.
- Confirm lockfile v2 includes kind, name, version, integrity, and dependency relationships.
- Confirm lockfile v2 can represent multiple versions of the same package.
- Confirm docs were updated for any user-facing behavior changes.

## Package migration
- Check that tool-specific database concepts were migrated to package-oriented concepts in small, understandable changes.
- Confirm any remaining `Tool`/`ToolVersion` naming is either truly tool-specific or intentionally left as compatibility debt.
- Confirm database migrations preserve existing tool data.
- Confirm package version rows support both tools and agents.
- Confirm runtime remains nullable and tool-specific.
- Confirm upload, signature, attestation, scan, install session, and bucket/stat references use package-oriented identity where appropriate.
- Confirm triggers, materialized views, indexes, and constraints were updated.
- Confirm package search/index refresh behavior works for tools and agents.
- Confirm namespace counters were updated intentionally.

## CLI correctness
- Check `agentpm init --kind agent` output against the schema.
- Check `agentpm lint` behavior for valid and invalid tool/agent manifests.
- Check publish branches by artifact kind and does not run tool packaging logic for agents.
- Check agent publish does not require `entrypoint`, `runtime`, `inputs`, `outputs`, or `files`.
- Check signing and attestation statements are package-kind-aware.
- Check install carries artifact kind through resolve/init/download/extract/finalize.
- Check install writes agents to `.agentpm/agents/...` and tools to `.agentpm/tools/...`.
- Check install does not vendor tools under each installed agent.
- Check install output wording is package-aware where appropriate.
- Check `--frozen`, `--refresh`, `--update-range`, `--require_attestation`, `--quiet`, and `--token` still behave correctly.

## Lockfile correctness
- Confirm v2 lockfile keys or entries cannot collide when two versions of the same package are installed.
- Confirm agent-to-tool relationships are recorded.
- Confirm reserved future references from agent manifests are preserved.
- Confirm v1 lockfiles are read for existing tool-only flows where practical.
- Confirm normal installs write v2.
- Confirm unsupported v1 frozen agent scenarios fail with a clear and actionable message.
- Confirm existing examples were updated or verified after the lockfile change.

## Backend correctness
- Check publish auth uses `packages:publish` according to the spec.
- Check any temporary `tools:publish` compatibility is deliberate and documented.
- Check package publish init/finalize supports both tools and agents.
- Check package install resolve/init/finalize supports both tools and agents.
- Check direct agent install resolves its tool dependencies.
- Check reserved future dependencies are not resolved or downloaded.
- Check semver resolution still works for tools.
- Check semver resolution works for agent specs.
- Check 404/409/422 errors are still clear and do not leak private package information beyond existing behavior.

## SDK correctness
- Confirm Node SDK can load installed agents from `.agentpm/agents/...`.
- Confirm Python SDK can load installed agents from `.agentpm/agents/...`.
- Confirm SDKs expose manifest, installed path, resolved tool refs, and reserved references.
- Confirm SDKs do not execute, invoke, chat with, or orchestrate agents.
- Confirm existing tool-loading SDK behavior still works.

## Registry correctness
- Confirm search supports tools, agents, namespaces, and all.
- Confirm search result serialization includes `itemType: "agent"`.
- Confirm totals include agents where appropriate.
- Confirm pagination/cursor behavior still works after agents are added.
- Confirm agent cards are visually distinct from tool cards.
- Confirm agent detail pages show description, README, examples, dependencies, reserved references, version, namespace, and install command.
- Confirm existing tool pages still work.
- Confirm optional “used by agents” behavior is implemented only if cheap and covered by tests/manual checks.

## Regressions
- Check existing tool publish flow.
- Check existing tool install flow.
- Check existing tool run flow.
- Check existing lint behavior.
- Check existing init behavior.
- Check existing search behavior for tools and namespaces.
- Check existing docs/examples that mention tools, lockfiles, publish scopes, or install layout.
- Check package URLs and any redirects/backward-compatible routes.

## Tests and verification
- Confirm the work was verified according to `test-plan.md`.
- Confirm new manifest schema behavior has tests.
- Confirm package migration has tests or documented migration verification.
- Confirm lockfile v2 has tests, especially for multiple versions of the same tool.
- Confirm direct agent install has tests.
- Confirm SDK agent loading has tests.
- Confirm registry search/detail behavior has tests or documented manual verification.
- Confirm high-risk behavior changes are not relying only on manual confidence.

## Pattern adherence
- Check that existing repo patterns were reused before adding new abstractions.
- Check that new package abstractions are justified and not broader than the spec requires.
- Check that migration chunks are small and reviewable.
- Check that any compatibility layer is documented and not accidentally permanent unless intended.
- Check that naming is moving toward package terminology without hiding truly tool-specific behavior.
- Check that error messages are clear and consistent with existing CLI/backend style.

## Notes for reviewer
- Pay special attention to lockfile v2 and multi-version tool support. This is a core Phase 3 requirement, not a nice-to-have.
- Pay special attention to places where `tool_id`, `tool_version_id`, `/tools`, `tools:publish`, `Tool`, or `ToolVersion` remain. Some may be intentional compatibility, but each should be reviewed.
- Pay special attention to install layout. Tool packages should not be duplicated under installed agents.
- Pay special attention to agent publish. It should package composition metadata, not require runtime executable files.
- Pay special attention to registry search pagination after adding agents to mixed `all` search.
- Pay special attention to docs and examples because lockfile and package terminology changes are user-visible.
