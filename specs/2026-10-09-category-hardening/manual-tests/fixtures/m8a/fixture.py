"""Read-only M8A browser/API checks using existing local publications."""

import argparse
from datetime import datetime, timezone
import hashlib
from html import escape
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import shlex
import time
from urllib.error import HTTPError
from urllib.parse import quote, urlsplit
from urllib.request import HTTPRedirectHandler, ProxyHandler, Request, build_opener
import uuid
import xml.etree.ElementTree as ET

SOURCE = Path(__file__).resolve()
REPO = SOURCE.parents[5]
STANDARD = {"id": "agentpm", "version": "1.0.0"}
REFERENCE = "/standards/agentpm/1.0.0"
CANONICAL = "https://agentpackagemanager.com" + REFERENCE
SCHEMA = "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json"
ROUTES = dict(
    tool="tools",
    agent="agents",
    skill="skills",
    template="templates",
    knowledge="knowledge",
    memory="memory",
    profile="profiles",
    loop="loops",
)
OBSERVATIONS = (
    "reference",
    "source-links",
    "keyboard",
    "packages",
    "versions",
    "evidence",
    "responsive",
)


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def stamp():
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")


def local_origin(value):
    parsed = urlsplit(value)
    if (
        parsed.scheme != "http"
        or parsed.hostname not in ("localhost", "127.0.0.1", "::1")
        or parsed.username
        or parsed.password
        or parsed.path not in ("", "/")
        or parsed.query
        or parsed.fragment
    ):
        raise ValueError(
            "Use a plain HTTP loopback origin, e.g. http://localhost:3000; no remote servers"
        )
    return value.rstrip("/")


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError(
            f"Unexpected redirect from {req.full_url}; no redirect was followed"
        )


def fetch(origin, path):
    """GET only, no authentication/proxy environment, redirects or remote origins."""
    origin = local_origin(origin)
    if not path.startswith("/") or path.startswith("//"):
        raise ValueError("Expected a relative local path")
    request = Request(
        origin + path, headers={"User-Agent": "agentpm-m8a-manual/1", "DNT": "1"}
    )
    opener = build_opener(ProxyHandler({}), NoRedirect())
    time.sleep(0.12)
    try:
        response = opener.open(request, timeout=30)
    except HTTPError as error:
        response = error
    with response:
        body = response.read(5 * 1024 * 1024 + 1)
        assert len(body) <= 5 * 1024 * 1024, "Unexpectedly large local response"
        return response.code, {k.lower(): v for k, v in response.headers.items()}, body


class Page(HTMLParser):
    """Inspect rendered HTML, excluding scripts and hidden framework templates."""

    def __init__(self, body):
        super().__init__()
        self.ids, self.links, self.text, self.canonicals, self.robots = (
            set(),
            [],
            [],
            [],
            [],
        )
        self.ignored = 0
        self.active_link = None
        self.feed(body.decode("utf-8"))

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag in ("script", "style", "template"):
            self.ignored += 1
        if self.ignored:
            return
        if attrs.get("id"):
            self.ids.add(attrs["id"])
        if tag == "a":
            self.active_link = [attrs.get("href", ""), ""]
            self.links.append(self.active_link)
        if tag == "link" and attrs.get("rel") == "canonical":
            self.canonicals.append(attrs.get("href"))
        if tag == "meta" and attrs.get("name") == "robots":
            self.robots.append(attrs.get("content", ""))

    def handle_endtag(self, tag):
        if tag in ("script", "style", "template") and self.ignored:
            self.ignored -= 1
        if tag == "a" and not self.ignored:
            self.active_link = None

    def handle_data(self, value):
        if not self.ignored:
            self.text.append(value)
            if self.active_link is not None:
                self.active_link[1] += value

    def contains(self, value):
        # React separates interpolated text with HTML comments, not spaces.
        return value in " ".join("".join(self.text).split())


