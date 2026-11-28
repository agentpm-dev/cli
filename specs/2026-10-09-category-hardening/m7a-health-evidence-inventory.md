# Package Health evidence inventory

Builder audit, 2026-11-22. This is the M7A implementation/review artifact for
T-HEALTH-01/03, D12/D19 and the evidence-design portion of S2-AC-10. It is not
part of APDS, a public Health API, a certification scheme or a change to existing
Security presentation. It lives in the public CLI repository and is therefore
readable there; sibling Registry/web source references require workspace access.

## Source snapshot and decisions

Inspected source revisions before this work:

| Repository | Revision                                   |
| ---------- | ------------------------------------------ |
| CLI        | `5c1ef72022e2c301c2cd9823de2bb173338daf26` |
| Registry   | `984daa130e2278ad885cf2dc858be92f5a3ac323` |
| Web        | `2f9145257b769dfba67ad6cecd20cec68039a462` |

**D12 — selected design:** one shared evidence record with kind-specific facts,
assembled from existing version, artifact, provenance and scanner sources. Reuse
Security details and verification routines. Do not duplicate integrity math,
create per-kind evidence tables, or add a universal score. Alternatives rejected:
eight unrelated Health calculations; treating every nonempty Security field as
a passed check. M7B owns the actual DTO, aggregation and privacy implementation;
M10B owns presentation. This document does not settle tab placement.

**D19 — selected vocabulary:** retain wire origins `declared` and
`legacy_inferred`, existing APDS statuses and exact validation levels. Display
copy may explain an explicit declaration, but must not introduce an `explicit`
wire origin or turn inference into conformance. Missing evidence stays absent.
An existing `runner_evaluated: false` remains false.

Stage 1 M14C/Release Band 11 is recorded as deployed in
[`../2026-10-06-immediate-hardening/tasks.md`](../2026-10-06-immediate-hardening/tasks.md).
The later [Release Band 2 verification](release-band2-verification.md) includes
released-CLI signed/attested installs. Thus root-authenticated verification is
available, not hypothetical. Those dated observations are not a fresh verification
of every release or the current key-set expiry. This audit makes no production
requests and cannot issue a new production-wide trust verdict.

**Operator follow-up:** the retained production set expires
`2026-12-07T22:31:48Z`. Reconfirm and renew the served root-signed key set before
then using the existing Stage 1 procedure, retaining historical verification
keys. Renewal is distinct from rotating root/attestation keys and must not wait
for M7B delivery. M7B's checklist also requires an expired-key-set Health/cache
fixture: loss of current trust becomes unknown/not evaluated, not a Health
regression or proof of invalid package bytes. Strict CLI verification remains
fail-closed. This audit has not renewed or re-inspected the production set.

## Authority and scope

- **Identity:** package/namespace identity, publisher association, stars and
  aggregate installs. The physical `tools` table contains all eight kinds.
- **Release:** the selected published `tool_versions` row, not necessarily latest.
  Every evidence record must retain package kind/name and selected version.
- **Artifact:** a specific release artifact/target with its full digest; for a
  historical single-archive release, that archive. Do not apply one target's scan
  or download check to the whole release.
- **Local environment:** an observed Manager/Runner installation, interpreter,
  provider, credentials and configuration. Anonymous Registry metadata cannot
  supply local readiness.

In the matrix, `V` means `app/tools/domain/tools/entities.py::PackageVersion`
(physical `tool_versions`); `A` means `PackageVersionArtifact`
(`package_version_artifacts`); `S` means `PackageVersionScan`
(`tool_version_scans`). Paths starting `app/` are in `agentpm-api`.
`package_version_id` is the ORM alias of physical `tool_version_id` where noted
by those models. No new database columns are proposed here.

## Signal-to-source matrix

