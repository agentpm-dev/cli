import hashlib
import io
import json
import os

# Only the exception type is real; process execution is mocked.
import subprocess  # nosec B404
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import Mock, patch

import check_resolver_targets
import check_resolver_mirror
import resolver_pins
import resolver_smoke


def smoke_pylock():
    packages = []
    for name, version in [
        ("click", "8.1.8"),
        ("pydantic", "2.11.7"),
        ("colorama", "0.4.6"),
    ]:
        packages.append(
            f'[[packages]]\nname = "{name}"\nversion = "{version}"\n'
            + ("marker = \"sys_platform == 'win32'\"\n" if name == "colorama" else "")
            + '[[packages.wheels]]\nhashes = {sha256 = "'
            + "a" * 64
            + '"}\n'
        )
    packages.append('[[packages]]\nname = "pydantic-core"\nversion = "2.33.2"\n')
    for index, filename in enumerate(
        [
            "core-cp311-cp311-macosx_11_0_arm64.whl",
            "core-cp311-cp311-macosx_10_12_x86_64.whl",
            "core-cp311-cp311-manylinux_2_17_x86_64.whl",
            "core-cp311-cp311-win_amd64.whl",
        ]
    ):
        packages.append(
            f'[[packages.wheels]]\nurl = "https://index.test/{filename}"\n'
            'hashes = {sha256 = "' + str(index) * 64 + '"}\n'
        )
    packages.append('[packages.sdist]\nhashes = {sha256 = "' + "f" * 64 + '"}\n')
    return 'lock-version = "1.0"\n' + "".join(packages)