def paths(entry):
    base = f"/{ROUTES[entry['kind']]}/{entry['id']}/v{quote(entry['version'], safe='')}"
    return dict(
        overview=base + "/overview",
        security=base + "/security",
        raw=f"/packages/{entry['id']}/{quote(entry['version'], safe='')}/agent.json",
    )


def implementation_pins():
    files = [
        REPO.parent / "agentpm-web" / name
        for name in (
            "src/components/tools/PackageDefinition.tsx",
            "src/components/tools/PackageConformance.tsx",
            "src/lib/security-schema.ts",
            "src/lib/seo.ts",
            "src/lib/get-package-manifest.ts",
            "src/lib/apds-catalog.ts",
            "src/content/standards/agentpm-1.0.0.json",
            "src/app/packages/[packageId]/[version]/agent.json/route.ts",
            "src/app/standards/[standardId]/[version]/page.tsx",
        )
    ]
    files += [
        REPO.parent / "agentpm-api" / name
        for name in (
            "app/tools/application/package_health.py",
            "app/tools/application/tools.py",
            "app/tools/common/dtos.py",
        )
    ]
    return {str(path): digest(path) for path in files}


def load(root):
    session = read(root / "session.json")
    assert session["root"] == str(root), "Wrong run directory"
    assert (
        digest(SOURCE) == session["helper_sha256"]
    ), "Helper changed; prepare a fresh run"
    assert (
        digest(root / "catalog.json") == session["catalog_sha256"]
    ), "Catalog changed; prepare a fresh run"
    for path, expected in session["implementation_pins"].items():
        assert (
            digest(Path(path)) == expected
        ), f"Implementation changed: {path}; prepare a fresh run"
    for path, expected in session["input_pins"].items():
        assert (
            digest(root / path) == expected
        ), f"Copied input changed: {path}; prepare a fresh run"
    local_origin(session["web"])
    local_origin(session["api"])
    return session, read(root / "catalog.json")


def prepare(root, m4a, m5a, web, api):
    local_origin(web)
    local_origin(api)
    assert (
        root.parent == (REPO / "web-m8a-test").resolve()
    ), "Run must be directly under agentpm/web-m8a-test/"
    assert not root.exists(), "Refusing to overwrite an existing run"
    catalog = {}
    for kind in ROUTES:
        folder = m4a / "cases" / kind
        receipt, manifest = read(folder / "receipt.json"), read(
            folder / "source/agent.json"
        )
        identity = str(uuid.UUID(receipt["package_id"]))
        catalog[kind] = dict(
            id=identity,
            kind=kind,
            name=manifest["name"],
            version=receipt["version"],
            origin="declared",
            source=str(folder),
            manifest=manifest,
        )
    for label in ("echo-v1", "echo-v2", "legacy"):
        folder = m5a / "cases" / label
        manifest = read(folder / "source/agent.json")
        output = (m5a / "evidence" / f"publish-{label}.stdout.txt").read_text()
        identities = set(
            re.findall(
                r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", output
            )
        )
        assert len(identities) == 1, f"Could not identify published package for {label}"
        catalog[label] = dict(
            id=str(uuid.UUID(identities.pop())),
            kind="tool",
            name=manifest["name"],
            version=manifest["version"],
            origin="legacy_inferred" if label == "legacy" else "declared",
            source=str(folder),
            manifest=manifest,
        )
    assert (
        catalog["echo-v1"]["id"] == catalog["echo-v2"]["id"]
    ), "Version pair must be the same package"
    root.mkdir(parents=True, mode=0o700)
    for label, entry in catalog.items():
        write(root / "fixtures" / label / "source-manifest.json", entry.pop("manifest"))
        entry["paths"] = paths(entry)
    write(root / "catalog.json", catalog)
    write(
        root / "session.json",
        dict(
            root=str(root),
            web=web,
            api=api,
            helper_sha256=digest(SOURCE),
            catalog_sha256=digest(root / "catalog.json"),
            implementation_pins=implementation_pins(),
            input_pins={
                str(path.relative_to(root)): digest(path)
                for path in (root / "fixtures").rglob("*.json")
            },
            sources=dict(m4a=str(m4a), m5a=str(m5a)),
            scope="Existing local publications; GET only; no publication or namespace changes",
        ),
    )
    values = dict(
        M8A_REPO=str(REPO),
        M8A_ROOT=str(root),
        M8A_FIXTURE=str(SOURCE),
        M8A_WEB=web,
        M8A_API=api,
    )
    shell = "\n".join(f"export {k}={shlex.quote(v)}" for k, v in values.items())
    shell += '\nm8a_fixture() { python3 -B "$M8A_FIXTURE" "$@" --root "$M8A_ROOT"; }\n'
    (root / "paths.sh").write_text(shell)
    index(root)
    print(
        f"Prepared {len(catalog)} existing releases. No network requests or writes to services.\nsource {shlex.quote(str(root / 'paths.sh'))}\nBrowser index: {root / 'index.html'}"
    )


