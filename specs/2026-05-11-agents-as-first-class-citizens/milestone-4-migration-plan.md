# Milestone 4 Migration Plan

Milestone 4 moves the canonical registry identity from "tool" terminology toward
"package" terminology without forcing the rest of the backend to switch all at
once.

## Scope for this milestone

- Add `kind` to the canonical package identity row with allowed values `tool`
  and `agent`.
- Keep the existing physical table names stable for now (`tools`,
  `tool_versions`) to avoid breaking install/publish/search routes mid-migration.
- Introduce package-oriented ORM/domain names on top of the existing tables so
  later milestones can migrate behavior incrementally.
- Add a package-aware namespace counter while preserving the existing
  tool-specific counter for compatibility with current UI and API surfaces.
- Update the published-version trigger so package counts are correct as soon as
  non-tool packages are introduced.

## Deliberate non-goals for this milestone

- Do not rename routes or response DTOs yet.
- Do not generalize publish/install behavior yet.
- Do not rename uploads, signatures, attestations, scans, or install sessions
  yet.
- Do not replace the existing tool search/index/UI surfaces yet.

## Database changes

1. Add `tools.kind text not null default 'tool' check (kind in ('tool','agent'))`.
2. Backfill existing rows to `kind = 'tool'`.
3. Add `namespaces.num_packages integer not null default 0`.
4. Backfill `num_packages` from `num_tools`.
5. Update the `on_tool_version_published()` trigger function so:
   - `num_packages` increments for every first published package
   - `num_tools` increments only when the published package kind is `tool`

The existing `UNIQUE(namespace_id, name)` constraint on `tools` already enforces
unique package names within a namespace regardless of kind, so no new uniqueness
constraint is needed for this milestone.

## ORM/domain shape

- Add `Package` and `PackageVersion` as the preferred ORM names.
- Map them to the existing `tools` / `tool_versions` tables.
- Keep `Tool` and `ToolVersion` as compatibility aliases so existing backend
  code does not need to move all at once.

## Manual verification for existing data

After applying the migration in a non-production environment:

1. Confirm existing rows were backfilled:
   - `select count(*) from tools where kind <> 'tool';` should be `0`
2. Confirm namespace package counts were backfilled:
   - `select namespace_id, num_tools, num_packages from namespaces;`
   - existing namespaces should have matching `num_tools` and `num_packages`
3. Confirm existing publish flows still read/write the same tables:
   - publish a tool package in staging
   - verify the new `tools.kind` value is `tool`
   - verify both `num_tools` and `num_packages` increment for the namespace
4. Confirm existing reads still work:
   - tool detail
   - namespace tools listing
   - install resolve/init for a tool

## Follow-on milestones

- Milestone 5 will migrate uploads/signatures/attestations/scans/install
  sessions to package-oriented identity.
- Milestone 6 will generalize publish behavior to real `kind: "agent"`
  packages.
- Milestone 9 will generalize install resolution to real agent packages.