class ResolverPinsTests(unittest.TestCase):
    def test_read_only_checks_do_not_import_native_smoke_or_tomllib(self):
        # A fresh process ensures this test module's smoke import cannot hide coupling.
        result = subprocess.run(  # nosec B603
            [
                sys.executable,
                "-B",
                "-c",
                "import sys, check_resolver_targets, check_resolver_mirror; "
                "assert 'resolver_smoke' not in sys.modules; assert 'tomllib' not in sys.modules",
            ],
            cwd=resolver_pins.ROOT / "scripts",
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_old_python_ingestion_fails_with_explicit_requirement_before_work(self):
        with patch.object(sys, "version_info", (3, 9, 6)), patch(
            "sys.argv",
            ["resolver_pins.py", "0.13.5", "--stage-only", "--staging", "unused"],
        ), patch("sys.stderr", new_callable=io.StringIO) as stderr, patch.object(
            resolver_pins, "generate"
        ) as generate:
            with self.assertRaises(SystemExit) as raised:
                resolver_pins.main()
            self.assertEqual(raised.exception.code, 2)
            self.assertIn("Python 3.11+", stderr.getvalue())
            self.assertIn("../agentpm-api/.venv/bin/python", stderr.getvalue())
            generate.assert_not_called()

    def test_verifier_requires_a_successful_signature_check_for_the_exact_repo(self):
        with tempfile.TemporaryDirectory() as root:
            archive = Path(root) / "uv.tar.gz"
            archive.write_bytes(b"archive bytes")
            response = Mock()
            response.__enter__ = Mock(return_value=response)
            response.__exit__ = Mock(return_value=False)
            response.read.return_value = json.dumps(
                {"attestations": [{"bundle": {"fixture": "invalid"}}]}
            ).encode()
            with patch.object(
                resolver_pins.urllib.request, "urlopen", return_value=response
            ), patch.object(
                resolver_pins.subprocess,
                "run",
                side_effect=subprocess.CalledProcessError(1, "gh"),
            ) as verify:
                with self.assertRaises(subprocess.CalledProcessError):
                    resolver_pins.verify_archive(archive, "gh")
                command = verify.call_args.args[0]
                self.assertEqual(command[command.index("--repo") + 1], "astral-sh/uv")
                self.assertIn("--bundle", command)
                self.assertTrue(verify.call_args.kwargs["check"])

    def test_attestation_request_uses_available_token_and_allows_anonymous_use(self):
        cases = [
            (
                {"GH_TOKEN": "fixture-gh", "GITHUB_TOKEN": "fixture-actions"},
                "fixture-gh",
            ),
            ({"GITHUB_TOKEN": "fixture-actions"}, "fixture-actions"),
            ({"GH_TOKEN": "", "GITHUB_TOKEN": "fixture-actions"}, "fixture-actions"),
            ({}, None),
        ]
        with tempfile.TemporaryDirectory() as root:
            archive = Path(root) / "uv.tar.gz"
            archive.write_bytes(b"archive bytes")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            for environment, expected in cases:
                with self.subTest(environment=environment), patch.dict(
                    os.environ, environment, clear=True
                ), patch.object(
                    resolver_pins.urllib.request,
                    "urlopen",
                    return_value=io.BytesIO(
                        json.dumps(
                            {"attestations": [{"bundle": {"fixture": True}}]}
                        ).encode()
                    ),
                ) as download, patch.object(
                    resolver_pins.subprocess, "run"
                ) as verify:
                    resolver_pins.verify_archive(archive, "gh")
                    request = download.call_args.args[0]
                    self.assertEqual(
                        request.full_url,
                        f"https://api.github.com/repos/astral-sh/uv/attestations/sha256:{digest}",
                    )
                    self.assertEqual(
                        request.get_header("Authorization"),
                        f"Bearer {expected}" if expected else None,
                    )
                    self.assertEqual(
                        request.get_header("Accept"), "application/vnd.github+json"
                    )
                    self.assertTrue(verify.call_args.kwargs["check"])

    def test_authenticated_attestation_failure_is_not_retried_anonymously(self):
        with tempfile.TemporaryDirectory() as root:
            archive = Path(root) / "uv.tar.gz"
            archive.write_bytes(b"archive bytes")
            with patch.dict(
                os.environ, {"GH_TOKEN": "fixture-gh"}, clear=True
            ), patch.object(
                resolver_pins.urllib.request,
                "urlopen",
                side_effect=urllib.error.HTTPError(
                    "https://api.github.com", 403, "rate limit exceeded", {}, None
                ),
            ) as download, patch.object(
                resolver_pins.subprocess, "run"
            ) as verify:
                with self.assertRaises(urllib.error.HTTPError):
                    resolver_pins.verify_archive(archive, "gh")
                download.assert_called_once()
                verify.assert_not_called()

    def test_github_token_is_not_sent_to_release_asset_download(self):
        target = resolver_pins.read_targets()[0]
        with tempfile.TemporaryDirectory() as root, patch.dict(
            os.environ, {"GH_TOKEN": "fixture-gh"}, clear=True
        ), patch.object(
            resolver_pins.urllib.request, "urlopen", return_value=io.BytesIO(b"archive")
        ) as download, patch.object(
            resolver_pins, "verify_archive"
        ), patch.object(
            resolver_pins, "binary_from_archive", return_value=b"verified binary"
        ):
            resolver_pins.stage_binary("0.13.5", Path(root), target, "gh")
            self.assertEqual(
                download.call_args.args[0],
                f"https://github.com/astral-sh/uv/releases/download/0.13.5/{target['uv_archive']}",
            )

    def test_inventory_cannot_change_before_uploading_new_bytes(self):
        target = resolver_pins.read_targets()[0]
        with tempfile.TemporaryDirectory() as root:
            history = Path(root) / "existing.json"
            history.write_text(json.dumps({"version": "0.12.6", "artifacts": {}}))
            with patch.object(
                resolver_pins, "read_targets", return_value=[target]
            ), patch.object(
                resolver_pins.urllib.request, "urlopen"
            ) as download, patch.object(
                resolver_pins, "verify_archive"
            ), patch.object(
                resolver_pins, "binary_from_archive", return_value=b"verified"
            ), patch.object(
                resolver_pins, "mirror_binary"
            ) as mirror:
                download.return_value.__enter__.return_value.read.return_value = (
                    b"archive"
                )
                with self.assertRaisesRegex(ValueError, "overwrite"):
                    resolver_pins.generate(
                        "0.12.6", Path(root), bucket="must-not-upload", history=history
                    )
                mirror.assert_not_called()

    def test_python_canonical_bytes_match_the_rust_contract_vector(self):
        value = json.loads(
            (
                resolver_pins.ROOT
                / "crates/agentpm-sdk/contracts/python-resolution-v1.json"
            ).read_text()
        )
        expected = (
            '{"packages":[{"hashes":["sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],'
            '"name":"foo","version":"1.2.0"}],"python":{"requires":">=3.11"},'
            '"requirements":["foo>=1"],"type":"agentpm.python-resolution.v1"}'
        )
        # This fixed contract has ASCII field names and string-only leaves, a JCS subset.
        encoded = json.dumps(
            value, sort_keys=True, ensure_ascii=False, separators=(",", ":")
        ).encode()
        self.assertEqual(encoded, expected.encode())
        self.assertEqual(
            hashlib.sha256(encoded).digest(), hashlib.sha256(expected.encode()).digest()
        )

    def test_checked_in_targets_pins_and_registry_inventory_agree(self):
        pins = json.loads((resolver_pins.ROOT / "platforms/uv-pins.json").read_text())
        targets = resolver_pins.read_targets()
        check_resolver_targets.validate(pins, targets)
        with self.assertRaisesRegex(ValueError, "disagree"):
            check_resolver_targets.validate(pins, targets + [{"id": "extra-target"}])
        broken = json.loads(json.dumps(pins))
        broken["artifacts"]["extra-target"] = {"sha256": "a" * 64}
        with self.assertRaisesRegex(ValueError, "disagree"):
            check_resolver_targets.validate(broken, targets)
        registry = (
            resolver_pins.ROOT.parent
            / "agentpm-api/app/tools/domain/tools/uv-pins.json"
        )
        if registry.exists():
            self.assertEqual(pins, json.loads(registry.read_text()))

    def test_release_matrix_derives_from_the_checked_source(self):
        workflow = (resolver_pins.ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("python3 scripts/check_resolver_targets.py --matrix", workflow)
        self.assertIn("matrix: ${{ fromJSON(needs.targets.outputs.matrix) }}", workflow)
        self.assertIn(
            "python3 scripts/check_resolver_mirror.py --base-url https://api.agentpackagemanager.com",
            workflow,
        )

    def test_failed_attestation_stops_before_upload_or_pin_output(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            response = Mock()
            response.read.return_value = b"UNVERIFIED"
            response.__enter__ = Mock(return_value=response)
            response.__exit__ = Mock(return_value=False)
            with patch.object(
                resolver_pins.urllib.request, "urlopen", return_value=response
            ), patch.object(
                resolver_pins,
                "verify_archive",
                side_effect=ValueError("invalid attestation"),
            ), patch.object(
                resolver_pins, "mirror_binary"
            ) as mirror:
                with self.assertRaisesRegex(ValueError, "invalid attestation"):
                    resolver_pins.generate(
                        "0.12.6", directory, bucket="must-not-upload"
                    )
                mirror.assert_not_called()
                self.assertFalse((directory / "uv-pins.json").exists())

    def test_missing_target_fails_before_any_upload(self):
        targets = resolver_pins.read_targets()
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_pins, "read_targets", side_effect=[targets[:1], targets]
        ), patch.object(
            resolver_pins.urllib.request, "urlopen"
        ) as download, patch.object(
            resolver_pins, "verify_archive"
        ), patch.object(
            resolver_pins, "binary_from_archive", return_value=b"verified binary"
        ), patch.object(
            resolver_pins, "mirror_binary"
        ) as mirror:
            download.return_value.__enter__.return_value.read.return_value = b"archive"
            with self.assertRaisesRegex(ValueError, "missing supported target"):
                resolver_pins.generate("0.12.6", Path(root), bucket="must-not-upload")
            mirror.assert_not_called()

    def test_generator_computes_digests_from_binary_bytes(self):
        target = resolver_pins.read_targets()[0]
        binary = b"verified binary bytes, not an upstream checksum file"
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_pins, "read_targets", return_value=[target]
        ), patch.object(
            resolver_pins.urllib.request, "urlopen"
        ) as download, patch.object(
            resolver_pins, "verify_archive"
        ), patch.object(
            resolver_pins, "binary_from_archive", return_value=binary
        ), patch.object(
            resolver_smoke, "verify_smokes"
        ):
            download.return_value.__enter__.return_value.read.return_value = b"archive"
            pins = resolver_pins.generate("0.12.6", Path(root))
        self.assertEqual(
            pins["artifacts"][target["id"]]["sha256"],
            hashlib.sha256(binary).hexdigest(),
        )

    def test_failed_native_contract_cannot_upload_or_write_either_pin_table(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            output = root / "cli.json"
            registry = root / "registry.json"
            output.write_text("original CLI pins")
            registry.write_text("original Registry pins")
            with patch(
                "sys.argv",
                [
                    "resolver_pins.py",
                    "0.13.5",
                    "--stage-only",
                    "--staging",
                    str(root),
                    "--output",
                    str(output),
                    "--registry-output",
                    str(registry),
                ],
            ), patch.object(
                resolver_pins, "stage_binary", return_value=(root / "uv", b"verified")
            ), patch.object(
                resolver_smoke,
                "verify_smokes",
                side_effect=ValueError("broken output contract"),
            ), patch.object(
                resolver_pins, "mirror_binary"
            ) as mirror:
                with self.assertRaisesRegex(ValueError, "broken output contract"):
                    resolver_pins.main()
                mirror.assert_not_called()
            self.assertEqual(output.read_text(), "original CLI pins")
            self.assertEqual(registry.read_text(), "original Registry pins")
            self.assertFalse((root / "uv-pins").exists())
            with patch.object(
                resolver_pins, "stage_binary", return_value=(root / "uv", b"verified")
            ), patch.object(
                resolver_smoke,
                "verify_smokes",
                side_effect=ValueError("broken output contract"),
            ), patch.object(
                resolver_pins, "mirror_binary"
            ) as mirror:
                with self.assertRaisesRegex(ValueError, "broken output contract"):
                    resolver_pins.generate("0.13.5", root, bucket="must-not-upload")
                mirror.assert_not_called()

    def test_all_smokes_finish_before_first_upload(self):
        events = []
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_pins, "stage_binary", return_value=(Path(root) / "uv", b"verified")
        ), patch.object(
            resolver_smoke,
            "verify_smokes",
            side_effect=lambda *args: events.append("smoke"),
        ), patch.object(
            resolver_pins,
            "mirror_binary",
            side_effect=lambda *args: events.append("upload"),
        ):
            resolver_pins.generate("0.13.5", Path(root), bucket="fixture")
        self.assertEqual(
            events, ["smoke"] + ["upload"] * len(resolver_pins.read_targets())
        )

    def test_native_mode_writes_only_its_smoke_report(self):
        target = resolver_pins.read_targets()[0]["id"]
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_pins, "stage_binary", return_value=(Path(root) / "uv", b"verified")
        ), patch.object(
            resolver_smoke, "smoke", return_value={"target": target}
        ), patch.object(
            resolver_pins, "mirror_binary"
        ) as mirror:
            self.assertIsNone(
                resolver_pins.generate("0.13.5", Path(root), smoke_target=target)
            )
            self.assertTrue((Path(root) / "smoke" / f"{target}.json").is_file())
            mirror.assert_not_called()
            with self.assertRaisesRegex(ValueError, "cannot upload"):
                resolver_pins.generate(
                    "0.13.5", Path(root), smoke_target=target, bucket="fixture"
                )


