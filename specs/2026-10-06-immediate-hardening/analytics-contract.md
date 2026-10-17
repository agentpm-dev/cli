# Milestone 10A Event Contract

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

Ordinary properties are **allowlisted enums, bounded counts, and booleans**.
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

All events include `contract_version: 1`, `source: web|registry`, and
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

## Delivery and Dashboard Limits

Ordinary analytics are best-effort, bounded-time, and never change a successful
product operation into an error. No capture before DB commit; rollback discards
queued events. This milestone does not introduce a durable outbox, so an outage
or process crash can lose an event. Event delivery is not billing truth.

Use one **Early Product Funnel** dashboard. Show arrival/discovery, signup,
registry installs/publishes, pricing/checkout/subscriptions, stars, and feedback.
CLI/Harness panels are explicitly **pending Milestone 10B**, not zero-adoption
claims. Do not present anonymous CLI installs and website sessions as one
identified person-level funnel. Production project, survey launch, live payload
verification, and dashboard creation need operator confirmation.
