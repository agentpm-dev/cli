"""Evidence must fail closed; running the matrix is separate integration coverage."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import workload

SPEC = importlib.util.spec_from_file_location(
    "capacity_launcher", Path(__file__).with_name("run.py")
)
assert SPEC is not None and SPEC.loader is not None
launcher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(launcher)


class CapacityEvidenceTests(unittest.TestCase):
    def setUp(self):
        # unittest owns cleanup across the full test, including failed assertions.
        # pylint: disable=consider-using-with
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.addCleanup(patch.stopall)
        patch.object(workload, "ROOT", self.root).start()
        self.events = [
            {
                "event": "sample",
                "at": at,
                "memory_current": 100,
                "memory_peak": 110,
                "rss": {"7": 80},
                "temp_bytes": 0,
            }
            for at in [9, 10.5, 11.8]
        ]
        self.events.extend([{"event": "worker", "at": 1}, {"event": "worker", "at": 2}])
        for lane in ["off", "on"]:
            self.events.append(
                {"event": "storage_get", "request_id": lane, "key": "uploads/root"}
            )
            workload.save(
                self.root / f"plan-{lane}.json",
                {"cases": [{"label": "agent", "manifest": {"version": "0.1.0"}}]},
            )
            workload.save(
                self.root / "requests" / f"{lane}.json",
                {
                    "lane": lane,
                    "label": "agent",
                    "version": "0.1.0",
                    "workload": "graph",
                    "artifacts": [],
                    "result": {"status": 200},
                    "started": 10,
                    "ended": 11,
                    "settled": 12,
                    "request_id": lane,
                },
            )
        self.events.append({"event": "extract_start", "request_id": "on", "bytes": 100})
        self.write_events()

    def write_events(self):
        (self.root / "events.jsonl").write_text(
            "".join(json.dumps(event) + "\n" for event in self.events)
        )

    def test_local_execution_never_grants_activation_or_capacity_approval(self):
        workload.report()
        result = workload.read(self.root / "measurements.json")
        self.assertFalse(result["activation_approved"])
        self.assertFalse(result["capacity_gate_complete"])

    def test_missing_workload_cannot_be_reported_as_a_pass(self):
        (self.root / "requests/on.json").unlink()
        with self.assertRaisesRegex(RuntimeError, "Missing workload"):
            workload.report()

    def test_unexpected_server_error_is_not_an_expected_rejection(self):
        path = self.root / "requests/on.json"
        record = workload.read(path)
        record["result"]["status"] = 500
        workload.save(path, record)
        with self.assertRaisesRegex(RuntimeError, "Unexpected HTTP"):
            workload.report()

    def test_broken_sampler_cannot_certify_a_workload(self):
        self.events.append({"event": "sampling_error", "message": "permission denied"})
        self.write_events()
        with self.assertRaisesRegex(RuntimeError, "Sampler failed"):
            workload.report()

    def test_oom_kill_prevents_success_report(self):
        self.events[0]["memory_events"] = "oom_kill 1\n"
        self.write_events()
        with self.assertRaisesRegex(RuntimeError, "OOM kill"):
            workload.report()

    def test_unplanned_worker_restart_prevents_success_report(self):
        self.events.append({"event": "worker", "at": 3})
        self.write_events()
        with self.assertRaisesRegex(RuntimeError, "Unexpected worker restart"):
            workload.report()

    def test_temporary_file_residue_prevents_success_report(self):
        self.events[2]["temp_bytes"] = 100
        self.write_events()
        with self.assertRaisesRegex(RuntimeError, "temporary-file residue"):
            workload.report()

    def test_actual_read_bytes_are_separate_from_get_content_length(self):
        self.events.extend(
            [
                {
                    "event": "storage_get",
                    "request_id": "on",
                    "key": "uploads/root",
                    "content_length": 1000,
                },
                {
                    "event": "storage_read",
                    "request_id": "on",
                    "key": "uploads/root",
                    "bytes": 42,
                },
                {
                    "event": "storage_get",
                    "request_id": "on",
                    "key": "tools/dependency",
                    "content_length": 500,
                },
                {
                    "event": "storage_read",
                    "request_id": "on",
                    "key": "tools/dependency",
                    "bytes": 10,
                },
            ]
        )
        self.write_events()
        workload.report()
        row = next(
            row
            for row in workload.read(self.root / "measurements.json")["rows"]
            if row["lane"] == "on"
        )
        self.assertEqual(
            (
                row["root_gets"],
                row["dependency_gets"],
                row["downloaded"],
                row["dependency_downloaded"],
            ),
            (2, 1, 52, 10),
        )

    def test_stack_is_internal_and_resource_limited(self):
        launcher.compose(self.root, "agentpm-capacity-test", False)
        document = workload.read(self.root / "compose.json")
        self.assertTrue(document["networks"]["isolated"]["internal"])
        for service in document["services"].values():
            self.assertNotIn("ports", service)
            self.assertNotIn("env_file", service)
        registry = document["services"]["registry"]
        self.assertEqual(
            (registry["cpus"], registry["mem_limit"], registry["memswap_limit"]),
            (0.5, "512m", "512m"),
        )
        self.assertEqual(
            registry["environment"]["AWS_ENDPOINT_URL_S3"], "http://storage:5000"
        )

    def test_legacy_final_key_is_a_root_read_without_full_digest_in_path(self):
        path = self.root / "requests/off.json"
        record = workload.read(path)
        record["root_keys"] = ["tools/capacity/root/0.1.0/ec17e1e31526.tar.gz"]
        workload.save(path, record)
        self.events.append(
            {"event": "storage_get", "request_id": "off", "key": record["root_keys"][0]}
        )
        self.write_events()
        workload.report()
        row = next(
            row
            for row in workload.read(self.root / "measurements.json")["rows"]
            if row["lane"] == "off"
        )
        self.assertEqual((row["root_gets"], row["dependency_gets"]), (2, 0))


if __name__ == "__main__":
    unittest.main()