class ResolverSmokeTests(unittest.TestCase):
    def test_linux_glibc_runtime_query_recovers_from_an_empty_cpython_heuristic(self):
        with patch.object(
            resolver_smoke.platform, "system", return_value="Linux"
        ), patch.object(
            resolver_smoke.platform, "machine", return_value="x86_64"
        ), patch.object(
            resolver_smoke.platform, "libc_ver", return_value=("", "")
        ), patch.object(
            resolver_smoke.os, "confstr", return_value="glibc 2.39", create=True
        ) as query:
            self.assertEqual(resolver_smoke.native_target(), "x86_64-unknown-linux-gnu")
            query.assert_called_once_with("CS_GNU_LIBC_VERSION")

    def test_unknown_or_musl_linux_is_not_assumed_to_be_glibc(self):
        for result in [
            None,
            "musl 1.2",
            ValueError("unsupported query"),
            OSError("unavailable"),
        ]:
            with self.subTest(result=result), patch.object(
                resolver_smoke.platform, "system", return_value="Linux"
            ), patch.object(
                resolver_smoke.platform, "machine", return_value="x86_64"
            ), patch.object(
                resolver_smoke.platform, "libc_ver", return_value=("", "")
            ), patch.object(
                resolver_smoke.os,
                "confstr",
                create=True,
                **(
                    {"side_effect": result}
                    if isinstance(result, Exception)
                    else {"return_value": result}
                ),
            ):
                self.assertIsNone(resolver_smoke.native_target())

    def test_contract_fingerprint_is_stable_across_windows_checkout_line_endings(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "contract"
            with patch.object(resolver_smoke, "__file__", str(path)), patch.object(
                resolver_smoke, "COMMANDS", path
            ), patch.object(resolver_smoke, "TARGETS", path):
                path.write_bytes(b"contract\ncontent\n")
                unix = resolver_smoke.contract_digest()
                path.write_bytes(b"contract\r\ncontent\r\n")
                self.assertEqual(unix, resolver_smoke.contract_digest())

    def test_portable_output_contract_accepts_all_four_native_wheel_sets(self):
        resolver_smoke.validate_pylock(smoke_pylock())

    def test_output_contract_rejects_schema_hash_marker_and_platform_regressions(self):
        valid = smoke_pylock()
        cases = {
            "format": valid.replace('lock-version = "1.0"', 'lock-version = "2.0"'),
            "renamed field": valid.replace(
                'name = "pydantic-core"', 'distribution = "pydantic-core"'
            ),
            "digest": valid.replace(
                'hashes = {sha256 = "' + "a" * 64 + '"}', "hashes = {}"
            ),
            "marker": valid.replace("win32", "linux"),
            "sdist": valid.split("[packages.sdist]")[0],
            "one hash": valid.replace("1" * 64, "0" * 64)
            .replace("2" * 64, "0" * 64)
            .replace("3" * 64, "0" * 64),
        }
        start = valid.index('[[packages]]\nname = "colorama"')
        end = valid.index('[[packages]]\nname = "pydantic-core"')
        cases["missing Windows-only transitive"] = valid[:start] + valid[end:]
        for platform in [
            "macosx_11_0_arm64",
            "macosx_10_12_x86_64",
            "manylinux_2_17_x86_64",
            "win_amd64",
        ]:
            cases[platform] = valid.replace(platform, "unsupported_platform")
        for case, raw in cases.items():
            with self.subTest(case=case), self.assertRaises(ValueError):
                resolver_smoke.validate_pylock(raw)

    def test_native_binary_executes_the_production_commands_in_isolation(self):
        calls = []
        with tempfile.TemporaryDirectory() as root:
            binary = Path(root) / "uv"
            binary.write_bytes(b"attested fixture bytes")

            def run(command, **kwargs):
                calls.append((command[1:], kwargs))
                if command[1] == "export":
                    (kwargs["cwd"] / "pylock.toml").write_text(smoke_pylock())
                return Mock(stdout="uv 0.13.5\n")

            with patch.object(
                resolver_smoke, "native_target", return_value="native"
            ), patch.object(
                resolver_smoke.subprocess, "run", side_effect=run
            ), patch.dict(
                resolver_smoke.os.environ,
                {
                    "UV_INDEX_URL": "private",
                    "AGENTPM_TOKEN": "secret",
                    "HTTPS_PROXY": "proxy",
                },
            ):
                report = resolver_smoke.smoke(binary, "0.13.5", "native")
            self.assertEqual(
                [args for args, _ in calls],
                [
                    ["--version"],
                    [
                        "lock",
                        "--no-build",
                        "--no-python-downloads",
                        "--no-sources",
                        "--no-progress",
                    ],
                    [
                        "export",
                        "--locked",
                        "--format",
                        "pylock.toml",
                        "--output-file",
                        "pylock.toml",
                        "--no-dev",
                        "--no-emit-project",
                        "--no-python-downloads",
                        "--no-sources",
                        "--no-progress",
                    ],
                ],
            )
            for _, kwargs in calls:
                self.assertTrue(kwargs["check"])
                self.assertEqual(kwargs["timeout"], 180)
                self.assertNotIn("AGENTPM_TOKEN", kwargs["env"])
                self.assertNotIn("UV_INDEX_URL", kwargs["env"])
                self.assertEqual(kwargs["env"]["HTTPS_PROXY"], "proxy")
            self.assertEqual(
                report["sha256"], hashlib.sha256(binary.read_bytes()).hexdigest()
            )

    def test_foreign_binary_is_never_executed_on_this_host(self):
        with patch.object(
            resolver_smoke, "native_target", return_value="native"
        ), patch.object(resolver_smoke.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "native runner"):
                resolver_smoke.smoke(Path("nonexistent"), "0.13.5", "foreign")
            run.assert_not_called()

    def test_reports_must_match_binary_version_target_and_current_contract(self):
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_smoke, "native_target", return_value=None
        ):
            root = Path(root)
            target = resolver_pins.read_targets()[0]["id"]
            report = {
                "version": "0.13.5",
                "target": target,
                "sha256": "a" * 64,
                "contract_sha256": resolver_smoke.contract_digest(),
                "pylock": smoke_pylock(),
            }
            staged = [(target, root / "uv", "a" * 64)]
            with self.assertRaisesRegex(ValueError, "missing native result"):
                resolver_smoke.verify_smokes(staged, "0.13.5", root)
            destination = root / f"{target}.json"
            destination.write_text(json.dumps(report))
            resolver_smoke.verify_smokes(staged, "0.13.5", root)
            for key in ["version", "target", "sha256", "contract_sha256", "pylock"]:
                with self.subTest(key=key):
                    destination.write_text(json.dumps({**report, key: "wrong"}))
                    with self.assertRaises(ValueError):
                        resolver_smoke.verify_smokes(staged, "0.13.5", root)

    def test_wrong_binary_version_stops_before_resolution(self):
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_smoke, "native_target", return_value="native"
        ), patch.object(
            resolver_smoke.subprocess, "run", return_value=Mock(stdout="uv 0.0.0\n")
        ) as run:
            binary = Path(root) / "uv"
            binary.write_bytes(b"attested")
            with self.assertRaisesRegex(ValueError, "version does not match"):
                resolver_smoke.smoke(binary, "0.13.5", "native")
            self.assertEqual(run.call_count, 1)

    def test_failed_command_preserves_error_and_does_not_continue(self):
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_smoke, "native_target", return_value="native"
        ), patch.object(
            resolver_smoke.subprocess,
            "run",
            side_effect=[
                Mock(stdout="uv 0.13.5"),
                subprocess.CalledProcessError(2, "uv", stderr="fixture failure"),
            ],
        ) as run:
            binary = Path(root) / "uv"
            binary.write_bytes(b"attested")
            with self.assertRaisesRegex(ValueError, "lock failed: fixture failure"):
                resolver_smoke.smoke(binary, "0.13.5", "native")
            self.assertEqual(run.call_count, 2)

    def test_reports_cover_every_target_and_reject_duplicate_downloads(self):
        with tempfile.TemporaryDirectory() as root, patch.object(
            resolver_smoke, "native_target", return_value=None
        ):
            root = Path(root)
            staged = []
            for target in resolver_pins.read_targets():
                name = target["id"]
                report = {
                    "version": "0.13.5",
                    "target": name,
                    "sha256": "a" * 64,
                    "contract_sha256": resolver_smoke.contract_digest(),
                    "pylock": smoke_pylock(),
                }
                destination = root / f"uv-smoke-{name}"
                destination.mkdir()
                path = destination / f"{name}.json"
                path.write_text(json.dumps(report))
                staged.append((name, root / "uv", "a" * 64))
            resolver_smoke.verify_smokes(staged, "0.13.5", root)
            path.unlink()
            with self.assertRaisesRegex(ValueError, "missing native result"):
                resolver_smoke.verify_smokes(staged, "0.13.5", root)
            path.write_text(json.dumps(report))
            (root / path.name).write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "ambiguous native results"):
                resolver_smoke.verify_smokes(staged, "0.13.5", root)


