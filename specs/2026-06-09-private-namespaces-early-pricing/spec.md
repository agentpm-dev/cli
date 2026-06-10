# Feature
Phase 5: Private Namespaces + Early Pricing

## Problem / Goal
AgentPM currently behaves primarily as a public package registry. The backend data model already has early support for namespace `kind` (`user` / `org`), namespace `visibility` (`public` / `private`), and package `kind` (`tool` / `agent` / `template`), but the product and authorization paths do not consistently enforce private namespace behavior yet.

Phase 5 turns AgentPM into a team-ready private registry while keeping the public ecosystem free. A solo developer or team should be able to create a private namespace, publish private tools/agents/templates into it, control who can access the namespace, and install those private packages securely through the CLI.

The business goal is to validate paid private usage without overbuilding enterprise billing/admin systems.

Product thesis:

> AgentPM is the internal registry and packaging layer for your team's tools, agents, templates, and interoperability workflows.

## Current context / code assumptions
Codex should not assume any prior conversation. Use this section as the implementation context.

### Existing database shape
The current database already includes:

- `namespaces.kind`, constrained to `user | org`.
- `namespaces.visibility`, constrained to `public | private`, defaulting to `public`.
- `namespaces.owner_user_sub` referencing `users.sub`.
- `tools.kind`, constrained to `tool | agent | template`.
- `tools.namespace_id` referencing `namespaces.namespace_id`.
- `personal_access_tokens` for CLI/CI auth.
- `namespace_signers` for namespace signing keys.

Although the physical tables and some route/service names still say `tools`, Phase 5 should treat rows in `tools` as packages/artifacts with `kind = tool | agent | template`.

### Existing backend gaps to address
The current `NamespacesService.create_namespace` ignores requested `kind` and `visibility`, always creates a public user namespace, and blocks a user from having more than one user namespace. Phase 5 must remove those limitations.

The current publish helper `_can_publish_to_namespace(ns, user_sub)` allows only owners of `user` namespaces and returns `False` for org namespaces. Phase 5 must replace this with shared namespace authorization helpers that understand org membership and roles.

The current install resolve route does not use auth context. It should become `auth_optional` and pass `current_user_sub` into resolution.

The current install init route accepts optional auth but exact package resolution does not check namespace visibility/access before issuing presigned artifact URLs. Phase 5 must fix this.

The current search route is unauthenticated and `SearchService` SQL queries read from `tool_search_index` and `namespace_search_index` with no namespace visibility/access predicate. Phase 5 must make search auth-aware and prevent private namespace/package leakage.

The current namespace and package detail routes use public shared cache keys. Phase 5 must not cache private namespace/package responses using shared public cache keys.

### Current search indexes
`tool_search_index` should be updated to include:

```sql
ns.visibility AS namespace_visibility
```

`namespace_search_index` should be updated to include:

```sql
n.visibility AS namespace_visibility
```

The materialized views may contain private rows, but no public query may return those rows without an authorization predicate.

Recommended index additions:

```sql
CREATE INDEX IF NOT EXISTS tool_search_visibility_idx
  ON public.tool_search_index (namespace_visibility, namespace_id);

CREATE INDEX IF NOT EXISTS namespace_search_visibility_idx
  ON public.namespace_search_index (namespace_visibility, namespace_id);
```

### Manifest compatibility
The existing manifest schema already supports `kind: agent | tool | template` and package references like `@namespace/name`. Phase 5 should not require manifest schema changes.

## Non-goals
- Do not add package-level public/private visibility. Package visibility is inherited from namespace visibility.
- Do not support changing namespace visibility after creation in Phase 5.
- Do not support members on user namespaces in Phase 5.
- Do not add a `viewer` role in Phase 5.
- Do not implement advanced enterprise features such as SSO, audit logs, SCIM, custom contracts, invoice workflows, or sales-assisted enterprise plans.
- Do not implement usage-based billing, per-install billing, per-artifact billing, or metered storage billing.
- Do not change the `agent.json` manifest schema or package reference format.
- Do not let `agent.lock` grant access to private packages.
- Do not implement hard advertised team member caps on the pricing page.
- Do not overbuild a generic entitlement engine if a smaller provider-neutral internal model is sufficient.

