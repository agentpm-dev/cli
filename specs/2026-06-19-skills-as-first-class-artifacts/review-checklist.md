# Review Checklist

## Contract surfaces
- [ ] `agent.json` schema includes `kind: "skill"` intentionally and does not accidentally loosen unrelated package kinds.
- [ ] Skill manifests require `skill.entrypoint`.
- [ ] Skill manifests allow optional top-level `tools`.
- [ ] Skill manifests reject top-level `skills` for `kind: "skill"`.
- [ ] Agent manifests treat top-level `skills` as first-class package dependencies.
- [ ] Template manifests support `template.dependencies.skills`.
- [ ] The manifest still uses `agent.json`; no new `skill.json` contract was introduced.
- [ ] CLI package kind enums and SDK/API package kind enums include `skill` where required.
- [ ] Backend package kind validation accepts `skill` and error messages are updated.
- [ ] Database check constraints allow `skill`.
- [ ] Search API type/filter contracts include `skills`.
- [ ] Registry routes include `/skills/<package_id>/v<version>/overview` or the agreed Skill detail route.
- [ ] Docs were updated anywhere user-facing behavior changed.

## Correctness
- [ ] A minimal procedural Skill with only `agent.json` and `SKILL.md` validates, dry-run publishes, installs, and renders.
- [ ] A tool-backed Skill with declared `tools`, references, and scripts validates, dry-run publishes, installs, and renders.
- [ ] Skill publish tarballs include only manifest-declared Skill files plus required package metadata.
- [ ] Skill publish preserves relative paths for references and scripts.
- [ ] Missing declared Skill files fail fast with a clear error.
- [ ] Unsafe declared Skill paths fail fast with a clear error.
- [ ] Skill artifacts reuse existing tar safety rules and artifact size limits.
- [ ] Skill packages are stored with kind `skill` in the shared package table.
- [ ] Publish kind conflicts are detected when the same namespace/name exists with another kind.
- [ ] Publish finalize validates dependency access for Skill tool dependencies.
- [ ] Agent publish/finalize validates first-class Skill dependencies.
- [ ] Template publish/finalize validates Skill dependencies.
- [ ] Direct Skill install resolves, downloads, extracts, and locks correctly.
- [ ] Installing an agent with Skill dependencies resolves the agent, skills, and Skill tool dependencies.
- [ ] Direct Template install remains rejected through normal `agentpm install`.
- [ ] Private namespace access rules apply to Skill publish, install, search, and detail views.

## Lockfile and dependency graph
- [ ] `PackageKind::Skill` exists anywhere package keys or resolve plans are modeled.
- [ ] Package keys use `skill:@namespace/name@version`.
- [ ] Skills appear in lockfile `packages` with `kind: "skill"`.
- [ ] Lock roots use a first-class `skills` field.
- [ ] `reserved.skills` is no longer written for new locks.
- [ ] `knowledge`, `memory`, and `profiles` remain reserved.
- [ ] Old locks containing `reserved.skills` are read/migrated safely.
- [ ] Lockfile version handling is intentional and documented.
- [ ] Skill-to-Skill dependencies are not accidentally enabled.
- [ ] Agent-to-Agent dependencies are not accidentally enabled.
- [ ] Tool-to-anything dependencies are not accidentally enabled.
- [ ] Workspace lock generation handles Skill dependencies where expected.

## Init and export UX
- [ ] `agentpm init --kind skill` writes exactly `agent.json` and `SKILL.md` unless the spec was intentionally changed.
- [ ] Generated init Skill manifest validates.
- [ ] Generated init `SKILL.md` is workflow/playbook oriented and not tool-wrapper-specific.
- [ ] Existing `agentpm init --kind tool|agent|template` behavior still works.
- [ ] `agentpm export --skill <tool>` without `--manifest` preserves the existing scaffold shape and behavior.
- [ ] `agentpm export --skill <tool> --manifest` writes a valid Skill `agent.json`.
- [ ] Exported manifest pins the resolved tool version.
- [ ] Exported manifest references generated `SKILL.md`, `references/tool-contract.md`, `references/examples.md`, and `scripts/run.sh`.
- [ ] Export still warns about default output namespace collisions where applicable.
- [ ] Export overwrite behavior with `--force` still works.
- [ ] Remote export fallback does not install packages or mutate lock/workspace files.
- [ ] Remote export sends credentials when available for private package metadata access.

