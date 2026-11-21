# M5B — Frozen dependency closure and dependency-free Agents

Date: 2026-11-20. Status: implementation and local verification complete; user
confirmed independent Claude review passed with no findings on 2026-11-20.
User confirmed all hosted CI passed on 2026-11-20, then supplied the passing
51-result operator-run manual audit. **M5B is complete.** Release Band 2 remains
open. Builder verification used isolated fixtures without package publication
or deployment; later operator-run local publications are recorded below. Source
baselines: CLI `9d7abd509172ef256cebb200d5fd2f5090a713c4`; API
`c5e36c0571255a9525817faec624be02eab67929` (documentation only).

## Scope and implementation

S2-AC-07 and T-LOCK-02–06 are implemented in the CLI's existing installer. The
M5A metadata and Stage 1 integrity/target contracts remain in place. The user's
additional request to stabilize the Node invocation smoke test is included.
Harness/SDK Runner behavior, Registry runtime code, public APDS files, lockfile
schema/version and unrelated resolver architecture are unchanged.

- `crates/agentpm-cli/src/commands/install.rs`: require a lock for frozen replay;
  use actual locked kinds for direct package specs; retain every workspace root
  on an empty install; preserve manifest and lock bytes in frozen mode; check
  the actual selected kinds against older-lock compatibility rules.
- `commands/install/frozen.rs`: validate modern package keys, stored edge kinds,
  missing entries, local root identity and authored constraints before requesting
  downloads. Replay the entire exact locked set, including transitive packages
  and other recorded roots. A direct selector must identify one kind/version;
  it never falls back to the Registry resolver.
- Verified Agent/Skill manifests are inspected in a private temporary extraction
  tree before installed directories are replaced. Recorded root edges must
  match the manifest's requirements. Older locks need not have a separate root
  record for a transitive Skill: its requirements are checked against the exact
  locked set without choosing or resolving another version. Multiple satisfying
  locked versions remain installed; this is not a new Runner selection policy.
- `semver/types.rs`: frozen requirement matching checks ranges rather than
  assuming the lock is always compatible. Ordinary root construction shares the
  same matcher: exact versions retain build identity, caret/tilde and comparator
  ranges use SemVer precedence, space-separated comparators match Registry
  syntax, a numeric `1.2` selector means exact `1.2.0`, and default selectors do
  not admit prereleases. No new resolver or SemVer dependency was introduced.
- `io/download.rs`: reuse existing cache hashes, payload identity, extraction
  safety, provenance and target checks. Drain sibling extraction futures after
  a failure so the temporary inspection tree is not removed while blocking
  extraction work is still running. A prepared-cache extraction path preserves
  `--refresh` file repair while avoiding a second payload GET; the regression
  modifies installed Tool code, verifies restoration and counts downloads.
- `commands/run.rs`: the Node smoke test explicitly requests 30 seconds for
  startup under parallel suite load instead of inheriting its fixture's
  five-second deadline. No retry masks failure, and production timeout defaults
  and the separate timeout-enforcement tests remain unchanged.
- `tests/frozen_install.rs`: real CLI/HTTP integration coverage; native target CI
  now explicitly includes it. Public CLI README/CHANGELOG and
  `agentpm-api/docs/v0.1/cli/install-cmd.mdx` explain replay, cache/network behavior,
  empty Agents and migration errors. `scripts/README.md` documents test commands.

## Tests and acceptance mapping

| Requirement             | Executed coverage                                                                                                                                                                                                                                                                         |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T-LOCK-01/07; D06       | Existing strict v4/APDS tests and seven M5A install tests; legacy v1 Tool and metadata-free v2/v3 replay; no lock rewriting                                                                                                                                                               |
| T-LOCK-02               | Two repeated clean direct-Agent replays restore Agent, Skill, Knowledge, Memory, Profile, Loop and both exact Tool versions; compare all installed bytes, lock bytes and HTTP calls                                                                                                       |
| T-LOCK-03               | Direct Skill and local Skill replay preserve the Skill identity and inherited Tool; direct Tool still works; Template still directs users to `agentpm new`                                                                                                                                |
| T-LOCK-04               | Changed or removed local dependencies, local name/version drift, direct range mismatch/ambiguity, wrong kind/key, dangling edge, missing transitive Tool, immutable transitive range and mismatched stored root edge fail without replacing existing installations or changing lock bytes |
| T-LOCK-05               | Init digest drift, corrupt downloads, tampered cache repair, and existing payload/provenance/extraction suites; malformed/unsupported APDS remains covered by the M5A install suite                                                                                                       |
| T-LOCK-06               | Repeated normal/frozen local empty-Agent install is stable without HTTP; directly requested empty Agent still installs itself without any empty resolver request; empty multi-root workspace retains both roots                                                                           |
| T-LOCK-07               | Declared and legacy-inferred package metadata survive replay and ordinary multi-Agent workspace regeneration; source bytes remain immutable                                                                                                                                               |
| T-LOCK-08               | Existing target/payload CI tests and all ten opt-in native Python environment tests on local Apple Silicon; other supported targets require hosted CI                                                                                                                                     |
| Additional user request | Node invocation smoke passes in the full suite and 24 repeated runs using eight concurrent test processes; timeout enforcement remains separately tested                                                                                                                                  |

