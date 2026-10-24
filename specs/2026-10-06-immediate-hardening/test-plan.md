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

### Platform coverage for this stage

AgentPM CLI CI currently runs on `ubuntu-latest` only, while releases ship macOS and Windows binaries. This stage adds current-target detection, native-payload classification, archive extraction changes, and local runtime-environment provisioning — all of which behave differently per platform and cannot be meaningfully verified on Linux alone.

- The suites covering target detection, payload classification, extraction, and runtime-environment provisioning must execute on macOS and Windows as well as Linux.
- `#[cfg(unix)]`-gated tests must not be the sole coverage for behavior that also has a Windows path.
- If a full matrix per pull request is too slow, the platform matrix must run on merge to main and on release tags; it must not be omitted.
- Evidence for the payload-classification and target-detection cases below is incomplete if produced only from a Linux runner.

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
- `auth_optional` falls through as anonymous on an unverifiable JWT and on an unverifiable PAT, leaving `g.auth` `None`; `auth_required` still returns 401 for the same token.
- stale-session star state: a viewer whose token no longer verifies receives `starred: false` for a package they have starred, and the star control does not render a wrong state that then fails on click.
- revoked PAT installing a private package reports rejected credentials rather than a missing package.
- pinned artifact authorization/order/limit.
- trending full-result-set ranking.
- top-N presentation not truncating Explore sort.
- stable result keys/rendering where testable.
- package Security/detail DTO renders both legacy single-artifact versions and new-format release/artifact versions correctly once portability milestones land.

### Package detail shell verification

Milestone 6 uses the existing shared `StarRail` and `PackageStatsGrid` across Tool,
Agent, Skill, Knowledge, Memory, Profile, Loop, and Template pages.

Run in `agentpm-web`:

```bash
pnpm vitest run src/components/ui/StarRail.test.tsx src/components/tools/PackageDetailShell.test.tsx src/components/tools/PackageSecurity.test.tsx
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```

Keep `PackageStatsGrid` server-only: the Installs and Last publish cells, `Panel`
wrappers, and `agoText` stay server-rendered. Only `StarStatCell` reads client star
context, shared with `StarRail` through the identity-keyed `PackageStarProvider`.
Verify both mobile and desktop Stars cells change during an optimistic write,
accept the server's returned count, and roll back with the rail after a failure.
Seed star state only when the identity-keyed provider mounts. A successful write
must not call `router.refresh()`, and changed initial props for the same mounted
identity must not overwrite its local star state. Verify a different identity
remounts with its own server state. This deliberately favors preserving the user's
write over reconciling concurrent changes from another tab or device; fresh server
state is loaded when the provider remounts.

Run UTC calendar-boundary tests under different process timezones:

```bash
TZ=UTC pnpm vitest run src/lib/date/ago.test.ts
TZ=America/Los_Angeles pnpm vitest run src/lib/date/ago.test.ts
```

The focused tests cover successful star/unstar writes, pending-write suppression,
401 recovery through sign-in with a return URL, server/network failure rollback,
protection from stale same-identity props, identity remounts, and all eight kinds' footer placement, popularity panels,
version switching, install/load/bootstrap/inspect/query actions, and specialized tab
links. Security tests use the real fetchers and response schemas, switch between
two versions with different integrity/provenance/scan/license values, and verify
the selected version's rendered data. The latest-publish panel must pair the
latest timestamp with the latest version, even when an older version is selected.

Run the existing API suite with `uv run pytest -q` using the repository's test
environment. The current Security contract remains the extension point for later
release/artifact data. Milestone 14C owns the new-format Security/detail UI and
release inventory verification, including legacy rendering compatibility, alongside
its public Security/read DTO changes. Milestones 13A–14B supply the prerequisite
release/artifact data and signing foundation/statements.

Browser evidence (2026-10-14): all eight kinds were rendered against the local app
at 390px and 1440px widths. All 16 checks confirm the package header fits the
viewport and the star control ends at the header's bottom border. Retained
measurements: [detail-bounds.json](assets/milestone-6/detail-bounds.json).
Representative screenshots: [Tool mobile](assets/milestone-6/tool-mobile.png) and
[Template desktop](assets/milestone-6/template-desktop.png). Session-expiry recovery
is verified with mocked Clerk and write responses; a live account sign-out/sign-in
was not performed.

### Search relevance fixtures

Create and run a deterministic fixture corpus.

Required assertions:

- exact package-name match outranks semantic/dependency-only matches;
- namespace match behaves as expected;
- known typo still returns intended package;
- metadata-only match is discoverable;
- dependency-only match appears but ranks below the directly matching dependency, verified for each declaring kind — Agent Package, Template and Skill;
- a Template is discoverable through its declared Agent Package dependency, the one edge no other kind produces;
- only direct declared dependencies match; a transitively-reachable package does not surface its ancestor;
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
- a manifest carrying a permissive `standard` field lints clean for all eight kinds, while a misspelled top-level key (`standrad`, `tool`) is still rejected — proving the root relaxed by one field, not opened;
- `standard` plus one extra unknown top-level key survives `agentpm install <pkg>` write-back and `knowledge build` (the existing default write mode) with no field loss; use a local schema override to allow the extra key for Knowledge build without opening the bundled schema root;
- root kind-union dispatch: a kind-specific violation (e.g. `kind: "agent"` missing a kind-required field) reports the offending branch/field and path, not `/oneOf` with the manifest echoed;
- the diagnostic shape can carry an external rule identifier next to the path, so Stage 2's APDS rule IDs attach without re-cutting the renderer;
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

#### Milestone 8 evidence

Verified 2026-10-15, scoped to scaffolding, packaging validation, schema tolerance,
and lint diagnostics. The transport/inspect/key-message cases above belong to
Milestone 9 and were not implemented here.

Commands and results:

- `cargo fmt --all` and `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo test --workspace --offline --quiet`: 1,077 passed, zero failed or ignored
  (1,057 CLI unit tests, six authoring integration tests, seven machine-run
  integration tests, seven Rust SDK tests). Localhost access was enabled for the
  existing mock HTTP/process fixtures; no live registry/provider was required.
- Black check on `crates/agentpm-cli/assets/templates/tool_main.py.tpl`: passed.
- API pre-commit checks for the six updated MDX pages: whitespace, EOF, and
  merge-conflict checks passed; Python-only hooks correctly skipped MDX.
  Frontmatter parsed successfully for all six pages; no routes or ordering changed.

`crates/agentpm-cli/tests/authoring.rs` drives the compiled CLI to verify all eight
fresh scaffolds pass strict lint with no issues, each accepts arbitrary `standard`
values without emitting the field, and each still rejects unknown root keys.
It asserts matching Python runtime metadata (`type: python`, minimum `version: 3`)
so published scaffolds are discoverable by runtime. It executes the Tool's JSON
echo source, preserves existing source, packages the
scaffold, and rejects missing file/directory/glob-literal entries without leaving a
partial archive. It checks actual lint/publish output equivalence and exact normal
JSON/NDJSON report and issue field sets.

The existing install mock-server test now asserts that `standard` and a future
unknown field survive a real dependency installation and manifest write-back.
The Knowledge build integration test verifies `standard` with the bundled schema,
then preserves both fields using a local schema override for the unknown key.
Knowledge build already writes by default; no new `--write` flag was added.
`serde_json` remains without `preserve_order`.

`lint_output` tests cover every listed union cliff, semantic-first ordering,
duplicate suppression, distinct structural constraints, retained sole structural
errors, Unicode/control-character bounds, and an optional external rule ID.
Custom schemas with references outside `$defs` retain their original validation
diagnostic when human-only expansion cannot resolve the reference.

Retained command output is in
[`assets/milestone-8/lint-before-after.json`](assets/milestone-8/lint-before-after.json).
It includes the complete manifests, commands, exit codes, and before/after stdout
and stderr for the invalid Loop transition, unknown Memory operation, and an
Agent missing `tools`. All exit 1. The Loop now prioritizes semantic errors without
schema object dumps; Memory names `consolidate, transform, delete` once; the Agent
points to `/tools` and `/oneOf/0/required`. Nested human-only expansion does not
change machine issue fields; root kind dispatch intentionally supplies the actual
branch/field diagnostics in all formats.

Release acceptance still open: record the actual CLI version shipping the
permissive `standard` field during Release Band 5. The installed baseline and
unreleased working-tree binary both report `0.1.34`; this evidence is not a claim
that the published `0.1.34` supports the change. No version bump was made here.

Operator walkthrough:
[`manual-tests/CLI_M8_MANUAL_TESTS.md`](manual-tests/CLI_M8_MANUAL_TESTS.md).
`scripts/cli_m8_manual.py setup` prepares gitignored `cli-m8-test/` fixtures;
its eight independent `run` sections use the real compiled CLI and require no
live registry. The install section manages an ephemeral loopback registry,
including a real artifact download/hash check and manifest write-back. Expected
CLI failures are asserted without altering the operator's shell error settings.
Each command retains stdout/stderr and its exact exit code; the summary requires
every expected command in all eight sections from the same binary digest.
The optional blog-sitemap browser check is separate from the CLI summary.
Human readability acceptance remains pending until the operator completes the
runbook's record.

Builder verification of the runbook helper (2026-10-15): `cargo build -p agentpm-cli
--offline` passed; fixture setup, all eight sections (122 CLI/source commands),
and the final summary passed. A summary requested before running any section
correctly exited 1. `python3 -B -m unittest discover -s scripts -p
test_cli_m8_manual.py -v` passed five guard tests, including both directions of
the expected-exit check and incomplete-section rejection. Both helper files
passed Black. The install smoke check used only its temporary localhost server;
no live registry or provider was contacted. These are builder smoke results, not
the operator's readability approval.

#### Milestone 9 evidence