def index(root):
    session, catalog = load(root)
    expectations = (
        read(root / "expectations.json")
        if (root / "expectations.json").exists()
        else {}
    )
    rows = []
    for label, entry in catalog.items():
        links = " · ".join(
            f'<a href="{escape(session["web"] + path)}">{title}</a>'
            for title, path in entry["paths"].items()
        )
        expected = expectations.get(label, {})
        rows.append(
            f'<tr><td>{escape(label)}</td><td>{escape(entry["kind"])}<br>{escape(entry["name"])}@{escape(entry["version"])}</td><td>{escape(entry["origin"])}<br>{escape(str(expected.get("result", "Run check first")))}</td><td>{links}</td></tr>'
        )
    observations = ", ".join(OBSERVATIONS)
    (root / "index.html").write_text(
        f"""<!doctype html><html lang="en"><meta charset="utf-8"><title>M8A local manual checks</title><style>body{{font:16px/1.5 system-ui;margin:2rem;max-width:100rem}}td,th{{padding:.7rem;border:1px solid #ccc;text-align:left;vertical-align:top}}table{{border-collapse:collapse}}code{{overflow-wrap:anywhere}}</style><h1>M8A — local published fixtures</h1><p>Run folder: <code>{escape(str(root))}</code>. Existing publications only; original runs are untouched.</p><p><a href="{escape(session['web'] + REFERENCE)}">APDS 1.0.0 reference</a> · <a href="{escape(session['web'] + '/standards/agentpm/9.0.0')}">Unknown version (404)</a> · <a href="{escape(session['web'] + '/tools/' + catalog['echo-v1']['id'] + '/latest/overview')}">Tool latest (must select 2.0.0)</a></p><p>Use the normal signed-in browser for private access; the helper is always anonymous. <a href="expectations.json">Live expected evidence</a> is generated by <code>m8a_fixture check</code>. Record observations in the terminal: {observations}.</p><table><thead><tr><th>Fixture</th><th>Published release</th><th>Expected interpretation / live result</th><th>Local web links</th></tr></thead><tbody>{''.join(rows)}</tbody></table><p>The copied <code>fixtures/*/source-manifest.json</code> files show original authoring inputs. The web raw-manifest response must match the live Registry stored manifest, which is saved by check; this is not an archive-byte claim.</p></html>""",
        encoding="utf-8",
    )


def raw_headers(headers):
    assert headers.get("cache-control") == "private, no-store", headers
    assert "noindex" in headers.get("x-robots-tag", ""), headers
    assert headers.get("x-content-type-options") == "nosniff", headers
    assert headers.get("content-type", "").startswith("application/json"), headers


def evidence_page(page, expected):
    if expected["origin"] == "legacy_inferred":
        assert page.contains("Inferred legacy interpretation")
        assert not page.contains("Registry-recorded APDS result:")
    elif expected["api_status"] in ("unknown", "not_evaluated"):
        assert page.contains("APDS conformance evidence unavailable or not evaluated.")
        assert not page.contains("Registry-recorded APDS result:")
    else:
        assert page.contains(
            f"Registry-recorded APDS result: {expected['result']}"
        ), "Web/API projection differs"
        for level, report in expected["levels"].items():
            assert page.contains(
                f"{level}: {report['status']}; {len(report['unevaluated'])} unevaluated rules."
            ), f"Missing projected level: {level}"
        if expected["evaluated_at"] is None:
            assert page.contains("Evaluation time unavailable.")