| Signal / applicability                                | Scope                                                                               | Exact source and operation                                                                                                                                                                                                                                                                                     | Legitimate claim and missing-evidence boundary                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `package_identity`, all kinds                         | Identity                                                                            | `entities.py::Package`: `package_id`/physical `tool_id`, `namespace_id`, `name`, `kind`; `Namespace.handle`; `app/tools/application/tools.py::_get_accessible_tool_context`                                                                                                                                    | Observed identity with namespace visibility enforced. Publisher/namespace association is not proof of authorship or safety. Version-specific facts must not be cached only by identity.                                                                                                                                                                                                                                                                             |
| `apds_declaration`, all kinds                         | Release                                                                             | `V.manifest.standard`; `V.standard_id`, `standard_version`, `standard_origin`; `app/tools/application/publish_apds.py::version_fields`; `app/tools/application/install_apds.py::metadata`                                                                                                                      | `declared`: observed declaration, with support checked separately. Columns are a projection; reconcile with the selected manifest. A declaration published with enforcement off may have NULL validation; observing it alone does not establish support. Unsupported explicit selectors remain unsupported, never inferred. Partial/conflicting metadata is unknown/inconsistent pending investigation.                                                             |
| `apds_conformance`, all kinds                         | Release and evaluated graph/artifact context                                        | `V.apds_validation`; `publish_apds.validate_snapshot` runs `app/apds/engine.py::evaluate` at standalone, packaged and resolved levels; `app/tools/application/release_publish.py::_store_finalization_objects` checks targets and adds `artifact_targets`                                                      | A recorded validator outcome at the reported levels. Keep status, issues, evaluated/unevaluated rules and `runner_evaluated`. `incomplete` resolved evidence stays incomplete even when standalone passed. NULL means not evaluated/no retained result. Never infer passing from successful publication, signatures or a supported selector.                                                                                                                        |
| `apds_origin`, historical releases                    | Release                                                                             | `app/tools/application/apds_history.py::inspect_history`/backfill; `install_apds.metadata`; three `V.standard_*` columns                                                                                                                                                                                       | `legacy_inferred`: compatibility interpretation, not an authored declaration or verified conformance. Backfill leaves `V.apds_validation` NULL and immutable bytes unchanged. Do not retrospectively certify history. Missing standard is the narrow historical exception; unrelated validator issues remain visible.                                                                                                                                               |
| `release_digest`, new-format releases                 | Release                                                                             | `V.release_format`, `release_sha_256`, `release_manifest`; `app/tools/domain/tools/releases.py::ReleaseManifest.digest`/`digest_hex`; `release_storage.validate_release_inputs`                                                                                                                                | Full SHA-256 of the canonical logical release document binding manifest/resolution/artifact inventory. State whether merely recorded or actually recomputed and matched. It is not the tar hash. Historical single-archive releases have no logical release digest: not applicable, not failed.                                                                                                                                                                     |
| `artifact_digest` / integrity, all kinds              | Artifact                                                                            | `A.target`, `sha256`, `size_bytes`, `artifact_format`; historical `V.sha_256`, `size_bytes`; `app/tools/domain/tools/release_storage.py::verify_artifact_snapshot`; CLI `crates/agentpm-cli/src/io/download.rs`                                                                                                | Recorded expected full digest/size versus a particular observed verification of downloaded bytes. The Security legacy `sha_256` uses `tools.py::_short_hash` (four characters, ellipsis, four characters): display only. Do not reconstruct a digest from it or treat an S3 ETag as SHA-256. Successful hashing proves byte agreement, not benign content.                                                                                                          |
| `author_signature`, all kinds                         | Release (historical signatures bind their legacy archive statement)                 | `PackageSignature`/`tool_signatures`: `signer_id`, `algo`, `signature_b64`, `statement_json`, `created_ts`; `NamespaceSigner`: namespace/key/active/revocation fields; `release_signing.validate_signatures`; `provenance.verification_material`; Rust `crates/agentpm-sdk/src/provenance.rs::verify_artifact` | Separate stored signature presence, publish-time acceptance, and an independent check of bound identity/digest/signature/policy. Current `ToolDto.signed` counts rows and Security returns rows: neither is a fresh verification verdict. An inactive key can have a valid historical signature; ordinary revocation does not retroactively invalidate accepted releases. Do not invent a compromise policy. Optional unsigned is absent, not automatically failed. |
| `registry_attestation`, all kinds                     | Release                                                                             | `RegistryAttestation`/`registry_attestations`: `registry_key_id`, `algo`, `signature_b64`, `statement_json`, `created_ts`; `RegistrySigningKey` history; `registry_trust.SignedKeySet.verify`; `provenance.verification_material`; Rust `VerifiedKeySet` and `verify_artifact`                                 | Verified only after root/origin/time-valid signed-key-set authentication and the bound attestation signature/digest/identity/key-history checks. Security's `registry_attested` means row exists. A key-set fetch, known key ID, raw row, or signature count is insufficient. Absent/unavailable/unsupported material is not evaluated/unknown; an actual bad signature or binding check is failed.                                                                 |
| `malware_scan`, all archived kinds                    | Artifact; release summary only with complete target coverage                        | `S.artifact_id`, version ID, `status`, `malware_status`, `started_ts`, `finished_ts`, `notes`; `app/stats/application/tasks.py::_run_poll_once` reads the S3 `GuardDutyMalwareScanStatus` tag; `malware.py::release_scan_status`                                                                               | “Recorded completed scan: no threats found” only for complete + clean with every expected artifact covered. Running + clean remains running. Complete + unknown remains unknown. Missing scan is not run/unknown; failed retrieval/unsupported/access-denied is unavailable with reason, not infected. Suspicious/infected are explicit findings, never a safety score. Legacy scan rows without artifact IDs cannot cover a new multi-target release.              |
| Vulnerability/SBOM evidence, when supported           | Artifact/release according to an actual report                                      | `S.sbom_cyclonedx`, `grype_summary`, `grype_findings`, `pip_audit_findings`, `osv_findings`, `malware_findings`                                                                                                                                                                                                | Columns exist, but the inspected GuardDuty worker does not populate these tools' reports. Do not advertise SBOM generation, dependency vulnerability coverage, or zero vulnerabilities based on column existence/NULL/empty values. Unavailable/not evaluated until a real writer, method and binding are established.                                                                                                                                              |
| `license`, all kinds                                  | Release, optionally packaged file                                                   | `V.license_spdx`, `license_text`, `license_sha256`, `license_path`; selected `V.manifest.license`; `app/tools/application/release_display.py::display_metadata` (multi-artifact path), legacy `app/tools/application/cli.py` finalize metadata extraction and packaged APDS checks                             | Declared SPDX identifier/text; separately describe file presence/hash if checked. Missing is unknown/not supplied. A hash is not a legal opinion, license compatibility check, permission grant or automatic compliance verdict. Current Security omits `license_path`.                                                                                                                                                                                             |
| `release_date` / recency, all kinds                   | Release; latest-release recency separately identity                                 | `V.published_ts`; `Package.first_published_ts`; `tools.py::get_tool` currently emits `latest_published_ts` from the latest selected candidate                                                                                                                                                                  | Use the actual selected row's publication date. `latest_published_ts` is not the selected older release's date. `Package.updated_ts` is not maintenance evidence. An old release is dated/advisory, not failed, abandoned or unsafe; no arbitrary stale-age threshold or fabricated support promise.                                                                                                                                                                |
| Withdrawal / deprecation, all kinds                   | Release                                                                             | `V.status`, `yanked_ts`, `yanked_reason`; `app/stats/application/tasks.py::_run_poll_once` can auto-yank confirmed malware; no dedicated deprecation state/message found in the inspected model/services                                                                                                       | Published/yanked is available withdrawal information. Do not rename a yank as formal deprecation, expose the private actor, or interpret missing deprecation data as “actively maintained.” A distinct deprecation signal is unavailable until a real source exists.                                                                                                                                                                                                |
| `dependency_resolution`, kinds with dependencies      | Release and particular evaluated composition                                        | `V.manifest` authored selectors; `V.apds_validation.levels.resolved`; `app/tools/application/cli.py::_resolve_install_graph`; CLI `agent.lock.packages` and `roots` for a consumer's exact graph                                                                                                               | Distinguish authored ranges, a point-in-time resolved validation graph, and exact consumer lock edges. A successful old validation does not pin future range resolution. No-dependency is a real empty set when observed, not a retrieval failure. Missing/private/unavailable dependency is incomplete/unknown; never silently resolve “latest” into historical proof.                                                                                             |
| `dependency_integrity`, when exact graph proof exists | Contextual release graph / local install                                            | Exact resolved package versions and full digests from install resolve/init, `provenance.verification_material`, Rust `verify_artifact`, CLI download verification and frozen graph checks                                                                                                                      | Checks cover the explicitly selected graph and bytes, not every possible range outcome. Current Security has no durable per-dependency verification envelope. Local successful frozen replay is local evidence, not a public server readiness signal. Preserve authorized visibility for every referenced dependency.                                                                                                                                               |
| `tool_target_support`, Tool                           | Artifact target                                                                     | `A.target`; `V.release_manifest.artifacts`; Stage 1 target definitions in `crates/agentpm-sdk/src/target.rs`, CLI `crates/agentpm-cli/src/commands/package.rs` packaging/classification; `release_storage.validate_release_inputs` / release target inventory                                                  | Available artifact for a known target, with classification provenance where retained. Distinguish portable `any` from native targets. Artifact availability and declared compatibility do not prove it runs on every machine or enforce every advisory constraint. Other kinds get not applicable for Tool OS support unless a separate sourced execution surface warrants it.                                                                                      |
| Runtime / portability / compatibility, chiefly Tool   | Release declaration, artifact metadata, or local environment according to the check | `V.runtime`, `V.python_resolution`, `V.manifest.runtime`/`compatibility`, `V.release_manifest.pythonResolutionDigest`; CLI `crates/agentpm-cli/src/python_environment.rs` and installed runtime checks                                                                                                         | Report runtime and dependency constraints as declared/bound metadata. Provisioned interpreter/dependency/target verification is an observation on that host. Do not claim a Registry-wide runtime execution test, portability certification or enforcement of advisory compatibility. Missing runtime metadata is unknown where meaningful, not a green default.                                                                                                    |
| Harness support / local readiness, Agent compositions | Local environment                                                                   | CLI `harness_plan.rs`, `harness_plan/standard.rs` and their preflight reports; SDK Runner-advertised `supported_standards`                                                                                                                                                                                     | Runner support and configured local runtime are distinct from APDS. Public Registry cannot infer readiness from Loop presence, a valid graph, model hints or successful publish. Retain unsupported/not runnable/not evaluated distinctions. No anonymous “ready to run” Health signal.                                                                                                                                                                             |
| Stars / installs, all kinds                           | Identity; outside Health                                                            | `PackageStar`/`tool_stars`; `app/stats/domain/stats/entities.py::ToolBucket`; `tools.py::_get_tool_popularity` sums day buckets and counts stars by `tool_id`                                                                                                                                                  | Existing Stage 1 popularity calculations stay intact. No per-version stars, inherited dependency stars/installs, trust weighting, new score or ranking changes. Publisher membership also stays identity-level, not a signature result.                                                                                                                                                                                                                             |