Operator runbook:
[`manual-tests/CLI_M9_MANUAL_TESTS.md`](manual-tests/CLI_M9_MANUAL_TESTS.md).
`scripts/cli_m9_manual.py setup` creates empty, gitignored `cli-m9-test/`
directories and records the CLI digest; it does not run test cases or pre-create
their manifests/results. The operator runs each actual CLI command. The real
local backend is used for valid/revoked PAT plus private-package checks; the
optional loopback mock covers controlled status/decode failures only. Operator
acceptance was confirmed on 2026-10-16, independently of builder smoke
verification. The supplied local-registry transcript shows valid PAT identity
and validation-only login, successful private install, rejected credentials with
exit 1 after revocation, anonymous private not-found with exit 1, successful
anonymous public install, and informational no-token `whoami` against the closed
address. No credential value is retained in this evidence record.

Manual-runbook preparation verified 2026-10-16: six stdlib helper guard tests
passed; all 40 shell blocks passed both bash and zsh syntax checks; the local
Knowledge/Memory/Template commands were smoke-tested in a separate temporary
directory, including strict manifest lint. All six optional mock CLI commands
produced the documented actual messages/exit codes. Black and `git diff --check`
passed. The existing five CLI messaging integration tests passed on a serial
rerun with loopback access. An earlier parallel run encountered a transient
`WouldBlock` in the existing Rust mock socket reader; that fixture was not
changed by this runbook work. No operator PAT was created/revoked, and no live
private/public package install is claimed by this preparation check. The
default `cli-m9-test/` workspace was left uncreated for the operator.

README guidance follow-up: `new` now points to the generated `README.md` only
when it is a file; otherwise it suggests reviewing the generated project.
Authored entrypoints are unchanged. The new compiled-CLI regression failed on
the previous unconditional message and passes for both an absent README and a
directory named `README.md`. All six messaging integration tests passed serially;
workspace rustfmt/check and strict Clippy passed. The CLI docs' applicable
pre-commit text checks passed, and the manual runbook includes the fallback case.

Verified 2026-10-16. Changes are limited to non-Harness CLI messages/exit codes,
inspect presentation, and rejection of supplied invalid install credentials.

- `cargo fmt --all` and `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo test --workspace --offline --quiet`: 1,085 passed, zero failed/ignored
  (1,060 CLI unit tests, six authoring integration tests, five messaging
  integration tests, seven machine-run integration tests, seven Rust SDK tests).
- In `agentpm-api`, `uv run python -m pytest -q` with
  `AGENTPM_SEARCH_TEST_DATABASE_URI` pointing to local Postgres and a test-only
  registry signing key: 366 passed, 84 subtests passed, no skips.
- API pre-commit on the four touched Python files and ten CLI MDX pages:
  Black, isort, flake8, Pylint, pyupgrade, whitespace/EOF/conflict checks passed.
  Full mypy still fails on the previously recorded two assignments in untouched
  `app/tools/application/search.py` (lines 1486 and 1856). The touched files pass
  the same pinned mypy with `--follow-imports=silent`.
- The pinned pre-commit Bandit environment still lacks `pbr`. Running the same
  Bandit 1.7.5 separately with `pbr` and `setuptools` supplied passed on all four
  Python files, with zero findings. Neither baseline/tooling issue was changed.
- Frontmatter validation passed for all ten updated CLI MDX pages. Docs routes,
  order and navigation metadata were preserved.
- A PTY smoke walkthrough in a disposable directory exercised Tool init, strict
  lint, dry-run publish, Knowledge/Memory init/build/inspect, local-template
  `new`, failed `whoami`, unknown-key export and bare `serve`. The resulting
  output was inspected: success commands exited 0, the two runtime failures
  exited 1, and clap rejected missing `--mcp` with exit 2. No package was published.

`crates/agentpm-cli/tests/messaging.rs` executes the compiled CLI, rather than
only a formatter helper. Removing any install/export/new/whoami routing entry
reintroduces raw transport output and fails the test. The same coverage includes
publish, PAT login validation and namespace signer commands. It checks connection
failure exit 1, debug-only underlying transport details, successful and no-token
`whoami` exit 0, rejected credentials versus 403/404/500, invalid response handling,
missing `serve --mcp` exit 2, unknown-key recovery guidance, noninteractive/quiet
`new` success, and local Knowledge/Memory inspect headers. Credential/config
locations are isolated for these CLI test processes.

The sibling inspect test confirms the first four human lines are identity,
target, status, manifest, with no redundant `.` or working-directory absolute
prefix. It checks Knowledge staleness, full paths in each existing JSON layout,
and the missing Memory schema-reference message. Existing Memory status,
no-write and installed-package confinement tests remain passing. Unit coverage
additionally checks request timeout classification, URL/query suppression,
unchanged non-transport errors, and preservation of `..` components so display
cleanup does not pretend to resolve symlinks.

`tests/test_install_auth.py` verifies both resolve and init reject bad PATs,
expired JWTs and malformed auth while allowing no credentials and valid PATs.
Public optional-auth fallback and authenticated hidden/not-found behavior remain
covered. Its Postgres fixture uses the actual active-PAT lookup and real Argon2
verification: a generated PAT can access the fixture's private-package lookup,
then an actual SQL revocation makes both routes return 401 before that lookup.
The same private target returns 404 anonymously. Tables are transaction-local;
no real registry credentials, packages or membership data are changed.

Representative corrected output, also asserted by the command fixtures:

```text
Error: Cannot connect at http://127.0.0.1:<port> while trying to export Skill @example/tool. Check your network, registry address, and server availability, then retry.

Knowledge: fixture@0.1.0
Target: .
Status: fresh
Manifest: agent.json

Memory: fixture@0.1.0
Target: .
Status: fresh
Manifest: agent.json

Created project: generated
Next: review generated/README.md before running the generated entrypoints.

Error: No local key with id `missing-key` exists. Run `agentpm keys list` to find an available key.
```

Deployment dependency: deploy registry install-auth changes before or with the
CLI release. Older registries still silently downgrade invalid tokens, so a new
CLI cannot reliably identify rejection from their anonymous 404 alone. The
registry change is backward-compatible with existing clients' 401 handling and
retains anonymous public installs. No live credential was revoked during the
automated tests; the operator separately performed the local-registry revocation
walkthrough recorded above.
Human wording changed intentionally; inspect JSON field layouts and unrelated
machine protocols are unchanged. No CLI/SDK versions were bumped during
Milestone 9 implementation; the CLI version is prepared separately in Release
Band 5. Interactive wording/readability acceptance comes from the operator's
walkthrough above, not these automated results.

#### Release Band 5 preparation evidence

Historical preparation **2026-10-16** targeted CLI **0.1.35** / tag **v0.1.35**.
The CLI crate and Cargo lockfile agreed, and the website status page carried
that version/date; SDK markers were unchanged. **Superseded 2026-10-22:** the
shipping tolerance-release target and minimum APDS CLI floor is **0.1.36**,
prepared under Release Band 7 and now recorded in Band 5 and Stage 2's
spec/M1A.1/M3A/M4B. Publication is still unconfirmed. M8's actual-publication
task and Stage 2 M1A.1's publication task remain unchecked; **M3A must not emit
`standard` until actual publication of 0.1.36 is confirmed**. Prepared markers
and checked-off handoff tasks are not evidence that the release shipped. The
automated results below describe the original preparation, not publication.

- `cargo test --workspace --offline --locked --quiet`: **1,086 passed**, zero
  failed/ignored (1,060 CLI unit, six authoring, six messaging, seven machine
  integration, seven Rust SDK tests).
- `cargo clippy --workspace --all-targets --offline --locked -- -D warnings`:
  passed; `cargo fmt --all --check`: passed.
- Both manual-helper unittest suites: **11 passed**.
- Website `pnpm test`: **182 passed**; `pnpm typecheck`: passed; `pnpm lint`:
  zero errors, the same six existing unused-variable warnings.
- Website status-page Prettier write/check and `git diff --check`: passed.
  The metadata-only website update did not require restarting the running dev
  server; a production build was not rerun in this preparation step.

Deploy the registry's install-auth changes first, publish the CLI, then deploy
the website. No release/tag, push, deployment, or compatibility-gate activation
was performed by these preparation checks.

### Analytics

Use local/dev PostHog test configuration or event sink abstraction.

Verify:

- intended event emitted;
- unintended payload fields rejected/not sent;
- server events use authoritative completion points;
- billing webhook event mapping is idempotent;
- telemetry opt-out suppresses CLI events;
- anonymous install ID behavior is stable;
- private package identity does not enter telemetry payloads;
- a single Harness run emits exactly one `harness_started` and at most one `harness_completed`, and a TUI session running several prompts emits one pair per run rather than one pair for the session;
- the same run counted through `--headless` and through the TUI produces comparable events, differing only in the surface enum;
- a killed or panicking run leaves a start with no completion and does not retry or block;
- telemetry delivery failure and opt-out both leave the Harness exit code and output unchanged.

#### Milestone 10A Implementation Evidence (2026-10-16)

Implemented the explicit web/Registry allowlist, post-commit server events,
verified Lemon Squeezy webhook mappings, AgentPM feedback dialog, native
PostHog survey submission, and web/feedback docs/privacy disclosure. See
`analytics-contract.md` for the shared event/identity contract and
`manual-tests/ANALYTICS_M10A_SETUP.md` for the operator walkthrough.

Automated verification:

- API: `uv run python -m pytest -q`, with the existing local
  `AGENTPM_SEARCH_TEST_DATABASE_URI` configured: **383 passed, 88 subtests
  passed**, no skips.
- Web: `pnpm test`: **209 passed** across 59 files after the header-search
  follow-up; `pnpm typecheck` passed.
- `pnpm lint` passed with six existing unused-symbol warnings outside these
  changes. Prettier ran on every touched TypeScript/TSX file.
- `pnpm build` passed, including all 30 static pages, in an isolated temporary
  copy with fake build-only credentials. The operator's running dev server and
  `.next` cache were not replaced.
- API pre-commit ran against all 12 touched Python files and both MDX files.
  Black, isort, flake8, Pylint, pyupgrade and text checks passed. The full hook
  set is **not green**: existing imported `search.py` assignments at lines 1486
  and 1856 fail mypy, and the pinned Bandit hook environment cannot import
  `pbr`. Scoped same-version mypy with `--follow-imports=silent` passed all
  12 touched Python files. Bandit 1.7.5 with its missing runtime dependencies
  supplied (`pbr`, `setuptools`) reported no findings on those files. These are
  explicit baseline/tooling blockers, not claimed as successful hook runs.

