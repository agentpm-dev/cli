# M8A local web fixture helper

See [the runbook](../../WEB_M8A_MANUAL_TESTS.md). This standard-library Python
helper reads existing M4A/M5A publication receipts/logs and copies authoring
manifests into an ignored, owned run. It does not import the Registry app, read
credentials, publish, change namespaces or contact S3.

`prepare` is offline. `check` requests the real local API/web and inspects
rendered HTML, complete raw JSON, validated APDS projections and SEO/headers.
`observe` records explicit operator observations. `private` combines a clearly
operator-reported signed-in result with two real anonymous requests. `audit`
requires every part and leaves final operator sign-off outside the helper.

The generated index opens real app routes. It is a browser checklist, not another
product page. Old runs are untouched, implementation/input changes are detected,
failed retries invalidate prior current results, and attempts are retained.

Builder checks, from the CLI repo:

```sh
PYTHONDONTWRITEBYTECODE=1 ../agentpm-api/.venv/bin/python -B -m pytest -q specs/2026-10-09-category-hardening/manual-tests/fixtures/m8a/test_fixture.py
```

Unit tests use canonical manifest fixtures, temporary directories and controlled
responses. They do not count as real browser/Registry/private acceptance. The
builder's live GET replay, when run, is recorded separately in the milestone
verification notes; manual observations are never fabricated for that replay.
