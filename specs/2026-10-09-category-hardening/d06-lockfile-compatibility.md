# D06 — Extend lockfile v4 with a minimum-CLI rollout

**Decision date:** 2026-11-15. **Status:** selected compatibility policy; implementation remains M5A. The user expressed alignment after the builder/reviewer comparison and supplied Claude’s final recommendation to retain v4. This records the pre-freeze decision required by the specification; it does not complete any M5A implementation task.

## Selected policy

Keep `agent.lock` version 4 for APDS identity and explicit/inferred origin metadata. Retain strict CLI field validation and the forward-version guard. Do not introduce a generic unknown-field preservation mechanism or bump to v5 for these additions.

APDS metadata requires a minimum lockfile version of **4**, whether attached to a locked package or represented local root. Extend the existing content-derived `minimum_lock_version()` calculation accordingly. Metadata-free locks can retain their existing version; APDS metadata must not be emitted under v2/v3. Do not downgrade an existing higher supported lock version during regeneration.

The exact field layout and serializers remain M5A implementation work. The conceptual `standard` and `standard_origin` fields in the spec are not newly implemented wire fields by this decision.

## Compatibility evidence and tradeoff

The installed Homebrew CLI reports **0.1.44**. Builder probes created temporary locks, ran `agentpm run @probe/noop --machine --input '{}'`, and checked lock SHA-256 before and after:

| Lock input                                | Observed CLI result                                                | Lock bytes |
| ----------------------------------------- | ------------------------------------------------------------------ | ---------- |
| v4 with an unknown top-level field        | Serde unknown-field error                                          | Unchanged  |
| v4 with `standard` on a locked package    | Serde unknown-field error                                          | Unchanged  |
| v4 with `standard_origin` on a local root | Serde unknown-field error                                          | Unchanged  |
| v5                                        | Purpose-built unsupported-version diagnostic with upgrade guidance | Unchanged  |

The unversioned Tool reference intentionally takes the locked-version lookup path. An exact-version Tool invocation can bypass lock lookup; these results do not imply that every old CLI command parses or rejects the lock.

The better v5 diagnostic does not outweigh the SDK compatibility cost for this metadata change. Both published SDK **0.1.17** readers accept unknown lock fields but reject versions outside 2/3/4. Claude independently verified that behavior against published artifacts, as reported by the user; an earlier lexicographic version-selection error was corrected before the final recommendation. The builder separately verified both local SDK 0.1.17 source/builds through public Agent loading and a real CLI-backed Node Tool call: extended v4 passed, v5 failed at Agent lock loading, and lock bytes were unchanged. The builder did not download fresh published SDK artifacts for that probe.

Therefore:

- Older CLI operations reading the extension reject it instead of silently rewriting away metadata. The already-published diagnostic is less helpful; documentation must explain it and name the compatible CLI release.
- Existing read-only SDK loaders can continue consuming supported content with the additive v4 fields. This does not grant old clients new APDS validation or Runner capabilities, or guarantee support for future runtime constructs.
- A v5 bump would require SDK version-reader updates and coordinated SDK publication for a change they can already tolerate as v4 metadata. Both choices still require preservation through regeneration; v5 does not remove that work.

SDK tolerance of additional fields **while rejecting unsupported versions** is a load-bearing compatibility assumption. M5A must test it for both SDKs. Future SDK schema tightening must preserve this compatibility or explicitly revisit this decision before release. A future change whose required semantics cannot safely be ignored may justify a new format version; this is not a promise that every future addition fits v4.

## Metadata emission and legacy inference

Write fields when the operation has a supported effective standard identity and a justified origin to record. Do not populate fields with null placeholders, invent a declaration, or rewrite an old lock solely to decorate it with APDS metadata.

**Record `legacy_inferred` when the supported historical compatibility policy actually infers a standard.** M5A already requires retaining inferred-versus-explicit provenance. Do not suppress that distinction merely to avoid an older CLI error, and never convert inference into authored declaration or verified conformance.

Consequently, a normal install/regeneration that records inferred metadata for a legacy package can produce an extended v4 lock. This is not necessarily a conscious user opt-in. Existing untouched locks remain unchanged; newly computed metadata follows the declared rollout floor. When evidence does not justify an identity/origin, do not manufacture one to fill a field.

Cover both installed package entries and local roots represented by the lock:

- Installed-package metadata flows through Registry resolve DTOs and the CLI plan and is reconciled with the downloaded embedded manifest and package identity/integrity.
- Local-root metadata comes from the actual authored manifest and the applicable explicit/legacy interpretation. It cannot be supplied solely by Registry responses. Refresh it from that source during regeneration rather than blindly copying stale root metadata.

