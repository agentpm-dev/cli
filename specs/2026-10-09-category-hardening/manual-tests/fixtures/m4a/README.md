# M4A manual publication fixtures

Follow [the operator runbook](../../REGISTRY_M4A_MANUAL_TESTS.md) one block at a
time. It creates **36 visible cases** under gitignored
`agentpm/registry-m4a-test/<run-date>/`. These are test packages, not published
examples or changes to the canonical APDS bundle.

The operator commands use the real local API, the real local Docker application
database and development S3. The helper refuses Registry requests anywhere except
`http://127.0.0.1:5000`, refuses uploads outside HTTPS `agentpm-dev-tools` S3, and
opens database inspection connections read-only. It cannot backfill, repair or
delete DB rows. Publication and signer registration happen through normal API/CLI
operations. Published test packages and pending reservations remain in dev.

## Files you can inspect

- `source/agent.json` and companion files are the actual source for each case.
  Positive inputs start from copies of the frozen valid fixtures, renamed uniquely.
  Normal CLI commands build Knowledge hashes and Memory contracts.
- `case.json` explains the intended defect, transport mode, expected HTTP/code,
  rule/path and final state.
- `reservation-manifest.json`, where present, deliberately differs from the tar's
  manifest. `request.json` is the exact init body with honest archive size/digest.
- `any.tar.gz` or the two concrete target archives contain the bytes sent to S3.
  `describe` lists their members, including deliberate omissions.
- `finalize-good.json`, `finalize-unsigned.json` and
  `finalize-bad-signature.json` hold explicit request bodies. The helper signs
  real archive/release digests with a temporary Ed25519 seed. It does not replace
  the CLI keystore or generate a Registry/root signing key.
- `init.json` holds the API reservation and presigned storage instructions;
  `receipt.json` records successful publication identity. These files are private
  workspace evidence, not material to commit or share publicly.
- `evidence/` contains exact HTTP status/body and expected status, separate CLI
  stdout/stderr/exit records, read-only DB inspection, discoverability GETs and
  optional helper-launcher settings. Tokens/Authorization headers are not recorded.

## Helper actions

The runbook's `m4a_fixture ACTION [CASE]` function always supplies the current
workspace. Each network stage is explicit:

| Action                          | Effect                                                                                                                              |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `prepare`                       | Write local source cases and one private temporary author seed; no network                                                          |
| `pack-all` / `pack CASE`        | Create raw-client archives, hashes and signed request bodies locally; no upload                                                     |
| `describe CASE`                 | Show exact source/reservation/expected result and archive members                                                                   |
| `serve --enforcement off/on`    | Start the actual `agentpm-api/run.py` app against local Docker DB/dev S3; temporarily disable throttling only in that local process |
| `preflight`                     | Read local namespace and check actual APDS columns; no DB writes                                                                    |
| `signer-id`                     | Read the UUID for this run's already registered dev public key                                                                      |
| `init CASE`                     | POST the saved request to the real local Registry and retain its reservation                                                        |
| `upload CASE [--target TARGET]` | PUT exactly one archive to the returned development S3 URL                                                                          |
| `finalize CASE`                 | POST the saved signature body; compare exact HTTP/code and negative rule/path                                                       |
| `inspect CASE --state STATE`    | Read persisted version/signature/attestation/scan/artifact state, then make a real API detail GET                                   |
| `audit`                         | Check saved results for all 36 cases and signature failures; do not rerun tests or publish                                          |

Run the API normally from IntelliJ with its existing local configuration. Change
`APDS_PUBLISH_ENFORCED` from false to true between phases, leaving
`RELEASE_PUBLISH_ENABLED=true` for both. The audit uses observed API responses and
persisted metadata; `server-off.json` / `server-on.json` are not required.

`serve` is an optional convenience. Its process needs the usual backend settings;
it does not inherit IntelliJ variables when run from a separate terminal. To use
it without exporting secrets, duplicate your existing IntelliJ configuration,
retain its interpreter/environment and API working directory, and use this
helper file as its script with arguments `serve /absolute/path/to/the/run
--enforcement off` (then `on` after restarting). Include the API repository root
on `PYTHONPATH`. It imports the same real `run.py` app and temporarily disables
HTTP throttling only in that process. Normal IntelliJ `run.py` operation keeps
rate limits, so allow for minute/hour reset windows if requests return 429.

The off/on transition belongs to the **local test process only**. Completed
retries keep the same version identity; legacy receipt wording may change.
The matrix's two descriptors exercise server association/atomicity, not native
compilation. The ordinary CLI Tool case uses Python and should publish one `any`
artifact; the raw API Tool case uses Node and the existing single-archive path.

## Builder checks versus operator acceptance

`test_fixture.py` is **isolated builder verification**. It checks preparation,
real archive expectations against the independent publisher, signature encodings,
transport guards, and physical inspection SQL against disposable PostgreSQL
schemas. It mocks network transport/storage for those checks. It does not call
the operator's API, publish to dev S3, or establish completion of the manual run.

From the CLI root, with the normal API test venv and a reviewed debug CLI built:

```sh
PYTHONPATH="$PWD/../agentpm-api:$PWD/../agentpm-api/tests" \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
  ../agentpm-api/.venv/bin/python -B -m pytest -q \
  specs/2026-10-09-category-hardening/manual-tests/fixtures/m4a/test_fixture.py \
  --basetemp "$PWD/registry-m4a-test/builder-helper-checks"
```

The PostgreSQL helper tests require the existing **disposable** database via
`AGENTPM_M13A_TEST_DATABASE_URL`, such as
`postgresql+psycopg://postgres:postgres@127.0.0.1:5433/agentpm_m13a`.
They are skipped if it is unset; **do not substitute the real application DB**.
For these isolated checks only, the dummy signing key enables test imports. The
operator's real API must keep its actual development Registry signing key.

Operator acceptance is the separate real publication run in the linked guide.
Neither the helper tests nor the functional operator run establish M4B's required
dependency-heavy capacity/timeout/disk/concurrency evidence.