Some paths above describe an operation, not a public report endpoint. The
verification record lists executable checks against those boundaries. Existing
Security DTOs are defined in `app/tools/common/dtos.py::ToolSecurity`,
`AuthorSignature` and `ReleaseArtifactSecurity`; reads are assembled by
`app/tools/application/tools.py::ToolsService.get_tool_security`.

## Status, verification and freshness rules

Use these shared meanings when implementing the later DTO. Preserve native
status/result fields alongside the shared status so the projection loses no
information; these are design semantics, not a new enum shipped by M7A.

| Meaning               | Required basis                                                                               | Examples                                                                                               |
| --------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Verified              | A named cryptographic/integrity check matched the bound subject, with observer/trust context | Full digest matched downloaded bytes; release signature verified against trusted material              |
| Passed                | A named non-cryptographic check completed at an explicit level/coverage                      | Recorded standalone validation passed; completed clean scan of the listed artifact                     |
| Failed                | An actual check produced a negative result                                                   | Wrong digest/signature, conformance error, identified malware; include which check failed              |
| Incomplete            | A check has partial/unevaluated scope                                                        | Resolved APDS graph includes unverified historical metadata; missing target scan                       |
| Unknown / unavailable | Evidence cannot establish the result, or a source cannot be retrieved                        | No scan/report, failed scanner access, inconsistent stored metadata, unsupported verification material |
| Not evaluated         | The particular check was not performed or no applicable verifier/report is retained          | Gate-off publication without APDS report; raw signature returned without independent verification      |
| Declared / advisory   | An author/metadata source states a fact or hint; it is not an independent result             | Supported standard declaration, license, compatibility hint, old publication date                      |
| Not applicable        | The signal has no meaning for this subject                                                   | Logical release digest for an old single-archive format; Tool OS support for an Instruction Profile    |

