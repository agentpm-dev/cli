# M8A: Versioned APDS reference and package links

Claude review is complete with no remaining findings. The user confirmed all
hosted CI passed on **2026-11-25**. The user supplied the passing 11-fixture,
60-request, seven-observation audit the same day; saved evidence was independently
rechecked. **M8A is complete.** Production deployment remains separate.

This guide uses your **real local web and API**, with the API launched through
your usual IntelliJ configuration against the **local Docker database**. It
reuses existing M4A/M5A publications and the private **`zack-internal`** namespace.
The helper makes anonymous **GET requests only**. It does not publish, change
namespace visibility, edit history, download S3 artifacts or toggle enforcement.
Ordinary browser page-view analytics may still be recorded by the local app.

All generated files stay under gitignored **`agentpm/web-m8a-test/<run>/`**.
Original fixture runs are read, never changed. No signing keys, tokens or
credentials are copied. A browser index makes the selected releases and expected
results visible; each case links to its Overview, Security and raw manifest.

| Section | What you prove                                                                    |
| ------- | --------------------------------------------------------------------------------- |
| 1–2     | Start the real local services, select existing fixtures, prepare visible inputs   |
| 3       | Check live API/web responses, selected records, APDS projection, SEO and headers  |
| 4       | Readable frozen reference, rule navigation and exact source links                 |
| 5       | Separate manifest/specification links across all eight kinds                      |
| 6       | Same-package version switching and concrete `latest` manifest link                |
| 7       | Declaration, legacy inference, recorded evaluation and timestamps remain distinct |
| 8       | Desktop/narrow-screen layout and keyboard usability                               |
| 9       | Real private-package signed-in success and anonymous denial                       |
| 10      | Audit captured responses and explicit browser observations                        |

## 1. Start your local services

Start or restart the Registry using **your existing IntelliJ API configuration**
with its local Docker DB, Redis and development settings. Use the reviewed API
checkout containing the new `apds_evidence` response field.

In the web repo, retain the normal development Clerk configuration and set the
existing API origin to:

```dotenv
API_BASE_URL=http://127.0.0.1:5000
```

There is no `/v1` suffix: the web's package detail routes use `/tools/...`.
Restart the web dev server after environment changes. If it is not running,
start it in a separate terminal:

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm-web
pnpm dev
```

Expected origins: **web `http://localhost:3000`**, **API
`http://127.0.0.1:5000`**. Keep using `localhost` consistently for browser sign-in.
If your local ports differ, change only the two prepare flags in section 2.
The helper rejects remote origins and refuses redirects rather than following
them to a production service.

## 2. Prepare one new run

Use one macOS/zsh terminal for the remaining commands. Start in the CLI repo:

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
export M8A_REPO="$PWD"
export M8A_ROOT="$M8A_REPO/web-m8a-test/$(date -u +%Y%m%dT%H%M%SZ)-$$"
export M8A_FIXTURE="$M8A_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m8a/fixture.py"
umask 077
python3 -B "$M8A_FIXTURE" prepare \
  --root "$M8A_ROOT" \
  --m4a-run "$M8A_REPO/registry-m4a-test/20261118T021239Z" \
  --m5a-run "$M8A_REPO/cli-m5a-test/20261120T170444Z-21931" \
  --web http://localhost:3000 \
  --api http://127.0.0.1:5000
