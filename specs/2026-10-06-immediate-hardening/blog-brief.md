# Blog Brief

Milestone 10B may support a narrowly scoped engineering article about minimal
CLI telemetry: typed allowlists, anonymous installation IDs, opt-out before
state creation, per-run versus per-session measurements, and deliberately
lossy nonblocking delivery. The key tradeoff is preserving execution/exit
behavior rather than treating analytics as a reliable run ledger. Publication
must wait for the Privacy Policy and telemetry release; live dashboard
acceptance is still pending. This is implementation evidence, not a content
strategy or a draft.

## Milestone 11A Evidence

Potential later engineering material: separating authored Python requirements,
portable exact resolutions and machine-local runtime environments; pinning and
attesting the dependency engine without requiring users to adopt its project
format; retaining distribution digests across supported targets; and refusing
to publish dependency declarations before their exact state can be preserved.
M11A prepares resolution only. Do not describe clean-machine dependency
installation or execution as shipped until 11B/11C integration and its
cross-platform tests are complete. Resolver mirror deployment and live storage
retention confirmation remain operator gates. This is implementation evidence,
not a content strategy or finished article.

## Milestone 11B Evidence

Potential engineering material: one lockfile can distinguish an author's local
Tool resolution from installed package state without inventing a self-referential
package digest. Committing that resolution gives local and matrix builds the same
reviewable dependency input. Regression evidence includes real pre-11B CLI lock
snapshots, unchanged-byte writes, monotonic format versions, future-version
rejection, and mutation checks for frozen-plan and root-state loss. Publication
binds the Tool's resolution to its artifact independently of the publisher's
workspace. M11B delivers dependency metadata; managed installation/execution and
the cross-machine guarantee still need 11C. This is evidence for a later brief,
not a content strategy or an article draft.

## Milestone 11C Evidence

Potential engineering material: turning a portable dependency record into a
verified local environment without resolving again; separating artifact bytes,
portable lock state and machine-local interpreter identity; and making failed
or interrupted installation refuse execution until repaired. The dependency-
using Pydantic example connects author development with consumer install/run
and the existing Harness ToolRuntime. Local real-uv/native-wheel tests pass;
hosted platform/cross-machine and operator live acceptance remain open. Do not
describe this as a released cross-platform guarantee before those gates and the
release band are complete. This is engineering evidence for later planning,
not an article draft or a new content strategy.