The tests exercise production capture routes, reject extra/sensitive fields
and browser attempts to forge authoritative events, and prove commit-before-
capture, rollback/commit-failure suppression, and nonfatal capture outages.
Actual install-finalize, star/unstar, user-creation and publish paths are
exercised. The decorated billing service test verifies all four webhook
mappings, delivery only after commit, duplicate suppression, and no conversion
event from reconciliation. Web tests cover all eight package layouts,
cookie/auth correlation, opt-out, shared-browser identity rotation, feedback
failure/retry, and public-origin checks behind Render's internal hostname.

Playwright/Chrome checks passed on desktop (1440x1000) and mobile (390x844):
dialog bounds, Escape, retained answers after a 503, successful retry, stable
submission ID, and no page errors. Analytics and feedback requests were
intercepted: these checks sent **no real PostHog data**. Screenshots are retained
at `assets/milestone-10a/feedback-desktop.png` and `feedback-mobile.png`.

Still unmet: inspect the remaining real development event payloads;
replay test-mode billing webhooks against the live development integration;
finish the remaining **Early Product Funnel** dashboard panels; review production project/survey configuration,
privacy copy and feedback access/retention. Do not close Milestone 10A on mocked
results alone. Dashboard CLI/Harness panels must explicitly say pending
Milestone 10B. CLI opt-out and anonymous install-ID tests above belong to 10B,
not this implementation. Ordinary delivery is bounded-time/best-effort, not a
durable outbox; outages or a process crash can lose analytics events.

Operator confirmed live `Identify` and `landing_viewed` events and the expected
`contract_version`, `environment`, and `source` properties. That live inspection
also found an ingestion-added IP address despite `$geoip_disable: true`.
AgentPM's capture payload does not include an IP; project-side **Discard client
IP data** must be enabled and a new event inspected before accepting IP
retention behavior. This is now an explicit development/production setup check.
Operator subsequently enabled the development project's discard setting and
confirmed a newly captured landing event has the expected product properties
and no IP property. Development landing capture/IP retention is accepted;
production configuration and the remaining live checks are still pending.

Operator also configured the API survey/question IDs and confirmed a real
response in PostHog survey reporting: both text answers stored, email blank,
follow-up No. Its separate `feedback_submitted` event contains the expected
common metadata, `has_email: false`, and `follow_up: false`, without the submitted
text, email address, or IP property. This accepts live feedback storage and the
unchecked-consent branch. Subsequent screenshots confirm the API survey is
Running/Active, both submissions are Completed, and the supplied-email /
follow-up Yes case stores the email only in the survey. The corresponding
ordinary event has `has_email: true` and `follow_up: true`, with no answer text,
email address, or IP property. Both live consent branches are accepted.
The persistent Responses 1 / Completed 2 display is consistent with PostHog's
default unique-person performance counter: the two submissions share one
respondent. AgentPM does not emit survey impression events, so the displayed
Shown/response-rate metrics are not meaningful for this integration.

Operator created the single **Early Product Funnel** dashboard and supplied a
screenshot of its saved **Arrivals** panel: `landing_viewed` unique users and
total count, last 7 days, filtered to `environment = development`. Dashboard
creation and arrivals reporting are accepted; the other insights remain open.

Operator also supplied the saved **Discovery** panel with all four discovery
events as total counts, last 7 days, and `environment = development`; that panel
is accepted. Live review identified that global-header search did not emit
`explore_searched`. The header now captures at explicit nonempty submission,
using the existing sort/filter-count allowlist, never the query or URL. Arrival
on Explore stays silent to avoid double-counting the header submission.
`SearchAnalytics.test.tsx` exercises both real components and the real capture
helper with intercepted HTTP: Enter/form submission once, empty/whitespace
suppression, no capture while typing or on mount/rerender, exact allowlisted
request bodies without query/cursor, and header-to-Explore navigation without
a second event. Removing the production header capture call made the targeted
regression test fail with zero captures; the call was restored afterward.
Final follow-up verification: all 209 web tests, typecheck, lint (the same six
existing warnings), Prettier, and the isolated production build passed. Chrome
with analytics intercepted confirmed one search event from the header, still
one after Explore arrival and browser reload, and two only after an explicit
Explore submission. Both request bodies had exactly the allowed event and
sort/filter-count fields. The API MDX change passed applicable text pre-commit
hooks. Operator subsequently submitted a fresh header search and confirmed the
event appeared in PostHog Activity; live header-search emission is accepted.

Fresh-account signup: operator completed real email verification and observed
`signup_started`, `account_created`, and `$identify` in Activity. The signup
event's raw anonymous `distinct_id` exactly matches the identify event's
`$anon_distinct_id`; identify and account creation use the same account person.
The linking payload is accepted. Operator then saved **Signup** with Sequential
step order, Overall conversion, and a 7-day conversion window. The unobscured
chart shows one person at `signup_started` and the same completed journey at
`account_created`: one conversion, 100%, zero drop-offs. Fresh-signup identity
linkage and the Signup panel are accepted. Activity's original anonymous label
does not imply a failed merge.

Registry-use walkthrough: local API health and the real published
`@zack/bound-agent@0.1.0` were verified read-only. Operator ran an isolated
anonymous install against that local API and confirmed `package_install_completed`
in PostHog. A dependency-free Profile scaffold was created separately for smoke
verification; installed CLI `agentpm lint` and local-targeted `publish --dry-run`
both passed. No real publish was performed by the builder. The local-only
publish commands, local PAT handling, and cleanup are recorded in the manual
walkthrough. Operator subsequently completed the real local-only publish and
provided its saved **Registry use** panel: `package_install_completed` and
`publish_completed`, both total counts, last 7 days, filtered to
`environment = development`. Registry install/publish capture and that panel
are accepted; this does not claim CLI-local usage telemetry or a person-level
join from the anonymous install to a website visitor. Remaining live payload
inspection is still tracked separately.

Operator supplied the saved **Engagement** panel with `package_starred`,
`package_unstarred`, and `feedback_submitted`, all total count, last 7 days,
filtered to `environment = development`. This panel is accepted. Operator also
confirmed the CLI/Harness availability text card was added with the Milestone
10B disclosure and the registry-completion/execution distinction. The M10A card
is accepted; actual CLI-local measurement remains 10B, not implemented or
claimed here. Billing panels and the remaining operator acceptance checks are
still open.

Billing connectivity: operator confirmed the IntelliJ test-mode flag is true,
restored ngrok forwarding to backend port 5000, and ran a real unsigned POST to
the public `/billing/webhooks/lemonsqueezy` callback. It returned HTTP 403 with
`Invalid webhook signature`. Reachability and unsigned rejection are accepted;
this is not evidence of successful signed delivery or a subscription conversion.
Operator then reached a visibly test-mode checkout and confirmed `pricing_viewed`
and `checkout_started` in PostHog. After test purchase, operator reported the
expected `subscription_started` properties: `plan: pro`, `test_mode: true`,
`source: registry`, `environment: development`, `contract_version: 1`, and GeoIP
disabled. The conversion event is accepted. Operator replayed the webhook and
confirmed HTTP 200 with `ok: true` and `duplicate: true`. Backend duplicate
recognition is accepted. Operator confirmed PostHog still contains only one
`subscription_started` after replay, completing the duplicate-conversion check.
Two subsequent `subscription_updated` events correspond to distinct webhook
deliveries: `subscription_updated` at 02:22:15 UTC and
`subscription_payment_success` at 02:22:16 UTC on October 17. Operator verified
these event names and received timestamps from the database. Both map to the
generic analytics update event; they are not evidence of duplicate conversions.
Operator then cancelled the test subscription through the merchant dashboard and
reported `subscription_cancelled` with `plan: pro`, `test_mode: true`,
`source: registry`, `environment: development`, `contract_version: 1`, and GeoIP
disabled. Merchant-side cancellation capture is accepted. Operator then sent the
available `subscription_payment_failed` test simulation and confirmed the event
in PostHog with development environment, registry source, test mode true,
contract version 1, and GeoIP disabled. Its plan was `unknown`: current analytics
derives plan only from webhook product/variant IDs, not stored entitlement data.
Invoice payloads lacking those fields therefore have incomplete plan attribution;
cancellation status is not used to select the analytics plan. Payment-failure
event delivery is accepted, not plan-level classification. These checks do not
verify customer-portal cancellation or an actual failed renewal charge.

Billing dashboard: operator supplied a screenshot of the saved **Billing intent
and conversion** funnel with `pricing_viewed` -> `checkout_started` ->
`subscription_started`, last 7 days, and `environment = development`. It shows
one person reaching the final step at 100% conversion. This accepts the billing
intent/conversion panel. Operator subsequently supplied the saved **Billing
lifecycle** daily line chart with `subscription_updated`,
`subscription_cancelled`, and `subscription_payment_failed` total counts of
4, 1, and 1 respectively on October 17 UTC. The lifecycle panel is accepted;
the screenshot does not independently show its environment-filter setting.

Access review: operator confirmed they are the only PostHog organization member.
This accepts the organization-membership check. Operator separately confirmed
public sharing is disabled on the Development Early Product Funnel dashboard.
Operator subsequently confirmed public sharing is disabled on the Production
copy too.
Operator also confirmed Pay-as-you-go was selected at initial setup; no billing
change was made. PostHog's pricing documentation lists seven-year event/metadata
retention for paid plans. After being informed that stored survey text and
optional email use that retention, operator accepted the provider default for
this release. No shorter retention/deletion automation was introduced, and this
is not a promise of deletion exactly at seven years. The predeployment
access/sharing/retention/configuration review is accepted; remaining payload
inspection and production verification checks stay open.