## Search and registry UX
- [ ] Search supports `skills` type/filter.
- [ ] `all` search can include Skill results.
- [ ] `totals_by_type` includes `skills`.
- [ ] Relevance, Newest, Trending, and Most downloaded package streams handle Skill rows.
- [ ] Cursor/seek logic handles Skill result items.
- [ ] Skill results do not get coerced into Tool DTO/card rendering.
- [ ] Skill detail URL generation works in backend receipts, API responses, and frontend links.
- [ ] Skill detail page emphasizes manual/procedure content.
- [ ] Skill detail page does not display tool-only runtime/input/output sections as if the Skill is directly executable.
- [ ] Skill detail page shows declared tools, references, scripts, compatibility metadata, install command, license, signatures, and visibility where available.
- [ ] Private Skills do not appear to unauthorized users.
- [ ] Private Skills do appear to authorized namespace members.

## Regressions
- [ ] Existing tool publish dry-run and real publish behavior still work.
- [ ] Existing agent publish dry-run and real publish behavior still work.
- [ ] Existing template publish dry-run and real publish behavior still work.
- [ ] Existing direct tool install still works.
- [ ] Existing direct agent install still works.
- [ ] Existing manifest-driven local agent install still works.
- [ ] Existing workspace install still works.
- [ ] Existing `agentpm new` template behavior still works.
- [ ] Existing export scaffold generation still works for installed tools.
- [ ] Existing search filters for tools, agents, templates, and namespaces still work.
- [ ] Existing private namespace visibility behavior still works.
- [ ] Existing signing/attestation behavior still works for tools, agents, and templates.

## Tests and verification
- [ ] CLI/Rust tests were added for manifest parsing, init, publish packaging, install/lockfile, and export behavior.
- [ ] Backend tests were added for Skill publish, install resolve/init/finalize, dependency validation, and search.
- [ ] Frontend tests or documented manual checks cover Skill search and detail rendering.
- [ ] Tests cover invalid paths and missing files.
- [ ] Tests cover dependency access failure for private/inaccessible Skill dependencies.
- [ ] Tests cover old lockfile migration from `reserved.skills`.
- [ ] Existing tests updated for the new first-class `skills` lock root shape.
- [ ] Verification followed `test-plan.md`.
- [ ] Any skipped verification is explicitly documented with rationale.

## Pattern adherence
- [ ] Implementation reuses existing package publish/install patterns instead of creating a separate Skill subsystem.
- [ ] Implementation avoids broad physical table/model renames in this phase.
- [ ] Implementation reuses existing safe path and tar append helpers.
- [ ] Implementation reuses existing access-control helpers for private namespaces.
- [ ] Implementation reuses existing search materialized view shape where possible.
- [ ] New abstractions are justified by repeated package-kind branching, not speculative future needs.
- [ ] Compatibility metadata remains descriptive only and does not introduce runtime enforcement.
- [ ] Skill export remains a scaffold generator, not an install command.

## Notes for reviewer
- Pay close attention to places that enumerate package kinds as `tool|agent|template`; these are the most likely missed updates.
- Pay close attention to lockfile root shape. Skills must move out of `reserved`, but old locks must not lose data.
- Pay close attention to publish packaging. Skills should package declared files only, not crawl the entire directory.
- Pay close attention to remote export. It may fetch metadata from the registry, but it must not mutate the local workspace.
- Pay close attention to registry rendering. Skills are not executable tools, so tool-only runtime/input/output UI should not be reused blindly.