## Constraints / Invariants

### Product invariants
- Namespace visibility is authoritative for package visibility.
- Public namespace packages are public.
- Private namespace packages are private.
- Both user and org namespaces can be public or private.
- User namespaces are single-owner in Phase 5.
- Org namespaces support members and roles.
- Public ecosystem usage is free.
- Private solo usage is paid via Pro, trial, or manual grant.
- Private org/team usage is paid via Team, trial, or manual grant.
- Public org namespaces are allowed.
- Visibility is selected at namespace creation and cannot be changed through UI or API in Phase 5.

### Security invariants
- UI hiding is not authorization.
- Server-side checks must enforce every private namespace/package access decision.
- Private namespace artifacts must never appear in anonymous search.
- Private namespace artifacts must never appear in unauthorized authenticated search.
- Private package detail routes must not leak metadata to unauthorized users.
- Private namespace routes must not leak metadata to unauthorized users.
- Private install resolve must not leak private package/version metadata to unauthorized users.
- Private install init must not issue presigned artifact URLs to unauthorized users.
- A lockfile referencing private packages does not grant access.
- Publish to a private namespace requires both role permission and an active entitlement/trial/manual grant.
- Billing/trial checks must be enforced server-side.
- S3 object privacy must rely on backend-issued presigned URLs after authorization.
- Existing public package behavior must remain backward compatible.

### Lockfile invariant
`agent.lock` records reproducibility. AgentPM auth controls access.

A user with an `agent.lock` that references private packages still needs valid AgentPM auth with access to the namespace before install/resolve/download can succeed.

### Cache invariant
Public namespace/package responses may use existing shared public cache keys.

Private namespace/package responses must bypass shared public caching in Phase 5. This applies to:

- namespace detail
- namespace packages
- namespace activity
- package detail
- package versions
- package readme
- package security

### Unauthorized response behavior
For private package/namespace discovery routes, prefer safe not-found behavior over explicit forbidden behavior when the requester is not authorized to know the resource exists.

Examples:

- Anonymous or unauthorized user attempts to view private package: return `404` or a safe not-found-style error.
- Authenticated namespace member attempts an action their role does not allow, such as a member yanking a package: return `403`.
- CLI install can use a helpful non-leaking message such as: `Package not found or you do not have access. If this is a private package, make sure you are logged in with an AgentPM account or PAT that has access to the namespace.`

## Namespace model

### Namespace kinds
Supported kinds:

```text
user
org
```

Supported visibility:

```text
public
private
```

Valid combinations:

```text
user + public
user + private
org + public
org + private
```

### User namespaces
User namespaces are for solo ownership.

Rules:

- Owned by one user via `owner_user_sub`.
- No additional members in Phase 5.
- Users may create multiple user namespaces.
- Public user namespaces are free.
- Private user namespaces require Pro, Team, active trial, or manual grant.
- Pro should allow multiple private user namespaces.
- Use a configurable soft cap for private user namespaces, defaulting to `PRO_MAX_PRIVATE_USER_NAMESPACES_DEFAULT=10` unless implementation chooses a different value.

### Org namespaces
Org namespaces are for team/shared ownership.

Rules:

- Owned/administered by users.
- Support members.
- Support roles: `owner`, `admin`, `member`.
- Can be public or private.
- Public org namespaces are free to support public ecosystem and open-source/community usage.
- Private org namespaces require Team, active trial, or manual grant.
- Use configurable fair-use limits for org members and private org namespaces. Do not advertise a hard member cap on the pricing page.
- Suggested defaults:
  - `TEAM_MAX_ORG_MEMBERS_DEFAULT=25`
  - `TEAM_MAX_PRIVATE_ORG_NAMESPACES_DEFAULT=5`

## Roles and permissions

Org roles:

```text
owner
admin
member
```

User namespaces do not have members in Phase 5; the owner has owner-equivalent permissions.