Production survey: operator supplied the API survey JSON with the expected four
questions, optional email and follow-up, and Yes/No choices. The subsequent JSON
has `start_date: 2026-10-17T03:09:01.260000Z` and `end_date: null`, confirming
launch. Survey configuration is accepted. Operator subsequently confirmed all
specified production backend/web environment values were added to Render;
secrets were not shared or inspected. Environment configuration is recorded as
operator-confirmed, not deployed verification. Backend deployment, web rebuild,
and the live checks will follow the M10A code push: operator explicitly chose
to save Render environment values without redeploying. In particular,
deployed event capture and feedback storage, IP suppression, and the remaining
privacy checks are not yet verified. Operator confirmed all dashboard panels
were copied to Production and saved with `environment = production` filters.
Production dashboard configuration and disabled public sharing on that copy
are accepted; live production data is not yet verified.

Independent review: operator confirmed Claude's code review is already complete
and everything looked good. This is operator-reported review acceptance, not a
new test run, deployment confirmation, or resolution of the portal residual.

Release progress: operator subsequently reported everything was pushed out.
The release push is recorded; successful backend/web service rollout and live
production event capture, IP suppression, and feedback storage still require
verification. No deploy or runtime check was performed by the builder here.
Operator subsequently observed `landing_viewed` events in AgentPM Production
and shared properties from the event at `2026-10-17T04:11:04.638Z`:
`environment: production`, `source: web`, `contract_version: 1`, and GeoIP
disabled, without an IP property. This accepts the deployed web-to-registry
capture path and stored-event IP suppression for that event. It is not
independent inspection of Render build logs or verification of production
feedback storage and dashboard data, which remain open.

Production feedback storage: operator supplied the stored AgentPM Feedback
response from the production smoke test. Goal and obstacle appeared under the
correct questions, email was blank, and follow-up was No. Survey storage and
question mapping are accepted. Operator then supplied the separate
`feedback_submitted` event at `2026-10-17T04:14:21.632Z` with
`environment: production`, `source: registry`, `contract_version: 1`,
`has_email: false`, `follow_up: false`, and GeoIP disabled. The shared property
list contains no answers, email, or IP. Generic feedback-event flags and
separation from survey content are accepted for this production submission.

Production dashboard: operator confirmed charts now update with production
data. They also performed real star and unstar actions and observed those events
in Engagement. This accepts production dashboard data and star/unstar capture;
the ordinary-event payload spot-check was completed afterward. Operator reported
no unexpected properties when inspecting existing Development
`explore_searched`, `package_viewed`, `package_install_completed`, and
`publish_completed` against their allowlists. The privacy spot-check and scoped
M10A live operator acceptance are complete. The portal redirect and invoice-event
plan-attribution limitation remain recorded residuals, not resolved by these
analytics checks. CLI/Harness emission remains outside M10A and deferred to M10B.
This final checklist update did not change application code or rerun automated
tests; the earlier automated results and baseline hook limitations still apply.

Customer portal residual: Manage billing returns HTTP 200 and a URL at
`https://agentpm.lemonsqueezy.com/billing` with a signature, expiry, and
`test_mode=1`. Operator confirmed the portal is enabled. Following a fresh link
redirects to the merchant dashboard in the regular session and to
`https://auth.lemonsqueezy.com/login` in a fresh Incognito session. URL structure
checks do not establish signature validity or successful customer authentication.
Root cause and customer cancellation through the portal remain unresolved; do
not classify this as an expected test-mode restriction or a passed portal flow.

#### Milestone 10A Mobile Global Search Follow-up

Post-release operator testing identified a separate active mobile-header
component, `MobileGobalSearchClient.tsx`, that routed to Explore without calling
the analytics helper. Added capture at the existing nonempty submission point,
using the same `explore_searched` event, Relevance sort, and filter-count
calculation as desktop. No query, URL, or new event property is captured.

Expanded `SearchAnalytics.test.tsx` to exercise the real capture helper through
desktop header, mobile header, and Explore. Before the fix, three mobile cases
failed with zero requests; after the fix all 17 cases pass. They cover Enter and
form submission exactly once, silent typing and blank submissions, no event on
mount/rerender, exact allowlisted payloads, preserved search navigation, and no
second event when arriving on Explore.

Follow-up verification:

- Full web suite: **215 passed** across 59 files; typecheck passed.
- Lint passed with the same six existing unused-symbol warnings. Prettier check
  passed for both changed TSX files and the updated API MDX documentation.
- API documentation pre-commit text hooks passed; Python hooks were skipped
  because no Python files changed in this follow-up.
- Isolated production build passed, including all 30 static pages. No secrets
  were copied into the build fixture and the running dev server's `.next` cache
  was not replaced.
- Headless Chrome at 390x844 and 1440x1000 against the local dev server verified
  one search event per header submission, no typing/Explore-arrival/reload event,
  and a second event only after explicit Explore submission. Both requests had
  only the expected event/sort/filter-count fields; no page errors occurred.
  Analytics requests were intercepted, not sent to PostHog. Screenshots were
  inspected for the mobile header and saved in `/private/tmp/agentpm-search-*`.

Production mobile-header capture after deployment remains an unchecked operator
follow-up in the manual guide; the earlier production acceptance did not cover
this separate mobile component.

#### Milestone 10B Dashboard Acceptance (Pending Implementation)

The two unchecked dashboard questions under M10A are owned by the explicit M10B
dashboard tasks, not satisfied by emitting the events alone. Verify:

- The existing Early Product Funnel dashboard in both Development and Production
  contains a CLI adoption chart based on `cli_first_used` and unique anonymous
  installations, matching the chosen first-use frequency. It is not presented
  as all CLI command usage or package installs.
- A Harness activity chart shows `harness_started` and `harness_completed`
  counts matching real Development test runs; opt-out emits no local events.
- Each project's saved insights use the correct environment filter. Verify
  production capture after the Release Band 7 rollout without fake events.
- Update the M10A availability card only after release and verification. Preserve
  the registry-completion/local-execution distinction and the disclosure that
  missing or opted-out telemetry does not establish zero usage. Do not imply
  that anonymous CLI installations are identified website accounts.

These are future acceptance checks; no M10B telemetry or charts were implemented
or verified when adding this tracking section.

#### Milestone 10B Implementation Evidence (2026-10-22)

Implemented only M10B across the CLI and Registry; no SDK, web, manifest schema,
version or release changes. The CLI uses the existing Registry analytics
integration through anonymous, size/rate-limited `POST /analytics/cli`; the
PostHog project key remains server-side. `analytics-contract.md` recorded
once-per-installation first-use attempts, per-run Harness events, the three
surfaces and the seven terminal outcomes before instrumentation.

Verification:

- `cargo test --workspace --quiet -- --test-threads=1`: **1,103 passed**
  (1,071 CLI unit tests, 25 CLI integration tests, 7 Rust SDK tests).
  Eleven new unit/engine tests and six real-binary integration tests cover
  atomic concurrent ID/first-use initialization, persistence, invalid/unreadable
  state, both opt-outs, mid-session disable, all outcome categories, identical
  content-free per-run events across surfaces, multiple runs in one session,
  interactive approval pause/cancel, handled runtime errors, real engine panic,
  killed processes, bounded/disconnected queues, stalled delivery, unchanged
  success/failure output and exit codes, and CLI help disclosure. Fixture
  manifests pass the real `agentpm lint --strict`; existing subprocess tests
  explicitly opt out rather than writing real installation state.
- The default parallel workspace run exposed an existing `harness_config`
  fixture collision: `temp_workspace("valid")` uses time-only names and one
  test can delete another test's workspace. That unrelated helper was not
  changed; the complete serial run above passed. An asynchronous capture-count
  race in the new opt-out test was fixed by using a separate receiving endpoint;
  the kill fixture now remains running until explicitly killed.
