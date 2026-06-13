# Test Plan

## Required verification
This phase is security-sensitive. Work is not complete until both public compatibility and private access-control behavior are verified.

Required verification areas:

- Database migrations apply cleanly and can be rolled back if the repo supports downgrade testing.
- Namespace creation supports multiple user namespaces, org namespaces, and public/private visibility.
- Namespace visibility cannot be changed after creation.
- Org membership roles enforce the expected permission matrix.
- Private namespace/package data is hidden from anonymous and unauthorized users.
- Authorized users can view/search/install private packages from namespaces they can access.
- Search filtering works across all search modes and result types.
- Install resolve/init/finalize enforce private access.
- Publish init/finalize enforce role permissions and entitlement status.
- Private package presigned URLs are never issued to unauthorized users.
- Public package flows remain compatible.
- Pricing page renders real content.
- Lemon Squeezy webhook handling is idempotent and maps provider events to internal billing state.
- Signing key add flow is removed from web UI while revoke remains available.
- Docs are updated for user-facing behavior changes.

## Automated checks
Run the repo's standard automated checks. Adjust commands to the exact repo scripts if names differ.

Backend/API:

- `pytest`
- `pytest tests -k namespace`
- `pytest tests -k search`
- `pytest tests -k install`
- `pytest tests -k publish`
- `pytest tests -k billing`
- `ruff check .` or the repo's configured lint command
- `mypy .` if mypy is part of the repo's current verification
- Alembic migration check, for example `flask db upgrade` or the repo's migration command

Frontend/web:

- `pnpm lint`
- `pnpm typecheck` if available
- `pnpm test` if available
- `pnpm build`

CLI:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Search/index migration:

- Run migrations that update `tool_search_index` and `namespace_search_index`.
- Refresh materialized views successfully.
- Confirm indexes exist for visibility filtering.

Billing/webhook:

- Run webhook unit tests using captured/sample Lemon Squeezy payloads.
- Verify webhook signature tests include valid and invalid signatures.
- Verify duplicate webhook events do not double-apply state changes.

## Manual checks

### Namespace creation
- Sign in through the web UI.
- Create a public user namespace.
- Create a second public user namespace.
- Create a private user namespace trial.
- Create a public org namespace.
- Create a private org namespace when Team entitlement/manual grant is present.
- Confirm private namespace creation is blocked or redirected to upgrade when entitlement is missing.
- Confirm visibility cannot be edited after creation.

### Org member roles
- Create an org namespace as owner.
- Add/invite another user as member.
- Confirm member can view private namespace packages.
- Confirm member can publish a package version.
- Confirm member cannot yank a package version.
- Promote member to admin.
- Confirm admin can manage members and display metadata.
- Confirm admin cannot manage billing.
- Confirm last owner cannot be removed or demoted.

### Private package publish/install
- Publish a private tool to a private user namespace as the owner.
- Publish a private agent to a private namespace as an authorized user.
- Publish a private template to a private namespace as an authorized user.
- Attempt to publish to a private namespace as an unauthorized user and confirm failure.
- Run `agentpm install` for a public package without auth and confirm success.
- Run `agentpm install` for a private package without auth and confirm safe failure.
- Run `agentpm login`, then install a private package with an authorized account and confirm success.
- Try installing the same private package with a non-member account and confirm safe failure.
- Confirm the install init response never includes private presigned URLs for unauthorized users.
- Confirm an `agent.lock` referencing a private package still requires auth.

### Search
- Search anonymously for a known public package and confirm it appears.
- Search anonymously for a known private package and confirm it does not appear.
- Search anonymously for a private namespace handle and confirm it does not appear.
- Search as an authenticated non-member and confirm private namespace/package does not appear.
- Search as an authorized member and confirm private namespace/package appears.
- Verify `type=all`, `type=tools`, `type=agents`, `type=templates`, and `type=namespaces`.
- Verify `sort=Relevance`, `sort=Newest`, `sort=Trending`, and `sort=Most downloaded` where applicable.
- Verify relaxed/fuzzy search fallback does not leak private results.
- Verify `totals_by_type` does not count unauthorized private results.
- Paginate search results with cursors and confirm private access rules continue to apply.

### Package and namespace detail pages
- Visit public namespace page anonymously and confirm it loads.
- Visit private namespace page anonymously and confirm safe not-found behavior.
- Visit private namespace page as authorized member and confirm it loads.
- Visit private package detail/readme/security/versions anonymously and confirm safe not-found behavior.
- Visit private package detail/readme/security/versions as authorized member and confirm they load.
- Verify private responses are not served from public shared cache keys.

### Billing and pricing
- Visit `/pricing` and confirm it renders Free, Pro, and Team tiers.
- Confirm pricing displays `$0`, `$7/month`, and `$19/month`.
- Confirm pricing page includes a comparison table.
- Confirm pricing page does not advertise a hard Team member cap.
- Start Pro checkout from UI and confirm redirect/session creation.
- Start Team checkout from UI and confirm redirect/session creation.
- Simulate or trigger Lemon Squeezy webhook events and confirm internal billing state updates.
- Confirm expired trial blocks private publish and private install.
- Confirm past-due paid status blocks private publish but allows private install.
- Confirm manual grant/override can restore or raise access/limits.

### Signing UI cleanup
- Open namespace management UI.
- Confirm web UI no longer has an add-signing-key flow.
- Confirm existing signers are listed.
- Revoke an existing signer and confirm it works.
- Confirm UI copy points users to CLI for adding/registering signing keys.

### Clerk identity compatibility
- Create or simulate same-email local identities with different Clerk `sub` values.
- Confirm profile/session namespace listing still shows the expected owned and member namespaces.
- Confirm same-email alias identity can view a private namespace/package it should already have access to.
- Confirm same-email alias identity can search for authorized private namespace/package results in the expected search modes.
- Confirm same-email alias identity can install from a private namespace it should already be able to access.
- Confirm same-email alias identity can publish into a namespace it should already be able to manage.
- Confirm member add by email resolves an existing AgentPM user without requiring the raw user sub.
- Confirm entitlement/trial semantics do not silently change just because same-email compatibility helpers exist.

## Expected evidence
Report back with:

- Commands run and their pass/fail status.
- Migration output or confirmation that migrations applied cleanly.
- Relevant test names added/updated.
- Screenshots or short descriptions for:
  - pricing page
  - namespace creation with public/private options
  - private package search visible to authorized user
  - private package hidden from anonymous/non-member user
  - org member management
  - signing key UI without add flow
- CLI output snippets for:
  - public install success without auth
  - private install failure without auth
  - private install success with auth
- Webhook test evidence, including idempotency check.
- Any behavior that could not be verified and why.

## Out of scope
- Full enterprise SSO/SCIM/audit-log testing.
- Usage-based billing or per-seat overage billing.
- Annual billing unless implemented as part of provider setup.
- Changing namespace visibility after creation.
- Package-level visibility toggles.
- User namespace members.
- Full email deliverability testing for invitations if Phase 5 starts with add-existing-user-by-email.
