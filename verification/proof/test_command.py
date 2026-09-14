import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).with_name("command.py")
SPEC = importlib.util.spec_from_file_location("proof_command", MODULE_PATH)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
CommandFailure = RUNNER.CommandFailure
run_command = RUNNER.run_command

ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
XYZ_SHA256 = "3608bca1e44ea6c4d268eb6db02260269892c0b42b86bbf1e77a6fa16c3c9282"


class CommandTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.cwd = self.root / "source"
        self.cwd.mkdir()
        self.output = self.root / "evidence"

    def tearDown(self):
        self.temporary.cleanup()

    def environment(self):
        return {"PATH": "/usr/bin:/bin", "CARGO_BUILD_JOBS": "2"}

    def test_relative_executable_is_refused_before_output_creation(self):
        with self.assertRaises(CommandFailure):
            run_command(["true"], self.cwd, self.environment(), self.output)
        self.assertFalse(self.output.exists())

    def test_unknown_environment_key_is_refused_before_output_creation(self):
        environment = self.environment() | {"DUMMY_SECRET": "not-a-secret"}
        with self.assertRaises(CommandFailure):
            run_command(["/usr/bin/true"], self.cwd, environment, self.output)
        self.assertFalse(self.output.exists())

    def test_wrong_cargo_build_jobs_is_refused_before_output_creation(self):
        environment = self.environment() | {"CARGO_BUILD_JOBS": "3"}
        with self.assertRaises(CommandFailure):
            run_command(["/usr/bin/true"], self.cwd, environment, self.output)
        self.assertFalse(self.output.exists())

    def test_existing_output_is_refused_and_untouched(self):
        self.output.mkdir()
        sentinel = self.output / "sentinel"
        sentinel.write_bytes(b"abc")
        try:
            with self.assertRaises(CommandFailure):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        finally:
            self.assertEqual(sentinel.read_bytes(), b"abc")
            self.assertEqual({path.name for path in self.output.iterdir()}, {"sentinel"})

    def test_output_inside_cwd_is_refused_before_output_creation(self):
        output = self.cwd / "evidence"
        with self.assertRaises(CommandFailure):
            run_command(["/usr/bin/true"], self.cwd, self.environment(), output)
        self.assertFalse(output.exists())

    def test_existing_output_through_symlink_parent_is_refused_and_untouched(self):
        alias = self.root / "source-alias"
        alias.symlink_to(self.cwd, target_is_directory=True)
        output = alias / "evidence"
        output.mkdir()
        sentinel = output / "sentinel"
        sentinel.write_bytes(b"abc")
        try:
            with self.assertRaises(CommandFailure):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), output)
        finally:
            self.assertEqual(sentinel.read_bytes(), b"abc")
            self.assertEqual({path.name for path in output.iterdir()}, {"sentinel"})

    def test_fresh_output_through_symlink_parent_is_refused_without_launch(self):
        alias = self.root / "source-alias"
        alias.symlink_to(self.cwd, target_is_directory=True)
        output = alias / "fresh-evidence"
        with mock.patch.object(
            RUNNER.subprocess, "run", side_effect=AssertionError("child launch attempted")
        ):
            with self.assertRaises(CommandFailure):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), output)
        self.assertFalse(output.exists())

    def test_missing_executable_is_refused_before_output_creation(self):
        missing = self.root / "missing-executable"
        with self.assertRaises(CommandFailure):
            run_command([str(missing)], self.cwd, self.environment(), self.output)
        self.assertFalse(self.output.exists())

    def test_surrogate_argument_is_refused_before_output_creation(self):
        with self.assertRaises(CommandFailure):
            run_command(["/usr/bin/true", "\ud800"], self.cwd, self.environment(), self.output)
        self.assertFalse(self.output.exists())

    def terminal_state(self, unit, *, exit_code="7", memory_max="4294967296"):
        return {
            "Id": unit,
            "ActiveState": "failed",
            "SubState": "failed",
            "Result": "exit-code",
            "ExecMainCode": "1",
            "ExecMainStatus": exit_code,
            "ExecMainPID": "2468",
            "MemoryPeak": "4096",
            "CPUUsageNSec": "123456",
            "ExecMainStartTimestampMonotonic": "1000000",
            "ExecMainExitTimestampMonotonic": "2000000",
            "CPUQuotaPerSecUSec": "2s",
            "MemoryHigh": "2147483648",
            "MemoryMax": memory_max,
            "MemorySwapMax": "0",
            "TasksMax": "256",
            "LimitFSIZE": "67108864",
            "RuntimeMaxUSec": "10min",
        }

    def boundary(
        self,
        *,
        launch_exit=0,
        memory_max="4294967296",
        transport_error=False,
        reset_exit=0,
        cleanup_exit=0,
        cleanup_state="absent",
        child_exit="7",
    ):
        def invoke(argv, **kwargs):
            if argv[0] == "/usr/bin/systemd-run":
                if transport_error:
                    raise OSError("synthetic control transport failure")
                if launch_exit == 0:
                    stdout_path = next(
                        value.removeprefix("--property=StandardOutput=append:")
                        for value in argv
                        if value.startswith("--property=StandardOutput=append:")
                    )
                    stderr_path = next(
                        value.removeprefix("--property=StandardError=append:")
                        for value in argv
                        if value.startswith("--property=StandardError=append:")
                    )
                    Path(stdout_path).write_bytes(b"abc")
                    Path(stderr_path).write_bytes(b"xyz")
                return subprocess.CompletedProcess(argv, launch_exit)
            if argv[:3] == ["/usr/bin/systemctl", "--user", "show"]:
                unit = argv[3]
                if argv[4] == "--property=Id,LoadState,ActiveState,SubState":
                    if cleanup_state == "absent":
                        text = (
                            f"Id={unit}\nLoadState=not-found\n"
                            "ActiveState=inactive\nSubState=dead\n"
                        )
                    elif cleanup_state == "loaded":
                        text = (
                            f"Id={unit}\nLoadState=loaded\n"
                            "ActiveState=active\nSubState=running\n"
                        )
                    else:
                        text = "malformed cleanup observation\n"
                    kwargs["stdout"].write(text.encode("utf-8"))
                    return subprocess.CompletedProcess(argv, cleanup_exit)
                state = self.terminal_state(unit, exit_code=child_exit, memory_max=memory_max)
                text = "".join(f"{key}={value}\n" for key, value in state.items())
                return subprocess.CompletedProcess(argv, 0, stdout=text, stderr="")
            if argv[:3] == ["/usr/bin/systemctl", "--user", "stop"]:
                return subprocess.CompletedProcess(argv, 0)
            if argv[:3] == ["/usr/bin/systemctl", "--user", "reset-failed"]:
                return subprocess.CompletedProcess(argv, reset_exit)
            raise AssertionError(f"unexpected subprocess boundary call: {argv}")

        return invoke

    def receipt(self):
        return json.loads((self.output / "receipt.json").read_text(encoding="utf-8"))

    def test_nonzero_child_exit_is_a_completed_unqualified_observation(self):
        argv = ["/usr/bin/true", "literal-argument"]
        environment = self.environment()
        with mock.patch.object(RUNNER.subprocess, "run", side_effect=self.boundary()):
            result = run_command(argv, self.cwd, environment, self.output)
        self.assertEqual(
            {key: result[key] for key in ("status", "qualification", "argv", "env", "exit_code", "signal")},
            {
                "status": "command-completed",
                "qualification": False,
                "argv": argv,
                "env": environment,
                "exit_code": 7,
                "signal": None,
            },
        )
        self.assertEqual(result["stdout_sha256"], ABC_SHA256)
        self.assertEqual(result["stderr_sha256"], XYZ_SHA256)
        self.assertEqual(self.receipt(), result)

    def test_systemd_launch_preserves_literal_dollar_argument(self):
        argv = ["/usr/bin/true", "$LIMINAL_UNSET_ARGUMENT_CANARY"]
        expansion_disabled = False

        def systemd_boundary(boundary_argv, **kwargs):
            nonlocal expansion_disabled
            if boundary_argv[0] == "/usr/bin/systemd-run":
                expansion_disabled = "--expand-environment=no" in boundary_argv
            child_exit = "0" if expansion_disabled else "19"
            return self.boundary(child_exit=child_exit)(boundary_argv, **kwargs)

        with mock.patch.object(RUNNER.subprocess, "run", side_effect=systemd_boundary):
            result = run_command(argv, self.cwd, self.environment(), self.output)
        self.assertEqual(result["argv"], argv)
        self.assertEqual(result["exit_code"], 0)

    def test_observed_memory_limit_drift_is_incomplete_with_state_retained(self):
        boundary = self.boundary(memory_max="4294967295")
        with mock.patch.object(RUNNER.subprocess, "run", side_effect=boundary):
            with self.assertRaisesRegex(CommandFailure, "limits differ"):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "execution-incomplete")
        self.assertNotIn("exit_code", receipt)
        states = [json.loads(line) for line in (self.output / "states.jsonl").read_text().splitlines()]
        self.assertEqual(len(states), 1)
        self.assertEqual(states[0]["MemoryMax"], "4294967295")

    def test_launch_exit_one_is_incomplete_and_retains_launcher_exit(self):
        with mock.patch.object(
            RUNNER.subprocess, "run", side_effect=self.boundary(launch_exit=1)
        ):
            with self.assertRaisesRegex(CommandFailure, "service launch failed"):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "execution-incomplete")
        self.assertNotIn("exit_code", receipt)
        launch_exit = json.loads((self.output / "launch.exit.json").read_text())
        self.assertEqual(launch_exit, {"exit_code": 1})

    def test_control_transport_error_is_incomplete_without_fake_child_exit(self):
        with mock.patch.object(
            RUNNER.subprocess, "run", side_effect=self.boundary(transport_error=True)
        ):
            with self.assertRaisesRegex(CommandFailure, "synthetic control transport failure"):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "execution-incomplete")
        self.assertNotIn("exit_code", receipt)
        self.assertFalse((self.output / "launch.exit.json").exists())

    def test_reset_failure_with_observed_absence_keeps_completed_result(self):
        with mock.patch.object(
            RUNNER.subprocess, "run", side_effect=self.boundary(reset_exit=1)
        ):
            result = run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        self.assertEqual(result["status"], "command-completed")
        self.assertEqual(result["exit_code"], 7)
        self.assertEqual(
            result["cleanup"],
            {"stop_exit": 0, "reset_exit": 1, "state_exit": 0, "unit_absent": True},
        )

    def test_loaded_unit_after_cleanup_makes_receipt_incomplete(self):
        boundary = self.boundary(cleanup_state="loaded")
        with mock.patch.object(RUNNER.subprocess, "run", side_effect=boundary):
            with self.assertRaisesRegex(CommandFailure, "absence was not established"):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "execution-incomplete")
        self.assertEqual(
            receipt["cleanup"],
            {"stop_exit": 0, "reset_exit": 0, "state_exit": 0, "unit_absent": False},
        )
        self.assertIn("LoadState=loaded", (self.output / "cleanup-state.stdout").read_text())

    def test_cleanup_show_nonzero_makes_receipt_incomplete_and_preserves_raw_output(self):
        boundary = self.boundary(cleanup_exit=1)
        with mock.patch.object(RUNNER.subprocess, "run", side_effect=boundary):
            with self.assertRaisesRegex(CommandFailure, "absence was not established"):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "execution-incomplete")
        self.assertFalse(receipt["cleanup"]["unit_absent"])
        self.assertEqual(
            (self.output / "cleanup-state.stdout").read_text(),
            f"Id={receipt['unit']}\nLoadState=not-found\nActiveState=inactive\nSubState=dead\n",
        )

    def test_malformed_cleanup_state_makes_receipt_incomplete_and_preserves_raw_output(self):
        boundary = self.boundary(cleanup_state="malformed")
        with mock.patch.object(RUNNER.subprocess, "run", side_effect=boundary):
            with self.assertRaisesRegex(CommandFailure, "absence was not established"):
                run_command(["/usr/bin/true"], self.cwd, self.environment(), self.output)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "execution-incomplete")
        self.assertFalse(receipt["cleanup"]["unit_absent"])
        self.assertEqual(
            (self.output / "cleanup-state.stdout").read_bytes(),
            b"malformed cleanup observation\n",
        )


if __name__ == "__main__":
    unittest.main()