**Observer attribution is mandatory in public display copy.** The observer and
the specific check must appear with every verified label, including compact
badges and their accessible names: for example, “Signature verified by Registry”
or “Artifact digest verified by Registry.” A bare “Verified” badge is prohibited;
putting the observer only in a tooltip, drill-down or payload does not satisfy
this rule. A Registry-performed check is a Registry-reported result, not
independent protection against a compromised or mistaken Registry, even when a
web backend forwards it. “Verified locally by CLI” requires an actual completed
local check against the applicable trust context, including pinned roots where
required. “Verifiable locally: `agentpm install --require-signature`” describes
how to perform a signature check, not a check already completed; attestation
verification requires its applicable policy as well. Missing observer or trust
context leaves the result not evaluated/unknown, never an unqualified verified
claim. M7B must carry this attribution and M10B must preserve it when rendering.

`queued`/`running`, `unsupported`, `legacy_inferred` and validator outcomes remain
precise native qualifiers. “Unknown,” “unsupported” and “not applicable” must
not be collapsed into “passed.” Optional absent signatures are not a failed
signature check; a policy requiring one can separately fail. Never use an
overall “safe,” “fully verified,” “healthy,” reputation score or percentage.

Evidence must identify its source operation/report, subject version and target
or graph context, check/method and limitations. Use the full checksum when
available. Record `observed_at` separately from an existing check's timestamp.
Reading a report today does not mean it was evaluated today.