def invalidate_summary(root):
    write(
        root / "evidence/summary.json",
        dict(
            status="incomplete",
            operator_signoff=False,
            note="Run audit again after completing the current checks and observations",
        ),
    )


def check(root):
    session, catalog = load(root)
    invalidate_summary(root)
    attempt = root / "evidence" / stamp()
    write(
        root / "checks.json",
        dict(status="running", evidence=str(attempt.relative_to(root))),
    )
    results, expectations = [], {}

    def get(origin, path):
        status, headers, body = fetch(origin, path)
        number = len(results)
        write(
            attempt / f"{number:03d}.http.json",
            dict(origin=origin, path=path, status=status, headers=headers),
        )
        (attempt / f"{number:03d}.body").write_bytes(body)
        results.append(dict(path=path, status=status))
        return status, headers, body

    status, _, body = get(session["web"], REFERENCE)
    page = Page(body)
    assert status == 200 and page.canonicals == [
        CANONICAL
    ], "Reference/canonical mismatch"
    assert page.contains("AgentPM APDS 1.0.0") and page.contains(
        "A structural pass alone does not establish full conformance"
    ), "Reference content missing"
    assert SCHEMA in [href for href, _ in page.links], "Canonical schema link missing"
    assert (
        len([key for key in page.ids if key.startswith("apds-")]) > 40
    ), "Rule anchors missing"
    for href, _ in page.links:
        if href.startswith("#") and href != "#":
            assert href[1:] in page.ids, f"Broken local anchor: {href}"
        if href.startswith(
            "https://github.com/agentpm-dev/cli/"
        ) and not href.startswith("https://github.com/agentpm-dev/cli/issues"):
            assert "/apds-v1.0.0/" in href and "/main/" not in href, href
    for path in ("/standards/agentpm/9.0.0", "/standards/unknown/1.0.0"):
        status, _, body = get(session["web"], path)
        page = Page(body)
        # Next may stream its not-found boundary for client hydration. Inspect
        # HTTP/SEO here; the operator verifies the rendered explanation.
        assert status == 404, "Unknown standard did not return HTTP 404"
        assert not page.canonicals and any(
            "noindex" in rule for rule in page.robots
        ), "Unknown standard SEO mismatch"
    for label, entry in catalog.items():
        api_path = f"/tools/{entry['id']}/{quote(entry['version'], safe='')}"
        status, _, body = get(session["api"], api_path)
        assert (
            status == 200
        ), f"{label}: existing fixture missing from local API ({status}); do not republish blindly"
        detail = json.loads(body)
        manifest = detail["manifest"]
        assert (manifest["kind"], manifest["name"], detail["version"]) == (
            entry["kind"],
            entry["name"],
            entry["version"],
        ), label
        assert manifest.get("standard") == (
            None if entry["origin"] == "legacy_inferred" else STANDARD
        ), label
        status, _, body = get(session["api"], api_path + "/security")
        assert status == 200, f"{label}: Security unavailable ({status})"
        security = json.loads(body)
        assert (
            "apds_evidence" in security
        ), "Restart local API with reviewed code: apds_evidence missing (or wait for the existing public Security cache to expire)"
        signals = {value["signal_key"]: value for value in security["apds_evidence"]}
        declaration, conformance = (
            signals["apds_declaration"],
            signals["apds_conformance"],
        )
        assert declaration["value"]["standard_origin"] == entry["origin"], label
        assert conformance["status"] in (
            ("not_evaluated",)
            if entry["origin"] == "legacy_inferred"
            else ("passed", "incomplete", "unknown", "not_evaluated")
        ), f"{label}: unexpected APDS evidence {conformance['status']}"
        expectations[label] = dict(
            origin=entry["origin"],
            result=conformance["value"].get("status", conformance["status"]),
            api_status=conformance["status"],
            reason=conformance.get("reason"),
            levels=conformance["value"].get("levels", {}),
            evaluated_at=conformance["evidence"].get("evaluated_at"),
            version_id=detail["tool_version_id"],
        )
        status, headers, body = get(session["web"], entry["paths"]["raw"])
        assert (
            status == 200 and json.loads(body) == manifest
        ), f"{label}: web raw manifest differs from selected API record"
        raw_headers(headers)
        status, _, body = get(session["web"], entry["paths"]["overview"])
        page = Page(body)
        assert status == 200, f"{label}: overview status {status}"
        assert [
            entry["paths"]["raw"],
            "View agent.json",
        ] in page.links, f"{label}: selected raw link missing"
        assert any(
            href == entry["paths"]["security"] and "APDS evaluation" in text
            for href, text in page.links
        ), label
        expected_copy = (
            "Legacy manifest: no explicit APDS declaration"
            if entry["origin"] == "legacy_inferred"
            else "APDS declaration: agentpm / 1.0.0"
        )
        assert page.contains(
            expected_copy
        ), f"{label}: declaration/legacy label missing"
        status, _, body = get(session["web"], entry["paths"]["security"])
        page = Page(body)
        assert status == 200, f"{label}: Security page status {status}"
        evidence_page(page, expectations[label])
        print(
            f"PASS {label}: {entry['kind']} {entry['version']} | {entry['origin']} | {expectations[label]['result']}"
        )
    assert any(
        value["api_status"] in ("passed", "incomplete")
        for value in expectations.values()
    ), "Need at least one actual usable recorded evaluation; every case was unavailable"
    latest = catalog["echo-v2"]
    status, _, body = get(session["web"], f"/tools/{latest['id']}/latest/overview")
    assert (
        status == 200
        and [latest["paths"]["raw"], "View agent.json"] in Page(body).links
    ), "latest raw link is not concrete 2.0.0"
    assert (
        expectations["echo-v1"]["version_id"] != expectations["echo-v2"]["version_id"]
    ), "Version pair did not select distinct rows"
    status, _, body = get(session["web"], "/sitemap/pages.xml")
    assert status == 200 and CANONICAL in [
        node.text for node in ET.fromstring(body).iter() if node.tag.endswith("}loc")
    ], "Reference missing from pages sitemap; check normal local docs/blog ingestion"
    write(root / "expectations.json", expectations)
    write(
        attempt / "result.json",
        dict(status="pass", fixtures=len(catalog), requests=len(results)),
    )
    write(
        root / "checks.json",
        dict(
            status="pass",
            evidence=str(attempt.relative_to(root)),
            fixtures=len(catalog),
            requests=len(results),
        ),
    )
    index(root)
    print(
        f"PASS {len(catalog)} releases; {len(results)} local GETs. Evidence: {attempt}"
    )