class ResolverMirrorTests(unittest.TestCase):
    def test_operator_cli_exits_with_a_concise_error_instead_of_a_traceback(self):
        with patch(
            "sys.argv",
            ["check_resolver_mirror.py", "--base-url", "https://registry.test"],
        ), patch.object(
            check_resolver_mirror,
            "verify_mirror",
            side_effect=ValueError("API resolver route is not deployed"),
        ):
            with self.assertRaises(SystemExit) as raised:
                check_resolver_mirror.main()
            self.assertEqual(
                str(raised.exception),
                "Resolver mirror check failed: API resolver route is not deployed",
            )

    def pins(self, binary):
        return {
            "version": "0.13.5",
            "artifacts": {
                t["id"]: {"sha256": hashlib.sha256(binary).hexdigest()}
                for t in resolver_pins.read_targets()
            },
        }

    def test_release_checks_deployed_inventory_and_every_binary_digest(self):
        responses = []
        for _ in resolver_pins.read_targets():
            responses.extend(
                [
                    io.BytesIO(
                        json.dumps(
                            {
                                "presigned_url": "https://storage.test/uv?signature=secret"
                            }
                        ).encode()
                    ),
                    io.BytesIO(b"verified executable"),
                ]
            )
        with patch.object(
            check_resolver_mirror.urllib.request, "urlopen", side_effect=responses
        ) as download:
            check_resolver_mirror.verify_mirror(
                self.pins(b"verified executable"), "https://registry.test"
            )
        self.assertEqual(download.call_count, 2 * len(resolver_pins.read_targets()))
        registry_calls = [c.args[0] for c in download.call_args_list[::2]]
        for target in resolver_pins.read_targets():
            self.assertTrue(
                any(url.endswith(f'/0.13.5/{target["id"]}') for url in registry_calls)
            )
        self.assertTrue(
            all(call.kwargs == {"timeout": 30} for call in download.call_args_list)
        )

    def test_release_gate_rejects_a_mirror_digest_different_from_cli(self):
        with patch.object(
            check_resolver_mirror,
            "open_storage",
            side_effect=[
                io.BytesIO(b'{"presigned_url":"https://storage.test/uv"}'),
                io.BytesIO(b"wrong bytes"),
            ],
        ):
            with self.assertRaisesRegex(ValueError, "does not match CLI pin"):
                check_resolver_mirror.verify_mirror(
                    self.pins(b"approved bytes"), "https://registry.test"
                )

    def test_missing_inventory_and_missing_or_forbidden_object_are_not_proxy_errors(
        self,
    ):
        for location in ["registry inventory", "mirrored object"]:
            for status in [403, 404, 500]:
                with self.subTest(location=location, status=status), patch.object(
                    check_resolver_mirror.urllib.request,
                    "urlopen",
                    side_effect=urllib.error.HTTPError(
                        "https://storage.test?signature=SECRET",
                        status,
                        "failure",
                        {},
                        None,
                    ),
                ):
                    with self.assertRaises(ValueError) as raised:
                        check_resolver_mirror.open_storage(
                            "https://registry.test", location
                        )
                    message = str(raised.exception)
                    self.assertNotIn("proxy", message)
                    self.assertNotIn("SECRET", message)
                    self.assertIn(str(status), message)
                    self.assertIn(
                        "HTTP failure" if status == 500 else "not mirrored", message
                    )
                    if status == 404:
                        self.assertIn("API resolver-route deployment", message)

    def test_transport_failure_and_disallowed_url_are_distinct(self):
        with patch.object(
            check_resolver_mirror.urllib.request,
            "urlopen",
            side_effect=urllib.error.URLError("offline"),
        ):
            with self.assertRaisesRegex(ValueError, "network/proxy failure"):
                check_resolver_mirror.open_storage(
                    "https://storage.test", "mirrored object"
                )
        with self.assertRaisesRegex(ValueError, "requires HTTPS"):
            check_resolver_mirror.open_storage(
                "file:///private/credentials", "mirrored object"
            )

    def test_release_gate_stops_on_missing_inventory_or_object(self):
        for storage in [False, True]:
            failure = urllib.error.HTTPError(
                "https://registry.test", 404, "missing", {}, None
            )
            responses = [failure]
            if storage:
                responses.insert(
                    0, io.BytesIO(b'{"presigned_url":"https://storage.test/uv"}')
                )
            with self.subTest(storage=storage), patch.object(
                check_resolver_mirror.urllib.request, "urlopen", side_effect=responses
            ) as download:
                with self.assertRaisesRegex(ValueError, "not mirrored"):
                    check_resolver_mirror.verify_mirror(
                        self.pins(b"approved bytes"), "https://registry.test"
                    )
                self.assertEqual(download.call_count, 2 if storage else 1)

    def test_empty_and_oversized_objects_cannot_pass_release_gate(self):
        for data in [b"", b"too large"]:
            with self.subTest(data=data), patch.object(
                check_resolver_mirror, "MAX_BINARY_BYTES", 2
            ), patch.object(
                check_resolver_mirror,
                "open_storage",
                side_effect=[
                    io.BytesIO(b'{"presigned_url":"https://storage.test/uv"}'),
                    io.BytesIO(data),
                ],
            ):
                with self.assertRaisesRegex(
                    ValueError, "empty" if not data else "size limit"
                ):
                    check_resolver_mirror.verify_mirror(
                        self.pins(data), "https://registry.test"
                    )


if __name__ == "__main__":
    unittest.main()
