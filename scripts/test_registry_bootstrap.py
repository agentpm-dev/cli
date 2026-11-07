import base64
import copy
import unittest

from check_registry_bootstrap import ORIGIN, check


class RegistryBootstrapTests(unittest.TestCase):
    def test_release_cannot_use_missing_or_fixture_root_or_malformed_pin(self):
        fixture = {
            "roots": [
                {
                    "registry": "https://api.fixture.invalid",
                    "rootKeyId": "fixture",
                    "publicKeyB64": base64.b64encode(bytes([42]) * 32).decode(),
                }
            ]
        }
        empty = {"version": 1, "roots": []}
        check(empty, fixture)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            check(empty, fixture, release=True)
        root = {
            "registry": ORIGIN,
            "rootKeyId": "operator-root",
            "publicKeyB64": base64.b64encode(bytes([7]) * 32).decode(),
        }
        valid = {"version": 1, "roots": [root]}
        check(valid, fixture, release=True)
        for changed in [
            {**root, "publicKeyB64": fixture["roots"][0]["publicKeyB64"]},
            {**root, "publicKeyB64": "bad"},
            {**root, "rootKeyId": ""},
        ]:
            with self.assertRaises(ValueError):
                check({"version": 1, "roots": [changed]}, fixture, release=True)
        duplicated = copy.deepcopy(valid)
        duplicated["roots"].append(root)
        with self.assertRaises(ValueError):
            check(duplicated, fixture, release=True)

    def test_release_rejects_dev_pins_even_with_an_official_root(self):
        official = {
            "registry": ORIGIN,
            "rootKeyId": "production-root",
            "publicKeyB64": base64.b64encode(bytes([7]) * 32).decode(),
        }
        fixture = {"roots": []}
        for origin in (
            "http://127.0.0.1:5000",
            "http://localhost:5000",
            "https://127.0.0.1:5000",
            "https://localhost:5000",
            "https://[::1]:5000",
            "https://dev.example.invalid",
        ):
            with self.subTest(origin=origin):
                roots = {
                    "version": 1,
                    "roots": [official, {**official, "registry": origin}],
                }
                unchanged = copy.deepcopy(roots)
                check(roots, fixture)
                with self.assertRaisesRegex(
                    ValueError, "remove development/non-production"
                ):
                    check(roots, fixture, release=True)
                self.assertEqual(roots, unchanged)
        # A small official root ring remains valid for a coordinated root change.
        check(
            {
                "version": 1,
                "roots": [official, {**official, "rootKeyId": "production-next"}],
            },
            fixture,
            release=True,
        )

    def test_fixture_root_is_forbidden_for_every_origin(self):
        key = base64.b64encode(bytes([42]) * 32).decode()
        fixture = {"roots": [{"publicKeyB64": key}]}
        for origin in (ORIGIN, "http://127.0.0.1:5000", "https://dev.example.invalid"):
            with self.subTest(origin=origin), self.assertRaisesRegex(
                ValueError, "any Registry origin"
            ):
                check(
                    {
                        "version": 1,
                        "roots": [
                            {
                                "registry": origin,
                                "rootKeyId": "fixture",
                                "publicKeyB64": key,
                            }
                        ],
                    },
                    fixture,
                )

    def test_root_registry_must_be_an_exact_supported_origin(self):
        for origin in (
            "http://api.agentpackagemanager.com",
            "ftp://localhost",
            "localhost:5000",
            ORIGIN + "/",
            ORIGIN + "/path",
            ORIGIN + "?query=1",
            ORIGIN + "?",
            ORIGIN + "#fragment",
            ORIGIN + "#",
            "https://user:password@example.invalid",
            "https://example.invalid:invalid",
            "https://example.invalid:65536",
            "https://",
        ):
            with self.subTest(origin=origin), self.assertRaises(ValueError):
                check(
                    {
                        "version": 1,
                        "roots": [
                            {
                                "registry": origin,
                                "rootKeyId": "operator",
                                "publicKeyB64": base64.b64encode(
                                    bytes([7]) * 32
                                ).decode(),
                            }
                        ],
                    },
                    {"roots": []},
                )
