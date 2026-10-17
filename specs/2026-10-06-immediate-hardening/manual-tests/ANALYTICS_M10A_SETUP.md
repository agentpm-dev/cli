# Milestone 10A PostHog Setup and Verification

This is an operator walkthrough, not a claim that the live setup is complete.
Use **AgentPM Development** on **us.posthog.com** for local testing. Do not
configure that project's token on production. No snippet, browser SDK, wizard,
autocapture, session replay, or paid upgrade is required by the implementation.
If PostHog asks for billing to enable a required feature, pause and discuss it.

## 1. Configuration

Already selected: US Cloud, project renamed from Default Project to **AgentPM
Development**. Keep the project token out of source control and chat transcripts.
It is not a personal API key and does not grant dashboard administration.

In `agentpm-web/.env.local`:

```dotenv
NEXT_PUBLIC_POSTHOG_KEY=<development project token>
NEXT_PUBLIC_POSTHOG_HOST=https://us.i.posthog.com
```

In the **backend's IntelliJ run configuration**, not as literal values in
`config.py`:

```dotenv
POSTHOG_API_KEY=<same development project token>
POSTHOG_HOST=https://us.i.posthog.com
POSTHOG_ENVIRONMENT=development
```

Restart both local servers after environment changes. Do not start a second
server on an occupied port. The existing backend `Config` reads these values
from the environment. The browser sends only explicitly constructed JSON to the
same-origin `/api/analytics` route; the backend sends to PostHog's capture API.

