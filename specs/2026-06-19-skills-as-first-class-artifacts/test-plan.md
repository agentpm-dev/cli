# Test Plan

## Required verification
- Verify `kind: "skill"` is accepted by schema validation and parsed into typed manifest structures.
- Verify Skill manifests require `skill.entrypoint` and reject unsafe declared paths.
- Verify Skills may declare top-level `tools` and may omit `tools`.
- Verify Skills may not declare top-level `skills` in Phase 6A.
- Verify agents may declare top-level `skills` as first-class package dependencies.
- Verify templates may declare `template.dependencies.skills`.
- Verify `agentpm init --kind skill` creates a valid `agent.json` and `SKILL.md`.
- Verify `agentpm publish --dry-run` creates a Skill tarball containing exactly the manifest-declared Skill files.
- Verify backend publish init/finalize accepts Skill packages and rejects kind conflicts.
- Verify direct Skill install resolves, downloads, extracts, and writes lockfile entries.
- Verify installing an agent with Skill dependencies resolves the Skill and its tool dependencies.
- Verify lockfile roots move `skills` out of `reserved` and keep `knowledge`, `memory`, and `profiles` reserved.
- Verify old lockfiles with `reserved.skills` are migrated or handled safely.
- Verify `agentpm export --skill <tool>` remains backward compatible without `--manifest`.
- Verify `agentpm export --skill <tool> --manifest` generates a valid publishable Skill starter.
- Verify remote export can scaffold from a non-installed tool without installing the tool or mutating the lockfile.
- Verify search includes Skills in type filters, all results, totals, trending, newest, and most-downloaded sorts.
- Verify registry UI renders Skill cards and detail pages as Skills, not executable tools.
- Verify existing tool, agent, and template flows still pass.

## Automated checks
Run the closest equivalents available in the repo:

```bash
# CLI / Rust
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all

# Manifest/schema validation examples
agentpm lint --schema schemas/agentpm.manifest.schema.json --manifest examples/skills/minimal/agent.json
agentpm lint --schema schemas/agentpm.manifest.schema.json --manifest examples/skills/tool-backed/agent.json

# Init smoke test
rm -rf /tmp/agentpm-skill-init-test
agentpm init --kind skill --name incident-commander --description "Incident response coordination playbook" --out-dir /tmp/agentpm-skill-init-test
agentpm lint --manifest /tmp/agentpm-skill-init-test/agent.json

# Publish dry-run smoke test
cd /tmp/agentpm-skill-init-test
agentpm publish --dry-run --token dummy-token

# Backend / Python
pytest
ruff check .
black --check .

# Frontend, if present
npm test
npm run lint
npm run typecheck
npm run build
```

If exact command names differ, use the project’s existing CI commands and report the substitutions.

## CLI manual checks

### Minimal Skill authoring path
1. Run:

```bash
rm -rf /tmp/agentpm-skill-minimal
agentpm init --kind skill --name incident-commander --description "Incident response coordination playbook" --out-dir /tmp/agentpm-skill-minimal
cd /tmp/agentpm-skill-minimal
agentpm lint
agentpm publish --dry-run --token dummy-token
```

2. Confirm:
- `agent.json` exists.
- `SKILL.md` exists.
- Manifest kind is `skill`.
- Manifest has `skill.entrypoint == "SKILL.md"`.
- Dry-run tarball exists under `target/agentpm`.
- Tarball contains `agent.json` and `SKILL.md`.

### Tool-backed Skill publish path
1. Create or use a Skill with:

```json
{
  "kind": "skill",
  "name": "slack-incident-update",
  "version": "0.1.0",
  "description": "A playbook for posting incident updates to Slack.",
  "tools": [{ "name": "@zack/slack-post-message", "version": "0.1.1" }],
  "skill": {
    "entrypoint": "SKILL.md",
    "references": ["references/tool-contract.md", "references/examples.md"],
    "scripts": ["scripts/run.sh"]
  }
}
```

2. Run:

```bash
agentpm lint
agentpm publish --dry-run --token dummy-token
```

3. Inspect the tarball and confirm it contains:
- `agent.json`
- `SKILL.md`
- `references/tool-contract.md`
- `references/examples.md`
- `scripts/run.sh`

4. Confirm an undeclared file in the directory is not included.

### Unsafe path checks
Create Skill manifests with each unsafe path and verify lint or publish fails:

```json
{ "skill": { "entrypoint": "../SKILL.md" } }
{ "skill": { "entrypoint": "/tmp/SKILL.md" } }
{ "skill": { "references": ["references/../../secret.md"] } }
```

### Export compatibility path
1. In a project with an installed tool, run:

```bash
agentpm export --skill @zack/slack-post-message --output /tmp/export-no-manifest --force
```

2. Confirm the output contains the existing scaffold shape and no `agent.json` unless explicitly requested:

```text
SKILL.md
references/tool-contract.md
references/examples.md
scripts/run.sh
```

3. Run:

```bash
agentpm export --skill @zack/slack-post-message --manifest --output /tmp/export-with-manifest --force
agentpm lint --manifest /tmp/export-with-manifest/agent.json
```

4. Confirm the generated manifest is `kind: "skill"`, pins the resolved tool version, and references the generated files.

### Remote export path
1. In a clean directory with no installed tool and no lockfile, run:

```bash
agentpm export --skill @namespace/public-tool --manifest --output /tmp/remote-export-skill --force
```

2. Confirm:
- scaffold is generated
- generated `agent.json` validates
- no `.agentpm/tools` entry is created
- no `.agentpm/skills` entry is created
- no `agent.lock` is created or modified

For a private accessible tool, repeat with `--token` or existing login credentials and confirm authorized remote export works.

### Install path
1. Direct Skill install:

```bash
rm -rf /tmp/agentpm-skill-install
mkdir -p /tmp/agentpm-skill-install
cd /tmp/agentpm-skill-install
agentpm install @namespace/skill-name@0.1.0
```

2. Confirm:
- `.agentpm/skills/<namespace>/<name>/<version>/agent.json` exists
- `.agentpm/skills/<namespace>/<name>/<version>/SKILL.md` exists
- `agent.lock` has `skill:@namespace/skill-name@0.1.0` in `packages`

3. Agent dependency install:

```bash
agentpm install --manifest agent.json
```

using an agent manifest with both `tools` and `skills`.

4. Confirm:
- Skill packages are resolved and locked.
- Tool dependencies declared by those Skills are resolved and locked.
- Lock root has first-class `skills` field.
- `reserved` does not contain `skills`.

## Backend manual/API checks
- Publish a Skill to a public namespace and confirm receipt has `kind: "skill"` and a `/skills/...` URL.
- Attempt to publish a Skill with the same namespace/name as an existing Tool/Agent/Template and confirm a kind conflict response.
- Attempt to publish a Skill that depends on an inaccessible private Tool and confirm publish finalize fails with dependency access error.
- Install-resolve a Skill directly and confirm the response item returns stored kind `skill`.
- Install-resolve an Agent that depends on a Skill and confirm response includes the Agent, Skill, and Skill tool dependencies.
- Search with `type=skills` and confirm Skill results only.
- Search with `type=all` and confirm Skill results and `totals_by_type.skills`.
- Search as anonymous user and confirm private Skills are hidden.
- Search as namespace member and confirm accessible private Skills appear.

## Frontend manual checks
- Search page has Skills filter/tab.
- All search displays Skill results with a Skill badge/card treatment.
- Skill cards link to `/skills/<package_id>/v<version>/overview`.
- Skill detail page renders without trying to show tool-only runtime/input/output sections.
- Skill detail page shows the manual/readme content, declared tools, references, scripts, compatibility metadata, install command, license, signature status, and namespace visibility where available.
- Trending/homepage package areas include Skills when package lists are mixed.

## Expected evidence
Report back with:
- exact commands run
- passing test suite output or summarized pass counts
- sample generated `agent.json` from `agentpm init --kind skill`
- tarball entry listing for minimal and tool-backed Skill dry-runs
- sample lockfile excerpt showing first-class `skills`
- sample backend publish receipt for a Skill
- sample search API response or screenshot showing `skills` totals/filter
- screenshot or route confirmation for the Skill detail page
- any skipped checks and why they could not be run

## Out of scope
- Performance/load testing of search or install at registry scale.
- Full SPDX validation beyond existing license behavior.
- Runtime enforcement of `skill.compatibility` metadata.
- Verifying every third-party skill-aware ecosystem export target.
- Adding `agentpm export --skill --install`.
- Skill-to-Skill dependency behavior.
- Broad database/table renaming from `tools` to `packages`.
