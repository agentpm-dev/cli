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
- Python/Node SDK lockfile and managed Python Tool compatibility (M17);
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
- **Pre-11B lock fixtures** are committed as test data and round-trip byte-identically through the new code: no added `null` fields, no key reordering, no `generated` rewrite, no version bump. Locks generated by the new code do not satisfy this — they round-trip by construction.
- A `--frozen` install and a normal install of the same lock provision identically, including the Python resolution. Verify by mutation: without extending `ResolvedPackage`, the frozen path drops it silently.
- `--frozen` against a v2/v3 lock with a dependency-bearing Python Tool fails with the established "run `agentpm install` without `--frozen`" guidance, not by proceeding.
- `minimum_lock_version` does not move an existing lock's version backward, and does not bump a project with no v4 state to v4.
- Removing `dependencies` from `agent.json` drops the carried-forward local-root resolution on the next plain `agentpm install`; a stale resolution cannot persist in the lock indefinitely.
- `PackageKind` reconciliation leaves on-disk serialization unchanged for every existing kind.
- The forward-version guard still accepts v1/v2/v3, and the Harness `>=2` path is unaffected.
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
- A `kind: tool` project records a local **Tool** root, not `local:agent`. An existing lock holding the old `local:agent` key for a Tool project still reads, and one project never ends up with both keys.
- The project's own resolution lives on the local root, never as a `LockedPackage`; no entry describing the project itself appears in `packages`, and no placeholder is written to `integrity`.
- A full `agentpm install` preserves the local root's resolution and fingerprint. **Verify by mutation**: without the carry-forward, a plain install erases it — assert the post-install lock still contains it.
- `packages` remains a pure projection: an entry absent from the resolve plan does not survive a full install, so preservation did not leak beyond the local-root lane.
- Preserved-but-stale state fails the publish freshness check. Carry-forward must not let an out-of-date resolution publish.
- Publish succeeds with a cold author cache and a committed lock, performing no re-resolution — the property multi-target CI depends on. Two matrix-equivalent runs from the same commit emit the same `pythonResolutionDigest`.
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

Resolver Contract rate-limit follow-up (2026-10-23):

- A subsequent PR native job failed before uv execution: GitHub's attestation
  API returned HTTP 403 `rate limit exceeded`. The Python request did not use
  workflow authentication. The native step now supplies the built-in
  `github.token` as `GH_TOKEN`; the fixed GitHub attestation API request uses
  `GH_TOKEN`, falling back to `GITHUB_TOKEN`, when available. Anonymous local
  operation remains supported. No PAT, new secret or permission increase is needed.
- Authentication is not attached to release-asset downloads. Signature/repository
  verification and failure-before-upload behavior are unchanged. Failed
  authenticated requests do not fall back to anonymous verification.
- All **34** resolver ingestion/mirror/smoke tests pass, including authentication
  precedence, anonymous use, failure handling and release-download isolation.
  Actionlint passes for all three workflows. Hosted execution of this follow-up
  remains pending the next PR run; no S3 objects or pins changed.

Later integration/handoff boundaries (outside M11A acceptance):

- Portable lock persistence, resolved-state publication and install regeneration
  remain 11B work; actual managed environment provisioning and the resolve-on-one/
  install-on-another proof remain the 11A-11C integration gate. Release manifest
  and artifact inspection consumers use the same type in their owning milestones.
- Stage 2 M1B has a working-tree schema handoff note, not merge/freeze evidence.

#### Milestone 11B Evidence (2026-10-23, America/Denver)

Scope and decisions:

- Author Python state is on the explicit `local:tool` root: the shared
  `PythonResolution` plus the 11A input fingerprint. Installed Tool state is on
  `LockedPackage`, projected from the Registry resolve plan. No synthetic self
  package, placeholder integrity, selected target or interpreter state is added.
- Dry-run preparation/refresh writes the author lock; live publication requires
  current matching lock state before acquisition/upload. A committed matching
  lock wins over the disposable OS per-user `python-resolutions/` cache. The old
  project `target/agentpm/python-resolutions/` location is neither read nor written.
- Full and targeted regeneration preserve the matching author's resolution only
  while Python dependencies remain declared. Removing declarations drops both
  resolution and fingerprint. A legacy Tool `local:agent` root is rewritten as
  `local:tool`, never duplicated. Installed packages remain a plan projection.
- `minimum_lock_version` computes capability requirements; mutation paths retain
  the maximum of that minimum and the existing lock version. A local Tool key
  alone does not require v4. Removing Python state can leave an existing v4 lock
  at v4. An older Knowledge-root pruning test now expects v3 retention rather
  than the former downgrade to v2; this is the deliberate regression-lens policy.
- Lock version is checked before shape deserialization, including Harness reads;
  unsupported versions and unknown structural fields fail closed. Writes guard
  both existing and proposed locks, refuse downgrades, and skip unchanged logical
  state without reordering bytes or changing `generated`. Existing V1-shaped
  locks remain readable; Python preparation requires migrating that shape before
  acquisition. Existing v2/v3 Agent/Skill state remains supported.
- CLI lock/semver code now reuses the SDK's one `PackageKind` definition. All
  eight spellings serialize identically; unsupported template installs retain
  their explicit existing error rather than hitting a partial conversion panic.
- `PythonReleaseMetadata` reuses the validated v1 type for build/artifact state.
  Its checked digest also supplies the prefixed `sha256:...` input for the later
  release manifest. Full release documents/reservations/signing formats remain
  with M12/M14; no multi-artifact release implementation was pulled into 11B.
- Published Tool archives include generated `agentpm-python.json`, containing only
  that Tool's resolution/digest, and omit the publisher's root `agent.lock`.
  Registry init validates intent, entries, markers and digest; finalize validates
  the actual tar manifest/metadata against the reservation before copying and
  recording the version. Missing/mismatched state and ambiguous metadata paths
  fail. Both the CLI's and Registry's blanket 11A publication blocks are lifted.
- Additive Pyway migration `V1.97__Persist_Python_Resolution.sql` adds nullable JSONB
  to physical tables `uploads` and `tool_versions`; legacy rows/releases need no
  backfill. The frozen wire object is not stored inside authored `agent.json`.
  The Registry uses pinned `dep-logic==0.7.2` for symbolic PEP 508 operations,
  avoiding publisher-host sampling or a second dependency resolver. It validates
  listed entries and root coverage; graph-free v1 still cannot prove transitive
  closure. See the library's primary documentation:
  https://github.com/pdm-project/dep-logic

Required regression evidence:

- Genuine pre-11B v2/v3 snapshots are checked in under
  `crates/agentpm-cli/tests/fixtures/locks/`. The installed Homebrew CLI 0.1.36
  generated them against a temporary HTTP fixture Registry after its scaffolded
  Agent/Skill manifests passed strict lint. They are unmodified old CLI bytes,
  not output of the new serializer. LF checkout is fixed in `.gitattributes`.
  Tests prove byte-identical parse/serialization and unchanged writes, including
  timestamps, key order, omitted fields, version and final newline.
- Frozen-plan and normal resolve DTOs contain identical typed Python state.
  Deliberately replacing `ResolvePlan::from_lock`'s Python copy with `None` made
  the targeted digest/DTO test fail; the correct implementation was restored.
- Deliberately disabling local-root carry-forward made the real CLI full-install
  test fail on the missing resolution/fingerprint. Restoring it passes. That test
  also proves out-of-plan packages are removed, unchanged installs preserve all
  bytes, dependency removal clears author state without lowering v4, and old
  mislabeled local roots migrate without duplicates.
- Older frozen author locks fail before resolution. Frozen consumers using the
  real v2/v3 snapshots also fail with the established "run `agentpm install`
  without `--frozen`" guidance when Registry metadata reports Python dependencies;
  no artifact download, finalize or lock mutation occurs.
- Two matrix-equivalent live CLI publishes against a local HTTP fixture succeed
  with a valid committed lock, deleted author cache/resolver and deliberately
  invalid override path. Their init payloads and uploaded Tool metadata carry
  the same resolution digest. This proves reuse without independent resolution.
- The shared digest agrees through lock, resolve/install DTOs, artifact/build
  metadata and the later release-manifest digest input. API tests also prove
  init reservation persistence, actual streamed artifact binding, finalize
  persistence and resolve/install-init responses. Invalid hashes/digests/intent,
  overlapping branches, missing Windows/future-Python coverage and metadata path
  aliases fail; roots inapplicable to supported environments may resolve empty.

Realistic scale and serialization decision:

- Real pinned uv 0.13.5, verified against its executable pin, resolved
  `openai>=1.51.0` plus `pydantic>=2.8,<3` for Python >=3.11. The committed
  `python-resolution-realistic.json` fixture contains **14 distributions / 113
  hashes** and passes both Rust and Registry validation unchanged. Its canonical
  bytes are **12,790**; digest:
  `da043515cde4480c1d2aae8a8914eb87ea7ac7d0454d7df0bcff6ffecdf3499b`.
