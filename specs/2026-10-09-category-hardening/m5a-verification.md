# M5A — Resolve, install and portable lock identity

Date: 2026-11-20. Status: implementation and local verification complete; user
confirmed Claude review passed with no remaining findings and supplied a passing
38-result operator-run manual audit. Hosted CI and production release remain pending.

Starting revisions: CLI `9f9fdff`, Registry `112420c`, Node SDK `a67ddc1`, Python
SDK `b4f3d6e`. Evidence describes the working-tree changes on those baselines.
The builder did not make production requests/writes/deployments or publish
packages. Later operator-run local dev publications are recorded below.

## Contract and decisions

**S2-AC-06 / D06 / D19:** optional known fields on resolved packages, install
artifacts, locked packages and represented local roots:

```json
{
  "standard": { "id": "agentpm", "version": "1.0.0" },
  "standard_origin": "declared"
}
```

`legacy_inferred` is the alternative origin. `declared` deliberately matches
the Registry's existing provenance vocabulary; the conceptual `explicit` spelling
in the planning example is not a wire value. Neither origin is validation
evidence, a signature result or Runner readiness. A pair must be complete and
supported. Unknown origins, partial pairs and unsupported identities fail.

The lock remains **v4**, with the existing strict readers and forward-version
guard. Metadata on either a package or represented local root requires v4.
Absent metadata is omitted, preserving historical shape; ordinary regeneration
retains an existing higher supported version. Package keys and the bare lowercase
64-hex integrity contract are unchanged. Logical release digests still use the
canonical release manifest, while payload digests, target selections and Python
interpreter state remain machine-local.

CLI **0.1.47** is prepared as the first compatible release, **unreleased**.
Normal legacy installs require no new command or flag. A normal install can write
inferred metadata and thereby require the upgrade even for historical packages;
untouched locks are not rewritten just to decorate them. Frozen installs leave
lock bytes unchanged. This implements the selected D06 policy, not a v5 proposal.

## Source flow and boundaries

- Registry `app/tools/application/install_apds.py` observes the cached declaration,
  reconciles the persisted identity/origin columns, and derives historical
  interpretation only when the declaration is absent. Conflicting/partial stored
  metadata fails rather than being repaired or certified. No DB migration or
  inferred validation evidence is added. Existing namespace authorization and
  exact version selection run before this metadata is exposed.
- `app/tools/application/cli.py` adds the pair to resolve and install-init
  responses. Aggregate resolve limits include the additional bytes. Existing
  legacy and release-level integrity selection is retained.
- Rust SDK `apds.rs`, `models/install.rs`, `client.rs` and `provenance.rs` carry
  the pair, validate support/pair consistency, compare resolve/lock expectations
  against install-init and proof manifests, and retain expected metadata during
  older-Registry URL renewal. Supplied conflicting metadata is never accepted as
  a missing-field fallback.
- CLI `semver/adapt.rs`, `semver/types.rs`, `commands/install.rs` and
  `workspace.rs` retain the fields through plan conversion, frozen supported
  paths and ordinary regeneration. The final lock is rebuilt after artifact
  reconciliation rather than saved from the pre-download plan. Local roots are
  refreshed from actual authored manifests, including a changed declaration,
  name/version, and supported legacy inference. Python resolution preparation
  refreshes the represented local Tool root through `python_resolver.rs`.
- `io/download.rs` validates embedded JSON, kind/local name/version and APDS
  identity/origin before replacing an installed directory. Existing release
  proof, selected descriptor and checksum verification are retained. Cache hits
  check the installed manifest against the hash-verified archive too, so an
  older response without metadata cannot promote a modified installed selector
  into lock truth. Invalid installed manifests are repaired from verified cache;
  failed download/extraction does not replace the prior install or write a lock.
- Other Rust literal changes supply the newly optional fields or preserve them
  from an existing package/plan. These are model propagation and fixture updates,
  not new Knowledge/Memory/Harness execution features.

The public schema and all published APDS 1.0.0 files are untouched. M5B's broader
frozen graph closure, selector/range and empty-Agent work is not implemented or
claimed here. Current supported frozen paths have metadata regressions; that
does not complete the entire later frozen-install acceptance matrix.