Commands run from the CLI root (exit 0 unless explicitly noted):

```bash
cargo test --workspace --no-fail-fast
cargo test -p agentpm-cli --test frozen_install --test apds_install -- --test-threads=4
cargo test -p agentpm-cli --bin agentpm commands::install::frozen::tests -- --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo fmt --all --check
python3 scripts/check_apds_schema.py
python3 scripts/check_sdk_contract.py --sdk-root ../agentpm-sdk-node --sdk-root ../agentpm-sdk-python
```

Native environment regression (the five opt-in cases are included explicitly):

```bash
env -u AGENTPM_PYTHON \
  AGENTPM_TEST_UV=/Users/zackhine/Library/Caches/com.agentpm.AgentPM/runtimes/uv/0.13.5/aarch64-apple-darwin/uv \
  AGENTPM_TEST_PYTHON=/opt/homebrew/opt/python@3.13/bin/python3.13 \
  AGENTPM_TEST_CLI="$PWD/target/debug/agentpm" AGENTPM_TELEMETRY=0 \
  cargo test -p agentpm-cli --bin agentpm python_environment::tests \
    -- --include-ignored --test-threads=1
```

From the API root, documentation ingestion/link/contract checks:

```bash
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
  .venv/bin/python -B -m pytest -q tests/test_docs_content.py
```

That placeholder key permits test-module imports only; it is not a production
credential. Documentation results: **5 passed, 166 subtests passed**. Native Python
results: **10 passed**. The SDK contract and immutable APDS checksum guards pass.
Final workspace results: **1,332 passed, 0 failed, 6 opt-in ignores**, including
**21 new install integration tests** and two new selector/graph unit tests.
Clippy with `-D warnings`, Rust formatting, Markdown/MDX/YAML formatting for the
changed feature docs/workflow, and whitespace checks pass. The final native rerun also passes all **10 tests**, including all five native
cases ignored by the ordinary workspace command. The remaining ignored SDK
provenance case requires proof exported by the disposable PostgreSQL backfill
test; that separate Registry fixture was not run for this CLI/docs change.

Loopback fixture tests require local port binding; the execution sandbox initially
blocked that and the suite was rerun with permission. The stricter local identity
check exposed an older Python-resolution test that generated a different local
name/kind than its historical lock: the test now authors the historical root's
identity, while the original v2/v3 fixture bytes remain unchanged. This preserves
the test's intended Python-resolution migration assertion. Final results, rather
than intermediate construction runs, are the acceptance evidence.

## Focused local CLI smoke

The actual built CLI created and linted an empty Agent Package, installed it
normally against non-listening `http://127.0.0.1:9`, then replayed frozen with
byte-identical lock contents. Both install commands emitted:

```json
{ "schema_version": 1, "status": "success" }
```

This confirms the dependency-free path does not make a Registry request. A direct
frozen request from a separate directory without a lock returned machine error
status and exit 1 with `--frozen requires agent.lock`. The disposable workspace
was removed after inspection. Full graph and failure checks use the actual CLI
against controlled loopback Registry responses, not production/local-user data.

## Compatibility, risks and remaining gates

- **D06 retained:** no new portable lock fields or version bump. The M5A extended
  v4 reader floor remains the prepared, unreleased CLI **0.1.47**. Existing SDK
  0.1.17 remains sufficient for lock loading. Reader-first release/installability,
  developer/CI/container upgrades and restoring the SDK CI refs to `main` remain
  Release Band 2 operator gates.
- Frozen replay restores the complete recorded package set, not just a requested
  root's subset. It does not resolve newer versions or prune the lock. Existing
  authored changes that previously slipped through frozen mode now fail and
  require deliberate ordinary regeneration.
- Graph preflight uses temporary disk and extraction I/O. It reuses verified
  cached archives for the actual installation, so successful preflight does not
  cause a second payload download. A clean install can extract an archive twice.
  Existing per-artifact extraction limits remain; this adds no aggregate budget
  or cross-package filesystem transaction. Each package's replacement retains
  existing atomic extraction behavior; later disk/provision/finalize failures
  retain their existing guarantees.
