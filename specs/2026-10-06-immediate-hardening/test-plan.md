# Test Plan

## Required verification

The stage is not complete until the major user flows pass end-to-end and legacy compatibility is preserved.

Verification must cover:

- registry search/discovery;
- pagination;
- filters;
- relevance;
- stars/trending;
- namespace curation;
- package detail shared shell;
- technical SEO;
- CLI messaging/lint;
- analytics/privacy/feedback;
- Python dependency portability;
- legacy Python Tool install;
- target-aware multi-artifact publish/install;
- release integrity/signing;
- GitHub Actions or equivalent CI flow.

Where the repo already has canonical test commands, use those commands rather than inventing parallel verification.

## Automated checks

### Registry / frontend / API

Run the existing frontend and backend unit/integration suites.

Add automated cases for:

- Explore page-size agreement.
- Cursor history Page 1 ↔ Page 2.
- 3+ page sequential navigation.
- >10 page backward navigation.
- query preservation.
- kind preservation.
- sort preservation.
- filter preservation.
- private-result visibility across all supported sorts.
- mixed namespace/package search.
- strict/relaxed relevance continuity.
- no-result state.
- error state.
- namespace-scoped search reuse.
- star create/delete/count.
- private star authorization.
- pinned artifact authorization/order/limit.
- trending full-result-set ranking.
- top-N presentation not truncating Explore sort.
- stable result keys/rendering where testable.
- package Security/detail DTO renders both legacy single-artifact versions and new-format release/artifact versions correctly once portability milestones land.

### Search relevance fixtures

Create and run a deterministic fixture corpus.

Required assertions:

- exact package-name match outranks semantic/dependency-only matches;
- namespace match behaves as expected;
- known typo still returns intended package;
- metadata-only match is discoverable;
- dependency-only Agent Package match appears but ranks below the directly matching dependency;
- full README-only terms do not influence results;
- filters are hard constraints;
- no-result query stays empty;
- result ordering is deterministic for ties.

### Schema / Template metadata

Validate:

- `agentpm-harness` accepted for Template execution surface;
- invalid execution surface rejected;
- existing execution surfaces remain valid.

### CLI

Run existing Rust/CLI tests.

Add fixture/command tests for:

- all eight `agentpm init --kind` scaffolds lint clean;
- Tool scaffold contains valid minimal files/entrypoint;
- oneOf/anyOf suppression;
- duplicate schema error removal;
- bounded instance echoes;
- semantic errors ordered first;
- closed-union expected-values message;
- JSON and NDJSON lint formats remain valid;
- `publish --dry-run` lint context;
- connection failures for install/export/new/whoami;
- `whoami` exit code 1 on connection failure;
- inspect output normalization;
- no `/./` path rendering;
- `new` success output;
- clap failure for missing `--mcp`;
- concise bad key-id message.

### Analytics

Use local/dev PostHog test configuration or event sink abstraction.

Verify:

- intended event emitted;
- unintended payload fields rejected/not sent;
- server events use authoritative completion points;
- billing webhook event mapping is idempotent;
- telemetry opt-out suppresses CLI events;
- anonymous install ID behavior is stable;
- private package identity does not enter telemetry payloads.

### Python dependency schema / lock / runtime environment

Add tests for:

- Python Tool with dependencies accepted.
- Node Tool with dependencies rejected.
- Invalid Python requirement syntax rejected.
- Duplicate/conflicting dependency handling.
- Portable Python resolution is deterministic.
- Resolution can represent conditional/environment-marker dependencies without publisher-target leakage.
- Current v2/v3 lock reads remain valid.
- New capability state writes minimum lockfile version 4.
- Future unsupported lockfile version is rejected before lossy deserialization/rewrite.
- Plain `agentpm install` lock regeneration preserves Python resolution state from the resolve plan.
- Package key remains logical `kind:name@version`, not target-qualified.
- New lock records release integrity but no local selected-artifact target/digest.
- Stale manifest/Python resolution mismatch blocks publish.
- Published release/artifact contains Tool-specific Python resolution metadata.
- Managed Python environment:
  - created under `.agentpm/` separate from payload;
  - keyed by Tool/version/target/interpreter identity;
  - exact applicable locked versions installed;
  - new-format Tool executes through managed interpreter;
  - changed `AGENTPM_PYTHON`/interpreter identity cannot silently reuse incompatible env.