The lock remains portable. This decision does not add selected host target/interpreter state, change package keys or redefine the existing integrity digest.

## Minimum CLI and rollout gate

The minimum compatible CLI is **the first released CLI that implements M5A’s APDS-aware v4 reader and writer**. No current version is falsely designated as supporting fields it cannot read. Record the concrete release number in implementation/release documentation before enabling emission.

Ship and verify that reader before examples, automation or shared-workspace workflows produce the extended locks. Publish actionable migration guidance explaining the older unknown-field error and upgrade command/version. Existing SDK 0.1.17 must not need a release solely to accept these added fields; any separately planned SDK capability changes retain their own compatibility and publication requirements.

The already-shipped CLI cannot be patched retroactively to display a better error. Do not claim that an added minimum-version field would make those old strict readers understand the requirement.

## M5A acceptance obligations

Implementation must prove:

1. New readers accept existing v2/v3/v4 fixtures; unchanged writes preserve historical bytes.
2. Known APDS fields on packages and local roots require v4 and round-trip with their origin intact. Other unknown fields and future versions still fail safely.
3. Ordinary install regeneration retains metadata through both the resolve-plan and local-root paths, including changed root identity/declaration and legacy inference. Missing evidence never becomes explicit or verified.
4. Frozen/clean reinstall preserves exact identities, graph closure, integrity and provenance without adding host-specific lock state.
5. An actual older published CLI rejects extended-v4 reads without changing lock bytes. Document its real diagnostic rather than promising a newer error from an old binary.
6. Both SDK 0.1.17 loaders tolerate the package/root additions and retain their read-only behavior; representative public loading/invocation paths work with the intended compatible CLI.
7. The concrete minimum CLI release, reader-before-emission ordering and legacy-metadata blast radius are documented and verified before rollout.

The proposed generic-extension lane and v5 alternative were evaluated and not selected. The implementation and exact serializer shape still require M5A review; the pre-freeze compatibility policy is settled.

## M5A implementation evidence — 2026-11-20

The selected pair is optional `standard: {id, version}` plus `standard_origin: "declared" | "legacy_inferred"`, aligned with Registry provenance (D19). The illustrative `explicit` label in the original conceptual snippet is not emitted. Both fields travel together; unsupported identities, partial pairs and unknown origins fail. Both installed packages and represented local roots require v4 when recording the pair. No null placeholders or selected host/interpreter state are emitted.

CLI **0.1.47** is prepared as the first compatible reader/writer, **not yet released**. Existing metadata-free v2/v3/v4 locks still read; existing higher supported versions are retained during regeneration. Reader-first publication/installability and developer/CI/container upgrades precede shared emission in Release Band 2. The Registry can add response fields first, since older response DTO readers ignore them and new CLI downloads also reconcile older responses. Ordinary legacy installation can produce an extended lock and require the CLI upgrade, without a new install flag.

[Verification](m5a-verification.md) includes actual fresh npm/PyPI 0.1.17 public Agent/Skill loading and real CLI-backed invocation, byte-identical locks, SDK v5 rejection, and released Homebrew CLI 0.1.46 unknown-field rejection without mutation. Reconciliation happens before extraction replacement, including the installed-cache skip path; old-Registry URL renewal retains expected pinned metadata. No APDS validation certificate is serialized. User confirmed independent Claude review passed with no remaining findings on 2026-11-20. User supplied the passing 38-result operator-run manual audit on 2026-11-20. User subsequently confirmed all hosted CI passed on 2026-11-20, completing M5A. Reader-first release verification and SDK CI ref cleanup remain Release Band 2 gates.

## M5B replay evidence — 2026-11-20

M5B retains v4 and introduces no portable fields. Frozen replay restores the whole exact locked package set, checks authored selectors and recorded root edges, and never calls Registry resolution or rewrites the lock. It preserves multiple locked versions and validates transitive requirements against verified manifests before replacing installed packages. Older v1 Tool and metadata-free v2/v3 fixtures retain their bytes; unsupported/malformed inputs fail safely. This changes neither the M5A minimum CLI 0.1.47 reader-first rollout nor published SDK 0.1.17 compatibility. [Verification](m5b-verification.md) records the implementation, I/O tradeoff and tests. User confirmed independent Claude review passed with no findings on 2026-11-20. User confirmed all hosted CI passed on 2026-11-20. The user subsequently supplied the passing 51-result real-local manual audit, completing M5B. Reader-first rollout and the remaining Release Band 2 milestones are not closed.