- `--frozen` is not offline. Nonempty install still needs Registry authorization,
  target/provenance selection and finalization. Cached payloads avoid GETs but
  cannot bypass access checks. The unavailable-Registry test verifies safe failure.
- Test Knowledge/Memory fixtures are lint-valid authored scaffold snapshots for
  install graph checks. These tests do not claim publication/build readiness,
  full packaged APDS certification, live backend readiness or Harness behavior.
- **Review:** user confirmed Claude review passed with no findings on 2026-11-20.
- **Hosted CI:** user confirmed all hosted CI passed on 2026-11-20.
- **Manual acceptance:** user supplied the passing 51-result real-local audit,
  independently checked from saved files as recorded below. M5B has no remaining
  milestone acceptance gate; Release Band 2 rollout and M6 work remain separate.

## Manual runbook preparation — 2026-11-20

The [M5B runbook](manual-tests/CLI_M5B_MANUAL_TESTS.md) and
[fixture documentation](manual-tests/fixtures/m5b/README.md) provide visible
sources, exact commands, expected errors, current-directory instructions and
captured evidence under ignored `cli-m5b-test/`. The operator reuses four retained
M5A Tool/Skill/legacy identities and publishes six new declared local packages,
including actual CLI-built Knowledge and Memory. Local enforcement stays enabled.
The helper reuses the proven M4A publication/dev-S3/signature/read-only-DB guards.
The operator's normal IntelliJ API configuration remains in use. No published
examples or existing seed versions are edited.

A separate loopback server reads visible init-response JSON and same-size corrupt
archive files for controlled failures. Real CLI commands test clean graph replay,
exact roots and two Tool versions, empty consumers, range/identity/kind/edge/digest/
origin faults, availability, cache/refresh repair and metadata-free older locks.
The checker compares exact root edges and every installed file with the original
hash-verified archives and source copies. Request offsets tie GET counts and
no-resolve/no-finalize claims to actual log slices. The final audit requires all
**51** command outcomes and real local publication evidence; it cannot self-grant
operator approval or fabricate CI results.

Builder verification from the CLI root (exit 0):

```bash
PYTHONPATH="$PWD/../agentpm-api" \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
  ../agentpm-api/.venv/bin/python -B -m pytest -q \
  specs/2026-10-09-category-hardening/manual-tests/fixtures/m5b/test_fixture.py
```

**5 passed**. Checks cover CLI build/lint, actual author signatures, independent
Registry publisher validation for all six new archives, positive/negative installs,
byte preservation and exact root edges, stale environment protection, semantic
error matching and refusal of incomplete audit evidence. The fifth check replays
the guide's actual shell blocks for sections 3–11 using only the isolated HTTP
fixture. It deliberately does not claim actual namespace authorization or run the
real operator publication/DB phases. All **28** runbook shell blocks and the
committed shell functions parse with zsh. Black (line length 100), Ruff, Prettier,
ignore-path and whitespace checks pass. Builder workspaces are cleaned afterward.

Product code was not changed after the reviewed/hosted-CI-passed implementation.
These are runbook/helper/evidence additions. The operator subsequently completed
the runbook, as recorded below. Its summary keeps `operator_signoff: false` and
`hosted_ci_complete: false`; human approval and hosted CI are recorded separately.

## Operator manual acceptance — complete

The operator supplied the passing audit from
`cli-m5b-test/20261121T014721Z-49256/evidence/summary.json`: **51 command results**,
covering real local Registry graph replay and isolated controlled failures.
The saved files were independently inspected without new HTTP/DB calls:

- All **51** expected/actual exit records match, including **12** specific
  negative machine messages.
- All **30** source/lock/installed-byte proof records match their saved snapshots;
  all **18** request proofs match the corresponding raw request-log slices.
- All **11** final project locks match their expected exact package sets, roots,
  metadata, immutable archive digests and installed payload files. The two Tool
  results show the expected `1.0.0` and `2.0.0` versions.
- All six new local publication receipts are present, and the reviewed CLI
  binary still matches its recorded setup hash. The operator-run full audit
  already performed the real local DB publication checks; those SQL queries
  were not rerun during this file-only confirmation.

The raw summary retains `operator_signoff: false` and
`hosted_ci_complete: false`, because the fixture never grants those approvals.
It was not edited. The user's passing final audit report supplies operator
acceptance, alongside the previously confirmed Claude review and hosted CI.
**M5B is complete.** CLI publication/installability, reader-before-shared-lock
emission, SDK CI ref cleanup and the remaining M6/Release Band 2 gates remain
separate; no production release is claimed.