- `cargo fmt --all`, `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- API `uv run python -m pytest -q -rs`, with the standard fixture signing key:
  **415 passed, 7 skipped, 88 subtests passed**. The skips require
  `AGENTPM_SEARCH_TEST_DATABASE_URI` for existing revoked-PAT, search relevance
  and sitemap fixtures; no M10B test needs Postgres or live PostHog. The CLI
  ingress tests reject additional top-level/property fields and invalid
  categories, enforce event authority/UUIDv4/size, preserve anonymity despite
  supplied auth/browser headers, and exercise disabled/failed capture.
- API native pre-commit: Black, isort, Flake8, Pylint, pyupgrade and text hooks
  passed after formatting. Native mypy still reports the pre-existing
  `app/tools/application/search.py:1486,1856` assignments; native Bandit still
  cannot import its missing `pbr` dependency. An isolated run of pinned
  Bandit 1.7.5 with `pbr` supplied passed without findings. No project dependency
  or unrelated search code was changed to silence these baseline issues.
- Manual Python fixture formatted with the API's Black 23.9.1. Its documented
  headless and initialize/start-run/shutdown machine commands were exercised
  against loopback capture: one first-use event and the two surface-specific
  pairs, stable installation ID, no prompt/package/path/credential leakage.
  The fixture and machine process were stopped after verification. This was
  local capture, not a claim of live PostHog acceptance or a manual TUI pass.
- Operator-confirmed Development PostHog verification (2026-10-22): received
  `cli_first_used`, a headless start/completion pair, and two TUI pairs from
  separate prompts in one session. All share the same `cli_` distinct ID;
  reported properties have the intended CLI/platform/surface/outcome fields,
  `source: cli`, `environment: development`, contract version 1, GeoIP disabled,
  person-profile processing false, and no reported IP/private content. Saved
  CLI Adoption (Unique Installations) in the existing Early Product Funnel:
  `cli_first_used`, unique users, one installation. Saved Harness Activity
  (Per Run): both events by total count, `runtime_mode` breakdown, and the
  missing-completion caveat. Both insights have Development/source filters;
  operator confirmed headless totals 1/1 and TUI totals 2/2. This accepts the
  Development capture and chart counts, not Production readiness.
- Operator confirmed both copied insights now appear on Production's existing
  Early Product Funnel dashboard. Screenshots verify saved event filters
  `environment = production` and `source = cli` on both insights, unique-user
  aggregation for first use, total-count aggregation for both Harness events,
  `runtime_mode` breakdown, and the missing-completion caveat. No matching
  events is expected before CLI rollout; it is not proof of zero usage.
  Operator also confirmed Production dashboard public sharing remains disabled.
  Live post-release capture remains pending.

Remaining acceptance and release risks:

- Continue `manual-tests/CLI_M10B_MANUAL_TESTS.md` for post-release Production
  capture and final payload privacy checks. Leave the pending card until
  release/live verification; do not inject fake Production events.
  Cross-project dashboard checkboxes remain open until
  their Production requirements are satisfied.
- Deploy the API endpoint, docs and updated Privacy Policy **before** releasing
  the telemetry-enabled CLI. Release preparation below does not deploy, publish
  or close Band 7. Production capture remains an operator gate.
- There is intentionally no exit flush, join, retry or durable queue. Short
  commands and process termination can lose events, including first-use; missing
  completion is an upper bound on failures, not a failure rate. Unavailable
  config/state/entropy disables collection, not commands. CLI identities must
  never be interpreted as website accounts or reliable execution ledgers.

#### Release Band 7 Preparation (2026-10-22)

Prepared CLI **0.1.36** (`v0.1.36`) in the CLI crate and Cargo-generated
lockfile. Updated the website status card to **v0.1.36** with release date
**2026-10-22**. SDK crate and website SDK markers are unchanged. The earlier
M10B implementation/manual evidence used 0.1.35; this is the subsequent release
version preparation, not a claim that the release is already published.

Verification:

- `cargo check --offline -p agentpm-cli`: passed; lockfile diff is only the CLI
  package version.
- `cargo test --offline -p agentpm-cli --bin agentpm telemetry --quiet`:
  **11 passed** against the bumped version.
- `cargo run --offline --quiet -p agentpm-cli -- --version`: **agentpm 0.1.36**.
- `cargo fmt --all` and `cargo fmt --all --check`: passed.
- Website `pnpm typecheck`, `pnpm lint` and `pnpm test`: passed;
  **215 tests across 59 files**. Lint retains six existing warnings outside the
  status-page edit; no new lint errors.
- Scoped website Prettier write/check and both repositories' `git diff --check`:
  passed. A website production build was not rerun for this metadata-only change.

Release order remains Registry/API + docs/Privacy Policy first, CLI publication
second, and the matching website status marker afterward. No tags, publication,
deployment, Production telemetry acceptance or availability-card update were
performed by this preparation.

The M8 final publication task, Band 5 release-gate note, and Stage 2
spec/M1A.1/M3A/M4B handoff now designate **0.1.36** as the APDS minimum CLI floor.
This supersedes 0.1.35 only as the planned tolerance-release handoff; historical
test transcripts remain unchanged. Publication was unconfirmed at preparation;
the later operator confirmation below satisfies the publication prerequisite.

#### Release Band 7 Operator Publication / Production Capture Evidence (2026-10-22)

Operator reported backend/web deployed before publishing/installing CLI
**0.1.36**. Local inspection confirmed the installed binary resolves under
`/opt/homebrew/bin/agentpm` and reports **agentpm 0.1.36**. Its normal config
already contained a first-use marker from earlier local testing; the operator
backed up that empty marker while preserving the installation ID, then reran
`whoami` against Production. This explains the initially absent first-use event
without claiming an ingestion outage or changing the once-per-installation
contract.

Operator then confirmed Production `cli_first_used` at
**2026-10-22T21:13:36.219Z**, with `cli_version: 0.1.36`, `source: cli`,
`environment: production`, contract version 1, macOS/aarch64, GeoIP disabled,
person-profile processing false, and no reported IP/private content.

Operator also shared Production TUI `harness_started` at
**2026-10-22T21:18:08.215Z** and `harness_completed` at
**2026-10-22T21:18:10.377Z**, both carrying CLI 0.1.36, production/cli,
macOS/aarch64, contract version 1 and the same privacy flags. Completion carried
`terminal_outcome: handed_off`; operator confirmed this was an actual Agent in
`agentpm-examples`, not the deterministic fixture whose expected outcome is
`ended`. This is a valid completion outcome, not an assertion of fixture success.
Operator confirmed the Production CLI Adoption and Harness Activity dashboard
charts now reflect the installation and runs. Environment-filter and live-count
acceptance is complete in both projects; no fake Production events were needed.

M8 and Stage 2 M1A.1 publication tasks are now checked on operator confirmation,
not merely prepared version markers or an independent release-channel audit.
Stage 2 M3A's publication prerequisite is satisfied; other Stage 2 dependencies
and tests remain unchanged.

Final operator acceptance (2026-10-22): confirmed the dashboard availability card
was updated to distinguish anonymous installations, website accounts, per-run
Harness activity and Registry completions, including the warning that optional
or missing telemetry is not zero usage. Operator also confirmed the public
Privacy Policy spot-check includes minimal CLI telemetry and both opt-out
methods. This closes the remaining M10B dashboard and Release Band 7 acceptance
items on operator confirmation, not an independent browser audit. No runtime
code changed during this closeout; `git diff --check` passed.

### Python dependency schema / lock / runtime environment

Add tests for:

- Python Tool with dependencies accepted.
- Node Tool with dependencies rejected.
- Invalid Python requirement syntax rejected.
- Duplicate/conflicting dependency handling.
- Canonical `PythonResolution` object uses type `agentpm.python-resolution.v1`.
- `PythonResolution` contains `python.requires`, normalized authored `requirements`, and exact `packages`.
- Package entries contain normalized name + exact version + optional normalized PEP 508 marker.
- Resolution ordering is deterministic (`requirements` and `(name, marker-or-empty, version)` package ordering).
- Resolution can represent conditional/environment-marker dependencies without publisher-target leakage.
- Resolution rejects/never records wheel filenames, wheel tags, local paths, publisher target/architecture, interpreter paths, or local environment IDs.
- Every package entry carries a non-empty `hashes` array of `sha256:` + 64 lowercase hex digests.
- `hashes` is sorted lexicographically and participates in canonical-byte determinism.
- Digests are not keyed by target/filename/wheel tag; the same resolution is accepted on more than one target.
- A multi-wheel package records digests for the distributions all supported targets could install, not only the publishing machine's wheel. **Verify by resolving on one platform and installing on another** — a resolution that only covers the publisher's wheel passes a same-platform test and fails here.
- Resolution **fails** when a resolved version has no retrievable digest, rather than emitting an empty/omitted `hashes` array.
- A `PythonResolution` with any digest-less package entry is rejected before it can reach `agent.lock` or a release.
- Equivalent `PythonResolution` values produce identical RFC 8785/JCS bytes/digests in Rust and Python.
- Current v2/v3 lock reads remain valid.
- New capability state writes minimum lockfile version 4.
- Future unsupported lockfile version is rejected before lossy deserialization/rewrite.
- Plain `agentpm install` lock regeneration preserves Python resolution state from the resolve plan.
- The same resolution digests identically through `agent.lock`, the resolve DTO, and the release-manifest digest input; a divergent re-serialization on any one surface fails rather than yielding a second digest for the same resolution.
- Lock size is measured against a realistic multi-dependency Python Tool, not the unit fixtures, and the measurement is recorded as evidence. For scale, `pydantic>=2.8,<3` alone is 5 packages / 56 digests / ~5.7 KB canonical.
- Whatever serialization is chosen for v4, existing v2/v3 locks still round-trip byte-identically.
- Package key remains logical `kind:name@version`, not target-qualified.
- `PackageKind` variants stay in agreement across the CLI lockfile/semver types and the SDK install DTOs; a variant added to one side without the other fails a test rather than passing review.
- New lock records release integrity but no local selected-artifact target/digest.
- Stale manifest/Python resolution mismatch blocks publish, using the same input fingerprint the author-side resolution recorded rather than a second staleness rule.
- Once the M11A publication block lifts, a dependency-bearing publish writes resolution state into `agent.lock` and the published release carries it; a publish whose lock state is missing or stale still fails.
- The registry no longer rejects non-empty `runtime.dependencies` at `publish:init`, **and** a direct API publish that bypasses the CLI cannot create a dependency-bearing release with no persisted resolution. Verify the server path separately from the CLI path — the CLI gate alone is not the control.
- A publish that cannot succeed fails before resolver acquisition; assert no resolver download was attempted on a cold cache.
- Published release/artifact contains Tool-specific Python resolution metadata.
- Re-resolution fingerprint:
  - unchanged requirements/constraint/target-set/`uv` version → `agentpm publish` reuses the stored resolution and does not re-resolve;
  - changed authored requirement → re-resolves;
  - **bumped `uv` pin with identical requirements → invalidates the stored resolution.** This one fails if the `uv` version was left out of the fingerprint, which is the easy omission.
- Managed Python environment:
  - created under `.agentpm/` separate from payload;
  - keyed by Tool/version/target/interpreter identity;
  - exact applicable locked versions installed;
  - new-format Tool executes through managed interpreter;
  - changed `AGENTPM_PYTHON`/interpreter identity cannot silently reuse incompatible env;
  - provisioning replays the stored set and cannot install a package absent from it, even when a dependency's own metadata requires one;
  - an interrupted provisioning run converges to the correct complete set on retry rather than leaving partial state;
  - environment creation does not depend on the interpreter's `venv`/`ensurepip`. **Verify against an interpreter with `ensurepip` unavailable** — this is the Debian/Ubuntu `python3-venv` case and the reason `uv` is on the consumer side at all.
- Install-time digest verification:
  - distribution whose bytes match no recorded digest fails the install fatally, with no warning path and no retry from an alternate source;
  - failed verification does not leave an environment that looks complete or a successful install session;
  - distribution missing from the index, digest mismatch, and no-wheel-for-this-target produce three distinguishable errors.
- Wheel-only install:
  - a locked version with no target-compatible wheel fails with the no-compatible-wheel error naming package, version, and target;
  - no source build is attempted even when an sdist digest is recorded and a toolchain is present.
- Managed resolver acquisition:
  - lazy — no resolver download occurs for a Tool with no declared Python dependencies;
  - cached binary is reused on a second install **with no network access**;
  - downloaded bytes failing the compiled-in SHA-256 are rejected and not cached;
  - a PATH-installed `uv` of a different version is never selected;
  - `AGENTPM_UV_PATH` at the pinned version is used;
  - `AGENTPM_UV_PATH` at a different version is rejected with an error naming the required version (not a warning);
  - an unsupported target fails **before any network access** — assert no request was attempted, not merely that the command failed — and the error names the supported targets and the override;
  - unsupported target, network failure, digest mismatch, override version mismatch, and resolver execution failure are five distinguishable errors.
- Resolver pin table:
  - the generator fails when any supported AgentPM target lacks an artifact;
  - the generator rejects an upstream artifact whose attestation does not verify against `astral-sh/uv`;
  - the committed pin table and the `release.yml` target matrix are checked for agreement, and the check **fails** when a target is added to one and not the other. Verify by mutation: add a target to the matrix only.
- Legacy `_vendor` Tool still uses legacy run behavior.

#### Milestone 11A Evidence (2026-10-22, America/Denver)

Scope: declaration/lint, portable resolution, resolver acquisition and input
fingerprints only. No v4 lock writes, install DTO regeneration, managed Python
environment, payload classification, multi-artifact release or Node/Python SDK
changes were implemented. The overlapping lock/DTO/build/inspection/release
checklist items remain unchecked for their owning later milestones. Both CLI
and Registry `publish_init` reject non-empty Python dependency declarations
before upload until portable lock/release integration exists; direct API calls
cannot use those declarations to bypass the staged publication restriction.
Legacy declarations and empty Python dependency arrays retain their existing
validation path. Dry runs prepare typed author-side state under
`target/agentpm/python-resolutions/`, already excluded by the packager; this is
not advertised as portable `agent.lock` state or published resolution metadata.

Implementation coverage:

- Optional Python-only schema field and runtime readers, PEP 508/440 parsing,
  normalized names/extras/requirements, contradictory constraints and overlapping
  duplicate roots. Node rejects the field even when it is an empty array.
- Closed `agentpm.python-resolution.v1` type, exact normalized versions,
  mutually exclusive marker branches, mandatory full lowercase SHA-256 sets,
  deterministic array ordering, canonical bytes and digest. Rust and Python
  tests share the string-only contract vector in
  `crates/agentpm-sdk/contracts/python-resolution-v1.json`; JCS serialization in
  the producer uses `serde_jcs`, without changing legacy author-signature JSON.
- The target enum is generated from
  `crates/agentpm-sdk/platforms/targets.json`; current target uses Cargo's full
  build target, not analytics OS/CPU fields. CI/release matrix and generated pin
  coverage use the same list. Mutation tests add targets to only one side and
  assert rejection.
- Exact override validation, no implicit PATH search (including a bare override
  name), lazy acquisition, digest/version-verified cache reuse, valid downloads,
  tampering rejection without caching/execution, unsupported target with an
  asserted zero-connection listener, and distinct operator error categories.
- A real CLI subprocess routes through a local HTTP proxy and verifies that
  publish credentials are not forwarded to resolver/storage requests. Required
  downloads keep standard proxy support with explicit 10-second connect and
  120-second total deadlines, which also bound idle reads. Optional telemetry's
  no-proxy behavior is unchanged.
- Fingerprints include normalized roots, Python minimum, supported targets and
  the resolver version. Matching state is reused without acquiring uv; changed
  roots, missing/stale fingerprints and explicit refresh re-resolve; corrupt
  resolution digests fail closed. The uv version is compiled directly from the
  generated table, so adopting a new pin changes the fingerprint automatically.
- Registry presigning uses existing S3 configuration and a fixed, allowlisted
  immutable object key, including historical per-version inventories. Unknown
  versions/targets cannot select arbitrary keys. Direct publication rejection is
  tested before any database or S3 work.

Live upstream verification and resolution smoke test:

- Final pin: **uv 0.13.5**, the upstream release observed on 2026-10-22. The
  checked-in generator downloaded all four supported archives, retrieved public
  GitHub attestation bundles, verified them with `gh attestation verify --repo
  astral-sh/uv --bundle ...`, and independently computed the raw executable
  digests. Generated CLI/current Registry/version-history inventories agree.
  No upstream checksum file was treated as the trust anchor. Artifacts were
  **staged only**, not uploaded to S3.
- On macOS arm64, the attested 0.13.5 binary resolved `click==8.1.8` and
  `pydantic==2.11.7` for Python `>=3.11` and all supported target environments.
  Seven distributions were recorded, including Windows-only Colorama and 24
  distribution hashes for `pydantic-core`, including its sdist. The canonical
  resolution digest was
  `ac4666eda2701aec443fcd31f6972fd94a7467a1a410c50a24c7d7f600775ef1`.
- Final engine path creates a fresh internal `uv lock --no-build`, then exports
  that exact lock using `uv export --locked --format pylock.toml`. This retains
  sdist digests without executing source builds. Temporary inputs come only from
  `agent.json`; user project/uv configuration and AgentPM credential variables
  are not inherited as resolver configuration. No consumer dependency
  installation or foreign-platform execution is claimed by this smoke test.

Verification commands/results:

- `cargo test --offline --workspace --quiet --no-fail-fast -- --test-threads=1`:
  **1,126 passed** (1,083 CLI unit, 29 CLI integration and 14 Rust SDK tests).
  Added 23 Rust cases. Initial full runs exposed intermittent reads in unchanged
  messaging/telemetry mock servers: accepted sockets could inherit nonblocking
  mode on macOS. Those test helpers now explicitly use blocking accepted streams;
  repeated full runs pass without changing production telemetry or messaging.
  Local compiler was Homebrew Rust 1.98.0; the existing CI 1.89 toolchain and
  foreign-platform execution were not run locally.
- `python3 -m unittest discover -s scripts -p test_resolver_pins.py -v`:
  **8 passed**, including failed provenance, incomplete target coverage,
  independent byte hashing, inventory immutability, target drift and the
  cross-language canonical vector.
- `python3 scripts/check_resolver_targets.py --registry-pins
  ../agentpm-api/app/tools/domain/tools/uv-pins.json`: passed; release matrix is
  derived from that checked source rather than a second hand-maintained list.
- API `.venv/bin/python -m pytest -q -rs` with the standard non-secret fixture
  signing key: **437 passed, 7 skipped, 88 subtests passed**. The 16 new resolver
  download/publication cases pass. Skips remain the existing optional PostgreSQL
  revoked-PAT, relevance and sitemap cases requiring
  `AGENTPM_SEARCH_TEST_DATABASE_URI`.
- `cargo fmt --all` / `--check`, workspace/all-target Clippy `-D warnings`,
  pinned Black 23.9.1 for maintainer scripts, and API native Black/isort/flake8/
  pylint/pyupgrade/text hooks passed on touched files. Full native hooks were
  attempted: mypy still reports the existing `search.py:1486,1856` errors, and
  the Bandit 1.7.5 hook environment still lacks `pbr`. The same pinned Bandit
  version with `pbr` supplied in an isolated tool environment scanned all touched
  Python code with no issues; five narrow, explained annotations cover the
  fixed HTTPS upstream fetches and trusted verifier subprocess/exception imports.
  No project dependency or baseline search code was changed to hide these issues.

Review hardening evidence (2026-10-22):

- `scripts/resolver_smoke.py` executes the attested native binary against a fixed
  Click/Pydantic fixture. Its lock/export commands and the production CLI now
  consume `platforms/uv-resolution-commands.json`; the fake resolvers independently
  assert the exact argv, including `--locked`, `--no-emit-project` and `--no-build`.
- Adoption requires pylock `lock-version = "1.0"`, valid package/version/hash
  fields, distinct native wheel hashes and Python 3.11 wheel coverage for **all
  four** supported targets, the native package's sdist hash and the Windows-only
  Colorama marker. Unknown target coverage fails closed rather than silently
  treating an older fixture as sufficient for a new platform.
- The `Resolver Contract` workflow is scoped to relevant pin/contract PR changes
  and manual candidate-version dispatch, with a native runner for each target.
  It only emits execution reports; ordinary Rust/Python tests remain offline.
  Full ingestion revalidates report output, binary digest, version, target and
  contract fingerprint before the first PUT or either pin-table write. Missing,
  duplicate and stale foreign-platform reports are errors. Fingerprints normalize
  Windows/Unix checkout line endings. Reports must come from a trusted, reviewed
  workflow revision; they are not substitutes for upstream artifact attestations.
- `python3 -B -m unittest discover -s scripts -p test_resolver_pins.py -v`:
  **20 passed**, including broken output/schema, each missing platform wheel,
  missing hashes/sdist/marker, stale evidence, exact command invocation and the
  no-upload/no-pin-write failure gate.
- `python3 -B scripts/resolver_pins.py 0.13.5 --stage-only --smoke-target
  aarch64-apple-darwin --staging ... --gh ...` passed end to end: upstream archive
  attestation, native execution, output validation, report emission. Output has
  7 packages, 23 distinct pydantic-core wheel hashes plus its sdist. It generated
  only the report in temporary storage; no pin files or S3 objects were changed.
- Full offline Rust workspace suite: **1,127 passed** (1,084 CLI unit, 29 CLI
  integration, 14 SDK). Formatter checks, Clippy `-D warnings`, pinned Black
  23.9.1 and Bandit 1.7.5 passed. Workflow YAML parsed successfully; GitHub-hosted
  execution is not claimed. The remaining native Intel Mac/Linux/Windows runs
  must pass before full ingestion; no foreign-platform evidence was fabricated.

Follow-up review evidence:

- Release-time drift prevention checks **deployed** state, not just working-tree
  equality: `check_resolver_mirror.py` requests every pinned version/target from
  the production Registry and streams each presigned object to verify its exact
  compiled-in digest. `release.yml` runs it before builds/assets. Missing
  inventories, missing/inaccessible objects, wrong bytes, empty/oversized objects
  and HTTP/transport errors fail the gate. No private-repo token is required.
  The real production probe has not been claimed successful: deployment and
  mirror upload remain open operator gates, and CLI releases intentionally block
  until those are satisfied.
- Both CLI acquisition GETs distinguish 403/404 (not mirrored/inaccessible),
  other HTTP service failures and transport/proxy failures. A local HTTP fixture
  exercises 403/404/500 at each hop, checks no failed download is cached and
  checks signed URL query secrets are not exposed in diagnostics.
- `PythonResolution` v1 remains graph-free. Its type docs and spec explicitly
  distinguish root coverage from unprovable general transitive closure. An SDK
  regression records that limitation; a parser regression checks retention of a
  Windows-only transitive's version, marker and hashes. The known real-resolver
  smoke fixture rejects removal of Colorama, not only an incorrect marker.
- A cold-cache real CLI subprocess rejects plain dependency-bearing `publish`
  at the 11B gate with and without an override, including `--refresh-python`.
  It asserts zero registry connections and no author-resolution state/lockfile.
  The gate runs after manifest validation but before acquisition or resolution.
- Full Rust workspace suite: **1,131 passed** (1,086 CLI unit, 30 integration,
  15 SDK); maintainer Python suite: **26 passed**. Formatting, Clippy and pinned
  Bandit passed. Release/resolver workflow YAML parses; GitHub-hosted execution
  and live production mirror availability remain unverified, not inferred from
  mocked tests. Backend runtime code was unchanged by these follow-up fixes.

Python compatibility follow-up: read-only target/mirror checks no longer import
the Python 3.11+ smoke machinery; ingestion rejects older Python explicitly
before work. CI/release check jobs pin Python 3.11. **28 maintainer tests passed**;
the target/inventory check and mirror CLI `--help` also passed under the Mac's
system Python 3.9.6. No Rust code changed or Rust suite rerun for this narrow fix.

Operator-diagnostics follow-up: **31 maintainer tests** and the focused Rust HTTP
status regression passed. Linux libc fallback and unknown/musl rejection are
covered. Both download diagnostics name API route deployment as a possible 404
cause; the mirror CLI exits with a concise error (also checked under Python
3.9.6), without a traceback. Formatters and applicable docs hooks passed.

Initial hosted PR validation rejected `runner.temp` in the smoke job's `env`:
that context is unavailable at job-env evaluation time. Moved the staging value
to step-level `env` and used `runner.temp` directly in the artifact step's path.
Pinned actionlint 1.7.12 now validates Resolver Contract, CI and release workflows
with no errors (external shellcheck/pyflakes checks disabled). This is expression
validation, not a claim that native GitHub jobs have passed; push the correction
to the existing PR for a fresh hosted run.

Initial Linux CI also exposed a real acquisition bug: the temporary executable's
write handle remained open during `--version`, which Linux rejects with ETXTBSY.
The writer is now explicitly closed after `sync_all`, before chmod/execution;
digest/version checks and atomic cache publication remain unchanged. An isolated
existing Linux container (network disabled, read-only root, executable tmpfs)
reproduced ETXTBSY with the handle open and successful execution after close.
The full macOS Rust suite passed **1,131 tests**, including the download/cache
regression; Clippy, Rust formatting and actionlint passed. The test now prints
the complete error chain on failure. This kernel reproduction is not a full
Linux Rust-suite run; the updated PR's hosted Linux CI must still confirm it.

Hosted verification completed (2026-10-23):

- The user reported successful CI and all four native Resolver Contract jobs on
  the opened PR. Resolver Contract run:
  https://github.com/agentpm-dev/cli/actions/runs/40043503095
- Inspected the four extracted JSON reports in
  `/Users/zackhine/Downloads/uv-smoke-artifacts`. Each canonical target appears
  exactly once. All reports match uv **0.13.5**, their checked-in executable
  SHA-256 pins and the current smoke-contract fingerprint
  `f0391b81f3feecde609595eaa53c89ecdee228993f9c9d513b679ef6da123f14`.
- Each embedded pylock passes the production smoke validator: version `1.0`,
  seven packages, 23 distinct pydantic-core wheel hashes plus its sdist,
  native Python 3.11 wheel coverage for all four targets, valid distribution
  digests and the Windows-only Colorama marker. Validation was read-only; no
  uv execution, downloads, pin changes, storage writes or deployments occurred.
- GitHub run status is user-confirmed; the extracted report contents and their
  compatibility with the current worktree were independently checked locally.
  This closes the hosted native-verification gate, not live mirror delivery.

Namespace guard verification:

- The operator confirmed the dev/prod namespace audit complete before proceeding
  to manual-test preparation. No automatic rename, deletion or migration is
  included. New creation is blocked before entitlement
  checks or writes. Creation has one service path; metadata PATCH does not rename
  handles. Unit coverage includes case/whitespace variants, both namespace kinds
  and visibility modes, unchanged `agentpm`/missing-handle behavior, and allowed
  similar names. A JWT-authenticated HTTP regression asserts 400 and no write.
  API full suite: **441 passed, 7 existing optional PostgreSQL cases skipped,
  129 subtests passed**; focused namespace suites: **64 passed, 45 subtests**.
  Native Black/isort/flake8/pyupgrade/text checks and isolated pinned Bandit with
  `pbr` pass. Full hooks were attempted: existing mypy `search.py:1486,1856`,
  Pylint findings in unchanged `get_pins` and the existing PAT mock's return,
  and the Bandit hook's missing `pbr` remain. No baseline suppression/config
  changes were introduced. Namespace behavior is documented in API security docs.

Storage operator progress (2026-10-23):

- Operator ran the generator with validated native reports and received
  `; mirrored` for both `agentpm-dev-tools` and `agentpm-tools`, using prefix
  `tools/runtimes/uv`. All four binaries subsequently passed the mirror probe
  through the real local Registry. The production probe stopped at HTTP 404;
  the operator confirmed the M11A API was not yet deployed. No re-upload is
  required merely to deploy the missing route/inventory.
- Both bucket policies were supplied and confirmed saved by the operator. They
  deny insecure transport, deny `DeleteObject`/`DeleteObjectVersion` on the
  resolver prefix and require conditional create-only object writes there.
  The supplied `tools/` lifecycle rules do not expire/transition current objects;
  multipart-abort and noncurrent-version cleanup remain. New uv version keys do
  not supersede older keys. Policies do not override lifecycle actions: future
  lifecycle changes must preserve resolver availability. See the Registry storage
  guide for the deployment mapping. These are operator confirmations and config
  review, not independently executed overwrite/delete tests.

M11A manual walkthrough prepared:

- `manual-tests/CLI_M11A_MANUAL_TESTS.md` and `fixtures/m11a/` provide individual
  visible commands; setup creates only a temporary root/empty HOME, and each
  section copies its own fixtures. A just-built CLI and local schema are explicit.
  The backend's existing Python supplies a compatible interpreter, not a new Tool
  environment. No real PAT/AWS keys, successful namespace creation or publication
  are needed. Negative shell fixtures are separate from real-mirror checks.
- All ten manifest fixtures were checked through real `lint --strict`, including
  expected dependency-field failures. In a different disposable verification
  workspace, the builder exercised the real dev Registry/S3 download, PATH
  exclusion, portable resolution, cold publish gate/transport failure, author
  record reuse/refresh/changed roots, second-project executable reuse, marker
  branch selection, exact/wrong overrides, cached-binary tampering, engine errors
  and recovery. All expected outcomes passed. Marker branches were checked by
  semantics, not a literal marker string. No user manual-test state was created.
- Existing CLI resolution integration suite: **5 passed**. The Python fixture
  passed pinned Black; shell fixtures passed `sh -n`; the Node fixture passed
  `node --check`. This is builder preparation evidence, not completion of the
  user's walkthrough or its browser namespace test. Completion boxes were left
  open during preparation; the later operator confirmation is recorded below.

M11A operator manual acceptance (2026-10-23, America/Denver):

- The operator confirmed the remaining manual tests passed after stepping through
  the guide. All eleven completion items, including local website rejection of
  the reserved `runtimes` namespace, are marked complete on that confirmation.
  This is local acceptance, not a production deployment or cross-machine install.
- Shared resolution output shows both roots (`click==8.1.8`,
  `pydantic==2.11.7`), Python `>=3.11`, seven distributions, all four supported
  platform markers, Windows-only Colorama, 24 pydantic-core artifact hashes,
  and passing full-lowercase/sorted-unique hash checks. Its digest matches the
  CLI summary: `ac4666eda2701aec443fcd31f6972fd94a7467a1a410c50a24c7d7f600775ef1`.
- Shared changed-root output shows the missing override rejected with exit 1,
  then successful real resolution with a new digest and two distinct input-
  fingerprint records. Forced refresh also rejects a missing override.
- Shared deliberate-engine-failure output shows the expected resolver diagnostic
  and exit 1, silent `cmp` preserving the valid record, then successful real-
  resolver refresh and archive creation. The fixture's `M11A` message is test
  output; it is not hardcoded in the shipped CLI diagnostic.
- The resolution summary now starts with a checkmark while retaining its count
  and full digest on stderr, including quiet/captured output. The existing
  integration test checks this; workspace tests, Clippy and formatting passed.

M11A production mirror acceptance (2026-10-23, America/Denver):

- The operator shared successful output from
  `../agentpm-api/.venv/bin/python -B scripts/check_resolver_mirror.py --base-url https://api.agentpackagemanager.com`.
  All four targets passed for uv 0.13.5: `aarch64-apple-darwin`,
  `x86_64-apple-darwin`, `x86_64-pc-windows-msvc` and
  `x86_64-unknown-linux-gnu`. The checker obtains each presigned URL from the
  deployed Registry, downloads its binary and verifies the compiled pin-table
  SHA-256. This supersedes the earlier pre-deployment 404 and closes the final
  M11A operator acceptance item. No CLI release/tag/version bump, independent
  build audit or production namespace-creation test is inferred from this probe.