source "$M8A_ROOT/paths.sh"
cd "$M8A_ROOT"
git -C "$M8A_REPO" check-ignore "$M8A_ROOT/session.json"
m8a_fixture describe
open "$M8A_ROOT/index.html"
```

Expected: **11 existing releases**, covering all eight kinds. Preparation makes
no network requests. It copies original authoring manifests for inspection and
records the package IDs from prior publication receipts/CLI output. The HTML
index is a local runbook aid; its links open the real application.

| Label                                                                          | Existing source                             | Expected definition                                         |
| ------------------------------------------------------------------------------ | ------------------------------------------- | ----------------------------------------------------------- |
| `tool`, `agent`, `skill`, `template`, `knowledge`, `memory`, `profile`, `loop` | Your M4A run, each selected `0.1.0` release | Explicit `agentpm / 1.0.0`; recorded result is checked live |
| `echo-v1`                                                                      | Your M5A Tool, `1.0.0`                      | Explicit declaration; first version of the same Tool        |
| `echo-v2`                                                                      | Same M5A Tool, `2.0.0`                      | Explicit declaration; a different selected version row      |
| `legacy`                                                                       | M5A historical undeclared Tool, `1.0.0`     | Legacy interpretation; no conformance certification         |

Inspect `catalog.json` and `fixtures/<label>/source-manifest.json` in your editor.
The original input copy is not assumed identical to a Registry's later stored
JSON serialization; section 3 compares the actual web response with the actual
selected API record.

If you open another terminal later, run the **literal `source .../paths.sh`
command printed by prepare**, or open your operator timestamp directory and run
`source ./paths.sh`. Choose your own run, not a `builder-*` directory. After that,
`m8a_fixture` works from any directory. Existing runs are never overwritten.

The helper pins its own bytes, the catalog, copied inputs and relevant reviewed
web/API source files. If those change mid-run, prepare a fresh run.

## 3. Run the automatic read-only checks

```bash
m8a_fixture check
cat expectations.json
open "$M8A_ROOT/index.html"
```

Expected: one `PASS` for every fixture, followed by **11 releases / 60 local
GETs**. A typical run takes tens of seconds, including local dev compilation.
The index now includes live expected results. The helper checks:

- Supported reference content, canonical URL, working rule anchors and pinned
  schema/source links; unknown standard identity/version returns 404 and noindex.
- Each real selected API identity and complete stored manifest against the raw
  JSON returned by the web, including fields omitted from the old Overview view.
- Raw JSON's `private, no-store`, `noindex` and `nosniff` response headers.
- Actual Overview links to the same release's raw JSON and Security tab.
- The Security page against the API's **validated `apds_evidence` projection**,
  including recorded levels, unevaluated-rule counts and unavailable timestamps.
- Qualified legacy handling, distinct Tool version rows, a concrete `2.0.0` raw
  link from `latest`, and the APDS reference entry in the pages sitemap.

Each attempt retains response bodies, status and headers under
`evidence/<timestamp>/`. A failed retry invalidates the current check result;
the audit cannot silently use an earlier success.

**Stop on failure.** Missing publications mean the local DB no longer contains
the earlier fixtures; the helper will not invent replacements. Missing
`apds_evidence` means the API may need restarting, or its existing public
Security cache has not expired yet. A sitemap error may indicate the normal
local docs/blog ingestion or web/API configuration needs attention. Fix the
local prerequisite and rerun `m8a_fixture check`; no DB migration or production
change is part of this guide.

## 4. Read and navigate the APDS reference

Open **APDS 1.0.0 reference** from the index.

Expected:

- The published README is the entry point, followed by the eleven-chapter rule
  navigation and complete semantics. The text distinguishes definition checks
  from Runner support, readiness, safety and quality.
- Expand **Rule links** and follow a rule such as `APDS-ID-001`. It reaches the
  matching heading. Test a chapter link and the overview/semantics links too.
- Follow the schema, semantics source and conformance links. Schema uses its
  exact canonical `$id`; source/fixture content uses `apds-v1.0.0`, never `main`.
  Issue-report links may naturally open the repository's current issue tracker.
- Open the index's **Unknown version (404)** link. It explains that the version
  is unsupported; it does not display 1.0.0 as a substitute.

These source links open public GitHub pages; the helper itself contacts only the
local application/API. Do not interpret a standard reference link as evidence
that the selected package was validated.

After inspecting, record your observations:

```bash
m8a_fixture observe reference pass
m8a_fixture observe source-links pass
```

Use `fail --notes 'what you saw'` instead if something is wrong. Record a pass
only after actually checking it; the helper does not infer browser observations.

## 5. Inspect package links across all eight kinds

Use each of the eight M4A rows in the index (`tool` through `loop`), then the
three M5A rows. You can keep the index open and open cases in new tabs.

For each release:

1. Open **overview**. Find **Package definition** with its declaration or
   qualified legacy explanation. Existing tabs, actions and specialized content
   remain usable.
2. Open **View agent.json**. The address contains that exact package ID and
   artifact version. It shows complete stored JSON, including `kind`, `standard`
   where present and `$schema` where stored.
3. Return and open **View APDS specification** for a declared release. Its URL
   is `/standards/agentpm/1.0.0`, independent of the package's `0.1.0`, `1.0.0`
   or `2.0.0` release version.
4. Open **APDS evaluation and separate security evidence**. It reaches that
   selected release's Security tab, not Overview or a different release.

For `legacy`, the reference link is explicitly qualified as compatibility;
there must be no ordinary declaration/conformance badge substituted for it.

```bash
m8a_fixture observe packages pass
```

## 6. Switch versions of the same Tool

Open `echo-v1` **overview** from the index. Use the page's version selector to
switch to `2.0.0`, then back to `1.0.0`.

Expected: the address, raw-manifest link and Security link follow your selection.
The APDS reference remains 1.0.0 because that is the standard both versions
declare. The raw manifests have different artifact `version` values.

Open **Tool latest** from the index. The page may retain `latest` in its address,
but **View agent.json** must point to the concrete selected **2.0.0**, not
`/packages/.../latest/agent.json`.

```bash
m8a_fixture observe versions pass
```

## 7. Read recorded evidence and legacy qualification

Open the **Security** links. Compare the **APDS declaration and evaluation**
panel with `expectations.json` for the same label.

Expected:

- Declared releases show a recorded declaration separately from any usable
  **Registry-recorded APDS result** and its named levels. Unevaluated-rule counts
  match the projection; an incomplete result remains incomplete. When the API
  projects `unknown` or `not_evaluated`, the web shows **APDS conformance evidence
  unavailable or not evaluated**, with no successful conformance result.
- Your M4A records predate the later publication-policy marker. The current API
  can correctly reject those old reports as `validation_report_inconsistent`,
  even if raw `apds_validation.status` says `conformant`. Keep that unavailable
  case; compare the UI with `apds_evidence`, not raw JSONB. The newer M5A Tool
  versions supply usable recorded-evaluation coverage. The helper requires at
  least one usable evaluation so a completely blank panel cannot pass the run.
- These older fixtures commonly have no evaluation timestamp. The UI must say
  **Evaluation time unavailable** where a usable recorded result exists, rather
  than reuse the publication time or invent a fresh check time.
- `legacy` shows **Inferred legacy interpretation** and explains that inference
  is not an explicit declaration or verified conformance. It has no successful
  conformance result inferred from the missing declaration.
- Signature/scan evidence stays separate. There is no new bare Verified badge,
  claim that the browser independently verified the package, or readiness/safety
  guarantee from a declaration or recorded definition check.

The corrupted-report and future-level cases are automated regressions. This
manual run does not change real stored reports or disable a production guard.

```bash
m8a_fixture observe evidence pass
```

## 8. Keyboard and narrow-screen inspection

On the reference page, use **Tab** to reach the links and rule disclosure
controls; use **Enter/Space** to activate a focused disclosure. Check visible
focus and that rule links reach readable headings below the sticky header.

At a narrow browser width (about **390 px**, using responsive DevTools or a
resized window), inspect the reference and an Agent/Tool package page. Expect
readable prose/code/links, usable wrapped navigation and no clipped critical
controls. Long code may scroll within its own block. The existing dark shell
and package identity styling remain; this milestone is not the shell redesign.

```bash
m8a_fixture observe keyboard pass
m8a_fixture observe responsive pass
```

## 9. Check a real private package in `zack-internal`

In your **normal signed-in local browser**, open your private user namespace
**`zack-internal`** through the account/namespace UI. Select one existing
published package, open Overview, and click **View agent.json**.

Expected: its complete manifest loads. In DevTools **Network**, confirm the
request to `/packages/<id>/<exact-version>/agent.json` returned **200**. This
positive request proves that the ID is real and accessible to you; a random
nonexistent ID returning 404 is not a privacy test.

While on that raw JSON tab, use **Cmd+L**, then **Cmd+C** to copy its local URL.
Back in the run's terminal:

```bash
m8a_fixture private --url "http://localhost:3000/packages/d28ee3ef-1754-4da4-bded-21dc8797babf/0.1.0/agent.json" --authorized-status 200
cat private.json
```

Expected: **anonymous web 404 and anonymous API 404**, with private/no-store
headers on the web response. The helper deliberately sends no browser cookies
or bearer token and refuses a nonlocal URL.

`private.json` distinguishes your **operator-reported signed-in 200** from the
two **actually requested anonymous 404s**. It does not claim to have automated
your authenticated browser. It stores no private manifest body or credentials.

Optionally paste the same URL into a signed-out/incognito window and confirm
that no manifest is visible there. Keep the package and namespace private; no
visibility changes or republishing are needed.

## 10. Audit and retain evidence

```bash
m8a_fixture audit
cat evidence/summary.json
```

Expected: **`status: "pass"`**, **11 checked fixtures**, **60 base local GETs**,
**7 browser observations**, an explicitly operator-reported authorized private
200 and automated anonymous web/API 404s. `operator_signoff` remains `false`:
the helper does not close the milestone or certify hosted CI. CI has already
been user-confirmed passed in the task record.

If an observation is missing or failed, the audit names it. Repeat that browser
check and record its actual result; history is append-only. A failed private
check cannot reuse an earlier private success. Keep the whole timestamp directory
for evidence and send the summary plus any findings back for closure. Do not
delete or edit the old M4A/M5A source runs.