def observe(root, label, result, notes):
    load(root)
    assert label in OBSERVATIONS, "Unknown observation"
    invalidate_summary(root)
    path = root / "observations.jsonl"
    with path.open("a", encoding="utf-8") as stream:
        stream.write(
            json.dumps(
                dict(
                    label=label,
                    result=result,
                    notes=notes,
                    at=stamp(),
                    operator_reported=True,
                )
            )
            + "\n"
        )
    print(f"Recorded operator observation: {label} = {result}")


def private(root, url, authorized_status):
    session, _ = load(root)
    invalidate_summary(root)
    write(root / "private.json", dict(status="pending", at=stamp()))
    parsed = urlsplit(url)
    assert (
        f"{parsed.scheme}://{parsed.netloc}" == session["web"]
        and not parsed.query
        and not parsed.fragment
    ), "Use the configured local raw-manifest URL"
    match = re.fullmatch(r"/packages/([0-9a-f-]{36})/([^/]+)/agent\.json", parsed.path)
    assert (
        match and str(uuid.UUID(match[1])) == match[1]
    ), "Expected an exact local raw-manifest URL"
    assert (
        authorized_status == 200
    ), "First observe a real signed-in HTTP 200 in the browser; a random/missing ID is not a private-access test"
    results = []
    for origin, path in [
        (session["web"], parsed.path),
        (session["api"], f"/tools/{match[1]}/{match[2]}"),
    ]:
        status, headers, body = fetch(origin, path)
        assert status == 404, f"Anonymous private access was not hidden: {status}"
        if origin == session["web"]:
            raw_headers(headers)
            assert json.loads(body) == {
                "error": "Package manifest not found"
            }, "Private raw response must not contain a manifest"
        results.append(dict(origin=origin, path=path, status=status))
        assert not Page(body).contains(
            "APDS declaration:"
        ), "Unexpected private page response"
    write(
        root / "private.json",
        dict(
            status="pass",
            authorized_status=authorized_status,
            authorized_observation="operator-reported signed-in browser result, not an automated authenticated request",
            anonymous=results,
            at=stamp(),
        ),
    )
    print(
        "PASS anonymous web/API both 404; signed-in 200 retained as an explicit operator observation"
    )