- The measured author lock is **16,213 bytes**. Native tests measure **15,104
  bytes** for Tool metadata, **322,233 bytes** for pretty serialized 20-Tool lock
  JSON (plus the writer's final newline), and **259,001 bytes** for its resolve
  DTO. The 20-Tool DTO/lock round-trip preserves every resolution/hash.
- Keep pretty JSON: these sizes remain reviewable and preserve existing v2/v3
  serialization. The 512 KiB lock assertion is a fixture-growth budget only,
  not an enforced Registry or local-lock limit. The actual checked limits are
  the **2 MiB per-Tool metadata** bound shared by packaging/Registry validation
  and, following the aggregate-size review, the Registry's **8 MiB compact JSON
  resolve-response budget**. The actual Registry service accepts the realistic
  20-Tool fixture and emits **259,002 bytes**, including the response newline,
  under its **8,388,608-byte** budget.
  There is no configured application-wide JSON request cap to claim as an
  additional tested constraint; production host/proxy limits are not inferred
  from local tests. No index access is needed for ordinary fixture tests.

Validation and boundaries:

- Full Rust workspace: **1,141 passed**, including 1,093 CLI unit tests, 33 CLI
  integration tests and 15 SDK tests. Formatting and workspace Clippy are checked
  before handoff. The temporary mutation versions are not left in the worktree.
- Full Registry suite: **467 passed, 129 subtests passed, 7 existing optional
  PostgreSQL search/auth/sitemap tests skipped** because their opt-in URI is unset.
- Migration SQL was executed in a fresh PostgreSQL 16 container with no network
  or published ports, against minimal pre-migration tables. Existing rows remained
  NULL; both tables retained the full JSONB value across sessions. The test
  container was removed; verification did not use an existing application database.
- Native Black/isort/flake8/Pylint/pyupgrade/text checks pass. Mypy still reports
  the two pre-existing `search.py` assignment errors at lines 1486/1856. The
  configured Bandit hook still cannot start because its environment lacks `pbr`;
  the same pinned Bandit 1.7.5 with `pbr` supplied in an isolated tool environment
  passes without findings. No unrelated baseline code or hook policy was changed.
- User-facing workflow and migration guidance are updated in
  `agentpm-api/docs/v0.1/advanced/python-dependencies.mdx`. Implementation tasks
  are checked off on this evidence. No production deployment, published CLI or
  version bump is claimed. Operator manual acceptance on 2026-10-24 is recorded
  below; other environments still need the new Registry dependency and V1.97
  through normal deployment before live use.
- The operator confirmed CLI hosted CI passed after the cold-cache fixture
  cleanup fix and confirmed the local/dev Registry walkthrough passed on
  2026-10-24. No M11B operator acceptance remains pending on this evidence.
  Managed Python installation/execution and the
  resolve-on-one/install-on-another proof remain 11C; they are not closed by
  metadata-only tests. Full multi-artifact release assembly remains M12/M14.

M11B review follow-up: refresh is preparation only.

- `publish --refresh-python` without `--dry-run` is rejected at command entry,
  before config/credentials, resolver acquisition, lock writes, packaging or upload.
  This also protects programmatic callers of `PublishArgs::run`. Help and user
  docs name the required preparation/review/publish sequence.
- The regression uses an already-valid v4 author lock and matching fingerprint,
  an invalid uv override, isolated empty cache/HOME, and an observed local HTTP
  endpoint. It asserts the specific flag error, byte-identical lock, no generated
  files/cache and zero requests. The former cold-lock test now exercises ordinary
  live publication separately; a flag error cannot masquerade as its freshness gate.
- Positive CLI coverage explicitly runs `publish --dry-run --refresh-python`,
  then publishes the resulting unchanged locked resolution normally.
- Verification: **50 publication unit tests and 9 Python CLI integration tests
  passed**; workspace Clippy, Rust formatting, documentation text checks and
  `git diff --check` passed.

M11B review follow-up: unrelated locks do not block non-Tool publication.

- Both resolution preparation and the live-publication resolution gate check
  package kind before reading `agent.lock`. Agent/Skill publication does not
  deserialize or mutate unrelated lock state.
- Unit coverage exercises both functions with malformed JSON, a future lock
  version and an unknown structural field, including refresh preparation, and
  asserts unchanged lock bytes and no resolver acquisition/cache creation.
- CLI coverage scaffolds and strictly lints Agent and Skill manifests, then
  successfully dry-runs publication with malformed/future locks and an invalid
  resolver override. The existing future-lock rejection for Tools still passes.
- Verification: **16 resolver unit tests and 10 Python CLI integration tests
  passed**; workspace Clippy, Rust formatting, documentation text checks and
  `git diff --check` passed.

M11B review follow-up: explicit author-resolution removal coverage.

- A direct `preserve_local_tool_resolution(..., false)` regression checks both a
  freshly regenerated destination and a destination already carrying stale
  resolution state. Both Python fields are omitted while local Tool identity and
  lock v4 are retained; the original lock is not mutated.
- The existing CLI publication/install regression already removes
  `runtime.dependencies` from the actual manifest and verifies that ordinary
  install drops both author fields. It also verifies the preceding installs keep
  live author state, so an inverted or dropped preservation argument is covered
  through the production manifest-loading path.
- Verification: **8 lock-v4 unit tests and the focused CLI publication/install
  integration test passed**; workspace Clippy, Rust formatting and
  `git diff --check` passed. Production behavior was unchanged.

M11B review follow-up: aggregate resolve-plan size is bounded.

- `/v1/tools/install/resolve` now enforces `MAX_RESOLVE_RESPONSE_BYTES = 8 MiB`
  while assembling the distinct package entries. Accounting includes compact
  JSON for every package/resolution, separators, the response envelope and final
  newline. The current realistic 20-Tool case uses about 3.1% of this budget;
  it is a response-size policy, not a cap on incoming requests, local lockfiles,
  debug pretty-printing or the entire resolver's process memory.
- Crossing the budget stops graph expansion with HTTP 422 and
  `resolution_too_large`, returning the configured byte limit and guidance to
  reduce root dependencies. No partial plan or truncated resolution is returned.
  Publication's dependency-access traversal retains its existing behavior.
- Tests verify exact-limit success, rejection one byte over, deduplication and
  aggregate rejection for individually valid sub-2-MiB resolutions. The latter
  stops at the eighth entry rather than resolving all ten requested packages.
- Verification: **469 API tests and 129 subtests passed**, with the same 7
  optional PostgreSQL tests skipped. Black, isort, flake8, Pylint, pyupgrade,
  documentation text checks, Rust formatting and `git diff --check` passed.
  The existing two `search.py` mypy errors and the configured Bandit hook's
  missing `pbr` dependency remain; the pinned Bandit version runs successfully
  with `pbr` supplied in an isolated tool environment, with no findings.

M11B non-blocking review notes:

- The non-frozen install path can call `preserve_local_tool_resolution` inside
  `build_updated_lock` and again before `write_lock`. The operation is idempotent;
  this is redundant work, not a preservation defect. No call-site refactor was
  included in this review follow-up.
- Rust and Python canonicalization tests currently assert matching independently
  maintained byte vectors in their respective repositories, not one shared
  cross-repository fixture/gate. A one-sided implementation plus expected-vector
  edit can pass that repository's tests; incompatible digests would then fail
  real publication. Automated cross-repository drift detection is not claimed.
- User documentation explicitly records that plain install in a Tool project
  succeeds with an empty AgentPM dependency set and creates/regenerates the local
  Tool lock root. Python resolution preparation remains the dry-run publish flow.
  This follow-up changes documentation only; documentation text checks and
  `git diff --check` passed.

M11B CI follow-up: portable cold-cache test cleanup.

- Hosted Linux CI reached successful dry-run preparation, then failed because
  the test unconditionally removed a temporary HOME directory that did not
  exist. Its existence is not a portable proxy for config/cache creation;
  inherited XDG settings can place Linux configuration outside HOME.
- The Python CLI fixture now isolates `XDG_CONFIG_HOME` and `XDG_DATA_HOME`
  beneath its temporary home, alongside its existing isolated cache. Cold-state
  cleanup tolerates only `NotFound`, retains errors for other filesystem failures,
  and asserts both directories are absent before removing the fixture resolver.
  The subsequent cold-cache publication also asserts byte-identical committed
  lock state, preserving the original regression's purpose.
- Verification: **all 10 Python CLI integration tests passed locally on macOS**;
  workspace Clippy, Rust formatting and `git diff --check` passed. The operator
  subsequently confirmed the hosted CI rerun passed; this is separate from the
  local macOS evidence. Production code was unchanged.

M11B manual walkthrough prepared (2026-10-24, America/Denver):

- [The M11B runbook](manual-tests/CLI_M11B_MANUAL_TESTS.md) and `fixtures/m11b/`
  provide individual commands and expected outcomes for the author lock,
  reviewed local commit, fresh checkout/cold cache, preservation/removal,
  freshness and compatibility cases. Setup changes user state only for launched
  CLI processes; the operator's shell HOME and normal credentials stay intact.
- The live portion is explicit: verify API requirements, normal Pyway V1.97
  startup and dev bucket, authenticate with a private dev PAT, exercise two
  rejected API requests, then publish one uniquely named Tool to dev and inspect
  reservation/version persistence. Normal, regenerated, fresh frozen and legacy
  consumer flows compare the exact shared resolution/artifact metadata.
- Shell blocks parse under both bash and zsh; embedded Python blocks parse, the
  fixture JSON parses, and the fixture Python passes Black. The offline sections
  (2–6 and 11) were executed in disposable directories using a controlled uv
  fixture and the real built CLI, including strict lint, lock writes, archive
  inspection, local Git/clone, cold-state reuse and negative/compatibility checks.
  This validates the command sequence, not a new real-uv or live Registry run.
- No real PAT, Registry publication, S3 write, migration or production change was
  performed while preparing the guide. Operator-run manual acceptance is recorded
  below. No 11C provisioning, runtime-import or cross-machine execution acceptance
  is claimed.

M11B operator manual acceptance (2026-10-24, America/Denver):

- The operator confirmed all manual tests passed after working through the
  local/dev runbook. Its 12 completion items are checked on that confirmation,
  covering author lock preparation, cold-checkout reuse, stale/removal behavior,
  API rejection, real dev publication/persistence, normal/regenerated/frozen
  consumers, legacy recovery and local compatibility guards.
- Repeating the original file-list append step left three `agent.lock` entries
  in the fixture manifest before publication. The runbook now deduplicates this
  operation. The published fixture was retained: repeated declarations do not
  change the resolution and the root lock is excluded from the artifact. The
  operator's passing consumer checks complete the metadata acceptance.
- This is operator-reported manual evidence, not an agent-executed live publish
  or independently captured CLI/package digest. With review fixes verified and
  CLI hosted CI confirmed green, M11B acceptance is complete. Production rollout,
  CLI release, 11C environments and later release-format milestones remain
  separate work.

#### Milestone 11C Evidence (2026-10-24, America/Denver)

Implementation and choices:

- Consumer Python environments live under `.agentpm/runtimes/python/`, separate
  from extracted Tool payloads. Their key includes Tool identity/version, the
  existing target vocabulary, resolution digest, pinned uv version and selected
  interpreter path/major-minor version/implementation/ABI. The full live inspection
  (including kernel and Python patch markers) is used for evaluation and runtime
  validation, rather than serialized into the directory/readiness identity. No
  machine-local environment fields were added to `agent.lock`, the resolution or
  Registry DTOs.
- Install validates artifact metadata against the Registry plan and manifest,
  selects Python with the existing override/minimum rules, then provisions before
  install-finalize and lock writes. Legacy Tools do not enter this path. The old
  advisory PATH warning is retained for legacy artifacts; dependency-bearing
  Tools receive the authoritative interpreter check during provisioning.
- The existing verified/cached `ManagedUv` acquisition is reused. `uv venv` creates
  scaffolding without interpreter `venv`/`ensurepip`; `uv pip sync` receives only
  applicable exact recorded pins and all their hashes, with `--require-hashes`,
  `--only-binary :all:`, explicit Python and no Python downloads. No solve or
  package-set expansion occurs. The shared SDK evaluates consumer markers and
  verifies active authored roots have coverage; uv dependency checks and exact
  inventory checks reject an incomplete recorded set without adding packages.
- Readiness is recorded only after sync, dependency and inventory checks pass.
  A failed refresh removes previous readiness. Retry repairs interrupted venv
  creation and interrupted sync, removing extra distributions. Local OS file
  locks prevent mutation while a Tool uses the environment and release after a
  process exits. A matching ready install validates inventory and reuses the
  environment without index access; cached uv is still verified.
- Run uses the managed interpreter and checks the current fingerprint/inventory
  without provisioning/network access. The chosen stale-state policy is an
  actionable reinstall error; run does not silently rebuild or use global Python.
  Changing the interpreter selection may require explicit install even when its
  major/minor matches. Windows external-path normalization is handled before
  prefix/base-interpreter comparisons. Harness readiness and its existing
  `agentpm run --machine` ToolRuntime share these checks.
- Managed runs remove `PYTHONHOME` and `PYTHONPATH` after environment defaults,
  manifest entries and invocation overrides are merged. Name comparisons are
  case-insensitive for Windows. Legacy Tools retain their environment behavior;
  Tool-local packages and explicit author-written `_vendor` imports still work.
- Existing author-written `_vendor` imports continue to work. Install warns on
  likely vendored dependency content alongside declared dependencies. If it finds
  none before reaching the 2000-entry/depth-4 bounds or encountering unreadable
  entries, it warns that the scan is incomplete and identifies the skipped
  categories. No payload
  compatibility classification, multi-artifact selection or new schema fields
  were introduced; those remain with later milestones.

Tests and example:

- Full Rust workspace after identity/import-path review fixes: **1,152 passed**, with **5 native-engine tests ignored in
  the ordinary suite**. These gated tests were run separately with the real
  verified **uv 0.13.5** on macOS Apple Silicon. All **10 environment tests** pass,
  including the 5 ordinary unit tests and 5 native tests. The native suite uses
  a compiled-pin-verified cached binary; local wheel fixtures test provisioning
  offline. Real Pydantic wheels additionally exercise public PyPI acquisition.
- The SDK tests select Python-version/Windows marker branches, reject an
  interpreter below the floor, and reject active roots not covered by the
  recorded platform branches. Pure/native/conditional packages, exact hashes,
  explicit interpreter changes, minimum-version rejection and future locks are
  covered without adding a second resolution type.
- Real-uv failures distinguish unavailable distributions (including HTTP 404/410),
  wrong hashes and no compatible wheel. An sdist with a recorded valid hash is
  rejected without executing its deliberate-failure build script. A wheel whose
  METADATA requests an unrecorded package cannot expand the installed set; the
  environment is not marked ready. A wrong-hash forced reinstall also invalidates
  an existing ready environment, and recovery succeeds with restored valid bytes.
  These categories depend on pinned uv stderr wording, not a typed error contract;
  the source now calls out that pin adoption must rerun the real-binary tests in
  Resolver Contract. Changing prose fixtures alone does not establish compatibility.
- Interruption tests start from incomplete venv scaffolding and separately remove
  the completion record after sync. Retry converges and drops an extra installed
  distribution. A disposable base interpreter with both `ensurepip` and `venv`
  imports deliberately unavailable still provisions successfully. This is an
  actual interpreter test with import blocking, not a claim of a Debian run.
- A real CLI/local HTTP Registry fixture verifies bad wheel hashes cause neither
  install-finalize nor a new workspace lock. Successful normal and frozen installs
  finalize and retain byte-identical lock state. Prior resolver acquisition,
  override/PATH, legacy lock and install-regeneration regressions remain in the
  passing workspace suite.
- Review caught a missing blocking-mode reset on this new fixture's accepted
  sockets: macOS/BSD can inherit the listener's nonblocking mode, so an earlier
  successful run did not rule out intermittent `WouldBlock` reads. The helper now
  explicitly calls `stream.set_nonblocking(false)` before setting its read timeout.
  The initial rerun after this correction used the complete native suite with
  verified uv 0.13.5 and Python 3.13 on macOS Apple Silicon: **8 passed, 0 failed,
  0 ignored** (52.67 seconds), including the affected real CLI/finalization/frozen
  lock fixture. Hosted matrix verification remains pending.
- Identity review removed the full live interpreter snapshot from the key.
  Regression tests simulate kernel release/build and compatible Python patch
  updates (including relocated physical base paths behind the same selected
  executable) and prove directory stability. Separate cases evaluate
  `platform_release`, `platform_version`, `python_full_version` and
  `implementation_version` changes that select an additional locked package:
  the same directory is selected, but the old completion record is rejected.
  ABI, implementation, selected executable and Python major/minor changes still
  select a different identity. Full current interpreter/base-path and package
  checks remain in force; a removed or mismatched venv binary needs same-directory
  repair, not a new patch-version key. No live OS/Python upgrade or automatic
  orphan pruning is claimed. The complete native suite after this change passed
  **10 tests, 0 failed, 0 ignored** (55.91 seconds); the full workspace also passed.
- The real-uv replay fixture now creates a shadow dependency on an external
  `PYTHONPATH` and first proves that ordinary Python imports that shadow. Managed
  runner invocation with explicit path/home overrides, plus actual CLI normal and
  `--machine` runs with ambient `PYTHONPATH`, all import the locked wheel instead.
  The explicit authored vendor helper keeps working. A separate ordinary runner
  test confirms a dependency-free legacy Tool still imports from its declared
  `PYTHONPATH`. After this change, **10 native environment tests passed** on
  macOS Apple Silicon (57.09 seconds), **1,152 workspace tests passed** with the
  5 native tests intentionally ignored there, and workspace Clippy/formatting,
  docs text checks and diff checks passed. Hosted matrix execution remains pending.
- Bounded-scan warning follow-up: disposable local Registry fixtures extended the
  actual summarize-text archive with 2005 extra entries, or a hidden `_vendor`
  directory below depth 4. Normal and fresh frozen installs for both variants
  emitted the corresponding incomplete-scan warning, completed provisioning and
  ran successfully against a local mock OpenAI endpoint, preserving frozen locks
  and importing native dependencies. No live publication or paid model calls
  occurred. The focused environment suite passed its 5 ordinary tests (5 native
  cases intentionally ignored in that invocation); workspace Clippy, formatting,
  docs text and diff checks also passed. Full workspace/native totals above are
  from the preceding identity/import-path verification, not reruns for this
  advisory-only change. The readable version/target directory layout is unchanged.
- `python-resolution-pydantic-v1.json` is a frozen 5-distribution/32-hash fixture
  taken from the earlier macOS author resolution, restricted to the Pydantic
  root/closure. It is replayed unchanged by native tests. Real `pydantic-core`
  imports work through direct run and the actual Harness ToolRuntime subprocess,
  with the reported Python prefix checked against the managed environment.
- `agentpm-examples/tools-python/validate-order` includes real Pydantic code,
  3 passing development tests, manifest, pyproject, prepared author `agent.lock`
  and author/consumer/Harness instructions. Strict `agentpm lint` passes. Its
  documented uv development invocation returns `{"total":9.0}`. A real dry-run
  preparation produced a Tool archive; a separate local mock Registry served
  that actual archive to normal and fresh frozen consumers. Both installs/run
  paths return the same result, preserve frozen lock bytes and finalize only
  after provisioning. This did not publish a live package or write S3 objects.
- `summarize-text` is prepared as version **0.1.9**, with declared OpenAI 1.x
  dependencies and a real pinned-uv author lock containing 17 distributions,
  including native `jiter` and `pydantic-core` wheels. Its five development tests
  exercise the actual OpenAI client with a local mock transport, a native import,
  and the existing success/error JSON interface. The archive explicitly includes
  source files rather than an entire directory, excluding old ignored `_vendor`
  trees and development bytecode. Strict manifest lint and dry-run lock reuse
  with an absent uv override/closed Registry pass. A local HTTP Registry served
  the final archive to normal and fresh frozen consumers; both ran the Tool
  through its real OpenAI client against a local mock endpoint, returned the
  expected summary, imported `jiter`/`pydantic-core` from the managed environment,
  and preserved frozen lock bytes. This made no paid model calls.
  No production publication or
  `devwork-copilot` dependency update is claimed; these are unchecked Release
  Band 8 rollout steps. `validate-order` remains dev/manual-test only.

Reproduce the native tests after building the CLI (use the exact pinned binary):

```sh
cargo build -p agentpm-cli
AGENTPM_TEST_UV=/absolute/path/to/verified/uv cargo test -p agentpm-cli --bin agentpm python_environment::tests -- --include-ignored --test-threads=1
```

`AGENTPM_TEST_PYTHON` can select the test interpreter explicitly; set
`AGENTPM_PYTHON` to that same path for public CLI/Harness subprocesses. The existing
Resolver Contract workflow now triggers for the runtime changes and runs this
suite on all four native targets after attested staging of the checked-in pin.
Candidate-version dispatches that differ from the compiled pin retain their
resolution smoke; Rust provisioning checks run when the checked-in pin matches.
Actionlint, resolver target checks, **34 maintainer tests**, workspace Clippy,
Rust formatting, example Black/JSON validation, docs text checks and
`git diff --check` pass. The initial 11C API edit was user docs only; the
manual-discovered Registry SemVer read-path correction is recorded below.

Hosted Windows compilation follow-up:

- The operator reported Resolver Contract failed while compiling the CLI test
  binary on Windows: the TUI's `process_is_alive` helper called Unix-only
  `libc::kill` without a platform guard, although its only caller was already
  Unix-only. The helper and its associated TUI test imports now use `cfg(unix)`.
  Unix-only test imports in Harness ToolRuntime and runner tests are guarded too,
  addressing the warnings reported in the same build. Managed Python tests remain
  enabled on Windows; no test was disabled to bypass the native provisioning gate.
- After the guard fixes, macOS workspace tests passed **1,152 tests** (5 native
  tests intentionally ignored in that invocation), and workspace Clippy,
  formatting and diff checks passed. Windows compilation/execution was not
  reproduced locally: this machine has only the macOS Rust target. The required
  hosted rerun was subsequently confirmed green by the operator below.

M11C operator-reported hosted acceptance:

- After the Windows guards were pushed, the operator confirmed **all four
  Resolver Contract jobs pass**: macOS ARM64, macOS x86_64, Linux x86_64 glibc,
  and Windows x86_64 MSVC. This closes the hosted native provisioning/replay gate,
  including the committed macOS-authored resolution replay on the other targets.
  This is operator-reported CI evidence, not a claim of local Windows execution
  or independent retrieval of the hosted logs.
- Earlier pending-hosted notes above describe prior verification states and are
  superseded by this confirmation. Operator manual acceptance is recorded below.

M11C manual walkthrough prepared:

- [The 11C runbook](manual-tests/CLI_M11C_MANUAL_TESTS.md) uses the real
  `agentpm-examples/tools-python/validate-order` source in a disposable author
  copy, a unique dev-only prerelease version, isolated per-process user state,
  and one explicitly identified dev publication. It covers author development,
  lock/metadata preparation, native imports from the managed environment,
  normal/frozen consumers, ready reuse, interrupted/unrecorded state repair,
  alternate interpreter selection, ambient import-path isolation, real headless
  Harness dispatch and a dependency-free legacy vendoring probe.
- `fixtures/m11c/` supplies a local-only Loop/config, scripted model and legacy
  Tool/helper. The model requests the advertised AgentPM Tool alias and completes
  only after observing total 9 in the actual Tool result. The guide prints one
  complete server command for a second terminal, with no dependence on that
  terminal inheriting setup variables. Local fixture Loop/legacy lock entries
  are explicitly identified as staged probes, not downloaded/published packages.
- Rehearsed the shell blocks with zsh against a disposable mock Registry serving
  the actual prepared archive and the real pinned uv binary. Six normal/frozen/
  repair/interpreter installs finalized, native Pydantic imports and direct/
  machine runs succeeded, frozen locks were unchanged, and the shadow control
  failed while managed runs succeeded. The Harness/legacy sections then passed
  using the same disposable consumer and the checked-in local model fixture;
  the run report recorded an ended run and at least one real Tool call.
  Rehearsal corrected the copied consumer's required description/schema fields
  and kept its development pyproject on a valid Python package version while
  the AgentPM manifest uses a unique SemVer prerelease.
- Live login/publication were deliberately excluded from rehearsal. No real
  dev/prod Registry rows or S3 objects were created, no production rollout or
  `summarize-text` publication was performed, and no paid model was called.
  This verifies the guide/fixtures, not operator manual acceptance.

Manual-discovered Registry SemVer correction:

- The operator published the runbook's valid SemVer prerelease and reported
  `packaging.version.InvalidVersion` while viewing it. The package detail/latest
  helpers in `app/tools/application/tools.py` were incorrectly comparing AgentPM
  versions with the PEP 440 Python-distribution parser. They now use the existing
  `semantic_version.Version` dependency, consistent with Registry install/publish
  version semantics. The exact published version remains valid; restarting the
  local backend is sufficient to load the fix. No republish, migration or data
  rewrite is required, and Python dependency version parsing remains PEP 440.
- Follow-up review identified that detail/latest parsing is unguarded, unlike
  `pick_semver`, which skips invalid rows. The operator verified both dev and
  production `tool_versions` contain no versions outside strict three-component
  SemVer. This is operator-reported database verification; the agent did not
  independently query either database. With existing rows verified and new
  publication constrained by the strict manifest schema, no tolerant parsing
  fallback or data migration was added in that parser-only fix. The subsequent
  approved latest-policy alignment below now reuses the resolver's guarded
  selection instead of an unguarded `max()` parse.
- Regression tests cover `0.1.0-m11c.20261024200841`, numeric prerelease ordering,
  stable-vs-prerelease precedence, build metadata and explicit/latest lookups.
  The Tool helper suite passed **32 tests and 8 subtests**. Black, isort, Flake8,
  Pylint, pyupgrade and text/diff checks passed. The configured Bandit environment
  lacked `pbr`; equivalent Bandit 1.7.5 checks with that prerequisite supplied
  passed without application dependency changes. Mypy remains blocked by two
  existing assignments in unchanged `search.py` (1486/1856); the identical errors
  were reproduced with both modified Python files shadowed by their HEAD versions.
  The operator subsequently confirmed the complete manual walkthrough passed,
  as recorded below.

Post-acceptance latest-version policy alignment:

- The operator approved one default policy for package pages and installation:
  highest non-yanked stable SemVer version. Prereleases require an explicit
  version or prerelease range; yanked versions remain viewable by exact version
  but are not install candidates. Selection derives prerelease status from the
  parsed version, fixing the callers' previous hardcoded-false hint.
- Detail reads and dependency links reuse the guarded resolver. Higher
  prereleases/yanked releases no longer replace the stable default. Prerelease-
  only/all-yanked packages have no default candidate. Explicit detail reads
  still succeed, with nullable `latest_version`/`latest_published_ts` instead of
  falsely labeling the selected version as a stable default.
- All eight web package kinds accept that nullable metadata. Their `/latest`
  routes show an informative no-stable-release state linking to published
  version pages; inaccessible/missing packages retain 404 behavior. Upstream
  failures are not mistaken for absent stable releases. The shared stats panel
  is labeled Latest stable, and empty-state metadata remains no-index.
- Caret/tilde lower bounds discard build metadata while retaining prerelease
  identifiers. Regression cases cover stable/prerelease bounds with metadata,
  including metadata hyphens that must not opt stable ranges into prereleases.
- Verification after that follow-up: **482 API tests passed, 7 skipped, 165 subtests passed**;
  **243 web tests passed**. Web typecheck, ESLint (baseline warnings only) and
  Prettier passed. API formatting/lint/security checks passed using the previously
  documented Bandit prerequisite workaround. Mypy reports only the same two
  unchanged `search.py` baseline errors. Regression coverage includes policy
  parity with actual install resolution, explicit prereleases/ranges, yanked and
  invalid rows, nullable detail JSON, empty `/latest` states and explicit
  prerelease pages across all package kinds, and private/missing-page handling.
- Deployment needs the updated web first, then API: the web parser must accept
  null latest-stable metadata before the API begins returning it. No deployment,
  database migration, package republish or SDK release was performed here.
  This is the approved follow-up policy fix, not a claim of new live operator
  acceptance or a reopening of the completed 11C walkthrough.

M11C operator manual acceptance (2026-10-24, America/Denver):

- The operator confirmed **all manual tests pass** after completing the 11C
  runbook. The published dev reference shown in the operator's output is
  `@zack/validate-order@0.1.0-m11c.20261024200841`.
- Accepted coverage includes author development/preparation, real dev
  publication, native imports from a separate managed environment, direct and
  machine execution, ready reuse, a fresh-user frozen install with an unchanged
  lock, interrupted/inventory repair, alternate Python selection, real Harness
  Tool execution through the local scripted model, and legacy author vendoring.
- The operator supplied explicit step 8 output: ordinary Python deliberately
  failed on the ambient shadow module with exit 1, while both managed run modes
  returned total 9 and machine mode reported success. The later all-tests-pass
  confirmation covers the rest of the walkthrough.
- This is operator-reported live dev acceptance, distinct from the agent's
  earlier mock Registry rehearsals. No production publication, CLI release or
  paid model call is implied. The production `summarize-text` publication and
  `devwork-copilot` rollout remain unchecked Release Band 8 tasks.
- Independent review, all four hosted Resolver Contract jobs and the manual
  walkthrough are complete. **M11C acceptance is complete; no milestone
  acceptance criteria remain open.** The unrelated baseline mypy findings
  recorded above remain outside this milestone's fix.

Acceptance status and remaining limitations:

- All four hosted target jobs are operator-confirmed green as recorded above;
  no additional simulated-OS substitute is needed for the native gate.
- The live dev Registry publish/install/run/Harness walkthrough is operator-
  confirmed complete as recorded above. Earlier pending-manual notes are
  superseded by this confirmation.
- A compatible local Python and compatible wheel must exist. New interpreter/ABI
  selection requires reinstall; source-build fallback and automatic Python
  downloads are deliberately unavailable. Old environment directories are local
  retained state, not a new garbage-collection feature.
- No production deployment, CLI release/version bump, schema change, SDK wire
  format change, M12 classification or later multi-artifact release work is
  included. These remain later rollout/milestone work rather than open 11C
  acceptance gates.

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

#### M12 format-policy follow-up — required verification

The reviewed policy in `spec.md` §22 is implemented. The following cases define
acceptance, with fresh local results recorded below; hosted execution and
operator manual acceptance are not implied by local results.

- Validate the shared `tool-payload-formats-v1.json` inventory and derive
  exhaustive test cases directly from it. Reject invalid type/version or shape,
  duplicate/conflicting aliases, malformed bytes, invalid offsets and impossible
  minimum header lengths. Rust must use this embedded inventory rather than a
  separately maintained exhaustive allowlist.
- Run the spec/document generation check against the same inventory. A
  deliberate edit to a documented suffix, basename or signature that disagrees
  with the data must fail the check; a policy-data edit without its derived
  document update must also fail. Verify the check is wired into normal CI.
  Do not use a third handwritten exhaustive list as the expected value. Retain
  independent representative binary fixtures, negative cases and precedence
  expectations to test behavior rather than just shared-data consistency.
- Enumerate the full text/source suffix and exact-basename inventory, including
  the original formats, `.pyx`/`.pxd`, notebook JSON, templates, queries,
  extensionless names and `py.typed`. Each otherwise-portable fixture qualifies
  as `any`, including mixed-case suffixes and lowercase special basenames.
  Retain whole-file UTF-8, binary-control, renamed-native and chunk-boundary
  regressions. Binary content cannot become portable merely by adopting a text
  suffix. Do not add warnings solely because `.pyx`/`.pxd`/`.ipynb` is present.
- Native filename inventory: enumerate all declared native suffixes and literal
  versioned-name fragments from the shared data. Real archive classification
  must report `native_content`, not merely a concrete target that could come
  from unknown-format fallback. Include uppercase names and a versioned native
  fragment before a portable final suffix such as `.json`; a matching parent
  directory alone must not affect a Python source basename. Reject malformed,
  duplicate and conflicting policy entries. Verify documented suffix/fragment
  edits and data-only native additions/removals fail the generated-section check.
- Explicitly cover `MANIFEST.in`, `requirements.in` and a mixed-case `.IN` name
  as portable text when otherwise eligible. Native/binary content or executable
  permissions must not gain an exemption through `.in`. Packaging these inputs
  does not execute a build step or resolve dependencies from their contents.
- Named dotenv cases: `.env`, `.env.local`, `.env.example`, `.env.production`,
  mixed-case `.ENV.LOCAL`, and nested `config/.env.local` with valid
  non-executable text can be `any`. Explicitly test basename equality/prefix
  modes rather than reusing the extension-enumeration test helper.
  `config.env.local`, `.envrc`, `.environment` and `.gitignore` must not match
  the dotenv rule. A parent directory named `.env.local` must not grant its
  unrecognized child files a portable classification. These are independent
  matching regressions, not a duplicated exhaustive policy list.
  Dotenv names do not bypass installed-environment,
  native suffix/signature or executable-bit checks. No secrets scanning or
  dotenv publishing prohibition is introduced. `.env.so` still narrows as a
  native library, and `.env.png` with a mismatched PNG signature still fails.
- Named metadata cases: single-file `example.egg-info` and
  `example.egg-info/PKG-INFO` with valid text can be `any`, as can conventional
  textual members such as `SOURCES.txt`/`requires.txt`. Their exact archive bytes
  remain present. A native, binary/unknown or executable member inside that
  directory still narrows the artifact and names the correct cause. A
  `.dist-info` path or known virtualenv/site-packages state remains conservative.
- For every recognized portable binary suffix, test a matching signature/header
  fixture, a truncated required signature/header, wrong-format bytes, and native
  bytes renamed to that suffix. Matching recognition can qualify as `any`;
  mismatch fails packaging with path and expected format. WebP requires both
  RIFF and WebP identifiers at their specified offsets. Include mixed-case
  names such as `.PNG` and `.JPEG`. These are recognition checks, not full
  decoder/parser acceptance tests.
- Cover the publish-time compatibility change with a previously accepted
  legacy Tool artifact containing GIF bytes named `assets/icon.png`: existing
  legacy reads/installs retain their behavior, but preparing its next version
  through the updated CLI fails in dry-run/live modes before publish-init/upload.
  Assert the error names the path/expected format and advises renaming to the
  actual format or replacing the data, with reference updates after a rename.
  The checker need not identify GIF or add a new format recognizer. Correcting
  the filename to `assets/icon.gif` and updating references clears the mismatch
  failure but still reports unrecognized-format narrowing; replacing the data
  with recognized PNG can qualify as `any` when all other members qualify.
  Do not attempt to overwrite an immutable existing version or retroactively
  assign it compatibility metadata.
- Confirm recognized binary assets bypass text validation but cannot bypass
  executable-bit or installed-environment narrowing. Executable Python source
  retains its existing exception. Preserve `.so.N`, all native signature and
  embedded wheel/archive regressions.
- Accepted M12 limitation: archive modes are publisher-platform-dependent.
  Unix preserves executable bits; Windows assigns ordinary recognized text/data
  `0644` by extension-based mode handling. A stray executable bit can therefore
  narrow only the Unix build. M12 does not normalize modes or emit a hypothetical
  Windows executable-member warning. Cross-build enforcement and its additional
  fixtures belong to M16's CI/headless-signing cases below; this note does not
  claim those tests have run.
- A known native member plus an invalid `.png` fails in **both archive member
  orders**. No early concrete-target return may conceal a fatal mismatch.
  Neither live publish nor dry run sends publish-init/upload requests on this
  error. A prior valid archive remains byte-identical, and authored manifest/
  lock bytes remain unchanged.
- Unknown-format cases, including `.rtf`, `.js`/`.ts`, a portable-but-unrecognized
  image such as `.tiff`, and `.pth`, are preserved, narrow to the current build
  target and name the relative file/reason/target. Do not claim their presence
  proves an undeclared runtime or native code. Differing reasons (native content,
  installed state, unknown format, executable member, unsupported member type)
  remain distinct. Multiple diagnostics have stable path ordering and no
  duplicate path/reason entries.
- Output/severity cases: narrowing is a warning on stderr and does not fail a
  valid build; a mismatched binary is an error on stderr with a nonzero exit.
  Both remain visible under `--quiet` and captured/non-TTY CI invocation.
  Publish `--strict` retains its existing manifest-lint behavior and does not
  promote these packaging warnings. A portable payload has no narrowing warning.
  Assert stable code/severity/path/message/help and applicable target or expected
  format on typed diagnostics, not only matches against rendered strings.
- Successful build descriptors and mock Registry publish-init requests carry
  structured `diagnostics` alongside target/format, in stable order, including
  warning data for concrete targets and an empty array when none apply. Fatal
  mismatch retains structured errors on the failed build result, produces no
  usable successful descriptor and submits no publication request. Do not add
  diagnostics to canonical release integrity inputs or Tool archive members.
  These M12 tests cover structured-data production and human stderr rendering;
  machine stdout and fan-in presentation remain M16 acceptance work, which must
  preserve the same diagnostics without scraping human messages. Do not claim
  a current publish `--machine` mode exists or add it for this follow-up.
- Real CLI dry-run/live mock Registry tests exercise both `any` and concrete
  target results with the expanded policy. Descriptor and displayed target
  agree; publisher telemetry remains separate; no consumer target is written
  into the portable lock and no legacy release receives an inferred `any`.
- Run relevant suites on both macOS architectures, Windows and Linux using the
  existing native matrix. Initial M12/M11C runs do not close this follow-up's
  hosted execution gate. Recheck both prepared dependency-bearing examples.

No directory-exclusion or new blanket rejection behavior is authorized by this
format-policy test block. A separate collection policy must first distinguish
incidental traversal results from explicitly listed paths. Existing missing-path
and embedded-archive failures remain in force.

#### M12 collection-policy follow-up — required verification

These are the reviewed requirements for step 3. Fresh local results below cover
the implemented collection policy; hosted and operator acceptance remain open.
Earlier format/classifier results do not establish collection acceptance.

- Shared-data consistency: classification and traversal derive named environment
  directories and markers from the same validated policy definitions. Exercise
  all data-defined collection rules/types and deliberate mutations that would
  diverge from the installed-state vocabulary or generated spec inventory;
  normal CI must fail. `.dist-info` remains classification-only and `.egg-info`
  retains its portable-text exception.
- Incidental traversal: an otherwise source-only Python Tool with `files: ["."]`
  omits `.git/` and a `.git` worktree pointer file, `__pycache__/`, `.venv/`,
  `venv/`, `site-packages/`, `dist-packages/`, named OS cruft and dotenv files
  under the draft's file/directory rules. Assert actual tar members, not just
  diagnostic strings. A normal regular file named `venv` is not mistaken for
  a directory. Unrelated package kinds keep their collection behavior.
- Environment markers: a nested custom directory such as
  `dev-python/pyvenv.cfg` is pruned as a whole, including siblings and deeper
  dependencies; omitting only its marker fails the test. Marker matching also
  works as `PYVENV.CFG` on a case-sensitive filesystem. Do not read marker
  contents, execute a Python interpreter, follow symlink targets or recurse
  merely to classify/count the excluded tree. Unreadable directory inspection
  fails clearly with the path rather than silently bypassing detection.
- Prune before collecting/counting/validating excluded descendants. An embedded
  wheel or unreadable child inside an incidentally excluded tree must not be
  processed, and excluded descendants cannot trigger `MAX_TAR_ENTRIES`. Existing
  limits and archive prohibitions still apply when explicitly preserved.
- Dotenv: incidental `.env`, `.env.local`, `.env.production`, `.env.sample`,
  `.env.template`, `.env.dist` and mixed-case equivalents are omitted. Only
  exact `.env.example` survives the exception, including case variants;
  `.env.example.local` does not. Explicit dotenv requests are preserved with
  override information and undergo the normal text/binary/executable checks.
  No secret-content inspection or dotenv publishing prohibition is introduced.
- Explicit precedence: `files: ["mypkg/"]` omits a nested cache/environment;
  `files: ["mypkg/__pycache__/"]` preserves its whole requested subtree.
  Include otherwise excluded descendants within an explicitly preserved root
  to verify full-subtree semantics. Explicit custom environment roots work too.
  `files: [".", ".git/config"]` preserves the explicit child despite pruned
  incidental `.git/` traversal. Reverse overlapping manifest entries and assert
  identical deduplicated member sets and decision diagnostics. Do not claim a
  pruned directory's contents are wholly absent when explicit children survive.
- Required inputs and failures: preserve the manifest, generated metadata and
  required entrypoint. An entrypoint inside an excluded location preserves only
  that required file unless more content is explicitly requested. Missing
  explicit `.env`/environment paths still fail instead of being ignored. Explicit
  preservation does not bypass blocked wheels/archives, path protections,
  reserved metadata, limits or binary signature mismatch errors. Failure leaves
  the prior valid archive and authored manifest/lock unchanged, with no upload.
- Preservation boundaries: keep `_vendor`, `.egg-info`, `.dist-info`, native
  content and ordinary project/template files unless a named collection rule
  applies. Makefile, Dockerfile, CODEOWNERS, `.gitignore`, `.editorconfig` and
  `.dockerignore` are not newly stripped or newly portable. Editing ignore-file
  contents does not change selection: no Git/npm ignore interpretation or
  negation/nesting/precedence support is introduced.
- Information surfaces: stable `info` omission/explicit-override diagnostics
  name relative paths and matching policy reasons. They appear on stderr in
  dry/live/quiet/non-TTY runs and in build/publish descriptor diagnostics, ordered
  and deduplicated. A pruned directory is reported as a skipped traversal root,
  never as a fabricated descendant-file count. `any` remains valid with these
  diagnostics; `--strict` and compatibility warning/error behavior are unchanged.
  A preserved excluded member can independently produce both override information
  and a normal compatibility warning, without conflating their severities.
- Verify case-insensitive collection on case-sensitive Linux/Windows/macOS
  fixtures as applicable, including `.ENV`, `.GIT` and `.DS_STORE`. Explicit
  requests preserve those names with their actual spelling. Link handling must
  not dereference external targets; apply the draft's named-entry rules before
  existing link/classification protections.
- Run the real CLI dry/live mock Registry flows and relevant checks on both
  macOS architectures, Windows and Linux. Recheck both prepared examples and
  operator omission/override reporting before closing M12/Band 8. No existing
  published artifact is recollected, reclassified or rewritten during install.

Collection is implemented with its own evidence below. The earlier format-only
record is historical; it does not substitute for collection verification.

#### Release Band 8 Version Preparation (2026-10-24, America/Denver)

Prepared CLI **0.1.37** in the crate manifest and Cargo-generated lockfile; the
rebuilt local binary reports `agentpm 0.1.37`. The website status page now pairs
**v0.1.37** with release date **2026-10-24**. SDK versions and the already
published **0.1.36** APDS tolerance floor remain unchanged.

Verification: offline CLI build, **18 passing** packaging/Python-resolution
integration tests, workspace Clippy/Rust formatting, cross-repo uv pin/target
agreement, policy documentation verification, website typecheck, targeted status
page ESLint and Prettier checks. Only the CLI package version changed in
`Cargo.lock`; no dependency upgrades were introduced. The recent full workspace
baseline remains **1,193 passed, 5 ignored** from the Node correction.

The operator requested deployment order **CLI → Registry → Web**. Wait for the
required hosted checks, then the operator publishes the CLI tag/release, deploys
the Registry, and deploys the website. The CLI release workflow retains its
production resolver-mirror verification gate. Verify the Registry's new Python
metadata contract before publishing/installing the dependency-bearing production
example; its rollout remains unchecked. Version preparation is not evidence of
tag publication, Homebrew/Scoop availability, or a production deployment.

Final CI handoff: the operator subsequently confirmed **CI is all green** and
is proceeding with the CLI merge/production release. This confirms the final
normal CI/native Resolver Contract gate after the Node correction and version
preparation. Together with completed independent review and manual acceptance,
**M12 acceptance is complete**. This is operator-reported CI evidence; no CLI
tag publication, installation or production deployment is claimed yet. Earlier
pending-CI notes below are historical and superseded by this confirmation.

#### Release Band 8 Production Deployment Confirmation

The operator confirmed the **CLI, Registry and Web production deployments are
complete**, following the requested CLI → Registry → Web rollout sequence.
This closes the deployment checklist item. It is operator-reported deployment
evidence, not an independent installed-version, live metadata-contract or
website-marker audit. The separate retry-fix CI evidence item is not inferred
closed from this deployment confirmation.

Prepared `summarize-text` **0.1.9** and `devwork-copilot` **0.1.7** publication,
the subsequent real consumer install/lock/environment update, direct and Harness
summarization, and legacy-version verification remain unchecked. No publication,
paid model call, commit or GitHub push was performed while recording this update.

#### Release Band 8 Production Example Smoke Verification

This later evidence supersedes the unchecked publication/install/direct-run
status in the earlier deployment and source-preparation records. The operator
confirmed both `@zack/summarize-text@0.1.9` and
`@zack/devwork-copilot@0.1.7` were published to production, then supplied a
successful real install in `agentpm-examples/agent-app-devwork-python`, including
Python environment preparation and install finalization.

Read-only local inspection confirmed:

- Installed CLI reports **0.1.37**. Consumer lockfile v4 selects Agent **0.1.7**
  and Tool **0.1.9**, and the installed Agent manifest matches the prepared
  source manifest.
- The consumer lock's Python resolution, the installed `agentpm-python.json`
  and the reviewed author lock agree, with resolution digest
  `50d5696264f687684b7665fcafd3bcfd95cdbb669a5b55cfb434efe7b15fbd1e`.
- The managed Apple Silicon environment has a matching readiness fingerprint
  for uv **0.13.5** and Python **3.13**, and exactly **16** installed
  distributions. The portable resolution contains **17**: Windows-only
  `colorama` is correctly absent here. OpenAI and the native `pydantic-core`
  and `jiter` modules import from the managed environment. No `_vendor`
  directory exists in the installed Tool, and `.agentpm/` is Git-ignored.
- The operator ran `agentpm run @zack/summarize-text@0.1.9` with supplied text
  and a 50-word budget and reported `ok: true` with an actual summary. This is
  operator-executed paid model evidence; the inspection made no model request.
- A read-only production status-page request returned HTTP **200**, showing
  CLI **v0.1.37** and `Release: 1d ago`, consistent with the checked-in
  **2026-10-24** source date. The rendered page uses relative age rather than
  exposing the literal ISO date. Successful real publication/install and the
  matching installed metadata verify the deployed Registry contract.

The initial production publish was blocked by a WAF false positive on the
README sentence containing the inline Python entrypoint command. Rewording
that sentence allowed the full credential-free metadata probe to reach Flask
(expected HTTP **401** with request ID); the operator then confirmed successful
signed publication. No firewall setting, signing key or token was changed.

The Python SDK app pin, existing test and README now identify Agent **0.1.7**;
its **5** unit tests and formatting checks pass. An offline metadata-loading
check fails because the existing SDK accepts only lockfile v2/v3. Its direct
Tool spawn path also does not select the CLI-managed Python environment. The
operator explicitly requested no SDK changes now, deferring that compatibility
work to Milestone 17. No successful SDK-app startup is
claimed, and no SDK source or contract was changed.

The operator subsequently supplied Harness run
`agentpm-examples/agent-app-devwork-python/.agentpm-state/runs/run-18e1aa0d9a8fe390-5`.
Read-only inspection of `events.jsonl` and `report.json` confirms:

- Two real `@zack/summarize-text` invocation/completion pairs, in
  `inspect-issue` and `draft-comment`, with source `agent_binding` and completed
  status. The events occur at lines 29–30 and 52–53 respectively.
- Preflight `ready`, `error_count: 0`, and terminal status `handed_off` after
  the `resolve-or-handoff` phase. This is actual Harness Tool execution, not
  merely a model-written summary.
- The consumer lock selects only `@zack/summarize-text@0.1.9` and Agent
  `@zack/devwork-copilot@0.1.7`. Exact versions are established from that
  inspected install state; the action events identify the Tool by name only.
  The report's Agent identity is `synthetic-agent@0.0.0`, so the report alone
  is not an exact published-Agent identity assertion. Tool arguments/results
  are redacted; no claim about their unredacted content is inferred.

This closes the Harness invocation check independently of the deferred SDK
app startup check. No run was re-executed and no additional model/API call was
made while inspecting the evidence. Hosted verification of the resolver retry
correction is also operator-confirmed complete. The only remaining Band 8
check is the existing vendored production-version runtime smoke test.

Legacy smoke attempt: the operator ran the installed
`@zack/summarize-text@0.1.8` with the Apple Silicon Homebrew Python **3.13**.
Failure artifacts under
`agent-app-devwork-python/.agentpm/runs/run-20261025041211534-88813` show the
original Tool entrypoint and vendored OpenAI/Pydantic modules loading, followed
by `ImportError` from `pydantic-core`: incompatible architecture, `x86_64`
versus the host's required ARM architecture. Read-only `file` inspection
confirms both bundled `pydantic-core` and `jiter` extensions are Mach-O
**x86_64**, not universal or ARM binaries. A matching Python minor version alone
does not make these native libraries compatible.

This demonstrates the old vendored execution path is still selected, but is
not a successful legacy runtime smoke test. The specific failure is explained
by the pre-existing Intel-only artifact; it does not establish a new CLI
regression. The operator no longer has the Intel Mac. Keep this check
**blocked by host availability**, requiring a compatible Intel macOS/Python
3.13 host or an explicitly accepted verification exception to close. No
Rosetta/emulation setup, package rewrite, vendor replacement, lock change or
additional model call was performed. The new **0.1.9** managed-environment
Tool remains successfully verified on Apple Silicon through direct CLI and
Harness execution.

Subsequent legacy smoke verification: the operator identified other vendored
production Tools and proposed sentiment analysis. The already-installed
`@zack/sentiment-analysis@0.1.5` in
`agentpm-examples/agent-app-support-assistant-workspace` was executed using
CLI **0.1.37**, Apple Silicon Python **3.13**, telemetry disabled and the
existing legacy runtime path:

```bash
AGENTPM_TELEMETRY=0 AGENTPM_PYTHON=/opt/homebrew/opt/python@3.13/bin/python3.13 \
  agentpm run @zack/sentiment-analysis@0.1.5 --machine \
  --input '{"text":"I love this tool. It works beautifully!"}'
```

The process exited **0** and returned `status: success`, exact Tool version
**0.1.5**, and output `ok: true`, `label: positive`, `compound: 0.8478`.
The installed manifest has no declared Python dependencies and no
`agentpm-python.json`; isolated import inspection confirms VADER loads from
the installed Tool's `sentiment_analysis/_vendor/` directory. Its vendor tree
also contains two `charset_normalizer` native extensions, so it is not asserted
to contain exclusively pure Python files; successful execution establishes
that those files do not block this legacy Tool on the tested host.

No API key, model call, package install/publication, lock rewrite, vendor
replacement or SDK change was needed. This supplies the successful existing
vendored-production Tool smoke evidence and closes the last Band 8 check.
The prior Intel-only summarizer failure remains documented as an artifact/host
limitation, not a passing native-legacy test. **Band 8 acceptance is complete**;
SDK compatibility remains deferred to M17.

#### Release Band 8 Post-Merge Resolver Spawn Race

After the CLI merge, the operator reported Linux CI failing
`valid_download_is_verified_cached_and_reused_offline` with `Text file busy`
(`ETXTBSY`, errno 26). The operator reran the failed jobs and confirmed they all
passed on the second attempt. This records an intermittent failure rather than
retracting the earlier successful M12 acceptance evidence.

The explicit writer close introduced during 11A is still present. A concurrent
fork can nevertheless inherit that writable descriptor and retain it until the
child execs/exits, even after the parent closes its handle; see
[rust-lang/rust#114554](https://github.com/rust-lang/rust/issues/114554). A local,
disposable Linux container reproduced that exact inherited-writer condition with
the parent handle already closed, then executed the fixture successfully after
the child released its inherited handle. The container used a cached image,
disabled networking, a read-only root and executable temporary storage. This
kernel-level Perl reproduction is not a Linux Rust-suite run.

The local correction retries only failed resolver `--version` spawns whose I/O
kind is `ExecutableFileBusy`: at most 20 retries, 25 ms between attempts, or
500 ms of added waiting. Permission/missing-file/other spawn errors return
immediately; a process that already started is never retried because of its
exit status. A persistently busy executable still returns the original error
through the existing context. Digest/version checks, explicit writer closure,
temporary-file cleanup and atomic cache publication remain intact. No resolver
resolution/provisioning command or network request is replayed by this helper.

Verification on local Apple Silicon macOS:

- New deterministic tests cover transient-busy recovery, bounded persistent-busy
  failure, immediate unrelated-error propagation, and no retry after a started
  process exits unsuccessfully. The full workspace suite passed **1,195 tests**
  with **5 existing opt-in tests ignored**; the original download/cache/offline
  test passed as well. Workspace Clippy and Rust formatting passed.
- An additional Linux-only Rust test holds a real writable descriptor, asserts
  a kernel `ETXTBSY`, releases it from the retry pause, and verifies successful
  execution and version validation. It is included in normal Linux CI, but
  cannot be counted as executed by the local macOS run.

The operator's green rerun used the pre-fix code. The new patch remains local,
uncommitted and unpushed at handoff; hosted verification of the correction is
still required before including it in a production CLI release. M12's completed
review/manual acceptance stands; this is a subsequent release-hardening fix.

Subsequent operator confirmation: **hosted CI verification of the resolver
retry fix is complete**. This closes that Band 8 verification item and
supersedes the earlier local-only/unpushed handoff status above. It is separate
from the original pre-fix green rerun and does not identify a new published
CLI version or imply an SDK release.

#### Release Band 8 Example Source Preparation

Prepared `agentpm-examples/agent-packages/devwork-python` as
`devwork-copilot` **0.1.7**, adding exact direct dependency
`@zack/summarize-text@0.1.9`. Bound the Tool to `inspect-issue` and
`draft-comment`; the existing Loop keeps Tools disabled in `resolve-or-handoff`.
The previous published Agent version remains unchanged. Read-only public
Registry metadata showed the existing Agent at **0.1.6** and summarizer at
**0.1.8** while selecting these prepared versions.

The summarizer was already prepared as **0.1.9**. Its source manifest,
`pyproject.toml` and reviewed `agent.lock` were retained byte-for-byte. The README
now identifies CLI **0.1.37+** and defaults to reusing the reviewed resolution;
refresh is an intentional dependency-update operation. The Agent README explains
Tool-before-Agent publication, subsequent real consumer install, direct/Harness
verification and credentials. The public Python-dependencies guide identifies
the matching CLI floor. No internal milestone/stage/release-band terminology
was added to the summarizer's published source or README.

Verification:

- Existing offline `uv` summarizer suite: **5 tests passed**, using the real
  OpenAI client with mock HTTP transport and native Pydantic imports; no network
  or paid model request occurred during those tests.
- Both manifests passed real `agentpm lint --strict` against the local schema.
- Fresh isolated copies passed strict CLI publication dry runs against a closed
  loopback endpoint. A deliberately missing uv override confirmed reuse of the
  summarizer's existing **17-distribution** resolution. Its archive reports
  `any`, includes matching `agentpm-python.json`, and contains no author lock,
  vendored environment or duplicate member. Authored Tool inputs stayed unchanged.
- The Agent dry run packages version **0.1.7**, the exact Tool pin and both phase
  bindings. It creates no author Tool lock or consumer install state. This is
  source/packaging verification, not a claim of a live installed Agent graph or
  successful Harness execution of the unpublished dependency version.

Existing consumer pins/locks, templates and SDK demo apps were left intact.
After Tool then Agent publication, run the documented real consumer install
(the README uses `app-harness-minimal-agent`) and review its generated dependency
request/lock. Direct paid summarization, an actual Harness Tool call and legacy
production-version smoke verification remain unchecked rollout work. No commits,
pushes, publication, consumer installation or paid model call were performed.

#### M12 Operator Manual Acceptance

The operator confirmed **all M12 manual tests are complete and pass**, after
rebuilding with the authorized Node packaging correction and resuming the guide.
This includes the corrected required-entrypoint expectation: selected source
under `__pycache__` remains preserved while correctly narrowing as installed
state. The rebuilt Node result was shown explicitly with six unique members and
no generated archive/temp member, retaining its selected dotenv/Git/environment
content.

Acceptance covers the full runbook: portable source/assets and both prepared
dependency-bearing examples; accurate native/unknown/executable/link/state
warnings; incidental omissions, required inputs and explicit overrides;
expected failures and atomic preservation; quiet/strict/stderr behavior;
loopback publish descriptors/uploaded members and no request on invalid
packaging; and unchanged authored manifests/locks. This is operator-reported
evidence, not an agent-executed real Registry publication or production rollout.

Earlier pending-manual notes below are historical and superseded by this
confirmation. Independent review and the initial updated four-target hosted
runs were already confirmed. The later final all-green CI confirmation above
also closes the fresh normal CI/Resolver Contract gate after the Node correction.
Release Band 8 production publication/install/run checks stay separate and
unchecked.

#### M12 Operator-Confirmed Hosted Acceptance

After the Windows fixture correction below, the operator confirmed normal CI
and all four updated Resolver Contract jobs passed: macOS ARM64, macOS x86_64,
Linux x86_64 GNU and Windows x86_64 MSVC. This closes the M12 native hosted gate,
including the expanded classifier/format inventory, collection and real CLI
payload integration suites. Independent review is also operator-confirmed
complete. Earlier pending-hosted/review notes describe historical verification
states and are superseded by this confirmation; no independent retrieval of
hosted logs is claimed here.

Operator manual acceptance remains open. The prepared
[M12 runbook](manual-tests/CLI_M12_MANUAL_TESTS.md) uses disposable Python/Node
payloads, explicit-versus-incidental selection checks, atomic format failures,
and a loopback publish capture, then rechecks isolated copies of both prepared
dependency-bearing examples. No real Registry publication, S3 write, dependency
refresh or paid model call is required. Completing the guide does not perform
the separate Release Band 8 production example rollout.

M12 runbook builder verification (local Apple Silicon macOS): all **19 literal
shell blocks** were extracted from the guide and executed in order in a fresh
zsh process. The guide's assertions passed, including three complete loopback
init/upload/finalize exchanges, typed native/collection diagnostics, selected
tar members, request-order equivalence, Node behavior, expected missing-path/
wheel/signature failures, and byte-identical prior archives/no HTTP request on
the live mismatch failure. Both prepared examples reused existing resolutions
despite an invalid uv override, reported `any`, and retained authored manifest/
lock bytes; no dependency refresh or actual publication occurred. Fixture Python
formatting/lint and the policy documentation check passed. This verifies the
guide and its inputs, not operator manual acceptance.

Operator walkthrough correction: the required-entrypoint section incorrectly
described an `any` result for `__pycache__/tool.py`. Its member-selection
assertions passed during builder verification, but did not check that prose
expectation. Installed-state classification correctly narrows the preserved
entrypoint to the current target; required inclusion does not exempt its path.
The guide now captures/asserts the concrete target, installed-state warning and
omission/override information as well as selected members. The corrected block
was rerun successfully with fresh fixtures on local Apple Silicon macOS. No
classifier behavior changed; overall operator manual acceptance remains open.

Operator Node walkthrough finding: the relative-manifest `files: ["."]` fixture
also included the in-progress `target/agentpm/*.tar.gz.tmp` and duplicate manifest/
entrypoint members. The non-Python collector retains leading `./` while its
generated-output exclusion and deduplication keys assume normalized names.
Source inspection of `1383c01^` confirms the relevant traversal/key logic
predates M12. Node's absent compatibility descriptor and preserved dotenv/Git/
environment content satisfy M12's unchanged-runtime boundary; the extra entries
are an existing packaging defect, not intended archive contents. The runbook
initially recorded that limitation and kept this fixture unpublished. The
operator then explicitly authorized fixing it before continuing manual testing.
The correction/evidence below supersede that initial deferral; Python-specific
collection policy still does not apply to Node.

Authorized Node correction regression requirements:

- Use the real CLI with default `agent.json`, dotted `./agent.json`, absolute
  and nested-relative manifest paths. Include dotted explicit file/entrypoint
  aliases, overlapping directory requests and a selected generated temp file.
- Inspect actual archive members: no duplicate manifest/entrypoint/data names,
  no leading `./`, and no generated output under `target/agentpm/`. Existing
  archives/temp files must not break either the first or repeated packaging.
- Preserve selected Node dotenv/Git/environment content, absence of Python
  classification/omission diagnostics, and authored manifest bytes.
- A real payload wheel outside the generated output tree still fails and
  preserves the last valid archive byte-for-byte.
- Rebuild and repeat the Node operator block in the same disposable workspace,
  including when its existing archive came from the earlier buggy binary.
  Fresh hosted confirmation for this correction remains required; the earlier
  four-target success is evidence for the implementation before this fix.

Authorized Node correction local evidence:

- The new real-CLI regression failed before the fix because a leading `./`
  caused an existing generated archive to be treated as a forbidden payload
  archive. After the fix, all four manifest forms pass two successive builds,
  normalized/deduplicated member checks, selected dotenv/Git/environment checks,
  and the blocked-wheel/unchanged-previous-archive assertions.
- `cargo test --workspace`: **1,193 passed, 5 ignored**; the payload integration
  suite now has **8 passing tests**. Existing opt-in real-uv tests remain ignored
  in the normal suite. Loopback fixtures ran outside the sandbox.
- Workspace Clippy, Rust formatting, policy documentation verification and its
  **2 tests**, documentation hooks and publish-guide MDX compilation passed.
- The updated Node runbook block was executed with fresh disposable inputs and
  an actual archive retained from the old buggy binary. Rebuilding replaced it
  successfully; a second package operation also passed. The final archive has
  six unique members (`agent.json`, `tool.js`, `README.md`, `.env`,
  `.venv/data.txt`, `.git/config.json`) and no generated output. No real Registry
  publication occurred. This is builder verification, not operator acceptance.
- Fresh hosted runs and the operator's resumed manual confirmation remain open.
  The correction changes generated archive bytes but does not modify authored
  contracts, existing published artifacts or Python compatibility policy.

#### M12 Windows CLI Fixture Correction (2026-10-24, America/Denver)

The operator reported six failures in the hosted Windows `payload_compatibility`
suite: CLI startup could not determine config directories. Its fixture set
`USERPROFILE` to an uncreated isolated home and did not supply isolated Windows
app-data paths. The fixture now creates the profile's `AppData/Roaming` and
`AppData/Local` directories before CLI startup and sets `APPDATA`/`LOCALAPPDATA`
alongside the existing HOME/USERPROFILE/XDG child-process environment. This is
a test-fixture correction, not a product config-directory fallback.

The seventh test previously accepted any failed command; it now asserts the
specific missing-manifest-path or blocked-embedded-wheel error, preventing a
startup failure from masquerading as packaging coverage.

Local verification on Apple Silicon macOS: all **7 payload integration tests**
passed, as did workspace Clippy and Rust formatting. The local HTTP fixtures
require loopback binding and ran outside the sandbox. A fresh hosted Windows
rerun is required to verify the native known-folder behavior; local results do
not close that acceptance gate. The operator has confirmed independent review
complete; fresh hosted verification and manual acceptance remain outstanding.

#### M12 Native-Inventory Review Fix Evidence (2026-10-24, America/Denver)

Moved the existing native filename suffixes and literal `.so.` basename fragment
into the shared format inventory. The classifier consumes that inventory; the
spec's native table is generated and checked alongside text, binary and
collection policy. No native filename rule was added or removed. Independent
native-content signature checks remain in place.

Local verification on Apple Silicon macOS:

- `cargo test --workspace`: **1,192 passed, 5 ignored**. The five existing opt-in
  real-uv tests remain ignored; loopback fixtures ran outside the sandbox.
- Shared SDK artifact/policy tests: **6 passed**, including data-driven native
  matching, uppercase names, literal versioned-name fragments, parent-directory
  negatives and invalid/conflicting inventory rejection. Classifier tests
  enumerate every native suffix/fragment and assert `native_content`, so unknown
  format fallback cannot hide a missing native rule.
- Documentation generator tests: **2 passed**. Native suffix additions/removals,
  fragment changes and manual native-table edits all fail the drift check unless
  the document is regenerated. The read-only inventory check, Clippy, Rust
  formatting, and Python formatting/lint passed.

Fresh hosted target runs and operator manual acceptance remain open; these local
results do not substitute for them.

#### M12 Collection Implementation Evidence (2026-10-24, America/Denver)

Implemented the separately reviewed collection policy for Python Tools only.
The shared JSON contract now owns environment directories/markers,
classification-only distribution metadata suffixes, incidental file rules and
the exact dotenv exception. Classification and collection consume the same
installed-state definitions; strict validation rejects conflicting roles and
divergent dotenv definitions. The generated collection table is checked in
normal CI with the rest of the format inventory.

Local verification on Apple Silicon macOS:

- `cargo test --workspace`: **1,191 passed, 5 ignored**. The five pre-existing
  opt-in real-uv runtime tests remain ignored by the ordinary suite; no new
  collection test is ignored. Loopback fixture tests ran outside the sandbox.
- The new collection suite has **10 passing tests**. Actual archives verify
  pruning, Git worktree pointer files, case variants, the narrow dotenv
  exception, normal-file versus directory kinds, required entrypoints, explicit
  descendants and full-subtree overrides, and entry-order-independent member
  sets/diagnostics. Marker detection handles uppercase `PYVENV.CFG` and unreadable
  marker contents without opening them. Explicit inclusion of that custom
  environment preserves its subtree. A 15,001-file environment does not consume
  the tar entry budget when incidental, but fails the existing cap when
  explicitly included. Fatal wheel/signature checks still apply to preserved
  trees and retain the prior valid archive on failure.
- Unix tests verify symlinks are not followed, explicit paths cannot traverse
  ancestor links, and unreadable directory inspection reports the affected
  path. These actual link/permission fixtures are Unix-only; shared rule/kind
  tests and the main collection/archive suite also run on Windows. Fresh hosted
  verification remains pending and is not inferred from local results.
- `payload_compatibility` has **7 passing CLI integration tests**, including a
  full mock init/upload/finalize exchange with directory collection and explicit
  dotenv inclusion. Assertions cover uploaded tar members, `any` with `info`
  diagnostics, stable rule/reason data, quiet/non-TTY/strict stderr behavior,
  and absence of incidental dotenv/Git/environment content. A Node directory
  package retains its previous collection behavior. Existing authoring and
  Python-resolution suites pass, including the established missing-file message
  and no-partial-artifact behavior.
- Shared SDK artifact/policy tests: **5 passed**. Data-driven environment/kind
  checks tie collection and classification to one vocabulary and preserve
  classification-only `.dist-info`, along with binary/format regressions.
- Documentation generator tests: **2 passed**; deliberate collection table/data
  drift fails verification. The read-only inventory check, Clippy, Rust and
  Python formatting/lint, documentation hooks and MDX compilation passed.
- Fresh isolated copies of prepared `validate-order` and `summarize-text` pass
  real CLI dry runs as `any`, without narrowing warnings, resolver acquisition
  or authored manifest/lock changes. Their explicit source-file lists remain
  compatible with the collection policy. No example source/version change or
  live Registry publication was performed.

Selection is resolved before archive writes, with deterministic member maps,
explicit-root guards, no-follow traversal and errors propagated with paths.
Directory marker probes are reused within one collection; traversal uses a heap
stack rather than recursive process-stack calls. Existing generated-output,
author-lock and metadata handling remain in force. The final selected files feed
the existing atomic archive/signature validation path, and collection itself
does not rewrite authored manifests or locks.

Information diagnostics have stable omission/override codes, relative paths,
rule IDs, reasons and applicable help. They are rendered on stderr even in
quiet/CI invocation and carried in build/publish metadata. They do not infer
compatibility from the presence of diagnostics: an `any` payload can contain
information. Format failures retain collection information in the typed error
without returning a usable descriptor or sending a publish request.

Remaining acceptance and intentional limits:

- Updated hosted macOS ARM/Intel, Windows and Linux runs, independent review
  and operator manual acceptance are still open. The existing Resolver Contract
  matrix includes the new collection suite as well as SDK policy, classifier
  and CLI integration tests. No hosted success or live publication is claimed.
- The omission list is intentionally small. Surviving project/template names
  such as `.gitignore` or Dockerfile are preserved and can still narrow the
  payload when their format is unrecognized. Git ignore semantics are not used.
- Exact explicit requests can deliberately preserve dotenv data or complete
  environment trees; full-subtree inclusion is intentional and reported, and
  does not bypass normal packaging/integrity restrictions.
- Collection policy/tooling changes can change subsequent archive bytes.
  Existing published releases are not recollected/reclassified during reads or
  installation. No release schema, Registry storage model, machine flag,
  fan-in workflow or other package kind behavior was added.

#### M12 Format-Policy Implementation Evidence (2026-10-24, America/Denver)

Implemented the reviewed format-policy follow-up while leaving directory/file
collection rules unchanged. The authoritative inventory is
`crates/agentpm-sdk/contracts/tool-payload-formats-v1.json`, embedded and validated
by the shared Rust SDK. It supplies every portable suffix/basename/prefix,
metadata exception and binary signature. The Markdown inventory regions are
generated from that data; the product docs link the canonical inventory instead
of duplicating it across repositories.

Local verification on Apple Silicon macOS:

- `cargo test --workspace`: **1,179 passed, 5 ignored**. The five pre-existing
  opt-in real-uv runtime tests remain ignored by this ordinary suite; no new
  format-policy test is ignored. Loopback-based integration tests were run
  outside the sandbox so their local Registry listeners could bind.
- The payload classifier suite now has **17 passing tests**. It enumerates the
  shared text/basename/prefix and binary-alias data, with separate independent
  dotenv/metadata, native-signature, Unicode/short-read and executable/state
  regressions. All binary aliases are checked for matching/truncated/wrong
  signatures. Fatal mismatches are found in either member order and preserve
  a prior archive. A historical GIF-as-PNG archive still extracts through the
  actual install extraction path without reclassification or byte changes.
- The shared SDK artifact/policy suite has **4 passing tests**, including
  malformed inventory, duplicate/conflicting/overlapping registrations,
  invalid or contradictory signature bytes/offsets/header bounds, exhaustive
  data matching and independent PNG/JPEG/WebP/PDF/WOFF2 signature fixtures.
  WebP requires both signatures. Invalid embedded inventory fails closed.
- `cargo test -p agentpm-cli --test payload_compatibility`: **6 passed**.
  Real CLI dry-run/live mock publication carries structured warning/target data
  separately from publisher telemetry; `any` carries an empty diagnostic array.
  Quiet/non-TTY/strict behavior, stable warning order and deduplication pass.
  GIF-as-PNG republication fails before any init/upload in both modes and both
  member orders, retains authored state and the old archive, cleans up the
  temporary candidate, and provides rename/replace/reference-update guidance.
  Renaming to `.gif` clears mismatch failure while retaining unknown-format
  narrowing. Non-Python packaging remains unaffected.
- `cargo test -p agentpm-cli --test python_resolution`: **10 passed**, including
  dependency-bearing metadata, lock replay, staleness and failure boundaries.
- `python3 scripts/check_payload_formats.py --check` passed.
  `python3 -m unittest discover -s scripts -p test_payload_formats.py -v`:
  **2 passed**. Mutations of documented suffixes/basenames/signatures, inventory
  changes without regeneration, and malformed generated regions fail the check.
  Normal CI runs the checker and tests. No second exhaustive runtime/test list
  is maintained.
- `cargo clippy --workspace --all-targets -- -D warnings`, Rust formatting,
  Black/isort checks, Flake8, diff whitespace checks and applicable documentation
  pre-commit hooks passed. Both affected product MDX documents compile with
  the web renderer's MDX/GFM tooling.
- Fresh isolated copies of `validate-order` and prepared `summarize-text` pass
  dry runs with `any`, no narrowing warnings, matching-lock reuse despite a
  deliberately missing uv override, and unchanged authored manifest/lock bytes.
  No example source/version changes or Registry publication were performed.

Failure handling is typed: `PayloadBuildFailure` retains ordered diagnostics
with error/warning severity, code, relative path, message/help and relevant
format/target fields. Validation occurs on the complete temporary gzip archive
before atomic replacement. Successful descriptors carry `diagnostics` in the
publish request's `artifact` metadata. No publish machine flag, fan-in,
release-manifest field or Registry persistence rule was added.

Remaining gates and limitations:

- Fresh hosted execution on both macOS architectures, Windows and Linux is
  still pending. The existing Resolver Contract matrix now includes all SDK
  artifact/policy tests and the expanded classifier/CLI suites, with path
  triggers for the policy data and its consumers. Previous hosted M11C/initial
  M12 runs do not satisfy this follow-up's gate.
- Independent review and operator manual acceptance of the follow-up remain
  pending. No hosted execution or live publication is claimed here.
- At the format-only phase, directory exclusions were still deferred. Use the
  separate collection implementation/evidence for the later omission rules;
  this older result does not establish their acceptance. Surviving unfamiliar
  files such as `.gitignore` can still narrow the payload under the minimal list.
- Binary recognition is bounded signature/header agreement, not complete
  format validation or malware scanning. `any` does not prove arbitrary source
  behavior or provision additional runtimes. Unrecognized formats retain the
  current build target. Existing Registry storage/installation semantics remain
  unchanged; M16 machine output/fan-in must reuse the typed diagnostics later.

#### Milestone 12 Initial Implementation Evidence (2026-10-24, America/Denver)

Historical baseline: this evidence describes the initial classifier before the
format-policy follow-up above. Use the separate fresh follow-up evidence for
the expanded policy; these original results do not establish its acceptance.

Scope: package-time Python Tool payload classification only. There are no new
Registry tables, artifact-selection rules, release formats, manifest fields,
lockfile fields or CLI subcommands. Existing published releases remain legacy
with unknown compatibility; no read/install path supplies a default `any`.

Implemented behavior:

- The classifier streams the completed tar archive, so unshipped workspace files
  and filtered build outputs cannot influence its result. UTF-8 validation
  covers entire source/data files with bounded memory, including chunk-boundary
  Unicode and binary suffixes.
- `any` requires regular files in the explicit source/text-data allowlist, valid
  UTF-8 without binary controls/native signatures, and no known environment or
  installed-package paths. Executable Python source can remain portable;
  unknown executables, binary/unknown formats, symlinks, native libraries,
  compiled Python and installed environment snapshots use `Target::current()`.
  This describes payload files, not OS behavior of arbitrary source code.
- Native detection covers ELF, PE/DOS, Mach-O and universal Mach-O, static
  archives and WebAssembly, including content disguised as Python source.
  Native suffixes include `.so`/versioned `.so`, `.dylib`, `.pyd`, `.dll`,
  `.exe`, `.node`, `.a`/`.o` and bytecode. Unpacked vendored native files are
  inspected like all other collected files. Pure `_vendor` source can be `any`.
- Dependency declarations and generated `agentpm-python.json` do not make
  source payloads target-specific. Embedded wheels remain blocked. Missing
  declared paths still fail atomically rather than replacing a valid archive.
- `ToolPayloadDescriptor` in the shared Rust SDK serializes canonical `target`
  plus `format: "tar.gz"`; neither field silently defaults on legacy input.
  Publish sends this under the request's `artifact` field, separate from
  publisher `client.os` / `client.arch`. Dry runs display the same result.
  There is no descriptor sidecar or new archive member. The current Registry
  ignores this additive field; persistence/enforcement belong to later work.

Verification on `aarch64-apple-darwin`:

- `cargo test -p agentpm-cli --bin agentpm commands::publish::payload`:
  **11 passed**, covering real collected archives, native suffixes/signatures,
  environment/vendor content, unknown executables, short reads, Unicode,
  executable Python, links, missing files and blocked wheels.
- `cargo test -p agentpm-cli --test payload_compatibility`: **4 passed**.
  The shipping CLI performs pure/native dry runs and full init/upload/finalize
  exchanges with a local mock Registry. Tests assert descriptors, unchanged
  publisher telemetry, unchanged authored manifest/lock bytes, missing-path and
  wheel failures, and unchanged non-Python Tool packaging.
- `cargo test -p agentpm-cli --test python_resolution`: **10 passed**,
  including dependency-bearing publication carrying `any` with the same
  prepared resolution, reservation and embedded dependency metadata.
- Shared SDK target/descriptor tests round-trip all four canonical concrete
  targets plus `any`, and reject missing/unknown target or format values.
- `cargo test --workspace`: **1,168 passed, 5 ignored**. The five existing
  opt-in real-uv environment tests were not rerun for this packaging-only change;
  no new M12 test is ignored. Local HTTP tests were run outside the sandbox,
  which otherwise prohibits binding their loopback listeners.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
  `cargo fmt --all` and formatter verification passed. Documentation pre-commit
  whitespace, EOF and conflict checks passed.
- Isolated copies of the prepared `validate-order` and `summarize-text` examples
  both passed real CLI dry runs with `Tool payload: any (format: tar.gz)`.
  An intentionally missing `AGENTPM_UV_PATH` confirmed matching locks were
  reused; authored manifest and lock bytes remained unchanged. No Registry
  publication or example source/version change was performed.

Remaining acceptance gates:

- The existing four-target Resolver Contract matrix now runs target/descriptor
  tests, classifier/archive tests and real CLI payload tests on both macOS
  architectures, Linux and Windows. Relevant path triggers were added. These
  updated hosted jobs have **not** executed from this local checkout; their
  previous M11C successes are not evidence for M12. Keep the platform checkbox
  open until the new jobs pass.
- Operator manual confirmation remains pending: source/dependency-bearing
  dry run reports `any`, a bundled native fixture reports only the current
  target, and wheel/missing-file cases fail without changing authored state.
- Conservative false negatives are intentional: unfamiliar formats, even
  portable binary data, receive the host target. Classification does not
  cross-compile or verify a foreign native binary's actual ABI. Legacy storage
  and installation continue unchanged; target-aware enforcement is later scope.

### M13A release/artifact foundation — local verification

Implementation completed locally on 2026-10-25. This is the additive data and
storage foundation only; the multi-artifact publish transaction remains M13B,
signing/provenance remains M14, target-aware installation remains M15, and the
new build/CI publishing workflow remains M16. Node/Python SDKs remain unchanged.

Implemented/verified:

- Typed Rust and Pydantic `agentpm.package.release.v1` models, JSON Schema and
  shared checked-in vectors. Canonical release arrays sort by target, duplicate
  targets fail, digest fields require full lowercase `sha256:` values, optional
  Python resolution digests are omitted rather than null, and private upload/
  machine fields are rejected. Vectors cover `any`, all four concrete targets,
  a bound Python resolution and non-ASCII manifest content. Rust uses existing
  `serde_jcs`; the Registry adds pinned `rfc8785==0.1.4`. Manifest integer-domain
  checks agree across languages; legacy signing serialization is unchanged.
- The authoritative schema/vectors live under the Rust SDK's `contracts/`;
  Registry snapshots are byte-identical. Both language suites assert the same
  expected canonical bytes and digests. `check_release_contract.py` succeeds
  across the two checkouts and fails on a deliberately changed snapshot.
  Registry tests compare sibling files when available. The initial check was
  manual-only; the review follow-up below adds a dedicated Registry CI job
  that checks out both repositories and executes the check automatically.
- The three fixed SQLAlchemy models/tables, nullable legacy scalar columns,
  explicit release format/digest/document, per-release target uniqueness and
  reservation metadata are implemented. The new format stores null scalar
  artifact fields and normalized child artifacts; legacy rows preserve their
  previous scalar authority. No existing row is reinterpreted or backfilled.
- Internal persistence constructs canonical `release.json`, binds manifest/
  resolution/identity, and creates normalized version/artifact rows. S3 writes
  are conditional create-only; exact-byte retries succeed, different existing
  bytes fail, and authorization errors are not treated as missing objects.
  These tests use a mocked S3 client, not live writes. Payload-byte verification
  and cross-store finalize/recovery orchestration remain later work.
- New object keys use bucket-root `packages/<namespace>/<name>/<version>/`,
  target directories and full artifact digests. Legacy `S3_PREFIX`/keys remain
  unchanged. UUID-based staging stays under `uploads/`. Expired reservations
  may share a deterministic final key with a later reservation; their temporary
  keys remain distinct. This avoids making retained reservation history block a
  retry. This is a model invariant, not implementation of the M13B retry API.
- An internal CLI preparation helper creates a separate, atomic new-format
  archive containing `agentpm-artifact.json` target/format metadata while
  preserving the original archive. Existing `agent.json` and optional
  `agentpm-python.json` are retained. No descriptor contains its own archive
  digest. Opt-in Registry validation binds archive identity, manifest,
  Python resolution and target descriptor to the expected release. Existing
  publish commands do not invoke this helper or create a new-format release.
- Install resolve and init reject any explicit unsupported release format with
  HTTP 409/`unsupported_release_format`, even if legacy scalar fields happen
  to be present. No install session or S3 URL is created. Legacy security reads
  reject new-format records rather than reporting a null/artifact-mislabeled
  digest. Existing legacy publish/install/metadata regressions remain passing.

Migration recovery and database evidence:

- The operator's local backend applied V1.98 while it was being developed and
  then correctly refused a changed checksum. The exact applied V1.98 was
  recovered at **EAF4B273** and frozen. V1.99 (**8FB2F27D**) carries function/
  inventory hardening; the operator confirmed the backend starts successfully
  after that recovery. No migration-history row/checksum was edited.
- The final reservation-key correction is a forward V1.100 migration, not an
  edit to V1.98/V1.99. Its operator startup/deployment confirmation is separate
  from the earlier V1.99 restart confirmation.
- **Seven** real PostgreSQL tests apply the actual V1.98 → V1.99 → V1.100 SQL to
  representative pre-migration tables and legacy data in fresh random schemas
  of a disposable localhost-only PostgreSQL 16 container. They assert preserved
  legacy values, explicit storage-shape constraints, four target reservations,
  duplicate/pending-reservation conflicts, one- and four-artifact ORM persistence,
  exact immutable document bytes, immutable published metadata/associations,
  yank updates, rejection of a pending-state bypass, incomplete-inventory failure
  at commit and expired-reservation final-key reuse. SQLAlchemy never generates
  schema DDL. These are not a full historical-migration/deployment rehearsal.
- Crossing V1.100 exposed Pyway 0.3.32's two-digit/string sorting limit. The
  app now delegates to the same Pyway CLI through `run_migrations.py`, changing
  only numeric ordering; stored version strings, filenames, checksum validation
  and SQL execution remain unchanged. A real CLI subprocess applies the three
  migrations to a fresh representative schema and verifies all three Pyway
  history records. Three additional tests cover numeric ordering of actual
  repository migration names/dict records, app invocation/password redaction,
  and the unchanged CLI help interface. No history rewrite or dependency
  upgrade is used to hide the ordering boundary.

Verification totals and tooling:

- Rust workspace: **1,200 passed**, **5 existing opt-in tests ignored**;
  includes four new Rust SDK release tests and one new-format archive test.
  Workspace Clippy `-D warnings` and Rust formatting pass.
- Full API suite: **519 passed**, **7 existing optional tests skipped**, with
  **165 subtests passed**. The final PostgreSQL suite separately passed all
  **7** cases using the complete three-migration sequence. Earlier final
  focused contract/upgrade verification passed **31** cases before adding the
  integer-domain and expired-reservation regressions; those additions are
  included in the full final suite. No new M13A DB case was skipped in this
  explicitly configured run.
- Required API Black, isort, Flake8, Pylint, Bandit, pyupgrade and whitespace/
  conflict hooks run. The cached Bandit environment's missing `pbr` dependency
  was repaired without changing hook configuration. The full mypy hook still
  reports two existing errors in unchanged `app/tools/application/search.py`
  at lines 1486 and 1856. Focused mypy with silent dependency following passes
  the eleven touched implementation/test/entrypoint modules; the full hook is not claimed
  green. Required formatting, shared-contract/payload checks, workflow YAML
  validation and compilation of the changed public MDX pass.
- Resolver Contract path triggers/test steps now include the release models,
  shared fixtures and new-format archive helper on all four native runners.
  The operator confirmed all four updated hosted target runs passed: macOS ARM,
  macOS Intel, Linux x86_64 and Windows x86_64. This is operator-reported hosted
  evidence, separate from the local verification totals above.

Independent review, normal CI, all four updated Resolver Contract runs and the
operator manual walkthrough are complete, as confirmed by the operator.
M13A acceptance is complete; the operator confirmed all manual tests passed
on 2026-10-25.
Hosted cross-repo parity and environment-specific deployment/storage-policy
checks remain Release Band 9 gates, as recorded in `tasks.md`.
The local migration recovery is operator-confirmed, but there were no live S3
writes, bucket-policy changes, production deployment, new public publish/build
workflow, source commits or pushes by the agent. The existing Node/Python SDK
compatibility gap stays deferred to M17.

#### M13A review follow-up: deferred child inventory checks

Review demonstrated that a committed pending version could gain, change or
lose artifacts without revalidating its manifest: the original deferred check
ran only on parent writes. V1.101 adds an artifact-table constraint trigger for
INSERT/UPDATE/DELETE, sharing one inventory-validation function with the parent
trigger. Reparenting checks both old and new parent identities; existing parent
row locking and published immutability remain intact. Checks remain deferred
and validate the final transaction state, so valid assembly/repair transactions
are allowed.

The SQL was first exercised as a draft outside the backend-watched migration
directory, then added as a forward migration. V1.98/V1.99/V1.100 retain their
original checksums **EAF4B273**, **8FB2F27D** and **3484543A**. No migration
history or existing inconsistent draft row was rewritten. Existing bad drafts
must be made consistent before later mutation/publication can commit.

Six additional real-PostgreSQL regression cases cover inconsistent pending
inserts, updates and deletes; a valid final state after a transient mismatch;
and moves that invalidate only the old or only the new parent. DML itself
succeeds before the invalid transaction is rejected at commit, confirming
deferral rather than an accidental immediate constraint failure. The full
13-case PostgreSQL suite and three Pyway ordering/invocation tests pass with
V1.98 → V1.99 → V1.100 → V1.101, including a real CLI upgrade/history test.
Full API regression suite: **525 passed**, **7 existing optional tests skipped**,
with **165 subtests passed**. Formatting and API static checks are run on the
changed tests; the pre-existing full-hook mypy failures in unchanged search
code remain separately documented above. No Rust or public API workflow
change is part of this follow-up.

#### M13A review follow-up: automatic cross-repo contract parity

Added `agentpm-api/.github/workflows/release-contract.yml`. It runs on Registry
pull requests and pushes to `main`, checking out the Registry and public
`agentpm-dev/cli` `main` side by side. The job invokes the existing
`check_release_contract.py --registry-root registry`, then runs the Registry
release-contract tests against those shared schema/vectors. The checkout layout
also enables the tests' sibling-source comparison rather than silently taking
their standalone fallback. Contents permissions are read-only, checkout
credentials are not persisted, and no cross-repo secret, live database or
service key is needed. Both checked revision hashes appear in the job log.

Coordinated contract changes merge to CLI `main` first; no CLI publication or
deployment is required for that ordering. Manual workflow dispatch accepts a
public CLI branch/tag/commit for pre-merge verification of both proposed changes.
That diagnostic run does not replace the required main-ref parity check for the
Registry revision being released. The operator confirmed that the CLI changes
will reach `main` only after M13B, as part of Release Band 9. The hosted main-ref
check is therefore tracked under that band's pre-Registry-deployment gates,
not as an M13A completion or M13B start prerequisite. Local cross-checkout parity
and contract tests provide the M13A evidence while the hosted gate stays open.

Local verification: the CI parity command succeeds and rejects deliberate
schema and fixture drift independently. All **27** Registry contract tests pass
with a cleared environment and developer dotenv loading disabled, using only
the workflow's dummy test signing key. Workflow YAML, checkout paths, read-only
permissions and command wiring were checked; YAML/docs formatting and diff
checks pass. GitHub-hosted execution remains pending after these files are
pushed and the corresponding authoritative CLI files are available at the
selected ref. No workflow was dispatched and no commit/push was performed.

#### M13A operator manual runbook

`manual-tests/RELEASE_M13A_MANUAL_TESTS.md` is operator-accepted. It uses
one macOS/zsh terminal, the existing Registry venv, shared contract vectors,
focused primitive tests and a separate localhost-only PostgreSQL container.
It applies the actual four migration files through the Pyway wrapper, checks
a representative legacy row, then records and inspects a four-artifact version
using mock S3. The data setup reuses the existing SQL test baseline; SQLAlchemy
does not generate schema DDL. A deliberate finalized-artifact mutation fails,
and the guide checks existing local API reads and browser navigation separately.

The executable blocks were rehearsed sequentially: **4 Rust SDK tests**, **1
Rust artifact-helper test**, **30 Registry contract/migration-order cases**, and
**13 real-PostgreSQL cases** passed. Manual fixture SQL inspection and unchanged
legacy-column comparison passed, canonical document bytes matched, a second
Pyway migration run succeeded without changes, and the expected immutable UPDATE
was rejected without altering the four artifact sizes. Local trending, search
and package-detail API reads returned HTTP 200. The disposable container was
stopped/removed. No application database writes, live S3 operations, real
publication or deployment were performed during this rehearsal.

The rehearsal above verifies the runbook commands. Separately, on 2026-10-25
the operator confirmed all manual tests passed, including the walkthrough's
local browser checks. M13A manual acceptance is complete. Hosted CLI-main parity
and environment-specific deployment/storage readiness remain Release Band 9
gates; this acceptance does not mark those later checks complete.

### Multi-artifact release / publish storage

Automated cases:

- schema tests build their starting state by replaying the **real historical migrations**, not a baseline written alongside the test. M13B's finalize failure was invisible because the hand-written baseline omitted `V1.55`'s unconditional unique index, so the tests ran against a schema that lacked the constraint breaking real databases.
- for any rule rescoped from version level to a narrower level, both scopes are parameterized — the narrower rule allows the expected number of rows, and the legacy scope still allows exactly one.
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
- M13B finalize rejects each missing, pending or failed reservation child even when the proposed release manifest and artifact inventory agree. Verify no published version or finalized artifact rows survive; the M13A database constraints do not enforce reservation completeness or stored-byte verification.
- finalization creates one logical version + N artifact rows atomically.
- legacy `PackageVersion.sha_256/size_bytes/s3_key` rows remain populated/readable.
- new `agentpm.package.release.v1` rows have legacy scalar artifact columns `NULL`, populate the release-level digest field, and use child artifact rows as the only authoritative target-artifact source.
- read paths branch on explicit release format, not on nullable-column heuristics.
- finalized artifact inventory is immutable.
- adding artifact to finalized version rejected.
- expired/failed staged objects can be cleaned up safely.
- M13B enforces release expiry and its chosen reservation-history retention policy; repeated cleanup preserves active reservations and finalized objects, including when an expired reservation shares a deterministic final key with a later reservation.
- a rollback after storing `release.json` leaves the release unpublished despite object presence; identical-byte retry succeeds, different-byte retry fails without overwriting, and the chosen recovery behavior is tested.
- legacy S3/read path remains valid.
- new S3 target object keys include canonical target + full digest.
- upload implementation streams rather than buffers complete artifact in memory (unit/integration evidence as appropriate).
- server-authoritative upload-size limit is honored.
- old client/new release produces clear unsupported-release-format error.
- malware scanning:
  - one scan scheduled per target artifact;
  - **verify by mutation that targets 2..N are actually enqueued.** The current `_enqueue_scan` returns early when any scan row exists for the version, so a test asserting "a scan exists" passes against the old one-per-version behavior; assert the row count equals the artifact count and that each row carries its own artifact key;
  - a new-format release enqueues scans even though `tool_versions.s3_key` is NULL, using the per-artifact key;
  - all-clean state requires appropriate artifact results;
  - a release with one clean and one queued/failed/unknown artifact does not report clean on the package detail surface;
  - infection of any artifact yanks/disables logical version;
  - the legacy single-artifact scan path is unchanged: a legacy publish still enqueues exactly one version-scoped scan row and the poller still resolves it;
  - the scan row's artifact association is nullable, so a row without one remains valid;
  - no SBOM/Grype/pip-audit/OSV column is written. These are unwritten schema today, so assert they stay NULL rather than assuming a prior baseline.
- release reservation outlives an individual presigned PUT window.
- a presigned PUT can be re-issued for a still-pending child artifact without invalidating the release or already-uploaded siblings.
- an expired release reservation produces an actionable, identifiable error rather than an opaque S3 or `409` failure.
- worst-case registry call count for a supported multi-target release stays within the effective publish rate-limit budget, including presign re-issues and one retried matrix job.
- an S3 error other than not-found on the finalize destination-exists check fails finalize instead of being treated as "object absent" and triggering a copy; `AccessDenied` specifically is covered.
- resume state is observable by the client — a resumed reservation is distinguishable from a fresh one, or the flag is absent from the response entirely.

### M13B implementation and local verification

Implemented locally on 2026-10-25. Independent review, updated hosted CI and
operator manual acceptance remain open. The implementation keeps
`RELEASE_PUBLISH_ENABLED=false` by default and accepts Python Tools only on the
separate `/v1/tools/publish/releases/...` endpoint family. Existing publish paths
retain legacy storage/format; shared Rust transport now streams every package
kind. Install and Security new-format gates remain intact. No new signature
statements, public package/build command, target selection, external SDK change
or dependency vulnerability analysis is included.

Implemented and exercised:

- Complete intended inventory is stored on the reservation; init/resume returns
  distinct child upload instructions, observable resume state, authoritative
  limits and reservation/PUT expiries. Child renewal preserves sibling objects.
  The policy is 24-hour reservations, 900-second maximum PUTs capped by remaining
  reservation time, and 30-day history retention after expiry. Expired requests
  fail with HTTP 410/`release_expired` and an actionable retry instruction.
- Namespace authorization, reservation ownership, Python-only kind restrictions,
  size bounds, unique target inventory and manifest/resolution binding are
  enforced. A default-off HTTP test and PAT scope rejection cover the guard.
  A real Flask/SQLAlchemy transaction fixture exercises failed finalize rollback,
  successful finalization and idempotent committed receipt recovery.
- Finalize validates every reserved upload before creating any version rows.
  Bounded disk snapshots are independently hashed, checked for size, and validated
  against the expected archive manifest, Python resolution and target descriptor.
  Snapshot bytes are used for conditional final-object writes so staging cannot
  change between verification and publication. Final bytes are checked again.
  Publisher-controlled S3 metadata is insufficient evidence. This supplies the
  byte checking required by this transaction; release signing/attestation remains
  later work, not an implicit implementation of the rest of M14B.
- Missing/corrupt/wrong-size/descriptor-mismatched uploads and access-denied final
  storage fail without surviving version/artifact/scan rows. Exact orphan
  `release.json` retries recover after DB rollback; different existing bytes fail
  with `release_document_conflict`, remain untouched, and require the original
  inventory or a new version. Object presence is not publication authority.
- One and four-artifact versions finalize with null legacy scalar columns and
  one immutable logical version. A dependency-bearing Tool also finalizes as a
  single `any` artifact with its exact resolution bound to the release. Each
  artifact has its own scan row, nullable
  artifact association and object key. The four-target case asserts both row
  count and exact associations. Injecting the old one-scan-per-version guard
  **causes that case to fail as expected**, proving targets 2..N are covered.
- The existing GuardDuty worker yanks the whole version on an infected artifact.
  Rollup stays unknown/non-clean with queued, running, failed or missing siblings;
  all required scans must be completed clean. Legacy scan queueing remains one
  row with a null artifact association. SBOM/Grype/pip-audit/OSV fields stay NULL.
  The new-format Security compatibility gate remains until later verification;
  the reusable rollup preserves existing DTO field shapes.
- Five-minute scheduled cleanup deletes only validated reservation-owned staging
  keys, rotates by updated-at to avoid retained-history starvation, expires
  pending reservations and removes history after retention. Active reservations
  and immutable final keys survive. Storage errors roll back DB cleanup for
  retry; repeated cleanup remains safe. Package-before-reservation lock ordering
  and rereading under lock avoid stale cleanup decisions.
- Rust uploads stream bounded chunks, reopen the file for retries and keep PAT
  authorization off storage requests. A sparse/large-file fixture verifies that
  server policy rejects oversize before transfer. The transfer client has no
  fixed total 600-second ceiling: connect/read and progress-idle checks are
  separate from the Registry API client. Legacy `resumed` is deserialized and
  surfaced in the publication receipt note. The independent CLI 3 GiB cap is
  removed; both legacy and guarded init/finalize enforce the same server policy.
- Dedicated release endpoint rate limits are cost 1, 60/minute and 600/hour.
  Four job init/resumes + four renewals + two finalize attempts use 10 Registry
  requests, or 20 with a full repeat; the schema maximum fits within 24.
  Legacy limits are preserved. Legacy AccessDenied during final-object HEAD
  now fails explicitly instead of being treated as object absence; a regression
  asserts that no copy or version write follows that failure.

Forward Pyway migration V1.102 adds the intended reservation document and nullable
scan-to-artifact FK with a partial unique index. Earlier migration files remain
unchanged. The real upgrade test now runs V1.98 → V1.99 → V1.100 → V1.101 → V1.102
and confirms five history records, legacy preservation and numerical ordering.
The migration was exercised as a draft against a disposable database before
being placed in the backend-watched migration directory. No database-history
checksum was changed, and SQLAlchemy never generated schema DDL.

Verification:

- Full Rust workspace: **1,202 passed**, **5 existing opt-in tests ignored**.
  Final two streaming HTTP tests pass after the idle-period adjustment and
  Windows-sized fixture correction. Workspace Clippy `-D warnings` and Rust
  formatting pass.
- Full API suite: **547 passed**, **7 existing optional tests skipped**,
  **165 subtests passed**. Final targeted release/migration/order verification:
  **38 cases**, all covered by the full passing suite, with no skipped PostgreSQL case. Tests use an explicitly supplied
  disposable localhost database and in-memory storage, including real HTTP
  route/database transaction boundaries. No application database or live S3
  writes were used for verification.
- API Black, isort, Flake8, Pylint, Bandit, pyupgrade, whitespace/end-of-file and
  merge-conflict hooks pass. Full-hook mypy still reports the two existing errors
  in unchanged `app/tools/application/search.py` at 1486 and 1856; it is not
  claimed green. Updated workflow YAML, contract parity, docs formatting and MDX
  compilation are checked separately.
- The native Resolver Contract workflow now includes SDK transport paths/tests.
  The API workflow adds a separate PostgreSQL release-upload/migration job, which
  does not require public CLI `main` to run. The original cross-repo parity job
  still requires the later CLI merge. No hosted execution is inferred from these
  local results.

Remaining gates and risks: independent review, hosted normal/native/API jobs and
operator manual acceptance. Release Band 9 additionally retains CLI-main parity,
deployment rehearsal and real dev/prod storage/GuardDuty coverage checks. The
new protocol must stay gated until verified target-aware consumption exists.
The subsequent bounded-finalization follow-up below supersedes the original
sum-of-artifacts snapshot disk requirement; server concurrency/request capacity
still requires verification.
Immutable orphan final objects are retained; exact retries recover, while changed
release documents intentionally require the original inventory or a new version.
Real storage permissions/lifecycle, managed-service timeouts and actual GuardDuty
integration are not established by mocked storage tests. No commit, push,
deployment, public activation or paid service call is part of this implementation.

#### M13B review follow-up: protected storage prefix and precise scan rollup

Implemented the operator-approved spec additions before independent review.
Release documents and final artifact objects now use
`<S3_PREFIX>/<namespace>/<name>/<version>/`, with `release.json` beside the
`artifacts/<target>/<full-digest>.tar.gz` subtree. This supersedes the initial
M13A bucket-root `packages/` layout recorded above. A shared prefix helper is
used by legacy publication, managed resolver downloads and the release key
builders. Missing/blank settings default to `tools`; surrounding slashes are
normalized and explicit nonempty custom prefixes are honored. Historical object
keys/objects are not rewritten. Staging remains under `uploads/`, and final
objects still use conditional create-only PUTs rather than a staging copy.

Tests cover default, missing, blank, slash-normalized and custom prefix values;
legacy tarball, release document and target artifact keys remain distinct under
one namespace/package/version directory. Actual storage-format XOR constraints
and refusal to replace a legacy version remain covered by the PostgreSQL tests.
One- and four-artifact release finalization is exercised under both the default
and custom configured prefixes, checking persisted keys and scan associations.
Resolver tests verify that the same prefix normalization reaches presigned keys.

The shared domain rollup now follows the revised precedence exactly. Malware
results are count-based against expected artifact IDs: infected, suspicious,
all expected rows clean, otherwise unknown. Progress is independently failed if
any failed, complete only if all expected scans complete, running if any running,
otherwise queued. New-format records never roll up to null progress, including
when no scans were enqueued. Aggregate notes are omitted; raw artifact notes are
not concatenated into UI text. Legacy one-row results/notes and historical
legacy no-scan behavior are preserved. No new per-artifact DTO array or new-format
Security/install activation is introduced; those remain later scope.

The existing missing-tag timeout now explicitly logs a warning with scan ID,
version ID, bucket and object key, pointing operators at prefix coverage,
permissions and scanner health. A real-PostgreSQL regression runs queued →
running → missing-tag wait → failed/unknown, asserts the timeout note and warning,
and confirms completed timeout rows do not emit duplicate warnings on later polls.
This reuses the existing timeout, not a new alerting service.

Verification: full API suite **577 passed**, **7 existing optional tests skipped**,
**165 subtests passed**, with all new database cases enabled against a disposable
localhost database and mock storage. Focused non-DB verification passed **97
cases** and **11 subtests**. The existing `PackageSecurity.test.tsx` suite passed
all **8** package-kind cases, preserving the flat read DTO/rendering contract
without web source changes. Required API formatting, Flake8, Pylint, Bandit,
pyupgrade and whitespace/conflict hooks pass; full-hook mypy still reports the
two documented errors in unchanged search code. Rust source/contracts are
unchanged by this follow-up, so the earlier Rust workspace evidence is retained.
The API PostgreSQL workflow explicitly includes the new rollup and resolver
regressions. No applied migration was edited, no data/object relocation or live
S3/GuardDuty operation was performed, and the release flow remains default-off.

The operator confirmed that the dev bucket `agentpm-dev-tools` already has
GuardDuty monitoring with the same configuration as production. Use that bucket
for real new-format upload/tag/poller checks during manual acceptance. This is
operator-confirmed configuration context, not evidence that those new objects
have already been scanned or tagged.

Operator gates stay open in Release Band 9: confirm actual scanning/tagging in
dev and production, and size scan volume/cost for the artifact objects, their
bytes and `release.json`. Prefix-aware code tests establish key placement, not
AWS protection-plan coverage. Scanning is asynchronous and does not block publish.
Independent review, updated hosted CI and manual acceptance remain pending.

#### M13B review follow-up: aggregate budget, transaction boundaries and checksums

Implemented review findings 1 and 2 together. `MAX_RELEASE_UPLOAD_BYTES` defaults
to 1,000,000,000 bytes across all artifacts, independently of the existing
per-artifact cap. Init rejects excess totals before reservation/storage and
returns `max_total_bytes`; the Rust response type retains this additive field.
Finalize rechecks both current budgets before transfer and before publication.

The equal 1 GB defaults are deliberate: artifact-first checking gives the more
specific single-file error, and the independently configurable artifact bound
matters if the total is raised. This clarification changes comments and policy
documentation only; limits and validation order are unchanged. Aggregate tuning
must revisit transfer/request/concurrency capacity; transfer-time DB locks and
sum-of-artifacts snapshot disk are no longer part of the implementation.

An initial read/auth/validation captures detached intent, then releases its DB
transaction/connection. Storage verification and create-only writes, including
`release.json`, run outside DB transactions with one temporary snapshot open at
a time. The final package/reservation locks protect only reauthorization,
unchanged intent/inventory, expiry/policy/size revalidation and atomic DB rows.
This supersedes the earlier sum-of-artifacts snapshot approach. Fresh final PUTs
require matching explicit S3 SHA-256 request/response checksums and avoid a second
GET; existing objects and conditional-write race winners are still downloaded
and hashed. ETags are never treated as SHA-256 evidence.

Fifteen added cases cover aggregate rejection before side effects, policy
rechecks, one-snapshot usage, no active DB transaction during every storage
HEAD/GET/PUT, and acquisition of the package lock from another PostgreSQL
connection using `FOR UPDATE NOWAIT` during those calls. They also cover expiry,
permission, inventory and budget changes during transfer, identical concurrent
finalization, absent/mismatched checksum responses, correct/different conditional
PUT winners, and rejection without discarding caller-owned uncommitted work.
Fresh writes assert a single staging GET and the explicit checksum header.

Verification: full API suite **592 passed**, **7 existing optional tests skipped**,
**165 subtests passed**, with database cases enabled on a disposable localhost
PostgreSQL instance and mock storage. Rust upload tests **2 passed**; workspace
Clippy with warnings denied passed. API formatting/security/lint hooks pass
except the same two pre-existing mypy errors in unchanged search code. Hosted
and real-S3 checksum behavior are not claimed by these tests. Per-request disk
is bounded by the largest accepted artifact, at most the smaller size cap;
concurrent finalizers may duplicate bounded work, so live concurrency/disk/request
capacity and GuardDuty gates remain pending alongside review/manual acceptance.

#### M13B review follow-up: scan parent integrity and coverage naming

Forward Pyway migration V1.103 adds a composite foreign key from scan
`(artifact_id, tool_version_id)` to artifact `(artifact_id, package_version_id)`.
A supporting unique constraint permits PostgreSQL to enforce the pair. The
association stays nullable; legacy version-only scans are unchanged. Earlier
migration files are not edited. Existing mismatches fail migration validation
visibly rather than being silently reassigned.

Database regressions reject mismatched inserts and updates of either association
column, prove a matching scan persists, retain null associations, and demonstrate
that applying the actual migration to pre-existing mismatched data fails without
rewriting it. The real Pyway subprocess now applies V1.98 through V1.103 in order
and preserves a pre-existing legacy scan. An additional rollup regression confirms
an unrelated artifact row cannot supply missing coverage.

The rollup's `has_full_coverage` name and comment now distinguish row existence
from scan completion. Result/progress rules are unchanged; the existing synthetic
clean-results/running-progress case remains covered.

Verification: focused migration, ordering, rollup and release-publish tests
**78 passed**; full API suite **598 passed**, **7 existing optional tests skipped**,
**165 subtests passed**, using disposable localhost PostgreSQL and mock storage.
Black, isort, Flake8, Pylint, Bandit, pyupgrade and whitespace/conflict checks pass.
Mypy still reports the same two errors in unchanged search code. No live database,
S3 or hosted CI verification is claimed for this follow-up.

#### M13B current acceptance status after review

The operator confirmed independent review is complete, including the follow-up
fixes above. This closes the review gate and supersedes the earlier review-pending
notes. The operator also confirmed updated hosted normal CI, all four Resolver
Contract jobs and the Registry PostgreSQL release-upload job pass. This supersedes
the earlier hosted-verification limitations; results are operator-confirmed.
Release Contract Parity failed as expected against CLI `main` before its contract
changes are merged. Its Release Band 9 gate remains open for the post-merge run.
Operator manual acceptance remains pending, along with the separate band-level
deployment/storage/GuardDuty readiness gates.

#### M13B operator runbook ready; acceptance remains open

[`manual-tests/RELEASE_M13B_MANUAL_TESTS.md`](manual-tests/RELEASE_M13B_MANUAL_TESTS.md)
and its checked-in `fixtures/m13b/fixture.py` helper now provide one-terminal
setup and explicit local-Registry/dev-S3 steps for portable, four-artifact and
dependency-bearing releases, partial/corrupt upload recovery, renewed PUTs,
receipt reuse, size guidance, immutable inventory, legacy publication/install,
unsupported new-format consumption, real GuardDuty tags and scheduled cleanup.
The helper generates read-only SQL scoped to reservation IDs; it never changes
development DB rows or scan tags. Expiry, retained history, rollback/orphan,
concurrency, parent-integrity and infected/missing-tag cases use the existing
78-case disposable-PostgreSQL suite; the two Rust HTTP cases cover transfer
retry/reopen and pre-transfer size rejection.

Runbook verification used actual CLI lint and dry-run for all four generated
manifests, then validated all six target archives and their canonical documents.
The local dependency check reused a matching real prepared example resolution
in its disposable author lock; the operator guide instead explicitly performs
fresh resolution. The helper's init/resume/upload/finalize/error/renewal/storage
and cleanup steps were exercised against the actual Flask release routes and
forward-migrated disposable PostgreSQL, with auth and S3 replaced by fixtures.
HTML read-gate responses are handled alongside JSON publish errors. This is
fixture verification, not real HTTP authentication or AWS/GuardDuty acceptance;
no development/production publication or external storage request was executed.

Manual acceptance remains open until the operator completes and confirms the
walkthrough. Release Band 9 main-ref parity, production coverage, deployment
permissions/retention and request/concurrency/cost sizing remain separate gates.

#### M13B manual discovery: historical scan-index conflict

The operator's real four-artifact finalize failed with the route's generic
409/`release_conflict`. V1.55's unconditional unique index
`uniq_tool_version_scan_one` still limited every version to one scan, including
artifact-linked rows. The representative test baseline had recreated the scan
table without that historical index, masking the deployment failure.

The baseline now applies the actual V1.54, V1.55 and V1.56 scan SQL. Before the
fix, the existing one/four-artifact regression reproduced the error: **2 passed,
2 failed**, with duplicate-key violations on `uniq_tool_version_scan_one` in
both four-target cases. Forward migration V1.104 replaces that unconditional
index with a partial unique index on version IDs only where `artifact_id IS NULL`.
V1.102's per-artifact unique index remains authoritative for artifact-linked
scans; null associations and the composite parent FK remain unchanged. Earlier
applied SQL files are unchanged.

The real HTTP/DB regression now creates the old index deliberately, attempts
four-artifact finalize, checks the constraint name is logged and all version/
artifact/scan writes roll back, then removes the reproduced old index and
retries the same reservation and existing objects successfully. A repeated
finalize retains exactly four artifacts/scans. Direct SQL regressions verify
both duplicate legacy/version-only and duplicate artifact scans remain rejected.
The actual Pyway subprocess upgrade now runs V1.98 through V1.104 and preserves
the existing legacy scan. Integrity logging includes only operation, constraint
name and SQLSTATE, preserving the public response without dumping SQL parameters.

Verification: full API suite **600 passed**, **7 existing optional tests skipped**,
**165 subtests passed**, with the corrected historical schema and forward SQL in
disposable localhost PostgreSQL. Formatting, lint/security checks pass apart from
the same two known mypy errors in unchanged search code. The manual runbook now
requires V1.104 and its controlled set contains **80 cases**. Real dev recovery,
independent review and Registry PostgreSQL CI for this follow-up remain pending;
earlier acceptance evidence applies to the preceding revision. No application
database or live S3 object was altered by this verification.

#### M13B operator acceptance after the scan-index fix

The operator confirmed all manual runbook steps now pass and that Claude reviewed
and signed off on the V1.104 fix. This closes manual acceptance and the follow-up
independent-review gate, including real development recovery of the previously
blocked four-artifact release. The walkthrough's development upload, GuardDuty
tag/scan and staging-cleanup checks are operator-confirmed; this supersedes the
earlier manual/review-pending notes above.

The operator subsequently confirmed the Registry PostgreSQL CI job passed with
V1.104. Together with the confirmed normal/native CI, independent review and
manual walkthrough, this completes M13B acceptance and supersedes the earlier
post-fix CI-pending notes. Rust/native source is unchanged by the follow-up.
Release Band 9's post-merge CLI-main parity,
production/deployment storage permissions and retention, production GuardDuty
coverage, capacity and scan-cost checks remain separate and unclosed. Development
manual acceptance does not establish those production readiness results.

#### Release Band 9 version preparation — 2026-10-26

Prepared CLI **0.1.38** (`v0.1.38`) in its crate manifest and Cargo lockfile;
the website status card now advertises that version with release date
**2026-10-26**. Local offline/locked compilation and execution report
`agentpm 0.1.38`. Crate/lockfile/status markers agree; Cargo formatting,
web TypeScript checking, status-page ESLint/Prettier and diff whitespace checks
pass. This metadata-only preparation adds no runtime behavior or new tests.

The operator will push and publish/deploy **CLI → Registry → Web**. CLI-main
contract parity must pass before Registry deployment, and Render must retain
`RELEASE_PUBLISH_ENABLED=false`. Preparation does not claim a tag, publication,
hosted post-bump CI run or production deployment. Existing SDK versions,
historical release records and the 0.1.36 APDS tolerance floor are unchanged.

#### Release Band 9 hosted parity after CLI-main merge

The operator confirmed CLI changes reached `main` but CLI deployment/publication
has not occurred, and both Registry jobs pass after rerunning them. GitHub run
[40258294740](https://github.com/agentpm-dev/registry/actions/runs/40258294740)
and its decoded job logs independently confirm:

- Authoritative CLI checkout used `main`, revision
  `1b321f09863fad7ed3f38872fb4952be6efcd806`.
- Checked Registry PR merge revision:
  `9e0f5324576a8f64520d0dd646d75f8cde0e4f6b`, merging source head
  `d5991535e895f8ff2c31b09cbfc615e8f7051625`.
- Release contract parity job `121929230136` succeeded: schema/shared vectors
  agree and **34 tests passed**.
- PostgreSQL upload/migration job `121929230564` succeeded: **210 tests passed**
  and **8 subtests passed**.

This closes the CLI-main parity gate for the tested Registry candidate and
supersedes earlier expected-failure/pending notes. Preserve this revision linkage
when shipping; later Registry changes need their corresponding checks. The
deployment sequence remains CLI **0.1.38** → Registry → Web, with the release
publish gate off. CLI release publication, Registry/Web deployment and the
remaining production readiness/smoke checks are not inferred from CI success.

#### Release Band 9 read-only storage inspection prepared

The pre-deployment inspection is documented in
[`manual-tests/RELEASE_BAND9_READINESS.md`](manual-tests/RELEASE_BAND9_READINESS.md),
including hidden prompts for reloading production credentials in a new terminal.
The checked-in inspection helper reads AWS identity, S3 configuration and
GuardDuty plan scope/status/tagging; it performs no object or policy mutation.
It saves a private local JSON report, excludes credentials and labels denied
checks unverified. Runtime Registry permissions and real scan delivery remain
explicitly unverified by configuration inspection alone.

Offline boto3/botocore fixtures verify the API calls, GuardDuty pagination and
bucket filtering, parsed policy JSON, denied/missing-credential handling, report
permissions and refusal to overwrite existing evidence. Formatting and lint pass.
The agent tool session has no AWS credentials, so actual production configuration
inspection awaits the operator running this command in their credential-bearing
terminal. No live AWS result, deployment or readiness completion is inferred.

#### Release Band 9 production deployment and inspection status

The operator confirmed CLI **0.1.38**, Registry and Web are deployed to production.
This closes the deployment item and supersedes the earlier deployment-pending
notes. Installed-version, deployed status date, Registry migration/legacy-flow
smoke checks and confirmation of the disabled production release gate remain
separate evidence requirements; deployment alone does not close them.

The operator also ran the production storage configuration inspector. Every
reported S3 configuration read returned `AccessDenied` and GuardDuty plan listing
returned `AccessDeniedException`. These results establish that the inspection
principal cannot read those settings; they do not establish whether object
uploads work or whether production protection is correctly configured. No
configuration check is marked passed. Continue with authorized AWS-console
inspection, starting with the production bucket's lifecycle rules. Live tagging,
permissions/retention, capacity and cost readiness remain open.

#### Release Band 9 production lifecycle inspection

The operator supplied the enabled lifecycle rules for production `agentpm-tools`:

- `abort-mpu-7d`, filtered to `tools/`: no current-version expiration or
  transition; noncurrent versions expire after 90 days while the three newest
  noncurrent versions are retained; incomplete multipart uploads abort after
  seven days.
- `abort-mpu-1d`, filtered to `uploads/`: current objects expire after one day;
  noncurrent versions have two-day expiration with no retained newer-version
  count; incomplete multipart uploads abort after one day.

These supplied rules preserve current final artifacts/documents and keep staging
temporary, consistent with the intended layout and 24-hour reservation policy.
This is operator-provided console evidence after API configuration reads were
denied. Confirm no additional enabled all-bucket/overlapping rules shorten final
object retention before closing the full lifecycle inspection. Effective IAM/KMS
permissions, production GuardDuty configuration and real scan delivery remain
unverified; the combined readiness gate stays open.

The operator also supplied the production bucket policy. It denies insecure
transport for the bucket and all objects, denies deletion/object-version deletion
only under `tools/runtimes/uv/*`, and requires an `If-None-Match` header for
resolver object creation under that same subtree. The resolver-specific denies
do not cover `tools/<namespace>/<package>/<version>/release.json`, its target
artifacts or staging `uploads/`; HTTPS requests to those paths are compatible
with the supplied bucket policy. This deny-only policy does not itself grant
the Registry's required object/tagging permissions. No policy change is needed
based on this inspection, and effective identity/KMS access remains separate.

Production GuardDuty console evidence is also operator-confirmed: bucket
`agentpm-tools`, plan `22cd1106e031d5e12cf6`, status **Active**, protected prefix
**`tools/`**, and **Tag scanned objects: Yes**. The operator corrected an initial
`/tools` transcription; the confirmed prefix has no leading slash. The plan uses
role `GuardDutyS3MalwareScanRole-6ed5a57b-4cfe-4777-b7ea-7f4fc8e63b00`.
This configuration covers the intended nested final-artifact/document key layout;
it does not establish actual production tag delivery or the Registry principal's
ability to read those tags. Those live/identity checks remain open.

The operator supplied the `agentpm-s3-writer` policy attached to IAM user
`agentpm-reader-writer` (one direct managed policy, no permissions boundary shown).
For `agentpm-tools`, it allows PutObject/GetObject/AbortMultipartUpload under
`uploads/*` and `tools/*`, DeleteObject only under `uploads/*`, and
GetObjectTagging/GetObjectVersionTagging under `tools/*`. ListBucket is allowed
with `s3:prefix` matching `uploads/*` or `tools/*`. The supplied development
policy is identical except for bucket `agentpm-dev-tools`; the prior completed
dev walkthrough is evidence that this permission shape supports that environment.
No IAM expansion is indicated by the policy comparison alone.

Production effective-access verification remains open: confirm the terminal
principal matches Render's configured identity, check missing-object HEAD
responses (404 versus 403) using random absent keys, and inspect encryption/KMS
if applicable before claiming full object access. The Registry fails closed on
HEAD errors other than not-found in both legacy and guarded finalize. This
preflight is read-only; no production object or policy change is authorized or
claimed by the inspection.

The operator ran the read-only missing-object HEAD preflight using production
credentials. STS identified caller
`arn:aws:iam::510092505185:user/agentpm-reader-writer`, matching the reviewed IAM
user. Both the random `uploads/.../missing.tar.gz` key and the random
`tools/zack/band9-check-.../0.0.0/artifacts/any/<digest>.tar.gz` key returned
**HTTP 404**, not 403. This verifies missing-object detection for the inspected
principal and paths; no ListBucket-policy change is indicated by this test.
No object was created. Existing-object reads, conditional writes, tag reads,
staging deletion, encryption/KMS and real production scan delivery remain
separate checks; this preflight does not establish those permissions.

The operator confirmed both production and development bucket defaults use
**SSE-S3** (Amazon S3 managed keys), with SSE-C blocked. The console also displays
Bucket Key enabled; that setting applies to SSE-KMS and does not change the
selected SSE-S3 encryption mode. New release writes use the bucket default and
need no additional KMS permissions for this configuration. No encryption change
is indicated. Historical object encryption is not inferred from the current
default. Configuration inspection and the missing-object preflight are recorded;
existing-object read/conditional PUT, actual GuardDuty tag delivery/readback and
staging delete still require live production evidence with the publish gate off.

#### Release Band 9 bounded live-probe helper prepared

The readiness guide now supplies explicit `prepare`, `upload`, `check` and
`cleanup` steps for a tiny direct-S3 probe. Its unique UUID owns one staging key
and two final keys under the real `tools/zack/<probe>/0.0.0/` release layout.
The source manifest is linted with the CLI and the canonical release document
validates the checked-in schema. Final payload bytes are bounded to 16 KiB;
the helper checks the previously verified production caller ARN, conditional
PUT responses, SHA-256 and SSE-S3 readback, and real scan-result tags on both
final objects. Only its own staging key can be deleted, after both tags report
clean; the final evidence is retained. No Registry rows, feature flags, bucket
policies or tags are modified by the probe.

Local verification used the actual CLI lint and release schema with fake AWS
clients. It exercises a wrong caller before mutation, partial upload retry,
failed checksum confirmation, repeat conditional-PUT rejection, pending and
clean scan states, cleanup gating, staging-only deletion and retained final
objects. Formatting and lint pass. This prepares the operator's live steps;
no production object write or scan result is claimed before their execution.

#### Release Band 9 production storage probe passed

The operator completed prepare/lint, upload, scan-tag readback and staging cleanup
using the reviewed production principal
`arn:aws:iam::510092505185:user/agentpm-reader-writer`. Probe UUID:
`c8e33c5e-cd5c-4a4e-9edf-0c5b038debc1`. The helper reported:

- Installed CLI **0.1.38** and a successfully linted manifest.
- A **491-byte** archive and **404-byte** canonical release document, with the
  archive also stored temporarily under its UUID-owned `uploads/` key.
- Successful conditional writes, readback, SHA-256 and SSE-S3 verification;
  repeat create-only archive PUT rejected with **412**.
- Both final archive and document tagged **`NO_THREATS_FOUND`** by real
  production GuardDuty and read successfully by the inspected principal.
- Its own staging object deleted, with both immutable final objects retained
  and reverified. No Registry rows, flags or policies were changed.

The local evidence file was independently read and confirms `phase: complete`,
`scans_clean: true` and both clean results:
`/var/folders/rk/2xd4x19d0z59ly99hyx9s5lw0000gn/T/agentpm-band9-probe.UToCEH/state.json`.
Finals remain under
`tools/zack/band9-storage-probe-c8e33c5ecd5c4a4e9edf0c5b038debc1/0.0.0/`;
artifact digest is
`sha256:8a86e88c53ac08c2f66d99e7863b08e7552f7f890b0d083f35624b8fac0b4b97`.
Together with the supplied console configuration and prior dev manual acceptance,
this closes the storage permission/retention/GuardDuty object-delivery check and
supersedes the earlier production-probe-pending notes. It does not establish
Registry scan-row/poller orchestration for a published new-format version.

Direct unauthenticated production HTTPS reads also returned **200** from
`https://api.agentpackagemanager.com/health` (`Healthy`) and
`https://www.agentpackagemanager.com/status`. The visible CLI card reads
`Latest: v0.1.38`, `Release: 1d ago`, consistent with the source date 2026-10-26.
The browser tool could not fetch these endpoints; direct HTTPS requests supplied
the evidence. Deployed migration/disabled-flag and legacy publish/install
confirmation, plus hosted capacity and scan-cost sizing, remain open. No large
or concurrent production load test is inferred from the tiny probe.

#### Release Band 9 operator cost acceptance

The operator confirmed cost is acceptable, closing the scan-volume/cost review
item for this rollout without further analysis. This is an operator acceptance
decision, not a claimed quantitative usage forecast or AWS pricing estimate.
The production storage/GuardDuty probe remains passed; deployment smoke and
hosted finalize capacity are tracked separately and are not inferred from cost
acceptance.

#### Release Band 9 production release gate confirmed

The operator explicitly confirmed `RELEASE_PUBLISH_ENABLED=false` in production,
closing the production flag check. The new-format publish flow remains disabled
as required. Production health/status and storage-probe evidence are already
recorded; migration/legacy publish-install confirmation and hosted finalize
capacity remain separately tracked.

#### Release Band 9 completion

The operator explicitly confirmed deployed Registry startup/migrations through
V1.104 and legacy publish/install. Together with the confirmed production release
gate set to false, CLI 0.1.38, live status/health checks, CLI-main parity, reviewed
storage configuration, successful real dev/production GuardDuty/object probes
and operator cost acceptance, this completes Release Band 9.

Hosted finalize disk/request/concurrency capacity has not been established by
the tiny probe. The implementation's requirement is to verify that capacity
before enabling the new-format publish flow; Band 9 ships it disabled. Its
unchecked task is therefore retained under Release Band 11's production
activation gate, with no requirement removed or result claimed. This supersedes
the earlier notes treating capacity as an open Band 9 completion item. No Band
11 activation, later signing/install implementation or production load test is
included in this acceptance.

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

#### Milestone 14A local implementation evidence

The Rust SDK now owns `canonical.rs` and typed `signing::AuthorSignatureV1`;
the Registry has corresponding `canonical.py` and historical
`legacy_signing.py` helpers. Both consume the byte-identical checked-in
`signing-canonicalization-v1.fixtures.json` snapshots alongside the existing
`package-release-v1.fixtures.json` release vectors. No new signature statement
versions or client verification behavior are implemented here.

Verified locally:

- Shared ASCII, Unicode/control-character keys, UTF-16 key ordering, key/whitespace
  differences and RFC 8785 numeric examples produce the expected bytes/SHA-256.
  The UTF-16/control-key fixture failed with `serde_jcs` 0.1.0 and passes with
  0.2.0; the expected fixture was retained. Existing release/Python fixtures keep
  their digests. Raw manifest floats remain supported; signed-contract floats
  and unsafe integer inputs fail.
- Artifact inventory order normalizes by target; Python requirements, packages
  and hash arrays normalize under the v1 contract without mutating the input.
  Registry upload validation still rejects non-normalized submitted resolutions.
- New timestamps normalize offset/fraction spelling to UTC `Z` while preserving
  nanoseconds; invalid dates, timezone-free input, leap seconds, excess precision
  and out-of-range years fail in both languages. Legacy timestamps are untouched.
- Typed author v1 output matches the old inline JSON for every current package
  kind, Unicode and non-UTC timestamp examples. The shared deterministic legacy
  Ed25519 fixture produces the same signature; Rust and Python verify fixed
  legacy author v1 / Registry v2 signatures and reject changed bytes. A Unicode
  Registry v2 fixture explicitly retains Python's historical ASCII escaping.
  These are fixtures, not a claim of reading a live historical production row.
- Changing identity, manifest/Python resolution digest, artifact target/digest/size
  or inventory membership changes the release digest; existing validation rejects
  unsupported format/content-type/type values. SDK plan → lock → frozen plan
  retains the bare 64-character release hex and adds no selected artifact state.
- Snapshot parity succeeds locally; a mutation test rejects edits/removal of
  each required contract file. CLI CI executes that test; Registry parity CI
  requires the signing snapshot and runs both Python contract suites. Native
  resolver workflow coverage now includes the canonical/signing modules.

Commands/results:

- `cargo test --offline --locked --workspace --quiet`: **1,209 passed, 5 ignored**.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all` and final formatter check: passed.
- Registry `.venv/bin/python -B -m pytest -q --tb=short -x`, using an explicitly
  configured disposable local PostgreSQL 16 database: **631 passed, 7 optional
  skips, 165 subtests passed**. This includes release finalize, actual Pyway
  migration and legacy publish/install regressions. The test container was removed.
- `python3 scripts/check_release_contract.py --registry-root ../agentpm-api`
  and `python3 -m unittest discover -s scripts -p test_release_contract.py -v`: passed.
- Required Registry pre-commit hooks: formatting, isort, Flake8, Pylint, Bandit,
  pyupgrade and file checks pass. Mypy still reports the two pre-existing
  assignment errors in unchanged `app/tools/application/search.py:1486,1856`;
  no new mypy errors were reported in the touched files.

The operator confirmed independent review of 14A is complete with no findings.
The operator subsequently confirmed hosted normal CI, the Registry PostgreSQL
job and all Resolver Contract target jobs passed. This records operator-reported
hosted results. Exact post-merge run URLs/revisions are recorded in the Release
Band 10 acceptance below. Operator manual acceptance is confirmed passed;
deployment acceptance is also recorded below.
The Registry parity run against public CLI
`main` needs the authoritative contract update merged first; dispatch against a
proposed CLI ref can check pre-merge coordination. This main-ref parity check is
an explicit Release Band 10 gate before Registry deployment, not a prerequisite
for 14A manual testing. Record both checked commit IDs. The Registry PostgreSQL
job can run before the CLI merge. Production
`RELEASE_PUBLISH_ENABLED` remains disabled. New release statements, trust
material and target-aware install acceptance belong to later milestones.

Manual runbook: [`manual-tests/SIGNING_M14A_MANUAL_TESTS.md`](manual-tests/SIGNING_M14A_MANUAL_TESTS.md).
All shell blocks passed `zsh -n`. Sections 1–4 were rehearsed from their actual
commands: shared fixture inspection, intentional invalid cases, 11 focused Rust
unit tests and 65 Python cases passed. The checked-in no-dependency Tool passes
the real CLI lint/dry run, contains exactly `agent.json`, `tool.py` and `README.md`
in its archive, and prints the expected JSON when invoked directly. Its Python
source was formatted with Black and passes Flake8. The section 5 security
inspection was rehearsed against synthetic signed responses and rejects missing
author signatures or attestations; that rehearsal made no HTTP request. No real
dev publish, signer registration, install, scan, production write or operator
manual acceptance is claimed by this runbook preparation. Acceptance boxes stay
unchecked until the operator runs the guide.

During the operator's signed dev smoke, publication succeeded for
`@zack/m14a-signing-20261027015908-37555@0.1.0` (package UUID
`4938fce9-3863-43ce-854f-bc3e03ab31e7`, version row 85). The subsequent security
read exposed an incorrect `/v1/tools/.../security` URL in the runbook. Corrected
it to the registered `/tools/.../security` detail route; publication does not
need repeating. Synthetic response rehearsal had not exercised routing. Actual
security verification and consumer install/run acceptance remain pending.

The corrected security read returned no stored author signatures, matching the
operator's empty signature table/missing Signed badge. A read-only check of the
local dev namespace's signer records confirmed that none matched the exported
public key selected for the smoke test. This is the existing optional-signing
behavior for an unregistered author key; the Registry attestation was present.
The guide now supplies the standard dev signer-registration command and requires
a fresh package name after registration, rather than retrying an immutable
already-published version. Archive inspection filters by the current package
name so the earlier attempt's archive cannot interfere. The corrected signed
dev smoke and operator acceptance remain pending; no signer or database row was
changed during diagnosis.

#### Milestone 14A operator acceptance — passed

The operator confirmed all M14A manual tests pass. This includes shared
canonicalization/normalization and release-digest checks, legacy signature
fixtures and focused tests, then a fresh signed development publication after
registering the selected public key in the dev namespace. The operator supplied
the successful real-archive author v1 verification and Registry-attestation
presence output, then confirmed the remaining consumer install/machine-run
checks passed. This closes the earlier pending manual/security-smoke notes;
the first unsigned dev attempt remains historical test evidence.

The saved `dev-security.json` was also inspected locally: successful package
`@zack/m14a-signing-20261027021020-37555@0.1.0`, version row **86**, contains
one author `agentpm.package.signature.v1` statement and
`registry_attested=true`. Evidence is in
`/var/folders/rk/2xd4x19d0z59ly99hyx9s5lw0000gn/T/agentpm-m14a.IsHb8t/evidence/dev-security.json`.

Together with the clean independent review and confirmed normal CI, Registry
PostgreSQL and all native Resolver Contract jobs, M14A is complete. The runbook
acceptance checklist records the operator's confirmation. At M14A acceptance,
Release Band 10 still required Registry parity against CLI `main` and production
deployment; these are now closed by the separately recorded evidence below.
No production feature activation or later signing/verification work is claimed
by this acceptance.

#### Release Band 10 production acceptance — complete

On 2026-10-27, the operator confirmed CLI **0.1.39**, Registry and Web are
deployed to production with all CI jobs passing. Production Registry retains
`RELEASE_PUBLISH_ENABLED=false`; Web status was updated to CLI v0.1.39 and
release date 2026-10-27. Deployment and feature-flag confirmation are
operator-reported, not inferred from the local debug binary or CI logs.

Independently read the Registry's post-merge push workflow
[`40602748466`](https://github.com/agentpm-dev/registry/actions/runs/40602748466),
which completed successfully for Registry `main`:

- Registry revision: `c38bf41a78bc755ce3f75c0e6acc7dd28e32dce3`.
- Authoritative CLI contract revision: `e6c0e42ba120320a7c381a8592f36432d9cb12b4`;
  checkout logs explicitly select public CLI `main`.
- Contract parity job `122534677081`: release schema/signing/shared vector
  snapshots agree; **65 Python contract cases passed**.
- PostgreSQL job `122534677271`: **210 tests and 8 subtests passed**.

This satisfies the main-ref parity gate for the Registry merge revision and
records both checked commit IDs. Earlier PR run `40433571414` also passed, but
the post-merge run above is the release evidence. Together with completed M14A
manual acceptance/review and the operator's production confirmation, Release
Band 10 is complete. No new-format activation, new statement versions or later
client verification is included; M14B is the next milestone.

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

### SDK lockfile and managed Python Tool compatibility (M17)

Implement these cases in both `agentpm-sdk-python` and `agentpm-sdk-node`.
This section plans future work; the current SDK app startup failure remains a
known limitation until M17 is implemented and verified.

- Shared representative fixtures cover supported lockfile v2/v3/v4, installed
  Agents and Skill-transitive Tools, local author roots, additive v4 fields,
  malformed locks and an unsupported future version. Both SDKs consume the
  same fixtures; require byte-identical lock contents before/after load/call.
- Exact Agent **0.1.7** and Tool **0.1.9** remain selected when older installed
  versions coexist. Installed Agent loading works without app `agent.json`;
  a local author root is not exposed as an installed release. Exercise each
  supported composition kind using the finalized M13–16 metadata contracts.
- An offline mock-CLI fixture asserts argv, working directory, input transport,
  project/lock context, exact Tool version, machine-envelope handling and
  returned Tool data for each SDK. Cover a Tool returning `ok: false` separately
  from a failed runner, malformed/incomplete machine output and nonzero exit.
- With the real compatible CLI, install a test Tool whose code imports a
  native dependency, then invoke it through each SDK's public callable/wrapper.
  Confirm the interpreter, imported package paths and versions belong to the
  verified managed environment, not app/global Python. Normal tests use a
  deterministic no-model-cost Tool/provider; a separate operator production
  summarizer smoke check may use credentials only when explicitly authorized.
- Loading/invoking cannot install, re-resolve, rebuild or rewrite portable
  state. Snapshot lock/install/environment state and assert no resolver/index
  requests, provisioning or install finalization occur during SDK load/call.
- Missing/old CLI, missing/not-ready/stale environment, changed interpreter,
  tampered resolution/installed metadata and incompatible target fail closed
  with repair guidance; no ambient-Python or older-Tool fallback occurs.
- Required/entrypoint/caller env layering, paths with spaces, stdin JSON,
  large/malformed output, stderr diagnostics, timeout and cancellation retain
  documented behavior. Ambient `PYTHONPATH` shadow and `PYTHONHOME` do not
  override managed execution. No key/token/passphrase leaks in captured output.
- Legacy/no-dependency Python Tools, vendored native content and Node Tools
  retain their previous runner behavior, including standalone use without a
  CLI where previously supported. Optional adapters remain optional.
- Run Python pytest/format/lint/mypy/package checks and Node test/lint/build/
  declaration/ESM/CJS checks using their repo-native commands. Execute real
  integration cases on supported Linux, macOS and Windows runners; fake target
  strings alone do not establish portability. Record actual matrix evidence
  and any unsupported SDK host explicitly before acceptance.
- Verify Python and Node example consumers actually invoke the Tool through
  their SDK wrapper, not merely through a separate direct CLI command. The
  existing Python devwork app is the recorded regression case.
- Check documented minimum SDK/CLI versions and install SDK distributions into
  clean consumer environments for operator-confirmed release smoke checks.
  Local builds or source overrides alone are not publication evidence.

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
- final job rejects mixed `any`/concrete-target matrix claims before any release
  publication. Use identical recognized text bytes (for example, `assets/example.csv`)
  in otherwise equivalent archives with `0644` and `0755` member modes: the former
  can be `any`, the latter narrows with typed `executable_member` diagnostics.
  Keep manifest/Python-resolution digests equal so the mixed-claim check itself
  causes rejection. Exercise the existing mode-producing packaging path on real
  Unix and Windows hosts as well as deterministic archive-mode fixtures; do not
  claim synthetic fixtures alone prove native platform behavior.
- mixed-claim failure identifies the affected builds and claimed targets, with
  the concrete build's diagnostic path/reason. The `any` build can have no warning;
  report that build without fabricating diagnostics or scraping human stderr.
  Assert no silent artifact selection, omission, relabeling or publication.
  Correcting stray permissions/reconciling claims and rebuilding can clear the
  gate without introducing an override flag.
- otherwise valid distinct concrete targets pass this consistency check; target
  equality is not required. Existing duplicate-target, identity/digest, missing
  build and local single-artifact checks remain in force. This fan-in policy
  does not add a Registry-wide ban on intentional `any` plus concrete inventories.
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
consolidating on an evergreen URL. Milestone 18 should carry this decision forward explicitly.
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
