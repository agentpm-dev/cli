# Review Checklist

## Contract surfaces
- Confirm `agent.json` schema and package reference format were not changed unless explicitly justified.
- Confirm `agent.lock` format did not change unless explicitly justified.
- Confirm lockfile behavior remains reproducibility-only and does not grant private access.
- Confirm public install API response shapes remain backward compatible for public packages.
- Confirm public publish API behavior remains backward compatible for public namespaces.
- Confirm PAT scope compatibility remains intact, including existing `tools:publish` compatibility for tool packages if still intended.
- Confirm search API response shape remains compatible, with any added fields documented.
- Confirm pricing page route `/pricing` is implemented and linked where appropriate.
- Confirm docs were updated for all user-facing behavior changes.

## Correctness
- Check that namespace visibility is selected at creation and cannot be changed in Phase 5.
- Check that package visibility is inherited from namespace visibility.
- Check that user namespaces remain single-owner and do not support members.
- Check that users can create multiple user namespaces.
- Check that org namespaces support owner/admin/member roles.
- Check that members can publish but cannot yank.
- Check that admins can manage display metadata and members but cannot manage billing.
- Check that only owners can manage billing and delete namespaces.
- Check that last-owner removal/demotion is blocked.
- Check that manual grants/overrides apply through entitlement helpers, not ad hoc route logic.
- Check that expired trials block private publish and private install.
- Check that past-due paid accounts block private publish but allow private install.
- Check that canceled subscriptions block private publish/install unless manual grant applies.

## Security and privacy
- Confirm anonymous users cannot view private namespace detail.
- Confirm anonymous users cannot view private package detail, versions, readme, or security data.
- Confirm authenticated non-members cannot view private namespace/package data.
- Confirm private packages do not appear in anonymous search.
- Confirm private packages do not appear in unauthorized authenticated search.
- Confirm authorized users can search private packages in namespaces they can access.
- Confirm all search SQL variants apply the same visibility/access predicate.
- Confirm `totals_by_type` does not count unauthorized private results.
- Confirm relaxed/fuzzy search fallback does not leak private results.
- Confirm search cursor pagination re-applies authorization on every page.
- Confirm install resolve does not leak private package/version metadata.
- Confirm install init never returns private presigned URLs to unauthorized users.
- Confirm transitive dependency resolution respects private namespace access.
- Confirm publish finalize re-checks authorization and entitlement, not only publish init.
- Confirm private responses do not use shared public cache keys.
- Confirm UI-only hiding is not relied upon as authorization.

## Billing and entitlements
- Confirm application authorization uses internal billing/entitlement state rather than direct provider calls scattered through the app.
- Confirm Lemon Squeezy-specific code is isolated behind a billing provider boundary.
- Confirm webhook signature verification is implemented.
- Confirm webhook handling is idempotent.
- Confirm provider event/status mapping is explicit and tested.
- Confirm Pro enables private user namespaces.
- Confirm Team enables private org namespaces and org member access.
- Confirm configurable fair-use limits exist for Pro private user namespaces, Team private org namespaces, and Team org members.
- Confirm limits can be raised through config or manual overrides without creating a new public tier.
- Confirm pricing page does not advertise a hard Team member cap.

## Regressions
- Check anonymous public search still works.
- Check public package detail pages still work anonymously.
- Check public package readme/security/version endpoints still work anonymously.
- Check public package install still works without auth.
- Check public package publish still works for authorized namespace owners.
- Check signing mode behavior still works.
- Check signing key revoke still works.
- Check CLI login/PAT auth still works.
- Check existing package kinds `tool`, `agent`, and `template` still resolve correctly.
- Check template install/new behavior did not become recursively expanded unexpectedly.
- Check materialized view refresh still works after adding visibility columns.
- Check cache changes did not disable useful public caching unnecessarily.

## Tests and verification
- Confirm automated checks from `test-plan.md` were run or explicitly marked as not run with a reason.
- Confirm backend tests cover namespace roles and entitlements.
- Confirm backend tests cover package detail route privacy.
- Confirm backend tests cover search privacy across sorts/types/totals.
- Confirm backend tests cover install resolve/init privacy.
- Confirm backend tests cover publish role and entitlement checks.
- Confirm billing webhook tests include valid signature, invalid signature, and duplicate event/idempotency cases.
- Confirm frontend build/type/lint checks pass.
- Confirm CLI tests/checks pass if CLI behavior changed.
- Confirm manual checks include at least one unauthorized and one authorized private package install path.

## Pattern adherence
- Confirm existing repo patterns were reused before adding new abstractions.
- Confirm any new authorization helper/module is used consistently across routes/services.
- Confirm new database tables/columns follow existing naming and migration conventions.
- Confirm provider-specific billing identifiers are not spread throughout unrelated code.
- Confirm new config values are documented and have safe defaults.
- Confirm public/private labels and badges reuse existing UI components where possible.
- Confirm signing key UI cleanup removes code paths that are no longer used.

## Notes for reviewer
- This phase is easy to implement incorrectly by securing the web UI but not the CLI path. Prioritize install resolve/init and presigned URL checks during review.
- Search is another high-risk leak surface because the materialized views may include private rows. The API query layer must apply authorization every time.
- Shared caching is dangerous for private resources. Verify private namespace/package responses bypass shared public cache keys.
- Do not accept a partial implementation where private namespace creation works before install/search/detail access control is complete.
- Do not accept route-local authorization logic if it diverges from the shared namespace helper behavior.
- Treat billing gates as server-side authorization checks, not UI state.
