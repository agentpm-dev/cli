# Registry capacity measurement

Run this from the **CLI repository root** (`agentpm`). It is one command; no
Render environment, local API settings, PAT, signing key or manual fixture steps
are needed. Docker Desktop must be running, the sibling `agentpm-api` and
`agentpm-examples` checkouts must be present, and `target/debug/agentpm` must
already be built. If needed, build it once with `cargo build --locked -p agentpm-cli`.

```bash
python3 specs/2026-10-09-category-hardening/manual-tests/fixtures/capacity/run.py
```

The default **representative** profile runs the measured workload. First-run
image/dependency downloads take additional time. The script prints its evidence
directory, runs without prompts and prints the final report. On failure it exits
nonzero and retains diagnostics. It stops/removes only its own Docker Compose
project, including disposable database/storage containers, after success or
failure. Downloaded/built images and local evidence remain for repeat runs.

For builder verification with smaller payloads:

```bash
python3 specs/2026-10-09-category-hardening/manual-tests/fixtures/capacity/run.py --profile quick
```

The quick profile exercises the same publication/failure machinery; **it is not
capacity acceptance evidence**. Every run gets a fresh directory beneath
gitignored `release-band1-evidence/capacity/`. Existing production audit backups,
normal local Docker services and examples source files are untouched.

## What runs

The normal Registry application starts through `gunicorn run:app`, including
Pyway migrations, route registration and scheduled jobs. Its container has a
**0.5 CPU quota, 512 MiB memory ceiling and no swap**. PostgreSQL, Redis, the
driver and Moto's S3 HTTP emulator run in separate containers. No host ports are
published. Their Docker network is internal, so application jobs cannot contact
production or external AWS services. The build copies application source through
a restricted context without host dotenv files, virtual environments or Git data.
Only disposable test credentials are configured.

Both enforcement settings use real authentication, author signatures, archive
uploads, database transactions and the actual publication endpoints. The
test-only Gunicorn hook measures S3 response streams/extraction and disables rate
pacing to isolate finalize capacity. This is not a test of production rate limits.
Timeout hooks exercise both a storage-read delay and hard kills during root/dependency extraction. For the extraction probes only, SIGABRT is ignored so the watchdog escalates to SIGKILL; Python finally blocks cannot perform cleanup. The actual production Gunicorn hooks must reclaim the orphaned directories and named snapshot. These delays are not normal-workload latency.

| Workload                                            | Purpose                                                                                          |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Largest complete local Agent and Template graphs    | Preserve actual dependency/binding relationships and packaged content from the examples checkout |
| Synthetic 20-Tool/3-Skill fan-out and diamond graph | Increase graph work; shared exact references should benefit from the request-local cache         |
| Three versions of the same Agent graph              | Measure repeated inspections without a cross-request cache                                       |
| 32 MiB incompressible payload                       | Exercise substantial compressed-byte transfers                                                   |
| 128 MiB highly compressible payload                 | Separate expanded-byte/disk work from compressed size                                            |
| Two-target Python Tool release                      | Exercise the multi-artifact protocol alongside ordinary single-archive publication               |
| Two concurrent Agent finalizes                      | Measure queueing/resource use with the configured worker model                                   |
| Skill archive missing its declared `SKILL.md`       | Check real APDS rejection after extraction, absence of a published version and cleanup           |
| Controlled storage and extraction timeouts          | Check real SIGKILL, production-hook reclamation, unpublished versions and cleanup                |

The real-example selector considers authored `agent-packages/` and
`template-packages/` roots, recursively traverses available local dependencies,
and selects the largest complete graph of each kind by unique node count. The
inventory records every candidate, missing reference and selected source. It does
not claim to inventory every artifact in production.

These are **new test artifacts**: names/namespace and versions are changed,
standard declarations are added, and references are consistently rebased. Tool
runtimes become inert Node fixtures; the multi-target case uses Python. No model
or Tool executes. Actual Knowledge/Memory content is rebuilt by the CLI. Every
authored source goes through `agentpm lint`; only the rejection case omits a
required file from the archive after source lint. Published package bytes and
versions are never edited or republished.

## Where results are and how to read them

Open the printed run directory in your editor.

| File                                                | Meaning                                                                                                                 |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `REPORT.md`                                         | Human-readable off/on workload measurements and limitations                                                             |
| `measurements.json`                                 | Per-request root/dependency GETs, bytes, extraction work, latency, memory/temp samples and cleanup observations         |
| `events.jsonl`                                      | Raw timestamped worker settings, cgroup memory/CPU, process RSS, temp-disk samples, storage reads and extraction events |
| `requests/`                                         | HTTP outcomes, exact artifact compressed/expanded sizes and request IDs                                                 |
| `example-inventory.json`                            | Real graph selection, source revision and adaptations                                                                   |
| `plan-off.json`, `plan-on.json`                     | Exact generated workloads for each setting                                                                              |
| `sources/`, `archives/`                             | Visible manifest/content sources and uploaded archives                                                                  |
| `lint.log`                                          | Actual CLI build/lint results for authored inputs                                                                       |
| `container-off.json`, `container-on.json`           | Applied Docker limits and container state                                                                               |
| `setup.json`, `source-checksums.json`, `context/`   | Revisions, actual source snapshot and hashes used by the image                                                          |
| `registry.log`, `services.log`, `orchestration.log` | Runtime/build diagnostics, including expected timeout output                                                            |
| `execution.json`                                    | Whether execution and teardown completed; never grants activation approval                                              |
| `local-operational-probe.json`                      | Filesystem/config and live Redis confirmation with normal production hooks; local evidence only                         |