| Capability | Owner | Admin | Member |
|---|---:|---:|---:|
| View private namespace | Yes | Yes | Yes |
| Search private packages | Yes | Yes | Yes |
| Install private packages | Yes | Yes | Yes |
| Publish package versions | Yes | Yes | Yes |
| Yank package versions | Yes | Yes | No |
| Change display metadata | Yes | Yes | No |
| Manage members | Yes | Yes | No |
| Change billing / plan | Yes | No | No |
| Delete namespace | Yes | No | No |
| Change visibility | No, not in Phase 5 | No | No |

Additional role rules:

- Owners alone can manage billing.
- Owners alone can delete namespaces.
- Admins can manage display metadata and members.
- Members can publish package versions.
- Members cannot yank package versions.
- Visibility cannot be changed after creation by any role in Phase 5.
- Do not allow the last owner to be removed from an org namespace.
- Admins must not be able to remove or demote the only owner.

## Authorization helpers
Introduce or centralize namespace authorization helpers. Avoid scattering private namespace checks across unrelated services.

Suggested helper surface:

```text
can_view_namespace(user_sub | None, namespace) -> bool
can_manage_namespace_metadata(user_sub, namespace) -> bool
can_manage_namespace_visibility(user_sub, namespace) -> bool  # returns false in Phase 5 after creation
can_publish_to_namespace(user_sub, namespace) -> bool
can_yank_from_namespace(user_sub, namespace) -> bool
can_install_from_namespace(user_sub | None, namespace) -> bool
can_manage_namespace_members(user_sub, namespace) -> bool
can_manage_namespace_billing(user_sub, namespace) -> bool
can_create_namespace(user_sub, kind, visibility) -> bool / entitlement result
can_use_private_namespace(namespace, operation) -> bool / entitlement result
```

Use these helpers in:

- namespace routes
- package detail routes
- search service
- publish init/finalize
- install resolve/init
- signer routes
- member routes
- billing routes

## Search behavior

Search must become auth-aware.

### Anonymous search
Anonymous search returns only:

- public namespaces
- public tools
- public agents
- public templates

Anonymous search never returns:

- private namespaces
- private packages
- private package metadata
- private snippets from manifests/descriptions/readmes

### Authenticated search
Authenticated search returns:

- public results
- private results from namespaces the authenticated user can access

Authorized private results should be labeled as private in the UI/API payload where useful.

### Search implementation requirements
- Add `@auth_optional` to the search route.
- Pass `current_user_sub` into `SearchService.search`.
- Update search SQL to include an access predicate for both tools/packages and namespaces.
- Add `namespace_visibility` to `tool_search_index` and `namespace_search_index`.
- Ensure `totals_by_type` respects the same access filtering.
- Ensure relevance, relaxed fallback, newest, trending, and most-downloaded queries all apply visibility/access filtering.
- Ensure search pagination/cursors do not leak private results across auth states. At minimum, every paginated query must re-apply the access predicate. Prefer resetting cursor history when the auth identity changes or including a viewer/access marker in cursor identity.

Recommended predicate shape:

```sql
WHERE (
  s.namespace_visibility = 'public'
  OR s.namespace_id IN (:authorized_namespace_ids)
)
```

For namespace search:

```sql
WHERE (
  n.namespace_visibility = 'public'
  OR n.namespace_id IN (:authorized_namespace_ids)
)
```

Implementation may instead use joins/subqueries against membership tables, as long as the same behavior is enforced.

## Install / CLI behavior

Private namespaces are not secure unless the CLI install path enforces access.

### Public package install
Existing behavior should continue:

- No auth required.
- Public packages resolve and install normally.
- Lockfile behavior remains unchanged.

### Private package install
Private packages require auth.

Expected flow:

```bash
agentpm login
agentpm install
```

Or PAT-based auth in CI.

Requirements:

- `/install/resolve` must use optional auth and receive `current_user_sub`.
- `/install/init` must continue using optional auth and must enforce private namespace access before issuing presigned URLs.
- Dependency expansion must enforce private access for transitive dependencies too.
- Private package resolution should return safe not-found behavior for unauthorized users.
- Public package resolution must remain backward compatible.
- A lockfile referencing private packages must not bypass authorization.