- APDS reports currently retain policy, levels, rules, outcomes and targets, but
  not an explicit evaluation timestamp, validator revision or self-contained
  digest of the evaluated graph. Their association is the committed version and
  immutable publication flow. Do not invent those missing fields or claim
  repeatable graph proof from the report alone. Publication time may be shown as
  publication time, not substituted for a validator timestamp. Multi-target
  publication checks each target, retains the last successful detailed result
  and adds the checked target inventory; it does not retain independent detailed
  reports for every target.
- `PackageSignature.created_ts` / `RegistryAttestation.created_ts` are storage
  times; statement `createdAt` / `storedAt` are signed statement times. Neither
  is the independent observer's verification time. Root-signed key sets carry
  `issuedAt`/`expiresAt`; key activation/retirement and exact Registry origin are
  part of verification. An expired/unavailable set cannot supply fresh trust.
- Scanner `started_ts`/`finished_ts` are available in storage, but current
  Security and artifact DTOs omit them. Preserve per-artifact times rather than
  inventing a release scan date. Scanner engine/signature-database versions and
  durable external report URLs are not currently retained. Method can name the
  actual GuardDuty tag polling operation, with this limitation.
- Immutable byte/checksum matches do not expire merely because a release is old;
  key trust, scan currency and live range resolution have their own context.
  No automatic freshness TTL or stale-to-failed policy is introduced here.

For future transport, carry the spec's conceptual `signal_key`, scope, status,
value, evidence, applicability and explanation using existing DTO conventions.
Evidence should refer to an authorized selected-version source, not arbitrary
user-supplied nested `health`, `verified`, scan or report fields in a manifest.
Such fields are assertions, not authority.

## Reports, privacy and access

The existing authorized source is `GET /v1/tools/{tool_id}/{version}/security`,
shared across kinds. It supplies selected-version Security material; it is not
a standalone verification report. Registry trust material is delivered at
`GET /v1/registry/signing-keys`; root authentication still must be performed.
No generic Health/report route is implemented by this audit.