Samples are collected inside the constrained Registry container every 200 ms.
Temporary-disk accounting includes named files and open unnamed/deleted `/tmp`
file descriptors, deduplicated by inode. Cgroup `memory.peak` also captures memory
peaks between samples. Sampled RSS is separate from cgroup memory, which includes
cache and other container overhead. Health checks and measurement hooks add some
overhead. Concurrent rows share container-level peaks; they are not independent
per-request memory allocations. Raw samples permit review of cleanup and worker
replacement rather than inferring them from an HTTP response alone.

Successful execution means the expected publications/rejections occurred and
measurement evidence was collected. Unexpected 401/403/429/500 responses, OOMs,
missing workloads or broken instrumentation are failures. The labeled controlled timeouts are expected to interrupt their requests; each worker must restart and its version must remain unpublished. Extraction-kill cases must return to the same temporary-file baseline after the master cleanup hook.

## Acceptance and production limits

The operator approved this local approach instead of provisioning another Render
service. This adjustment changes how evidence is collected; it does not
automatically approve activation or certify the maximum advertised upload size.

CPU quota is not equivalent processor speed. A local ARM Linux container is not
an x86 Render host. Local disk and Moto do not reproduce production S3 latency,
network behavior or Render proxy limits. Confirm effective production workers,
timeouts and disk headroom, then have Claude review those differences, the saved
history audit and measured headroom together. A passing sample does not prove
all 256-node graphs or 1 GB artifacts fit. If the acceptable envelope requires
lower limits, concurrency changes or more capacity, verify that mitigation
before activation.

**Production enforcement stays off.** Separate activation approval, coordinated
publishing pause/drain and small controlled production smoke checks remain in the
release checklist. Do not run the stress workload against production.

## Production confirmation after the cleanup deploy

After review, CI and deployment of the cleanup mitigation **with enforcement still
off**, run this from the existing Render shell directory containing `run.py`:

```bash
python scripts/inspect_publication_capacity.py
```

Retain its JSON output privately with the capacity evidence. This reads the
recorded `gunicorn run:app` configuration and the shell's environment, reports
the deepest `/proc/self/mountinfo` mount backing the temporary directory and
cgroup memory accounting, and checks Redis against the running worker. The only
HTTP request is a loopback `GET /health` with a fresh documentation-range IP
identity; new Redis counters for that identity must appear. Redis merely being
reachable is insufficient. The request creates ordinary expiring rate-limit
counters; there is no publication, migration, application startup or artifact
mutation. The probe prints selected settings and booleans, not credentials.
It does not inspect a live process's argv; confirm the start command remains
`gunicorn run:app` as already recorded.

Expected: one sync worker, one thread, 30-second timeout, enforcement false,
filesystem type known, sufficient free disk, and
`rate_limiter.redis_request_confirmed: true`. `inspection_named_paths` is a name
inventory, not a claim that live directories are orphaned. Unknown/legacy names
are never automatically removed; review any such paths while publishing is
paused/drained before activation. The new locked ownership protocol protects
live inspection directories across workers/masters and reclaims only dead work.
Historical audit directories are outside its namespace.

The operator confirmed the health-check URL is
`https://api.agentpackagemanager.com/health` and that there is **no additional
proxy/CDN**. Render documents [responses up to 100 minutes](https://render.com/docs/render-vs-heroku-comparison),
so a confirmed 30-second Gunicorn watchdog is the earlier server-side cutoff.
Its [HTTP health checks](https://render.com/docs/health-checks) need a response
within five seconds; consecutive failures remove traffic after 15 seconds and
restart the instance after 60 seconds. These are platform policies, not evidence
of a production stress test. A blocked single sync worker can therefore fail
health checks and temporarily lose routing. Sustained queueing or successive slow
finalizes can extend that interruption. Do not claim the restart-loop risk is
excluded by the local two-client probe.

[Cgroup memory accounting](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory) includes filesystem cache. Disk-backed `/tmp` is not automatically
excluded from the memory measurement, and adding all observed temporary bytes to
the measured peak would double-count some memory. Use filesystem type plus the
reported `memory_stat` file/shmem fields for the production comparison.

The builder tested the config loader with WSGI import forbidden. The automated
local harness separately restarts using the **normal production Gunicorn config**
with rate limiting enabled and verifies the complete live Redis probe. That local
result does not substitute for the operator's production output or Claude's
separate activation approval.

### Render's automatically supplied preload flag

For its native Python runtime, Render [automatically supplies `GUNICORN_CMD_ARGS`](https://render.com/docs/environment-variables) including `--preload`. It need not appear as a variable the operator manually added. The original production probe reported `preload_app=true`, unlike the measured local setup.

To align with that tested lifecycle, explicitly add this service environment override:

```text
GUNICORN_CMD_ARGS=--access-logfile - --bind=0.0.0.0:10000
```

Keep enforcement off during redeployment, then rerun the operational probe and require `preload_app=false` along with the previously listed checks. This retains Render's documented default bind/log settings. If the service uses a different configured port, preserve that actual bind instead.

Registry startup uses the database and starts its scheduler. Loading that startup before worker fork changes connection/thread ownership; [pooled DB connections must not be shared across fork](https://docs.sqlalchemy.org/en/20/core/pooling.html#using-connection-pools-with-multiprocessing-or-os-fork). The cleanup hooks alone do not resolve that lifecycle concern. The operator override is recommended for the measured configuration; it has not been applied by the builder. Activation still requires the updated production output and separate approval.