## Publish behavior

Publishing must enforce role permissions and entitlement status.

### Public namespaces
Publishing to public namespaces remains free when the user has publish permission.

### Private user namespaces
Allowed when:

- namespace kind is `user`
- namespace visibility is `private`
- authenticated user owns the namespace
- namespace/user has private user namespace entitlement via Pro, Team, active trial, or manual grant
- existing validation/scanning/signing rules pass

### Private org namespaces
Allowed when:

- namespace kind is `org`
- namespace visibility is `private`
- authenticated user is owner/admin/member
- namespace/org has Team, active trial, or manual grant
- existing validation/scanning/signing rules pass

### Publish gating
If a private namespace is expired, past due, or otherwise not entitled, publishing must be blocked.

More specific billing behavior:

- Expired private trial: block private publishing and private install.
- Past-due paid account: block private publishing, but allow private install for now.
- Canceled private subscription: block private publishing and private install unless a manual grant is active.
- Manual grant: behavior follows the grant/override settings.

## Entitlements and plan model

Use a provider-neutral internal entitlement model. Lemon Squeezy should update internal billing/subscription state; application authorization should depend on AgentPM internal state, not direct provider-specific checks scattered through the app.

Suggested internal concepts:

```text
plan = free | pro | team
billing_status = none | trialing | active | past_due | canceled | manually_granted
billing_provider = lemon_squeezy | manual | null
billing_customer_id = nullable
billing_subscription_id = nullable
trial_ends_at = nullable
current_period_end = nullable
```

Derived checks should answer:

- Can this user create a private user namespace?
- Can this namespace remain private?
- Can this user publish private versions here?
- Can this user install private packages from here?
- Can this org namespace add another member?
- Can this namespace use Team features?

### Trial
Phase 5 should include one limited private user namespace trial.

Suggested default:

```text
Trial duration: 14 days
```

Rules:

- Each user can create one private user namespace trial.
- Trial is single-owner only.
- Publishing is allowed during the trial.
- Installing is allowed during the trial.
- When the trial expires, private publish and private install are blocked until upgrade/manual grant.
- Namespace and package data are not deleted when the trial expires.

### Configurable limits and overrides
The pricing page should not advertise a hard Team member cap, but the backend should enforce configurable fair-use limits.

Suggested defaults:

```text
PRO_MAX_PRIVATE_USER_NAMESPACES_DEFAULT=10
TEAM_MAX_PRIVATE_ORG_NAMESPACES_DEFAULT=5
TEAM_MAX_ORG_MEMBERS_DEFAULT=25
```

These should be configurable without code changes where practical.

Support manual overrides for early customers/pilots/open-source grants. The exact data model can be simple, but it must support raising limits for a specific namespace/account without introducing a new public tier.

Possible override fields/table:

```text
max_members_override nullable integer
max_private_namespaces_override nullable integer
private_publish_allowed_override nullable boolean
private_install_allowed_override nullable boolean
manual_grant_reason nullable text
```

Use the data model that best fits the existing codebase.

## Pricing

Implement a real `/pricing` page. The current route shows a generic 404 and must be replaced.

Pricing tiers:

| Tier | Price | Positioning |
|---|---:|---|
| Free | $0 | Public packages and open ecosystem usage |
| Pro | $7/month | Private packages for solo developers |
| Team | $19/month | Private namespaces and package access for teams |

Pricing page requirements:

- Include cards for Free, Pro, and Team.
- Include a tier comparison table.
- Do not advertise a fixed Team member cap.
- Include private namespace trial messaging.
- Make the product distinction clear:
  - Free = public registry
  - Pro = private solo namespaces
  - Team = private org namespaces and team access
- Show “early pricing” or equivalent copy if desired.
- Include CTAs for current auth state where practical.
- Pricing page can launch monthly-only in Phase 5.

Recommended feature comparison:

| Feature | Free | Pro | Team |
|---|---:|---:|---:|
| Public namespaces | Yes | Yes | Yes |
| Public tools, agents, templates | Yes | Yes | Yes |
| Public publishing | Yes | Yes | Yes |
| Public installs | Yes | Yes | Yes |
| Private user namespace trial | Yes | Yes | Yes |
| Private user namespaces | Trial only | Yes | Yes |
| Multiple private user namespaces | No | Yes | Yes |
| Private org namespaces | No | No | Yes |
| Team member access | Public only | Public only | Yes |
| Owner/admin/member roles | No | No | Yes |
| Private publishing | Trial only | Yes | Yes |
| Private installs | Trial only | Yes | Yes |

## Billing provider
Use Lemon Squeezy as the first Merchant of Record provider, but isolate provider-specific code behind a small boundary.

Lemon Squeezy notes for implementation:

- Lemon Squeezy presents itself as a Merchant of Record for software companies and says it handles payments, fraud, sales tax, and compliance responsibilities for purchases.
- Lemon Squeezy provides API and webhook docs, including subscription/event synchronization.
- Implement webhook signature verification and idempotent event handling.

Provider boundary should look conceptually like:

```text
create_checkout(...)
create_customer_portal_session(...)
handle_webhook(...)
sync_subscription_status(...)
```

Application code should consume internal helpers:

```text
has_active_private_user_entitlement(...)
has_active_private_org_entitlement(...)
can_publish_private(...)
can_install_private(...)
can_add_org_member(...)
```

Do not scatter Lemon Squeezy-specific product/variant IDs throughout unrelated services.

## UI behavior

### Profile namespace panel
Update the existing namespace management UI:

- Remove the “only show create namespace button if user has no user namespace” behavior.
- Enable creating multiple user namespaces.
- Enable `kind=user` and `kind=org` creation.
- Enable `visibility=public` and `visibility=private` selection.
- Show pricing/trial/upgrade messaging when private is selected.
- Show private/public badges.
- Show namespace kind.
- Support display metadata management for owner/admin.
- Do not support visibility changes after creation.

### Org member management
Add UI/API for org namespace members:

- List members.
- Show role.
- Add/invite member.
- Change role.
- Remove member.
- Enforce owner/admin/member permissions.
- Prevent last-owner removal.

A full email invitation system is preferred but not mandatory if implementation complexity is high. A simpler first version may add existing AgentPM users by email, but the UX and docs must be clear.

### Signing UI cleanup
Remove web UI creation of signing keys from namespace management.

Keep:

- signing mode selection
- signing key list
- signing key revoke button

Remove:

- `+ Add` signing key UI
- public key textarea flow
- web UI signing key creation handler

Add copy telling users to add signing keys through the CLI. Update docs accordingly.

## API behavior

### Namespace routes
- `GET /namespaces/:namespace_id` should return public namespaces anonymously and private namespaces only to authorized users.
- `GET /namespaces/:namespace_id/tools` should return public namespace packages anonymously and private namespace packages only to authorized users.
- `GET /namespaces/:namespace_id/activity` should follow the same access rules.
- Private namespace responses must not use shared public cache keys.
- Create namespace route must accept and validate `kind` and `visibility`.
- Patch namespace route must allow owner/admin to update display metadata and signing mode as allowed.
- Patch namespace route must not allow visibility changes in Phase 5.

### Package detail routes
Apply visibility/access checks to all package detail endpoints, including:

- package detail
- package versions
- package readme
- package security

Private package responses must not use shared public cache keys.

### Search route
- Add optional auth.
- Pass current user into service.
- Filter results by access.

### Publish routes
- Continue enforcing PAT scopes.
- Enforce namespace role permissions.
- Enforce private namespace entitlement before reserving uploads or finalizing publish.
- Ensure finalize re-checks permission and entitlement, not only init.

### Install routes
- Add optional auth to resolve.
- Enforce access in resolve and init.
- Never issue private artifact presigned URLs without authorization.

### Billing routes
Add minimal provider-neutral routes as needed:

- create checkout for Pro
- create checkout for Team
- open/manage billing portal if provider supports it
- receive Lemon Squeezy webhooks

## Acceptance criteria