Later integration/handoff boundaries (outside M11A acceptance):

- Portable lock persistence, resolved-state publication and install regeneration
  remain 11B work; actual managed environment provisioning and the resolve-on-one/
  install-on-another proof remain the 11A-11C integration gate. Release manifest
  and artifact inspection consumers use the same type in their owning milestones.
- Stage 2 M1B has a working-tree schema handoff note, not merge/freeze evidence.

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
- release manifest type is exactly `agentpm.package.release.v1` and validates the fixed field contract.
- `pythonResolutionDigest` is omitted when no Python resolution exists and present/bound when one exists.
- release artifact inventory is target-unique and canonically sorted before digesting.
- `PackagePublishRelease` persists in `package_publish_releases`.
- `PackagePublishArtifact` persists in `package_publish_artifacts`.
- `PackageVersionArtifact` persists in `package_version_artifacts`.
- one release reservation owns multiple child artifact uploads.
- concurrent child artifact reservations for one version do not collide with legacy one-pending-upload rule.
- duplicate target rejected.
- mismatched identity/version rejected.
- mismatched manifest digest rejected.
- mismatched Python resolution digest rejected.
- one failed/missing upload leaves release unpublished.
- finalization creates one logical version + N artifact rows atomically.
- legacy `PackageVersion.sha_256/size_bytes/s3_key` rows remain populated/readable.
- new `agentpm.package.release.v1` rows have legacy scalar artifact columns `NULL`, populate the release-level digest field, and use child artifact rows as the only authoritative target-artifact source.
- read paths branch on explicit release format, not on nullable-column heuristics.
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
- release reservation outlives an individual presigned PUT window.
- a presigned PUT can be re-issued for a still-pending child artifact without invalidating the release or already-uploaded siblings.
- an expired release reservation produces an actionable, identifiable error rather than an opaque S3 or `409` failure.
- worst-case registry call count for a supported multi-target release stays within the effective publish rate-limit budget, including presign re-issues and one retried matrix job.
- an S3 error other than not-found on the finalize destination-exists check fails finalize instead of being treated as "object absent" and triggering a copy; `AccessDenied` specifically is covered.
- resume state is observable by the client — a resumed reservation is distinguishable from a fresh one, or the flag is absent from the response entirely.

