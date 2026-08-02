# Test Plan

## Required verification

Phase 6D is complete only when the Profile contract, CLI lifecycle, dependency graph, registry behavior, web experience, database migration, and both SDK loaders are verified together.

Required end-to-end scenarios:

1. Initialize and lint a Profile package.
2. Publish a valid Profile without a build step.
3. Install the Profile directly.
4. Add the Profile directly to a local Agent and verify manifest mutation plus lockfile relationships.
5. Install an Agent with multiple Profile dependencies.
6. Generate a workspace from a Template with Profile dependencies.
7. Search for and view the Profile in the registry.
8. Load the installed Profile with Node and Python SDKs.
9. Load an Agent and inspect resolved Profile relationships in both SDKs.
10. Verify old lockfile `reserved.profiles` migration behavior.
11. Verify Profile download statistics, search indexing, and trending inclusion after the database migration.
12. Verify existing package kinds continue to work.

## Automated checks

Run commands from the indicated repository unless the repository scripts have changed. When scripts differ, use the repository's configured equivalent and report the exact command used.

### CLI and Rust SDK

From the AgentPM CLI workspace:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`

At minimum, targeted tests must cover:

- manifest schema and parsing
- Profile semantic linting
- Profile init
- direct install
- Agent Profile dependencies
- Template Profile dependencies
- lockfile migration and frozen mode
- download/extraction roots
- publish dry-run
- workspace/new flows
- shared SDK install DTO serialization

### Registry API

From the API repository:

- `pytest tests/test_publish_helpers.py`
- `pytest tests/test_install_helpers.py`
- `pytest tests/test_search_helpers.py`
- `pytest tests/test_tools_helpers.py`
- Run the complete API test suite using the repository's normal command, typically `pytest`.

Add and run migration verification using the repository's established database-test process. Verify both upgrade and downgrade behavior where supported.

### Registry web

From the web repository:

- Run the configured formatter/linter.
- Run the configured TypeScript type check.
- Run the configured component/unit test suite.
- Run the production build.

Expected repository-script forms, if unchanged:

- `npm run lint`
- `npm test -- --run`
- `npm run build`

At minimum, targeted tests must cover:

- Explore Profile filter
- global search Profile dispatch
- Profile card links
- public and private Profile detail fetches
- optional manifest fields
- constraint display and non-enforcement wording
- Agent and Template Profile dependency links

### Node SDK

From the Node SDK repository:

- `npm run lint` if configured
- `npm test -- --run`
- `npm run build`

At minimum, run the Profile loader and Agent loader tests covering first-class Profile relationships.

### Python SDK

From the Python SDK repository:

- Run the configured formatter/linter/type checker.
- `pytest`

At minimum, run the new Profile loader tests and existing Agent loader tests.

## Contract tests

### Valid manifests

Verify:

- Minimal Profile with required core fields.
- Full Profile using every supported field.
- Open-ended tone strings.
- Every verbosity enum value.
- Required and preferred constraints.
- Profile with README and license metadata.
- Agent with one and multiple Profiles.
- Template with one and multiple Profiles.
- String and object package references.

### Invalid manifests

Verify rejection of:

- Missing top-level `profile`.
- `profile` on a non-Profile package kind.
- Missing identity, role, objectives, communication, tone, or verbosity.
- Empty required strings and arrays.
- Unknown verbosity or constraint strength.
- Invalid constraint ID.
- Duplicate constraint IDs.
- Extra Profile fields at any object level.
- Empty optional objects such as audience, vocabulary, compatibility, requires, or recommends.
- Duplicate list entries where uniqueness is required.
- Vocabulary terms present in both prefer and avoid after trimming/case-folding.
- Profile package dependency arrays.
- Singular/plural misuse such as a Profile package using top-level `profiles` as dependencies.

## CLI lifecycle checks

### Init and lint

Run a flow equivalent to:

```bash
mkdir /tmp/agentpm-profile-test
cd /tmp/agentpm-profile-test
agentpm init --kind profile --name customer-success --description "Customer success behavior profile."
agentpm lint
```

Verify:

- `agent.json` exists and uses `kind: "profile"`.
- `README.md` exists and matches `readme`.
- No Profile build directory, instruction Markdown, scripts, references, or generated metadata exists.
- The starter has no `display_name` and no dependencies.
- `agentpm lint --strict` behavior matches existing warning policy.

### Publish

Run Profile publish dry-run and a real publish against the test registry.

Verify:

- No build command is requested.
- README and declared license file are included.
- Dependency-bearing Profiles fail before publication.
- Invalid Profile schema fails before upload/finalize.
- Published package kind is `profile`.
- Package detail URL follows the canonical `/profiles/<package-id>/v<version>/overview` pattern.
- The archive contains only `agent.json` and supported manifest-declared common README/license files.
- Undeclared Markdown, scripts, references, instruction directories, and other source files are excluded.

### Direct install

Install a Profile with no local manifest and with a local Agent manifest.

Verify:

- Files are extracted under `.agentpm/profiles`.
- No executable Tool behavior is attempted.
- A local Agent receives/updates a top-level `profiles` reference.
- `--update-range`, `--refresh`, and `--frozen` follow existing dependency behavior.
- The lock uses `profile:@namespace/name@version`.

### Agent graph

Publish or fixture an Agent with multiple Profile dependencies.

Verify:

- Backend resolve expands all Agent Profile requirements.
- Shared Profiles are deduplicated.
- Profiles are leaves with no outgoing relationships.
- The Agent root contains first-class `profiles` keys.
- Reachability pruning retains Profiles referenced by reachable Agent roots and removes unreferenced Profile packages where current pruning behavior applies.
- A direct package spec returned by the registry as `kind: "profile"` is accepted even when the outbound request uses the existing default Tool kind.
- An Agent `profiles` dependency fails when the referenced name resolves to a non-Profile package.

### Legacy lock migration

Use a lockfile v3 fixture with Profiles only under `reserved.profiles`.

Verify:

- Resolvable entries move into first-class root `profiles` when the lock is regenerated.
- Unresolvable entries remain under `reserved.profiles`.
- Existing package integrity and unrelated reserved data are preserved.
- Frozen reads do not rewrite or migrate an existing v3 lock containing `reserved.profiles`.
- Regenerating the lock migrates resolvable entries into first-class `profiles`.
- A v1 or v2 lock produces the existing actionable compatibility error when first-class Profile relationships require v3.

### Template/new

Create a Template declaring direct Profiles and a generated local Agent declaring another Profile.

Verify:

- Resolver request includes all Profile requirements.
- Packages install under `.agentpm/profiles`.
- Synthesized root `agent.json` contains resolved direct Template Profiles.
- Generated local Agent manifests retain their own Profile declarations.
- Workspace lock roots contain correct Profile relationships.
- No Profile-specific prompt is shown.
- Template variables are not interpolated into installed Profile packages.
- A Template `dependencies.profiles` entry fails when the referenced package has a non-Profile stored kind.

## API and database checks

### Package-kind support

Verify Profile support in:

- publish init/finalize
- install resolve/init/finalize
- package/version details
- search and trending filters
- trending queries
- namespace visibility and authorization
- yanking/signing/security metadata
- statistics responses

### Migration

Apply the migration to a database containing existing Tool, Agent, Template, Skill, Knowledge, and Memory rows.

Verify:

- Existing rows and views remain valid.
- A Profile row can be inserted/published after upgrade.
- `tools_kind_check` rejects unsupported kinds and accepts all seven supported kinds.
- `trending_tools` contains Profile rows and ranks them within the Profile partition.
- `tool_search_index` contains Profile rows.
- Profile installs increment package and aggregate counters through `on_install_completed()`.
- All recreated indexes exist.
- View refresh operations succeed.
- Downgrade follows repository policy and does not silently corrupt data.

## SDK checks

### Node

Verify `loadProfile`:

- resolves versioned and unversioned specs according to existing loader rules
- supports `profileDirOverride`
- returns typed common and Profile metadata
- rejects wrong-kind manifests
- rejects missing/malformed manifests
- does not read README as behavioral instructions

Verify `loadAgent`:

- exposes multiple resolved Profiles
- exposes locked-but-missing Profile metadata with nullable paths according to current conventions
- preserves package kind, key, version, and integrity

Verify generic Tool `load()` guides Profile callers to `loadProfile`.

### Python

Run equivalent checks for `load_profile`, `load_agent`, public exports, and Tool-loader guidance.

Confirm Node and Python agree on manifest field names and relationship semantics.

## Manual checks

- Review an initialized Profile package and confirm it feels structurally distinct from a Skill package.
- Publish a Profile through the normal CLI flow and confirm no build-related message appears.
- Browse Explore with the Instruction Profile filter and confirm cards and links use public-facing terminology.
- Open a Profile detail page and confirm the structured sections are understandable, optional sections disappear cleanly, and README is visibly separate.
- Confirm required constraints are not described as technically enforced.
- Open Agent and Template detail pages containing Profiles and follow the dependency links.
- Install a private Profile while authenticated and confirm an unauthorized user receives the same safe not-found behavior as other private packages.
- Generate a Template workspace with multiple Profiles and inspect `agent.json`, `.agentpm/profiles`, `agent.lock`, and `agentpm.workspace.json`.
- Load the same installed Profile through both SDKs and compare the returned metadata.
- Verify no UI, CLI, API, or SDK surface refers to Profiles as personas, magic personalities, executable policies, or guaranteed guardrails.

## Regression checks

- Initialize, lint, and publish one Tool, Agent, Template, Skill, Knowledge, and Memory fixture.
- Install one Tool, Agent, Skill, Knowledge, and Memory fixture through their supported install flows.
- Generate a workspace from the Template fixture through `agentpm new` and confirm generic Template install behavior remains unchanged.
- Run existing Template `new` tests with no Profile dependencies.
- Run existing Skill Tool-dependency tests.
- Run existing Knowledge and Memory build/publish tests to ensure Profile work did not alter build requirements.
- Verify old Agent manifests with empty or absent `profiles` still validate according to existing compatibility rules.
- Verify old Template manifests without `dependencies.profiles` still validate and generate correctly.
- Verify old lockfile fixtures deserialize and normalize without losing existing relationships.
- Verify Tool-only PAT publishing compatibility remains Tool-only while `packages:publish` permits Profiles.
- Verify search, trending, and install statistics still include all existing kinds.

## Expected evidence

Report:

- Exact commands run and pass/fail status.
- CLI output for Profile init, lint, publish dry-run, install, and `new`.
- Relevant generated `agent.json`, `agent.lock`, and `agentpm.workspace.json` snippets.
- Database migration upgrade/downgrade results.
- Query output showing Profile rows in `tool_search_index` and `trending_tools`.
- Install-stat evidence after a completed Profile install session.
- Screenshots of Explore, Profile Overview, README, Security, and Agent/Template dependency presentation.
- Node and Python loader output snippets showing equivalent structured metadata.
- Any skipped commands, environmental blockers, or behavior that could not be verified.
- Any deviation from the manifest contract or scope in `spec.md`.

## Out of scope

Do not test or implement:

- system-prompt compilation
- model calls or behavioral compliance evaluation
- Profile selection or phase bindings
- Profile merging, layering, precedence, or conflict resolution
- runtime enforcement of required constraints
- Profile parameters or install-time variables
- progressive disclosure or freeform instruction files
- provider-specific compatibility behavior
- Profile build, inspect, query, run, or harness execution commands
- nested Profile metadata full-text search