### Namespace model
- A signed-in user can create multiple user namespaces.
- A signed-in user can create org namespaces.
- A namespace can be created as public or private.
- Namespace visibility cannot be changed after creation through UI or API.
- User namespaces do not allow members.
- Org namespaces allow members with owner/admin/member roles.

### Authorization
- Anonymous users can view/search/install public packages.
- Anonymous users cannot view/search/resolve/install private packages.
- Authenticated non-members cannot view/search/resolve/install private packages from namespaces they do not belong to.
- Authorized private namespace members can view/search/resolve/install private packages.
- Members can publish to org namespaces they belong to.
- Members cannot yank package versions.
- Owners/admins can yank package versions.
- Owners/admins can manage org members.
- Only owners can manage billing.

### Install / CLI
- Public package install remains compatible.
- Private package install succeeds when CLI/PAT auth has namespace access.
- Private package install fails safely without auth.
- Private package install fails safely for authenticated non-members.
- `agent.lock` does not allow unauthorized private installs.
- Install init never returns private presigned URLs to unauthorized users.

### Search
- Anonymous search returns only public namespaces/packages.
- Authenticated search returns public results plus private results from authorized namespaces.
- Search totals and cursors respect access filtering.
- Private packages do not leak through relaxed relevance fallback, trending, newest, most-downloaded, or totals-by-type.

### Billing / entitlements
- Free users can use public namespace/package flows.
- Free users can start one private user namespace trial.
- Pro unlocks private user namespaces.
- Team unlocks private org namespaces and org member access.
- Expired trial blocks private publishing and private install.
- Past-due paid subscription blocks private publishing but allows private install.
- Manual grants/overrides can enable or extend access/limits.
- Configurable fair-use limits are enforced server-side.

### Pricing page
- `/pricing` renders a real page, not a 404.
- Pricing page shows Free, Pro, and Team tiers.
- Pricing page shows `$0`, `$7/month`, and `$19/month`.
- Pricing page includes a comparison table.
- Pricing page does not advertise a hard Team member cap.

### Signing UI cleanup
- Web UI no longer allows adding namespace signing keys.
- Existing signing keys can still be listed and revoked.
- Signing mode can still be managed by authorized users.
- Docs point users to CLI for adding signing keys.

## Risks / edge cases
- Private namespace data leakage through cached public responses.
- Private package leakage through search indexes or relaxed search fallback.
- Private package leakage through install resolve, transitive dependency expansion, or install init presigned URLs.
- Search cursor pagination with changed auth state may produce confusing results if not reset or filtered carefully.
- Publish init and finalize can drift if only one side checks entitlement/role.
- Existing public routes may accidentally become auth-required and break public install/search behavior.
- Existing PATs with `tools:publish` compatibility must continue working for tool publishes where intended.
- Manual grants and plan overrides can become inconsistent if entitlement helpers are scattered.
- Org member removal/role changes can remove the last owner if not guarded.
- Past-due/canceled/trial-expired states may be confusing unless error messages are explicit for namespace owners.
- Lemon Squeezy webhook retries require idempotent event handling.
- Provider-specific IDs and status mapping can leak into unrelated code if no billing boundary is kept.

## Open questions
- Should the private user namespace trial be exactly 14 days, or should the duration be configurable?
- Should org private trials exist in Phase 5, or only private user namespace trials?
- Should adding members to public org namespaces remain fully free, or should this be gated later?
- Should the first member-add flow support email invitations, or only adding existing users by email?
- Should `owner_user_sub` remain the canonical owner while also inserting an owner row into membership for org namespaces?
- What exact internal data model should hold billing/subscription state: user/account-scoped, namespace-scoped, org-scoped, or a hybrid?
- What exact manual grant override mechanism best matches the existing admin/ops patterns?
- Should private namespace pages return 404 for all unauthorized cases, or 401 for anonymous users on some API routes?

## Related Specs
- Phase 1: Public package publishing/install baseline.
- Phase 2: CLI install/runtime/package flow.
- Phase 3: Signing, scanning, and registry attestation.
- Phase 4: Workflow templates and `kind=template` package behavior.
- Existing auth/PAT implementation.
- Existing search implementation and materialized view migrations.
- Existing namespace/profile UI.