## Tests and acceptance mapping

All commands below completed with exit 0 unless a deliberate negative assertion
is described. Initial sandbox TCP denials were rerun outside the sandbox against
loopback fixtures/disposable PostgreSQL. Old synthetic fixtures were corrected
where they lacked a real row's manifest or reused an archive for different
package names; the new validation was not weakened to accommodate them.

| Requirement      | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T-LOCK-01        | `semver/apds_lock_tests.rs`: pair roundtrip, plan/transport/regeneration, local-root refresh, v4 requirement, unknown/unsupported/partial fields and distinct exact identities. Existing `lock_v4_tests.rs`: metadata-free v2/v3/v4 roundtrips, unchanged writes, future-version guards, Python portability and digest semantics.                                                                                                                                                                                                           |
| T-LOCK-05        | `tests/apds_install.rs`: tampered cache and bad remote bytes preserve prior install/lock; modified installed manifest is repaired from verified cache, including an older Registry that omits the pair. Existing download size/hash/truncation/atomic-extraction tests remain green.                                                                                                                                                                                                                                                        |
| T-LOCK-07        | Seven real CLI install tests cover declared/inferred and advertised/older responses, ordinary regeneration, clean supported frozen replay, malformed/unsupported selectors, mismatched kind/name/version, false provenance, resolve/init drift and local declaration refresh. `io/download/target_tests.rs` asserts rejection before replacing an existing install and same-inventory URL renewal through an older response. Registry `test_install_apds.py` checks both origins, stored/unstored rows, conflicts and read-only derivation. |
| T-LOCK-08        | Existing target/release/provenance suites retain logical release digest versus selected payload behavior. Registry PostgreSQL target-init tests assert both metadata and exact/`any` selection. Real native Python SDK invocation passes on local Apple Silicon; native Rust opt-in verification is recorded below. Hosted matrix now explicitly runs `--test apds_install`; other architectures require hosted execution.                                                                                                                  |
| D06 SDK boundary | Both SDK bridge suites add extended-v4 package/root cases with byte-identical locks. Fresh published npm/PyPI 0.1.17 packages independently pass public Agent/Skill loading and actual CLI-backed Tool invocation; both reject v5 unchanged. Homebrew CLI 0.1.46 rejects extended v4 with its real unknown-field diagnostic and leaves bytes unchanged.                                                                                                                                                                                     |
| D19 honesty      | Origin is derived from an explicit selector or supported missing-selector interpretation, checked against the embedded archive; inferred state is never serialized as declared or validated. No `apds_validation` is put into install DTOs/locks.                                                                                                                                                                                                                                                                                           |

Local regression:

- CLI: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --no-fail-fast`: **1,309 passed, 0 failed, 6 ignored**.
  The five opt-in native tests and exported-proof provenance test are separate
  from this ordinary workspace count. A separate local Apple Silicon run of
  `cargo test -p agentpm-cli --bin agentpm python_environment::tests --
--include-ignored --test-threads=1` passed **10/10**, including all five native
  opt-in tests. It used verified uv 0.13.5, base Python 3.13.15 via
  `AGENTPM_TEST_PYTHON`, and no inherited `AGENTPM_PYTHON` override. The first
  attempt inherited an SDK virtualenv override and failed fixture environment
  assumptions; the CI-style base-interpreter setup passed without weakening
  checks or changing production runtime code.
- Registry: disposable PostgreSQL at `127.0.0.1:5433/agentpm_m13a`, full
  `.venv/bin/python -B -m pytest -q`: **1,103 passed, 11 skipped,
  347 subtests passed**. Existing warnings are SQLAlchemy legacy-query warnings.
- Node SDK 0.1.17: `pnpm test` with actual CLI and pinned uv/native fixture:
  **108 passed, 1 skipped**. `pnpm lint`, `pnpm exec tsc --noEmit` and
  `pnpm format` pass.
- Python SDK 0.1.17: `.venv/bin/python -m pytest` with actual CLI and native
  fixture: **109 passed, 1 skipped**. Black, Ruff and `mypy src` pass.
- Registry changed-Python hooks: cached pinned Black, isort (`--profile black`),
  flake8 (`--max-line-length=120`), pylint (`--rcfile=.pylintrc`), mypy
  (`--follow-imports=silent`), Bandit (`--skip B101`) and pyupgrade (`--py39-plus`)
  pass. The new validation module uses the same W503/Black exemption as the
  existing APDS modules. Whitespace/EOF/conflict and `git diff --check` run on
  changed files.
- Registry documentation checks: `tests/test_docs_content.py` passed **5 tests
  and 166 subtests** after the MDX changes.
- `python3 scripts/check_apds_schema.py` confirms published schema/bundle drift
  protection. `python3 scripts/check_sdk_contract.py --sdk-root
