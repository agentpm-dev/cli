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
