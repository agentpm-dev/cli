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

## Milestone 12 Evidence

Potential engineering material: keeping dependency portability distinct from
the files in a Tool archive. A Tool can declare native-wheel dependencies while
shipping portable Python source; bundling a native library changes that payload
claim. Classification inspects completed archive bytes, including disguised
binaries and binary suffixes after text, and deliberately chooses the current
target when uncertain. This is package-time metadata only: do not describe
Registry target selection, multi-artifact publication or cross-platform hosted
verification as delivered by this change. Those rollout and acceptance gates
remain separate. This is implementation evidence for later planning, not an
article draft.

M12 format-policy follow-up evidence: the portable inventory is now one
embedded JSON contract, with generated/CI-checked spec tables and data-driven
tests plus independent binary fixtures. A mismatch fails on the temporary
artifact before replacement/upload, while typed diagnostics preserve the
specific file/reason for future CI consumers. Dotenv prefix matching and
intentional egg metadata are distinct from environment snapshots and native
payloads. Directory exclusions remain a separate policy decision; do not claim
automatic project-clutter removal or hosted acceptance from local tests.

M12 collection follow-up evidence: explicit file/directory intent is resolved
before incidental traversal, with a deliberately small shared policy, no Git
ignore interpretation and no symlink-target traversal. Marker-based environment
pruning avoids copying custom-named environments; explicit roots can preserve
them deliberately. Informational omission/override records remain distinct from
compatibility warnings, including an `any` artifact carrying collection records.
Real archive/CLI tests cover a 15,001-file environment and publication metadata;
fresh hosted/operator acceptance remains open. This supersedes the earlier
statement about collection remaining deferred, without claiming release or
production deployment.
