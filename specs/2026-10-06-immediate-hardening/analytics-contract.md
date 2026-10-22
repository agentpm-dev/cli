# Milestones 10A-10B Event Contract

Version: 1. This vocabulary also governs Milestone 10B; 10A emits no CLI-local
or Harness telemetry. Development uses the PostHog US project **AgentPM
Development**. Never configure that project's token on production.

## Transport and Identity

Use PostHog's documented [capture API](https://posthog.com/docs/api/capture)
(`/i/v0/e/`) through existing `fetch`/`httpx` clients.
No browser SDK, HTML snippet, autocapture, replay, feature flags, or advertising
integration is installed. Browser events use a same-origin Next.js proxy.
The browser's random session cookie is analytics correlation, never auth.
Authenticated events use a one-way digest of the account subject, not email,
name, token, or namespace. PostHog's explicit identify event joins the browser
session to that pseudonymous account. Shared-browser sign-out rotates the
anonymous identity. Respect Do Not Track and Global Privacy Control for browser
events. No analytics configuration means no capture.

Ordinary properties are **allowlisted enums, bounded counts, booleans, and the
binary's compile-time CLI version**.
Never send raw query text, URL/query strings, package/namespace names or IDs,
file paths, auth data, request bodies, prompts, tool I/O, or application objects.
Account pseudonyms and random event IDs are protocol identity fields, not
general-purpose property escape hatches. Do not forward IP/user-agent/referrer
headers to PostHog; disable GeoIP enrichment.
Also enable **Discard client IP data** in each PostHog project's privacy
settings: GeoIP suppression alone does not prevent PostHog from attaching the
capture connection's IP. Verify newly stored events rather than assuming a
payload allowlist controls ingestion-side enrichment.

## Events

All events include `contract_version: 1`, `source: web|registry|cli`, and
`environment: development|test|production`. Each has a UUID for deduplication.

| Event | Authority / when | Additional allowed properties |
| --- | --- | --- |
| `landing_viewed` | Browser, rendered landing page | none |
| `docs_viewed` | Browser, rendered docs article | none |
| `explore_searched` | Browser, explicit nonempty submission from Explore or the global header; not results-page arrival/refresh | `sort`, `filter_count` |
| `package_viewed` | Browser, successfully rendered package detail | `kind` |
| `namespace_viewed` | Browser, successfully rendered namespace | none |
| `signup_started` | Browser, valid sign-up form submission | `method: email` |
| `pricing_viewed` | Browser, rendered pricing page | none |
| `checkout_started` | Browser, actual redirect to a returned checkout URL | `plan: pro|team` |
| `account_created` | Registry, first shadow-user creation, after commit | none |
| `package_install_completed` | Registry, pending install session becomes completed, after commit | `package_count` |
| `publish_completed` | Registry, verified publish becomes completed, after commit | `kind`, `visibility` |
| `package_starred`, `package_unstarred` | Registry, actual state change, after commit | none |
| `subscription_started`, `subscription_updated`, `subscription_cancelled`, `subscription_payment_failed` | Registry, verified supported Lemon Squeezy webhook applied, after commit | `plan`, `test_mode` |
| `feedback_submitted` | Registry, PostHog accepts the survey response | `has_email`, `follow_up` |

An install completion is registry-observed completion, not proof that an
installed executable was run. Checkout creation/clicks are not paid conversions.
Lemon Squeezy remains billing truth. Reconciliation must not manufacture webhook
conversion events. Duplicate webhook deliveries queue no second event; the
PostHog event UUID is also stable for the delivery.

## Feedback Exception

The deliberately submitted survey is the only free-text path: goal, obstacle,
optional email, and follow-up consent. Validate lengths/types and send only
these values in PostHog's [custom survey protocol](https://posthog.com/docs/surveys/implementing-custom-surveys),
using `survey sent`, configured survey/question IDs, and the question metadata
needed for position-based reports. Never copy answers into `feedback_submitted`, error logs, or
general analytics. An unchecked follow-up box does not authorize follow-up.
Survey delivery failure must not show a successful submission. The GitHub
issue link is an alternative, not the only feedback path.

## CLI Contract (M10B, Recorded Before Instrumentation)