### Canonicalization / stored-byte integrity / provenance

Create shared canonicalization fixtures consumed by Rust and Python.

Required cases:

- typed legacy author v1 statement serializes byte-for-byte like current implementation;
- RFC 8785/JCS implementation matches published/known JCS vectors in Rust and Python;
- new canonical key order is deterministic;
- non-ASCII Unicode serializes identically cross-language;
- artifact inventory order is normalized by target before JCS;
- `PythonResolution` array ordering is normalized before JCS;
- whitespace/input map order does not alter canonical bytes;
- new timestamps are normalized to UTC RFC3339 `Z` form before canonicalization;
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
- `GET /v1/registry/signing-keys` returns type `agentpm.registry.keyset.v1`.
- registry key set is RFC 8785/JCS-canonicalized and root-signed.
- CLI rejects a key set with an invalid root signature.
- CLI rejects an arbitrary attestation signing key delivered outside the verified key set.
- CLI can cache and reuse only a previously root-verified key set.
- unknown `registryKeyId` fails verification.
- retired signing key remains present in the verified key set and can verify an old attestation after rotation.
- root signing key itself is not confused with routine attestation signing keys.
- the currently configured attestation key ID (`apm-prod-1` by default) is present in the published key set.
- an **existing, already-published** version's `agentpm.registry.attestation.v2` row verifies end-to-end through the new client verification path — this is a regression test, since `--require-attestation` passes for those versions today.
- `--require-attestation` succeeds on a legacy single-artifact release whose attestation key was backfilled, and fails on one whose key ID is absent from the key set.
- `GET /v1/registry/signing-keys` fails closed when a valid root signature cannot be produced, rather than returning an unsigned or partial key set.
- the root public key pinned in the CLI matches the key the registry actually signs key sets with (bootstrap-order check, not only unit-level verification).

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
- install session remains valid through dependency provisioning, not only through artifact download;
- a presigned GET expiring mid-install produces an actionable error rather than a bare transport failure;
- download client enforces explicit connect and idle/progress timeouts instead of `reqwest` defaults;
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