- Legacy `_vendor` Tool still uses legacy run behavior.

### Payload compatibility

Fixtures:

- pure Python/data payload → `any`;
- `.so` payload → concrete target;
- `.dylib` payload → concrete target;
- `.pyd`/`.dll`/`.exe` payload → concrete target;
- known unpacked native dependency content → concrete target;
- unknown executable/binary → conservative concrete target;
- declared dependencies alone do not make payload target-specific;
- legacy package remains compatibility unknown/legacy;
- missing declared `files` path fails packaging rather than warning + omission.

Verify canonical target mapping for at least:

- `aarch64-apple-darwin`;
- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-gnu`;
- `x86_64-pc-windows-msvc` where test host/mocking permits.

### Multi-artifact release / publish storage

Automated cases:

- legacy single-artifact publish still finalizes.
- one-artifact new-format release finalizes.
- multiple-target new-format release finalizes.
- one release reservation owns multiple child artifact uploads.
- concurrent child artifact reservations for one version do not collide with legacy one-pending-upload rule.
- duplicate target rejected.
- mismatched identity/version rejected.
- mismatched manifest digest rejected.
- mismatched Python resolution digest rejected.
- one failed/missing upload leaves release unpublished.
- finalization creates one logical version + N artifact rows atomically.
- finalized artifact inventory is immutable.
- adding artifact to finalized version rejected.
- expired/failed staged objects can be cleaned up safely.
- legacy S3/read path remains valid.
- new S3 target object keys include canonical target + full digest.
- upload implementation streams rather than buffers complete artifact in memory (unit/integration evidence as appropriate).
- server-authoritative upload-size limit is honored.
- old client/new release produces clear unsupported-release-format error.
- malware scanning:
  - one scan scheduled per target artifact;
  - all-clean state requires appropriate artifact results;
  - infection of any artifact yanks/disables logical version.

### Canonicalization / stored-byte integrity / provenance

Create shared canonicalization fixtures consumed by Rust and Python.

Required cases:

- typed legacy author v1 statement serializes byte-for-byte like current implementation;
- new canonical key order is deterministic;
- non-ASCII Unicode serializes identically cross-language;
- artifact inventory order is normalized by target;
- whitespace/input map order does not alter canonical bytes;
- release digest matches in Rust/Python.

Stored-byte verification cases:

- client-declared metadata hash that does not match stored bytes cannot finalize a new release;
- stored object size mismatch cannot finalize;
- declared digest == trusted stored digest == release manifest digest on success.

Tamper tests:

- modified target artifact bytes fail artifact integrity;
- modified release manifest fails release integrity;
- changed target/artifact list fails signed release verification;
- changed manifest digest fails;
- changed Python resolution digest fails;
- invalid author signature fails;
- wrong/unregistered author key fails;
- invalid registry attestation fails;
- attestation signed by untrusted registry key fails.

Provenance-policy cases:

- namespace `required` enforces one valid signature.
- `--require-signature` requires one valid signature even under optional/off mode.
- `--require-attestation` requires cryptographic registry attestation, not a boolean.
- explicit `--sign` with a rejected signature fails publish with actionable diagnostic.
- new-format publish fails if registry attestation cannot be created.
- historical signature remains cryptographically valid after normal signer revocation while current revoked state is surfaced separately.
- legacy signature/attestation formats still verify as before.
- normalized key-ID display does not affect verification by full public key.
- historical registry key can verify an old attestation after rotation.

### Install resolution / cache / extraction

Test target behavior:

- exact target selected when available;
- `any` used when no exact target and portable artifact exists;
- exact target preferred over `any`;
- only selected artifact gets downloaded/presigned;
- no compatible artifact returns actionable failure with available targets;
- headless no-compatible path is deterministic;
- pinned/exact version never silently upgrades;
- unversioned recovery only with explicit interactive consent;
- portable lock remains unchanged across different local target selections;
- local `.agentpm/` state may differ by target/interpreter without touching `agent.lock`;
- cache entries cannot collide across kind/package/version/target/digest;
- cache hit re-verifies digest;
- downloaded size mismatch fails;
- symlink/hardlink archive entry fails explicitly if unsupported;
- traversal protection remains intact;
- decompressed-size cap enforced;
- extraction entry-count cap enforced;
- failed extraction/provisioning does not finalize install session;
- dependency provisioning failure leaves clear recoverable state;
- legacy package follows legacy install path.

### CI / headless signing

Automated or integration cases:

- encrypted AgentPM signing key works without TTY.
- missing key file/passphrase fails cleanly without secret leakage.
- wrong passphrase fails cleanly.
- namespace required-signing policy is satisfied by headless flow.
- package-build command emits machine-readable descriptor with:
  - target;
  - artifact SHA/size;
  - manifest digest;
  - Python resolution digest.
- tag/version mismatch fails workflow before publish.
- matrix artifacts are uploaded/downloaded through CI fan-in.
- final job rejects mismatched manifest/resolution descriptors.
- final job rejects duplicate targets.
- final atomic publish succeeds.
- missing/failed matrix target prevents publish.
- local single-target publish continues to work.

## Manual checks

### Registry

- Search for a query with 3+ pages and manually navigate:
  - Next twice;
  - Previous twice;
  - browser Back/Forward.
- Change filters while on later page and confirm reset to page 1.
- Verify Namespace typeahead usability.
- Verify Agent Package Includes AND semantics.
- Verify no-results state is understandable.
- Verify loading text:
  - mixed;
  - specific kind.
- Verify star rail visual transition is subtle and persistent.
- Verify Explore card star counts.
- Verify no empty Pinned section for visitor.
- Verify Owner/Admin empty pin state.
- Verify pinned ordering controls.
- Verify namespace-scoped search looks/behaves like Explore, not a separate system.
- Verify Tool Evaluations/Score & Rating placeholder is gone.
- Verify detail pages retain all specialized tabs.
- Verify a legacy Tool Security tab still renders correctly.
- Verify a new-format Tool Security tab shows release-level integrity/provenance and target-artifact inventory without presenting stars/popularity as trust signals.

### SEO

Using rendered HTML / browser dev tools:

- inspect title/meta/canonical/robots on:
  - landing;
  - pricing;
  - package version;
  - namespace;
  - docs;
  - Explore query URL;
  - 404;
  - authenticated/private route.
- inspect OG/Twitter preview values.
- verify sitemap contains intended public URLs only.

### CLI

Run representative commands in a real terminal and compare overall tone/shape:

- `agentpm init --kind tool`
- `agentpm lint`
- `agentpm publish --dry-run`
- `agentpm knowledge inspect`
- `agentpm memory inspect`
- registry-backed connection failure
- `agentpm keys export --key-id bad`
- `agentpm serve`

Confirm errors answer:
- what happened;
- where;
- why;
- what to do next.

### Analytics / feedback

- Inspect actual PostHog event payloads in dev.
- Confirm excluded fields never appear.
- Toggle telemetry off and rerun CLI milestones.
- Submit feedback form.
- Verify optional email and follow-up consent behavior.
- Verify reproducible-bug GitHub path.
- Exercise pricing → checkout click.
- Replay a Lemon Squeezy webhook in test/dev and verify authoritative event.

### Python portability

On at least:

- Apple Silicon macOS (`aarch64-apple-darwin`);
- Intel macOS (`x86_64-apple-darwin`);
- Linux x86_64 (`x86_64-unknown-linux-gnu`);

verify a new-format Python Tool whose dependency graph includes at least one package with native wheels can:

- resolve portable exact dependency state;
- package/publish under the new model;
- install on each target;
- select the correct target artifact or `any`;
- create an AgentPM-managed Python environment for that target/interpreter;
- install the correct target-compatible dependency distributions;
- run successfully through `agentpm run`.

Also verify:

- the same source-controlled `agent.lock` is usable across at least two target platforms without target-specific edits;
- changing the local selected artifact does not rewrite target information into the lock;
- changing `AGENTPM_PYTHON` to another compatible minor version does not silently reuse an incompatible native dependency environment;
- a legacy vendored Python Tool still installs/runs under legacy semantics;
- a local pure-source publish can claim `any`;
- a local native publish claims only the concrete current target;
- a multi-target CI publish produces one immutable logical version with multiple artifacts;
- adding a target requires a new Tool version.

### Integrity / provenance

Manually inspect a new release:

- canonical release manifest;
- artifact inventory;
- per-artifact digests;
- release digest;
- Python resolution digest;
- author signature v2;
- registry attestation v3;
- registry key ID/trust material;
- local selected artifact/runtime state under `.agentpm/`;
- `agent.lock` confirming no local target/artifact selection is persisted.

Attempt install with:

- valid author signature/attestation;
- author signature absent under optional policy;
- namespace required signature;
- `--require-signature`;
- cryptographic `--require-attestation`;
- signer that has since been normally revoked;
- intentionally corrupted local cache/artifact;
- tampered release manifest in a test fixture;
- no compatible target.

Confirm failure messages distinguish:

- artifact integrity failure;
- release-integrity failure;
- author-signature failure;
- registry-attestation trust/signature failure;
- compatibility failure.

### CI publish

Run or inspect a real GitHub Actions release workflow showing:

- tag matches `agent.json.version`;
- each matrix job runs package/build only;
- `actions/upload-artifact` produces per-target build outputs;
- final job uses `actions/download-artifact`;
- final job performs one AgentPM publish;
- encrypted key + noninteractive passphrase path works when signing;
- `AGENTPM_TOKEN` is not exposed in logs;
- no partial release is visible if one matrix target fails.

## Expected evidence

Codex should report back:

- all automated commands run and their status;
- test counts or relevant suite summary;
- the final search relevance fixture results;
- example corrected lint output before/after;
- example pagination API responses for Page 1/2/3;
- screenshots of:
  - Explore filters;
  - no-results state;
  - star rail;
  - namespace pins;
  - namespace-scoped search;
  - feedback UI;
- representative SEO metadata output;
- representative PostHog event payload with sensitive/excluded fields absent;
- example new `agent.lock` v4 entry showing portable Python resolution + logical release integrity and **no** local target selection;
- example machine-local runtime/install state showing selected target/artifact/interpreter fingerprint;
- example release manifest;
- example release reservation + child artifact DB/object inventory;
- example S3/object inventory including canonical target IDs and `release.json`;
- byte-compatibility evidence for typed legacy v1 author statement;
- example new release-level author-signature v2 and registry-attestation v3 statements;
- example trusted stored-byte checksum verification evidence;
- example registry public-key history/trust material;
- cross-platform publish/install matrix results;
- legacy Python Tool compatibility result;
- GitHub Actions run link or captured job output if available;
- anything that could not be verified and why.

## Out of scope

The test plan does not require:

- Stage 2 category-copy redesign;
- Agent → Agent Package full terminology migration;
- Package Health UX;
- semantic/embedding search;
- session replay;
- recommendation engine;
- ratings/reviews;
- universal quality scoring;
- broad eval framework;
- Rosetta/emulation compatibility;
- multiple CI providers;
- OIDC/keyless signing;
- general-purpose AgentPM Developer workflows.