`cli_first_used` is attempted once per anonymous installation (one CLI config
directory), not per version or command. A random UUIDv4 in that directory is stable
across upgrades; deleting that state creates a new installation. A separate
atomic first-use claim prevents simultaneous processes from double-counting.
This is an attempt, not guaranteed delivery: short-lived processes may exit
before capture, so adoption is a lower bound. There is no replay or retry.

`harness_started` is emitted once after the shared engine starts a **run**.
`harness_completed` is emitted at most once when that run reaches a terminal
outcome, including handled runtime errors. It is not success-only. Paused
interactive approval is not completion; terminal approval-required is.
There is no completion-on-Drop or panic recovery. A multi-prompt TUI session
therefore produces one pair per run, just like separate headless invocations.

The closed CLI wire schema is `{event, install_id, properties}`. Event-specific
properties are exactly:

| Event | Properties |
| --- | --- |
| `cli_first_used` | `cli_version`, `os_family`, `architecture` |
| `harness_started` | same, plus `runtime_mode: tui|headless|machine` |
| `harness_completed` | same, plus `runtime_mode` and `terminal_outcome: ended|handed_off|aborted|failed|cancelled|limit_reached|approval_required` |

Version comes from the binary; platform values come from closed mappings, not
hostnames. Install IDs are random, never machine/user/path-derived. Run/session
IDs, durations, package identities and command arguments are not sent. The
Registry rejects extra fields and invalid values, then adds only the existing
contract/environment/source and privacy protocol fields. CLI distinct IDs are
`cli_<UUID>` and are never identified or joined to browser/account identities.

Transport is an anonymous `POST /analytics/cli` on the CLI's selected Registry
base URL. It reuses M10A's server-side PostHog configuration, keeping project
keys and authentication out of the binary. Neither credentials nor browser
identity headers are sent. Redirects and inherited proxies are disabled.

The telemetry-only `.no_proxy()` policy is deliberate: optional usage capture
does not implicitly inherit general-purpose proxy routing or credentials from
`HTTP_PROXY`, `HTTPS_PROXY` or `ALL_PROXY`. This does not bypass network controls
or change the ordinary Registry client's proxy policy. If direct Registry
access is blocked on a proxy-required network, telemetry fails silently and
nonfatally rather than trying the proxy. Such environments will be
underrepresented in adoption/activity charts. The first-use claim is still
consumed even if delivery fails, so later unproxied access does not recover
that adoption event. Treat these counts as incomplete, network-biased signals,
not a census of CLI usage.

A bounded in-memory queue feeds one detached OS thread, with a one-second HTTP
timeout and no retries, outbox, flush, join, or exit-time wait. Queued events
can be lost at exit. Sending failures never alter output or exit status.

`AGENTPM_TELEMETRY=0` or `[telemetry] enabled = false` in the existing
`config.toml` disables collection before ID/claim creation. Unreadable or invalid
config/state fails closed without affecting commands. Both enqueue and delivery
recheck opt-out, so changing config during a TUI session suppresses later events.
Already transmitted events cannot be recalled.

Publish the updated Privacy Policy before distributing this binary. Development
and Production retain their separate PostHog keys/environment filters and IP
discard settings. Do not populate Production with synthetic test events.

## Delivery and Dashboard Limits

Ordinary analytics are best-effort, bounded-time, and never change a successful
product operation into an error. No capture before DB commit; rollback discards
queued events. This milestone does not introduce a durable outbox, so an outage
or process crash can lose an event. Event delivery is not billing truth.

Use one **Early Product Funnel** dashboard. Show arrival/discovery, signup,
registry installs/publishes, pricing/checkout/subscriptions, stars, and feedback.
CLI/Harness chart setup and live acceptance remain **pending operator
verification**, not zero-adoption claims. Do not present anonymous CLI installs and website sessions as one
identified person-level funnel. Production project, survey launch, live payload
verification, and dashboard creation need operator confirmation.

Label Harness charts **per-run**, break down by `runtime_mode`, and state:
missing completions are an upper bound on failures, not a failure rate. Kills,
panics, sleep, failed/dropped sends and mid-session opt-out can all contribute.