../agentpm-sdk-node --sdk-root ../agentpm-sdk-python` and
  `python3 -m unittest discover -s scripts -p test_sdk_contract.py -v` pass.
  The shared native fixture now declares a supported standard, so its authoring
  lint step works with current strict lint; all three copies match byte-for-byte.

For native SDK tests, both repositories used:

```bash
export AGENTPM_TEST_CLI=/absolute/path/to/agentpm/target/debug/agentpm
export AGENTPM="$AGENTPM_TEST_CLI"
export AGENTPM_TEST_UV=/absolute/path/to/verified/uv-0.13.5
export AGENTPM_TEST_PYTHON=/absolute/path/to/python3.13
export AGENTPM_PYTHON="$AGENTPM_TEST_PYTHON"
export AGENTPM_TELEMETRY=0
```

The SDK dependency fixture installs Pydantic 2.11.7/pydantic-core 2.33.2 without
building wheels and proves invocation does not provision or rewrite lock state.
It uses a loopback Registry and real package-index dependencies, not production.

### Published package and older-CLI probe

To reproduce with fresh published packages (the probe itself makes no Registry
requests):

```bash
M5A_SDK_PROBE="$(mktemp -d /tmp/agentpm-m5a-published.XXXXXX)"
npm install --prefix "$M5A_SDK_PROBE/node" --ignore-scripts --no-audit --no-fund @agentpm/sdk@0.1.17
uv venv "$M5A_SDK_PROBE/python"
uv pip install --python "$M5A_SDK_PROBE/python/bin/python" agentpm==0.1.17
python3 scripts/check_apds_sdk_compatibility.py \
  --cli target/debug/agentpm \
  --older-cli /absolute/path/to/released/agentpm-0.1.46 \
  --python "$M5A_SDK_PROBE/python/bin/python" \
  --node-module "$M5A_SDK_PROBE/node/node_modules/@agentpm/sdk/dist/index.cjs"
```

SDKs were installed fresh into `/tmp/agentpm-m5a-published.4UEDOh` using
`npm install --ignore-scripts --no-audit --no-fund @agentpm/sdk@0.1.17` and
`uv pip install --python <temporary-venv>/bin/python agentpm==0.1.17`.
The probe checks installed version metadata equals 0.1.17 for both packages:

```bash
python3 scripts/check_apds_sdk_compatibility.py \
  --cli target/debug/agentpm \
  --older-cli /opt/homebrew/bin/agentpm \
  --python /tmp/agentpm-m5a-published.4UEDOh/python/bin/python \
  --node-module /tmp/agentpm-m5a-published.4UEDOh/node/node_modules/@agentpm/sdk/dist/index.cjs