def audit(root):
    load(root)
    invalidate_summary(root)
    assert (root / "checks.json").exists(), "Run m8a_fixture check first"
    checks = read(root / "checks.json")
    assert (
        checks["status"] == "pass"
        and read(root / checks["evidence"] / "result.json")["status"] == "pass"
    ), "Run check successfully first"
    latest = {}
    path = root / "observations.jsonl"
    if path.exists():
        for line in path.read_text().splitlines():
            value = json.loads(line)
            latest[value["label"]] = value
    missing = [
        label for label in OBSERVATIONS if latest.get(label, {}).get("result") != "pass"
    ]
    assert (
        not missing
    ), f"Browser observations still missing/failed: {', '.join(missing)}"
    assert (root / "private.json").exists() and read(root / "private.json")[
        "status"
    ] == "pass", "Complete the real private-package check first"
    summary = dict(
        status="pass",
        checked_fixtures=checks["fixtures"],
        local_get_requests=checks["requests"],
        browser_observations=len(OBSERVATIONS),
        private_authorized="operator-reported 200",
        private_anonymous="automated web and API 404",
        operator_signoff=False,
        scope="Real local API/web read-only checks plus explicit browser observations; no production requests or new publications",
    )
    write(root / "evidence/summary.json", summary)
    print(json.dumps(summary, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ("prepare", "check", "describe", "observe", "private", "audit"):
        item = sub.add_parser(command)
        item.add_argument("--root", type=Path, required=True)
        if command == "prepare":
            item.add_argument("--m4a-run", type=Path, required=True)
            item.add_argument("--m5a-run", type=Path, required=True)
            item.add_argument("--web", default="http://localhost:3000")
            item.add_argument("--api", default="http://127.0.0.1:5000")
        elif command == "observe":
            item.add_argument("label", choices=OBSERVATIONS)
            item.add_argument("result", choices=("pass", "fail"))
            item.add_argument("--notes", default="")
        elif command == "private":
            item.add_argument("--url", required=True)
            item.add_argument("--authorized-status", type=int, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    if args.command == "prepare":
        prepare(
            root,
            args.m4a_run.resolve(),
            args.m5a_run.resolve(),
            local_origin(args.web),
            local_origin(args.api),
        )
    elif args.command == "check":
        check(root)
    elif args.command == "describe":
        session, catalog = load(root)
        print(
            json.dumps(
                dict(web=session["web"], api=session["api"], fixtures=catalog), indent=2
            )
        )
    elif args.command == "observe":
        observe(root, args.label, args.result, args.notes)
    elif args.command == "private":
        private(root, args.url, args.authorized_status)
    else:
        audit(root)


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, ValueError, OSError, KeyError) as error:
        raise SystemExit(f"M8A check stopped: {error}") from error
