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

### Python dependency schema/locking

Add tests for:

- Python Tool with dependencies accepted;
- Node Tool with dependencies rejected;
- invalid requirement syntax rejected;
- duplicate/conflicting dependency handling;
- lockfile v3 read;
- lockfile v4 read/write;
- Python resolved dependency state deterministic;
- publish rejects stale/mismatched dependency lock;
- published artifact includes Tool-specific lock snapshot.

### Payload compatibility

Fixtures:

- pure Python payload → `any`;
- `.so` payload → target-specific;
- `.dylib` payload → target-specific;
- `.pyd` payload → target-specific;
- vendored platform wheel contents → target-specific;
- unknown binary → conservative target-specific.

### Multi-artifact release

Automated cases:

- one artifact release finalizes;
- multiple target release finalizes;
- duplicate target rejected;
- mismatched manifest digest rejected;
- mismatched dependency lock rejected;
- one failed upload leaves release unpublished;
- finalized release artifact list immutable;
- adding artifact to finalized version rejected;
- failed staging objects can be cleaned up.

### Canonicalization / integrity

Create shared canonicalization fixtures consumed by Rust and Python.

Required cases:

- key order differences produce same canonical bytes;
- Unicode text serializes identically;
- artifact ordering is normalized;
- whitespace differences do not matter;
- timestamp normalization behavior is deterministic.

Tamper tests:

- modified target artifact bytes fail artifact integrity;
- modified release manifest fails release integrity;
- changed target label fails signed release verification;
- changed artifact digest fails signed release verification;
- changed manifest digest fails;
- changed dependency-lock digest fails;
- invalid author signature fails;
- invalid registry attestation fails.

### Install resolution

Test target matrix behavior:

- exact target selected when available;
- `any` used when no exact target and portable artifact exists;
- exact target preferred over `any` if policy says so;
- no compatible artifact returns actionable failure;
- headless no-compatible path is deterministic;
- pinned exact version never silently upgrades;
- unversioned request recovery path only occurs with explicit interactive consent;
- legacy package still follows legacy install path.

### CI/headless signing

Automated or integration cases:

- signing works without TTY;
- missing CI signing secret fails cleanly;
- required namespace signing policy enforced;
- matrix artifacts collected;
- final atomic publish succeeds;
- mismatched matrix artifact state rejected.

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

- macOS arm64;
- macOS x86_64;
- Linux x86_64;

verify a pure-Python Tool with a dependency that has native wheels can:

- publish under the new model;
- install on a different target;
- resolve the correct target-compatible dependency artifacts;
- run successfully.

Use an example such as a dependency chain containing a package with native components where practical.

Also verify:

- a legacy vendored Python Tool still installs/runs under legacy semantics;
- a local native publish only claims the publisher target;
- a multi-target CI publish produces one version with multiple artifacts;
- adding a target requires a new version.

### Integrity / provenance

Manually inspect a new release:

- release manifest;
- artifact list;
- per-artifact digests;
- release digest;
- author signature;
- registry attestation;
- selected artifact recorded locally.

Attempt install with:

- valid signatures;
- author signature absent under optional policy;
- `--require-signature`;
- registry attestation requirement;
- intentionally corrupted local cache/artifact.

Confirm failure messages are precise.

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
- example new `agent.lock` v4 entry;
- example release manifest;
- example S3/object inventory;
- example new release-level signature/attestation statement;
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