Milestone 7 regression commands:

- In `agentpm-web`: `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `pnpm build`.
  Build in a disposable copy when a development server is active; do not let two processes
  write to the same `.next` directory.
- In `agentpm-api`: `uv run pytest -q`. Set `AGENTPM_SEARCH_TEST_DATABASE_URI` to a local
  Postgres test connection to also run `tests/test_sitemap.py` and the existing search
  behavior fixtures. Sitemap fixtures use transaction-local tables and leave registry data intact.
- Run API pre-commit checks on `app/models/cache.py`, `app/stats/application/stats.py`,
  `app/stats/stats_routes.py`, and `tests/test_sitemap.py`.

Recorded evidence (2026-10-14): `assets/milestone-7/seo-evidence.json` contains 23
production-rendered route inspections without executing JavaScript, plus the sitemap inventory
(54 public/static/docs URLs and 72 public registry namespace/version URLs in the local dataset).
`assets/milestone-7/error-evidence.json` records a fault-injected HTTP 500 response with `noindex`.
The HTML audit covers all eight package kinds, public/private namespaces, private/missing
packages, docs aliases and concrete versions, search/filter URLs, sign-in and the account redirect.
Every successful primary public page has one H1 and no image missing its alt attribute.
The existing share image was checked at 1200x630. Older docs version behavior and private
payload rejection are additionally pinned by unit fixtures; no signed-in private browser session
was used for this audit.

Docs canonical decision: prefer version-specific documentation identity over consolidating
current-guide search signals on the stable `/docs/latest/...` URL. `latest` remains a usable,
shareable navigation alias, but metadata and sitemap entries identify the resolved concrete
version. Historical versions remain independently discoverable rather than pointing at guidance
for a newer release. This is an identity/consistency choice, not a claim of better search ranking.

Accepted tradeoff: when the current docs version changes, the canonical target of a `latest`
link changes too. Search signals may be distributed across version URLs over time instead of
consolidating on an evergreen URL. Milestone 17 should carry this decision forward explicitly.
If evergreen discovery becomes the priority, reconsider canonicalizing both current concrete
pages and their aliases to `/docs/latest/...`, while keeping historical pages self-canonical.
Such a change must update metadata, sitemap selection, tests, and documented policy together;
it should not happen incidentally during docs copy or navigation work.

Verification result: web tests, typecheck, lint and the isolated production build pass;
the complete API suite passes with Postgres fixtures enabled. The full API pre-commit invocation
still reports two existing imported-module mypy errors in `app/tools/application/search.py`
(lines 1486 and 1856), and the pinned Bandit hook lacks `pbr` in its isolated environment.
The four touched Python files pass mypy with imported-module checking suppressed, and pinned
Bandit passes with its missing tool dependencies supplied in a separate temporary environment.
All other applicable pre-commit hooks pass. These existing issues were not changed in Milestone 7.

Deployment note: deploy the registry's `/stats/sitemap` endpoint before or with the web update.
The sitemap index and its bounded registry partitions are generated at request time, not during
the web build. Sitemap responses have a five-minute cache window. Robots directives are not
access controls, and this work does not replace private namespace authorization.

Blog sitemap follow-up (2026-10-15, completed with Milestone 8): the pages partition
now includes individual `/blog/<slug>` URLs from the existing public blog feed,
in addition to `/blog`. URL components are encoded and the combined pages partition
retains the 50,000-entry guard. A failed or malformed blog feed fails sitemap generation
rather than publishing a successful but incomplete sitemap; a genuinely empty feed remains
valid. The blog index's existing empty-list fallback is unchanged.

Verification: all 182 web tests passed, including nine sitemap tests covering article
inclusion, encoded URLs in the emitted XML, valid empty feeds, and failed/malformed feeds.
`pnpm typecheck`, Prettier checks, and the isolated production `pnpm build` passed.
`pnpm lint` passed with six existing unused-variable warnings in untouched files.
The build required network access for the existing Google Font and was run in a disposable
copy so the live development `.next` directory was not disturbed. Applicable API docs
pre-commit checks passed for the sitemap documentation update. No registry API or CLI
behavior changed in this follow-up.

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
- a lock resolved on one platform installs with digest verification passing on a different platform — this is the real check that digest enumeration covered all supported targets rather than only the publisher's wheel;
- changing the local selected artifact does not rewrite target information into the lock;
- changing `AGENTPM_PYTHON` to another compatible minor version does not silently reuse an incompatible native dependency environment;
- a legacy vendored Python Tool still installs/runs under legacy semantics;
- a first Python-dependency install on a clean machine acquires the managed resolver, and a second install on the same machine performs no resolver download;
- an install succeeds on a machine whose Python lacks a working `ensurepip`/`venv` (for example stock Debian/Ubuntu without `python3-venv`);
- an install succeeds behind an HTTP proxy configured only through standard proxy environment variables;
- an install succeeds with no outbound access to GitHub, confirming no third-party runtime fetch remains in the install path;
- an airgapped machine with a preprovisioned pinned `uv` and `AGENTPM_UV_PATH` set can install a Python-dependency Tool;
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
- example new `agent.lock` v4 entry showing the exact `agentpm.python-resolution.v1` shape + logical release integrity and **no** local target selection;
- example canonical `PythonResolution` JSON + JCS digest;
- example machine-local runtime/install state showing selected target/artifact/interpreter fingerprint;
- example `agentpm.package.release.v1` release manifest + JCS bytes/digest;
- example release reservation + child artifact DB/object inventory showing `package_publish_releases`, `package_publish_artifacts`, and `package_version_artifacts`;
- evidence that new-format `PackageVersion` legacy scalar artifact columns are null while legacy rows remain populated;
- example S3/object inventory including canonical target IDs and `release.json`;
- byte-compatibility evidence for typed legacy v1 author statement;
- example new release-level author-signature v2 and registry-attestation v3 statements;
- example trusted stored-byte checksum verification evidence;
- example `agentpm.registry.keyset.v1` response, root signature verification, and registry signing-key history/retirement evidence;
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