In PostHog **Settings -> Project -> Privacy**, find **IP data capture
configuration** and enable **Discard client IP data**. Repeat this for the
production project before enabling captures there. AgentPM does not forward the
browser's IP to PostHog, but PostHog can add the capture connection's address
(the local backend's public egress address during local testing). The event's
`$geoip_disable: true` prevents location enrichment, not IP storage. PostHog's
[IP retention setting](https://posthog.com/docs/privacy/data-storage#discarding-ip-addresses-at-project-level)
is a separate requirement. Check a newly captured event after changing it;
do not assume this removes IPs from existing events.

## 2. First Events

Open the website landing page, docs article, package detail, namespace page, and
pricing page. Submit a nonempty search on Explore, then another using the global
header from a page where it is visible. Repeat the global-header submission at
a mobile viewport or on a phone; it uses a separate component from desktop.
Each submission should emit exactly one
`explore_searched` with only `sort` and `filter_count` plus common metadata.
Typing without submitting, empty/whitespace submissions, and arriving on or
refreshing the resulting Explore URL must not emit another search event.
In PostHog Activity, filter by the
explicit event names listed in [the contract](../analytics-contract.md).

Inspect each event's properties. Expect `environment: development`,
`contract_version: 1`, and `source: web` or `registry`. Do not expect `$pageview`
or `$autocapture`. PostHog's generic Web Analytics onboarding may keep asking for
those events; use Product Analytics for our deliberately named events instead.

There must be no raw search text, page URL/query string, package ID/name,
namespace ID/name, auth token, email, referrer or browser user agent in ordinary
event properties. Browser requests must not include page contents or passwords.
New events must not retain an **IP address** / `$ip` property after the project
setting above is enabled.
Try Do Not Track/Global Privacy Control and confirm browser captures stop.

Visit a package and toggle its star twice, then repeat an already-satisfied star
request. Only actual state changes should emit `package_starred`/`package_unstarred`.
For a fresh local account, verify `account_created` appears once, not at every
session refresh. Failed operations and rollbacks must not emit completion.

## 3. Feedback Survey

In PostHog Surveys, create an **API/custom UI** survey named **AgentPM Feedback**.
Do not choose an automatically displayed popover. Set up four questions in this
order (IDs, not question order, are used by the implementation):

1. Open text: **What were you trying to do?** Required.
2. Open text: **What got in the way or surprised you?** Required.
3. Open text: **Email (optional)**. Optional.
4. Single choice: **May we follow up?** Choices **Yes**, **No**. Optional.

If adding multiple questions requires a subscription/billing step, pause rather
than upgrading. The application code cannot create an account entitlement.

Record the survey UUID and the four question UUIDs from its configuration/API
view. These are identifiers, not credentials. Add the following backend IntelliJ
environment variables, replacing every placeholder with a real UUID:

```dotenv
POSTHOG_FEEDBACK_SURVEY_ID=<survey UUID>
POSTHOG_FEEDBACK_QUESTION_IDS={"goal":"<question UUID>","obstacle":"<question UUID>","email":"<question UUID>","follow_up":"<question UUID>"}
```

Use the complete JSON object as one environment-variable value. Launch the API
survey and restart the backend. The AgentPM form owns display and presentation;
there is no PostHog widget to install.

Open **Feedback** in the site's footer. Submit once without email/follow-up,
then once with both. Verify the answers appear under this survey in PostHog and
the consent choices remain distinct. Ordinary `feedback_submitted` has only
`has_email` and `follow_up` plus the common metadata, never the answers.

PostHog's survey performance counters default to unique respondents: two
submissions from the same account can show **Responses 1** while **Completed 2**
and the response table show both submissions. See its
[summary counter implementation](https://github.com/PostHog/posthog/blob/master/frontend/src/scenes/surveys/SurveyStatsSummary.tsx)
and [default unique-person setting](https://github.com/PostHog/posthog/blob/master/frontend/src/scenes/surveys/surveyLogic.tsx).
AgentPM does not emit `survey shown`, so Shown/response-rate metrics are not
measured by this integration; use completed submissions and the response table,
not the displayed response rate, to verify feedback delivery.

Without survey configuration or with delivery unavailable, the form must show
an error and retain the answers, not thank the user. Retry should reuse the
submission identifier. Test mobile layout, keyboard focus/Escape, and the GitHub
issue link. Review the updated web/feedback privacy disclosure before production.

## 4. Authoritative Registry and Billing Events

Use local test packages and Lemon Squeezy **test mode**, not a real paid checkout.

### Public Install

The operator's development registry was checked and contains
`@zack/bound-agent@0.1.0`. For a different database, choose an actual published
public package present there. Run each command yourself; no existing workspace
or credential file is changed:

```sh
M10_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m10-analytics.XXXXXX")"
mkdir -p "$M10_ROOT/install" "$M10_ROOT/home"
cd "$M10_ROOT/install"

env -u AGENTPM_TOKEN \
  HOME="$M10_ROOT/home" \
  USERPROFILE="$M10_ROOT/home" \
  XDG_DATA_HOME="$M10_ROOT/home/data" \
  XDG_CONFIG_HOME="$M10_ROOT/home/config" \
  APPDATA="$M10_ROOT/home/data" \
  agentpm --base-url http://127.0.0.1:5000 \
  install @zack/bound-agent@0.1.0
```

After Installed, inspect `package_install_completed` in Activity. Its ordinary
properties must not contain package identities. This registry completion event
does not identify the anonymous CLI install as a website visitor or imply that
the installed Agent was run.

### Local-Only Publish

Use the original local account that owns namespace `zack`, not the fresh signup
test account. Create a temporary PAT in that **local website's** profile with
`packages:publish` permission. Never paste it into chat, save it in the fixture,
or enable shell tracing. Substitute an owned local namespace if using another
database. All network commands below explicitly target the local registry.

```sh
M10_PUBLISH_DIR="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m10-publish.XXXXXX")"
agentpm init --kind profile \
  --name "m10-analytics-$(date -u +%Y%m%d%H%M%S)" \
  --description 'Local-only analytics verification' \
  --out-dir "$M10_PUBLISH_DIR"
cd "$M10_PUBLISH_DIR"
agentpm lint
```

If lint fails, stop before publishing. Otherwise read the local PAT without
echoing or storing it, confirm the account, and publish the new package:

```sh
printf 'Paste the temporary LOCAL registry PAT, then press Enter: '
read -r -s M10_LOCAL_PAT
printf '\n'
agentpm --base-url http://127.0.0.1:5000 whoami --token "$M10_LOCAL_PAT"
```

Stop if whoami fails or shows an unexpected account. Only after verifying it,
run the real publish:

```sh
agentpm --base-url http://127.0.0.1:5000 publish --namespace zack --token "$M10_LOCAL_PAT"
unset M10_LOCAL_PAT
```

A successful publish must produce `publish_completed` with `kind: profile`, `source: registry`,
and `environment: development`, without the package name/ID or PAT. Revoke the
temporary PAT afterward. The scaffold was linted and dry-run packaged by the
builder, but only this real publish verifies the live event.

### Remaining Registry and Billing Checks

Before creating a checkout, confirm `LEMON_SQUEEZY_TEST_MODE=true` in the running
backend, with test-mode API credentials and product/variant IDs. Use the fresh
local signup test account, not an account with an existing billing subscription.
The operator restored ngrok forwarding from a public HTTPS hostname to backend
port 5000. Its Lemon Squeezy **test-mode** webhook destination must end with
`/billing/webhooks/lemonsqueezy`; the signing secret must match
`LEMON_SQUEEZY_WEBHOOK_SECRET` in the local backend environment. Do not change a
production webhook or share credentials in chat or the test record.

With ngrok running, probe the callback without a signature (substitute the
actual forwarding hostname):

```sh
curl -i -X POST \
  https://YOUR-NGROK-HOST/billing/webhooks/lemonsqueezy \
  -H 'Content-Type: application/json' \
  --data '{}'
```

Expected: HTTP 403, `Invalid webhook signature`. This verifies routing and
rejection only, not successful signed delivery. Do not bypass the signature
check. Keep the tunnel running during the test and stop it afterward; it makes
the local backend publicly reachable.

In the fresh account's browser window, visit the local Pricing page, then
Profile -> Billing -> Start Pro checkout. Stop at the Lemon Squeezy checkout
and confirm it visibly indicates **test mode** before entering any payment
information. Inspect `pricing_viewed` and `checkout_started` in PostHog;
subscription conversion still requires a verified webhook after test purchase.
Use only Lemon Squeezy's documented test cards, never real card details.

Check these individually through normal product commands/actions:

- Finish an install: `package_install_completed` appears after finalization.
- Finish a publish: `publish_completed` appears only after successful commit.
- Submit checkout and reach the returned checkout URL: `checkout_started`.
- A failed checkout request must not emit a handoff or subscription event.
- Replay a verified test webhook for subscription creation/update/cancellation/
  failed payment: expect the corresponding explicit subscription event.
- Replay the identical delivery: no second logical event. Repeat an identical
  payload without a delivery ID to exercise the existing payload-hash guard.
- Signature rejection, rollback, ignored noncanonical subscription updates, and
  manual reconciliation must not manufacture conversion events.

No subscription ID, customer ID, payment details, or private package identity
belongs in analytics. The Registry retains billing truth independently of
PostHog. Delivery is bounded-time and best-effort, not a durable event outbox.

## 5. One Dashboard

Create one blank dashboard named **Early Product Funnel** in this project. Add
these insights, filtered by `environment = development` while testing:

| Insight | Events / interpretation |
| --- | --- |
| Arrivals | Unique visitors and event count for `landing_viewed` |
| Discovery | Trends for `docs_viewed`, `explore_searched`, `package_viewed`, `namespace_viewed` |
| Signup | Funnel: `signup_started` -> `account_created`; Sequential step order, Overall conversion, 7-day conversion window |
| Registry use | Trends for `package_install_completed`, `publish_completed` |
| Billing intent and conversion | Funnel: `pricing_viewed` -> `checkout_started` -> `subscription_started` |
| Billing lifecycle | Trends for `subscription_updated`, `subscription_cancelled`, `subscription_payment_failed` |
| Engagement | Trends for `package_starred`, `package_unstarred`, `feedback_submitted` |
| CLI/Harness availability | Text panel: **Local CLI/Harness telemetry is pending Milestone 10B; missing data is not evidence of zero usage.** |

Keep these in one dashboard, not one dashboard per event. Anonymous CLI installs
cannot truthfully be treated as the same identified website visitor. The signup
funnel's identity linkage must be checked with the real fresh-account test.

Operator progress: **Early Product Funnel** exists in AgentPM Development.
The **Arrivals** panel is saved with `landing_viewed` unique users and total
count. **Discovery** is saved with `docs_viewed`, `explore_searched`,
`package_viewed`, and `namespace_viewed`, each as total count. Both use last
7 days and `environment = development`. The operator also confirmed a fresh
global-header search appeared in PostHog after the instrumentation fix.
Fresh-account signup emitted both product events plus Identify; the operator
confirmed the signup's raw anonymous ID matches Identify's anonymous ID.
The saved **Signup** funnel subsequently showed one person at each step and
100% conversion, accepting the connected anonymous-to-account journey.
The operator also completed a local-only publish and saved **Registry use**
with both registry completion events as total counts, last 7 days, and
`environment = development`. **Engagement** is also saved with `package_starred`,
`package_unstarred`, and `feedback_submitted` total counts, last 7 days, and
the same development filter. The operator added the CLI/Harness availability
text card, explicitly distinguishing pending local telemetry from registry
completion counts. **Billing intent and conversion** is saved with
`pricing_viewed` -> `checkout_started` -> `subscription_started`, last 7 days,
and `environment = development`; the screenshot shows one person reaching the
final step at 100% conversion. **Billing lifecycle** is saved with
`subscription_updated`, `subscription_cancelled`, and
`subscription_payment_failed`; the operator screenshot shows total counts of
4, 1, and 1 respectively on October 17 UTC. All M10A dashboard panels are now
present; actual local CLI/Harness measurement remains deferred to M10B.

## 6. Production Handoff

Before production, arrange a separate production project if the account plan
permits it. Do not add billing or purchase a plan without explicit approval.
Production and local data must not silently mix. Configure a production survey
and project token, set `POSTHOG_ENVIRONMENT=production`, and copy the single
dashboard with production filters. Review privacy copy and access/retention
settings for feedback (which can contain personal information).

Use the Development dashboard's actions sidebar -> **Copy to another project**
to create an independent copy in AgentPM Production, including new copies of
its insights. In the production copies, change saved `environment` filters to
`production`; leave the Development dashboard and insights unchanged. Keep
public sharing disabled on the production copy too. An empty production chart
before deployment is expected and is not live capture verification.

Operator confirmed the organization has used Pay-as-you-go from initial setup;
no plan upgrade was requested or performed. PostHog's current
[pricing documentation](https://posthog.com/pricing) states paid-plan events and
metadata have seven-year paid-plan retention. After reviewing that this also
applies to stored survey text and optional email, operator accepted the existing
provider retention for this release. No shorter cleanup policy or automatic
deletion was added; this does not promise deletion exactly at seven years.
Production deployment/verification remains open. Operator supplied the
production API survey JSON with the expected
four questions and `start_date: 2026-10-17T03:09:01.260000Z`, `end_date: null`,
confirming launch. Operator confirmed all specified backend/web environment
values were added to Render and explicitly saved without redeploying; deployment
was planned with the M10A code push, which the operator now reports is pushed
out. Successful service rollout and live checks are not yet independently
verified. Operator also confirmed all dashboard panels
were copied to Production and their filters changed to `environment = production`.
Operator separately confirmed public sharing is disabled on the Production
dashboard. Operator then observed live `landing_viewed` events in AgentPM
Production, with production environment, web source, contract version 1, GeoIP
disabled, and no IP property in the shared property list. This verifies the
production web-to-registry capture path and stored-event IP suppression for the
homepage event. Operator then confirmed the production smoke-test feedback was
stored under the correct four survey questions, with blank email and follow-up
No. Operator also supplied the separate `feedback_submitted` property list with
production environment, registry source, contract version 1, both flags false,
GeoIP disabled, and no answers, email, or IP. Production feedback storage and
generic-event separation are accepted. Operator confirmed the Production
dashboard charts update and that additional real star/unstar actions appear in
Engagement. Dashboard live data and production star/unstar capture are accepted.

No CLI instrumentation, CLI opt-out contract, or CLI privacy-policy rollout is
implemented here; those are Milestone 10B.

## Operator Acceptance

- [x] Development project named and region confirmed.
- [x] Development project token placed in local web/backend environment settings.
- [x] Development: Discard client IP data enabled and verified on a new event.
- [x] Production API feedback survey created with the expected four questions and launched; operator supplied its JSON with a start date and no end date.
- [x] Production backend/web PostHog environment values added to Render, confirmed by operator; no secrets were shared.
- [x] Independent Claude code review completed; operator reported everything looked good. This does not establish live production behavior or resolve the recorded portal issue.
- [x] Operator reported the M10A release was pushed out; successful service rollout and live capture remain separate verification steps.
- [x] Production release reported pushed out; operator's live `landing_viewed` verifies the deployed web-to-registry capture path with the production environment configuration. Render build logs were not independently inspected.
- [x] Production: operator inspected a new `landing_viewed` event with GeoIP disabled and no IP property in the shared property list.
- [x] Production feedback submission stored under the correct survey questions; operator confirmed the smoke-test answers, blank email, and follow-up No.
- [x] Production `feedback_submitted` property list inspected: production environment, registry source, contract version 1, has_email false, follow_up false, GeoIP disabled, and no answers/email/IP.
- [x] Production dashboard displays real capture data; operator confirmed updating charts and real star/unstar actions appearing in Engagement.
- [x] Development ordinary-event payload spot-check completed: operator reported no unexpected properties after inspecting existing explore_searched, package_viewed, package_install_completed, and publish_completed against their allowlists.
- [x] API survey configured and real feedback storage verified with blank email / follow-up No.
- [x] Survey confirmed active/launched; email supplied / follow-up Yes verified in survey reporting and event flags.
- [x] Real local-registry install produced `package_install_completed` in PostHog.
- [x] Real local-only publish completed and `publish_completed` is included in the saved Registry use panel. Event-property spot-check is accepted above.
- [x] ngrok callback reaches the local backend and rejects an unsigned request with HTTP 403 / Invalid webhook signature.
- [x] Checkout visibly indicates test mode and real `pricing_viewed` / `checkout_started` events appear in PostHog.
- [x] Test purchase produced `subscription_started` with `plan: pro`, `test_mode: true`, `source: registry`, and `environment: development`; shared properties and GeoIP disabled also confirmed.
- [x] Merchant-side cancellation of the test subscription produced `subscription_cancelled`; operator confirmed `plan: pro`, `test_mode: true`, `source: registry`, `environment: development`, contract version 1, and GeoIP disabled.
- [x] Lemon Squeezy test simulation produced `subscription_payment_failed` in PostHog; operator confirmed contract version 1, development environment, registry source, test mode true, and GeoIP disabled. Observed `plan: unknown`; delivery is verified, but plan enrichment is limited to product/variant IDs present in the webhook and is not inferred from the stored subscription.
- [x] Exact webhook replay returned HTTP 200 with `ok: true` and `duplicate: true`, confirmed by operator.
- [x] Operator confirmed PostHog still contains only one `subscription_started` after duplicate replay. The two `subscription_updated` events correspond to distinct `subscription_updated` and `subscription_payment_success` webhook deliveries, verified from database event names and timestamps.
- [x] Early Product Funnel dashboard created and fresh-signup identity linkage verified.
- [x] Billing intent and conversion funnel saved; screenshot verifies the three expected steps, development filter, and one person reaching subscription creation at 100% conversion.
- [x] Remaining M10A dashboard insights configured and verified: Billing lifecycle screenshot shows all three required lifecycle events with total counts.
- [x] Organization membership reviewed: operator confirmed they are the sole member. Dashboard public sharing, retention, and production access settings are not established by this check.
- [x] Development Early Product Funnel dashboard public sharing disabled, confirmed by operator.
- [x] Production Early Product Funnel dashboard and insights copied; all environment filters changed to production, confirmed by operator.
- [x] Production Early Product Funnel dashboard public sharing disabled, confirmed by operator.
- [x] Existing billing plan confirmed as Pay-as-you-go; no plan upgrade required for the planned separate production project.
- [x] Existing provider retention reviewed and accepted for this release, including survey text and optional email; no shorter cleanup policy or automation added.
- [x] Predeployment access, dashboard sharing, retention, and production survey/environment/dashboard configuration reviewed. Subsequent live payload and production smoke checks are accepted above.

Scoped M10A operator acceptance is complete. Evidence is operator-reported where
noted; no new automated tests were run during this final checklist update.

## Mobile Search Follow-up

Post-release testing found that the mobile global header omitted search capture.
The local fix now uses the existing analytics helper and sort/filter-count
allowlist, matching desktop. Regression tests and intercepted browser checks
cover both headers and Explore without sending fixture events to PostHog.

- [x] Reproduce the missing mobile event with behavioral tests and fix the mobile submission path.
- [x] Verify desktop/mobile Enter and form submission once, silent typing/blank input, no query/cursor in event properties, and no additional event on Explore arrival or reload.
- [ ] After deploying the web patch, submit a nonempty search from the mobile global header on the production homepage. Confirm exactly one new `explore_searched` in AgentPM Production with production environment, web source, sort Relevance, filter_count 1, and no raw query/URL/IP. Reloading Explore must not emit another search event.

## Known Residuals

- Customer portal access remains unresolved: enabled portal and fresh signed
  test-mode link redirect to the merchant dashboard (regular session) or Lemon
  Squeezy login (fresh Incognito). Merchant-side cancellation verifies the
  analytics webhook, not the customer portal flow. This is recorded separately
  from M10A's analytics acceptance, not classified as expected behavior.
- Invoice-style payment events lacking product/variant IDs emit `plan: unknown`;
  event delivery is verified, but plan-level classification is limited.
- Local CLI/Harness telemetry remains deferred to M10B, not implemented here.