`app/tools/tool_routes.py::get_tool_security` caches public Security responses
for 300 seconds using `app/models/cache.py::make_tool_security_cache_key`
(package ID and requested version); private reads use the authorized service
path. A freshly fetched HTTP response can therefore contain an older scan/key
snapshot. Preserve source/check time separately from retrieval time and account
for that cache when the later aggregation adds mutable evidence. Resolve the
`latest` alias to a concrete selected version before associating Health claims.

Only link to a real, access-controlled report if one exists. An absent report
link stays absent. Do not manufacture scan dashboards, publish storage keys or
fetch manifest-provided schema/report URLs on the server. A schema link must use
the trusted pinned standard catalog; a digest is a checksum, not a URL.

The current Security service checks package visibility before selecting or
querying evidence. M7B must keep that ordering and apply authorization to each
dependency as well. A public root does not authorize the names, versions,
manifests or diagnostics of private dependencies. Public aggregation must not
expose internal bucket/key/region paths, raw operator notes, private account
subjects or unfiltered scanner findings. Existing attestation statements can
include a publisher account identifier: retain exact signed bytes where needed
for established proof verification, but do not copy that identifier into a new
public Health summary. Design the safe projection separately from proof bytes;
do not redact signed bytes and then call the modified statement verified.

No privacy migration, existing proof transport change, or public Health
authorization guarantee is delivered by M7A. Those are explicit M7B tests and
implementation gates.

## Kind applicability and implementation handoff

Universal identity/release/digest/provenance/scan/license/date facts apply to all
eight kinds when those sources exist. Absence is not a green default. Extend
the shared evidence shape rather than duplicating verification per kind:

| Kind      | Source for later optional extension                                                        | Claim boundary                                                                                  |
| --------- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| Agent     | Selected manifest dependencies/bindings, resolved APDS report                              | Definition/graph evidence, no anonymous local readiness                                         |
| Tool      | Actual target inventory, runtime/resolution and packaged metadata                          | Target-specific availability; no universal runtime guarantee                                    |
| Skill     | Manifest Tool dependencies, entrypoint/references/scripts and packaged checks              | Presence/procedural contract; scripts are not automatically executed                            |
| Knowledge | Manifest mode/corpus/index metadata and packaged content checks                            | Verified paths/hashes where evaluated; no embedding quality/provider availability claim         |
| Memory    | Manifest plus `V.memory_build_json`, `memory_contract_index_json`, `memory_contracts_json` | Spaces/types/operations/triggers and validated contracts; no persistent backend execution claim |
| Profile   | Manifest guidance, constraints and compatibility hints                                     | Guidance/advisory, not sandbox enforcement                                                      |
| Loop      | Manifest phase/transition graph and evaluated APDS rules                                   | Graph/contract evidence, not an executed or configured local Run                                |
| Template  | Selected scaffold manifest/content and declared stack                                      | Starter-system contract; no automatic Agent/Harness suitability                                 |

Before M7B can emit stronger labels it must address these concrete gaps:

1. Reconcile observed declarations and stored metadata; preserve absent,
   inferred, incomplete and unsupported outcomes. Never use the publication
   enforcement flag today to certify old versions.
2. Supply verification outcomes with trust/subject context, or explicitly show
   signature/attestation material as present and not independently evaluated.
   Reuse Stage 1 verifiers; the presence booleans are insufficient.
3. Use full legacy digests and the selected release's date, not abbreviated
   Security hashes or `latest_published_ts`.
4. Preserve scan coverage/progress/results independently, expose actual times
   when available, and qualify unavailable method/version/report data.
5. Enforce safe projections and per-dependency authorization; test forged
   status/report fields, version changes and recent/legacy fixtures for every
   kind. Keep identity popularity outside Health.

These are sourced implementation constraints for the next milestone, not
unimplemented M7A product features. S2-AC-10's public API and rendered UI remain
unmet until their assigned milestones. The user confirmed on 2026-11-22 that
Claude's independent review of the labels and evidence gaps passed with no open
findings; the M7A trust-label review checklist item is complete. The user also
confirmed hosted CI passed on 2026-11-22, completing M7A. Implementation and the
operator renewal follow-ups remain in M7B.