```

All reported outcomes are `pass`: public Agent/Skill loading and CLI invocation
with extended v4, SDK future-version rejection and older-CLI safe unknown-field
rejection. Lock SHA-256 before/after the successful read-only probes:
`5796f6f0eb88c753e0acc30679e958b707c5cebff49338b746d9dba28e141197`.
The installed older CLI identifies itself as 0.1.46. This is a synthetic installed
fixture for reader/invocation compatibility, not an archive integrity or
signature certificate; real download/integrity behavior is tested separately.

## Rollout, risks and remaining gates

Registry response additions can deploy before the new CLI: old response DTOs
tolerate additions. The new CLI also handles older responses by deriving/checking
identity from verified bytes. No new database migration or backfill is required.

Before shared locks are regenerated/distributed, publish CLI 0.1.47 (or a later
release containing this reader/writer), confirm installability through supported
channels, and upgrade developer machines, CI and container images. Existing SDK
0.1.17 readers already tolerate the pair; no SDK runtime change/version release
is required solely for this extension. SDK changes here are compatibility tests,
the synchronized fixture and README guidance.

For pre-merge SDK CI, select the matching CLI commit/branch using the existing
`cli_ref` workflow input or `AGENTPM_CLI_REF` repository variable. The shared
fixture guard correctly fails if an updated SDK snapshot is compared with a CLI
ref predating its added native-fixture declaration. Once the CLI source is
merged, the normal `main` selection can be restored; no SDK publication is
needed for these tests.

Older CLIs that read extended locks reject unknown fields, safely but with the
already-shipped less helpful diagnostic. Normal legacy installs can encounter
this compatibility boundary too. The documentation explicitly explains it.
The install checks validate identity and standard support, not full resolved or
packaged APDS conformance or Runner readiness. Historical artifacts remain
immutable; no examples or published packages were edited/regenerated.

**Review:** user confirmed Claude review passed with no remaining findings on
2026-11-20, after correcting the documented Node entrypoint to `dist/index.cjs`.
Both published entrypoints exist and are imported by the probe; the CJS probe
and nonexistent-entry rejection were independently rerun successfully.

**Remaining:** hosted Registry/CLI/SDK CI including other native target runners,
then the later reader-before-shared-emission release verification. The operator
ran manual testing before hosted CI as explicitly requested; its passing audit
is recorded below. M5B/M6 acceptance is outside this deliverable. M5A remains
open until its remaining verification/exit gates are met.

## Manual runbook preparation

The [M5A guide](manual-tests/CLI_M5A_MANUAL_TESTS.md) prepares five uniquely named signed dev releases, eight consumer workspaces and eleven controlled responses under gitignored `cli-m5a-test/`. The operator uses their real IntelliJ API, local Docker application DB and development S3 for normal publications/resolve/installs. One missing-selector historical simulation is explicitly seeded while **local** enforcement is off, then local enforcement is restored. Existing M4A transport/signature/dev-storage/read-only-DB guards are reused. Controlled archive faults use a separate loopback server and dummy token; they do not publish bad artifacts.

Builder verification: `PYTHONPATH="$PWD/../agentpm-api" REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= AWS_EC2_METADATA_DISABLED=true ../agentpm-api/.venv/bin/python -B -m pytest -q specs/2026-10-09-category-hardening/manual-tests/fixtures/m5a/test_fixture.py` passes **4 tests**. They verify source lint, author signatures and independent publisher semantics, nine exact negative install failures with unchanged state, cache/selector repair and lock guards, plus signed normal/direct-Agent installs, two exact Tool outputs, regeneration, supported frozen byte preservation and local-root edits using actual resolve logic with isolated records/storage. These checks do not contact the operator's real API/DB/S3 or claim operator acceptance.

The user explicitly requested manual acceptance first, then pushing changes together and hosted CI before M5A closure. The runbook preserves that order; the operator-run audit has now passed and hosted CI remains pending. Native release/target verification and production-root trust are not weakened to make local tests pass.

## Operator-run manual audit — 2026-11-20

The user supplied `cli-m5a-test/20261120T170444Z-21931/evidence/summary.json` with `status: "pass"` and **38 checked command results**, covering the real local API publication/resolve/install flow and separate controlled download faults. The saved summary and all 38 expected/actual exit records were read independently and agree. This is operator-run evidence, distinct from the four isolated builder helper tests.

The helper deliberately leaves `operator_signoff: false` and `hosted_ci_complete: false`: it cannot self-grant human approval or claim CI execution. The result JSON is retained unchanged. Hosted CI remains the next milestone closure gate; no production release or reader-first rollout completion is claimed.
