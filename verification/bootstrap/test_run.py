import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).with_name("run.py")


def load_runner(path=MODULE_PATH):
    spec = importlib.util.spec_from_file_location("bootstrap_runner", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class BootstrapRunnerTests(unittest.TestCase):
    def test_matching_pins_and_semantic_controls_succeed(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fixture = self.make_fixture(root)
            runner = self.load_fixture_runner(fixture)
            output = root / "evidence"
            calls = []

            def fake(command, cwd, env, timeout):
                calls.append(command)
                text = self.output_for(command, cwd)
                return subprocess.CompletedProcess(command, self.exit_for(command, cwd), text, "")

            code = self.invoke(runner,
                runner.Options(fixture / "verus", fixture / "tlc.jar", output),
                process_runner=fake,
            )
            self.assertEqual(code, 0)
            self.assertTrue(calls)
            result = json.loads((output / "result.json").read_text())
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["outcome"], "bootstrap-only")
            self.assertEqual(len(result["commands"]), 15)
            self.assertEqual(result["counts"]["cargo_verus_verify_2"]["vstd_verified"], 2045)
            self.assertEqual(result["counts"]["tlc_positive"]["queued_states"], 0)
            self.assertIn("runner", result)
            self.assertIn("staged_sources", result)
            self.assertIn("execution_sources", result)

    def test_tampered_or_missing_tool_refuses_before_process_calls(self):
        tools = ("verus/verus", "verus/cargo-verus", "verus/rust_verify", "verus/z3", "tlc.jar")
        for relative in tools:
            for mode in ("tampered", "missing"):
                with self.subTest(tool=relative, mode=mode), tempfile.TemporaryDirectory() as td:
                    root = Path(td)
                    fixture = self.make_fixture(root)
                    runner = self.load_fixture_runner(fixture)
                    tool = fixture / relative
                    tool.write_text("tampered") if mode == "tampered" else tool.unlink()
                    calls = []
                    code = self.invoke(runner,
                        runner.Options(fixture / "verus", fixture / "tlc.jar", root / "out"),
                        process_runner=lambda *args: calls.append(args),
                    )
                    self.assertEqual(code, 1)
                    self.assertEqual(calls, [])

    def test_ambient_verification_override_refuses_before_process_calls(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fixture = self.make_fixture(root)
            runner = self.load_fixture_runner(fixture)
            calls = []
            with mock.patch.dict(
                os.environ,
                {"PATH": os.environ.get("PATH", ""), "RUSTC_WRAPPER": "/tmp/replace-rustc"},
                clear=True,
            ):
                code = runner._run_bootstrap(
                    runner.Options(fixture / "verus", fixture / "tlc.jar", root / "out"),
                    process_runner=lambda *args: calls.append(args),
                )
            self.assertEqual(code, 1)
            self.assertEqual(calls, [])

    def test_wrong_lock_refuses_before_process_calls(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fixture = self.make_fixture(root)
            runner = self.load_fixture_runner(fixture)
            (fixture / "repo/verification/bootstrap/admission/Cargo.lock").write_text("changed")
            calls = []
            code = self.invoke(runner,
                runner.Options(fixture / "verus", fixture / "tlc.jar", root / "out"),
                process_runner=lambda *args: calls.append(args),
            )
            self.assertEqual(code, 1)
            self.assertEqual(calls, [])

    def test_every_changed_fixture_refuses_before_process_calls(self):
        for relative in self.source_files():
            with self.subTest(source=relative):
                def mutate(fixture, manifest, relative=relative):
                    del manifest
                    source = fixture / "repo/verification/bootstrap" / relative
                    source.write_bytes(source.read_bytes() + b"changed")

                self.assert_preflight_refused(mutate)

    def test_source_pin_map_is_exact_and_cannot_be_swapped(self):
        def missing(fixture, manifest):
            del fixture
            manifest["bootstrap_sources"].pop("admission/src/main.rs")

        def extra(fixture, manifest):
            del fixture
            manifest["bootstrap_sources"]["extra.rs"] = "0" * 64

        def swapped(fixture, manifest):
            del fixture
            pins = manifest["bootstrap_sources"]
            pins["admission/Cargo.toml"], pins["admission/src/main.rs"] = (
                pins["admission/src/main.rs"],
                pins["admission/Cargo.toml"],
            )

        for name, mutate in (("missing", missing), ("extra", extra), ("swapped", swapped)):
            with self.subTest(case=name):
                self.assert_preflight_refused(mutate)

    def test_claims_are_closed_exact_and_real_booleans(self):
        def missing(fixture, manifest):
            del fixture
            manifest["claims"].pop("bootstrap_tool_validation")

        def extra(fixture, manifest):
            del fixture
            manifest["claims"]["qualification"] = False

        def changed(fixture, manifest):
            del fixture
            manifest["claims"]["production_binding"] = True

        def wrong_type(fixture, manifest):
            del fixture
            manifest["claims"]["formal_check_tool_validation"] = 0

        cases = (("missing", missing), ("extra", extra), ("changed", changed), ("wrong_type", wrong_type))
        for name, mutate in cases:
            with self.subTest(case=name):
                self.assert_preflight_refused(mutate)

    def test_failed_positive_command_fails(self):
        def alter(command, cwd, normal):
            if "verify" in command:
                return subprocess.CompletedProcess(command, 9, "", "failed")
            return normal(command, cwd)

        self.assert_case_fails(alter, "expected exit 0, got 9")

    def test_false_rust_requires_expected_exit_and_diagnostic(self):
        cases = ((0, "postcondition not satisfied\n0 verified, 1 errors\n"), (1, "wrong failure\n"))
        for exit_code, output in cases:
            with self.subTest(exit_code=exit_code):
                def alter(command, cwd, normal):
                    if "admit_revision_false.rs" in " ".join(map(str, command)):
                        return subprocess.CompletedProcess(command, exit_code, output, "")
                    return normal(command, cwd)

                self.assert_case_fails(alter, "rust-false")

    def test_process_transport_error_fails(self):
        def alter(command, cwd, normal):
            if "admit_revision_false.rs" in " ".join(map(str, command)):
                raise OSError("transport down")
            return normal(command, cwd)

        self.assert_case_fails(alter, "process transport error")

    def test_missing_or_zero_verification_counts_fail(self):
        for text in ("verification complete\n", "0 verified, 0 errors\n"):
            with self.subTest(text=text):
                def alter(command, cwd, normal):
                    if "verify" in command:
                        return subprocess.CompletedProcess(command, 0, text, "")
                    return normal(command, cwd)

                self.assert_case_fails(alter, "missing vstd")

    def test_build_missing_or_zero_verification_counts_fail(self):
        for text in ("build complete\n", "0 verified, 0 errors\n"):
            with self.subTest(text=text):
                def alter(command, cwd, normal):
                    if "build" in command and Path(command[0]).name == "cargo-verus":
                        return subprocess.CompletedProcess(command, 0, text, "")
                    return normal(command, cwd)

                self.assert_case_fails(alter, "missing vstd")

    def test_partial_or_wrong_tlc_positive_output_fails(self):
        outputs = (
            "10 states generated, 5 distinct states found, 0 states left on queue.\n",
            "Model checking completed. No error has been found.\n10 states generated, 15 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 5.\n",
            "Model checking completed. No error has been found.\n10 states generated, 5 distinct states found, 10 states left on queue.\nThe depth of the complete state graph search is 5.\n",
            "Model checking completed. No error has been found.\n9 states generated, 5 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 5.\n",
            "Model checking completed. No error has been found.\n10 states generated, 5 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 6.\n",
        )
        for output in outputs:
            with self.subTest(output=output):
                def alter(command, cwd, normal):
                    if "tlc-positive" in str(cwd):
                        return subprocess.CompletedProcess(command, 0, output, "")
                    return normal(command, cwd)

                self.assert_case_fails(alter, "incomplete positive TLC evidence")

    def test_false_tlc_requires_exit_12_and_specific_invariant(self):
        valid_counts = "3 states generated, 3 distinct states found, 1 states left on queue.\n"
        cases = (
            (0, "Invariant EffectImpliesIntent is violated.\n" + valid_counts),
            (12, "Invariant Other is violated.\n" + valid_counts),
            (12, "Invariant EffectImpliesIntent is violated.\n4 states generated, 3 distinct states found, 1 states left on queue.\n"),
        )
        for exit_code, output in cases:
            with self.subTest(exit_code=exit_code):
                def alter(command, cwd, normal):
                    if "tlc-false" in str(cwd):
                        return subprocess.CompletedProcess(command, exit_code, output, "")
                    return normal(command, cwd)

                self.assert_case_fails(alter, "tlc-false")

    def test_timeout_preserves_partial_stdout_and_stderr(self):
        def alter(command, cwd, normal):
            if "verify" in command:
                raise subprocess.TimeoutExpired(command, 300, output="partial out\n", stderr="partial err\n")
            return normal(command, cwd)

        runner, output = self.run_failing_case(alter)
        del runner
        command_log = json.loads((output / "commands/03-cargo-verus-verify-1.json").read_text())
        self.assertEqual(command_log["stdout"], "partial out\n")
        self.assertEqual(command_log["stderr"], "partial err\n")
        self.assertIn("TimeoutExpired", command_log["transport_error"])

    def test_real_timeout_kills_owned_process_group_and_keeps_partial_logs(self):
        runner = load_runner()
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            pid_file = root / "child.pid"
            output = root / "evidence"
            output.mkdir()
            evidence = runner.Evidence(output)
            program = (
                "import pathlib, subprocess, sys, time; "
                "child=subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(2)']); "
                "pathlib.Path(sys.argv[1]).write_text(str(child.pid)); "
                "print('parent partial', flush=True); time.sleep(5)"
            )
            command = [sys.executable, "-c", program, str(pid_file)]
            started = time.monotonic()
            with mock.patch.object(runner, "TIMEOUT_SECONDS", 0.1):
                with self.assertRaises(runner.BootstrapFailure):
                    evidence.command(
                        "timeout-tree",
                        command,
                        root,
                        {"PATH": os.environ.get("PATH", "")},
                        runner._default_process_runner,
                    )
            elapsed = time.monotonic() - started
            child_pid = int(pid_file.read_text())
            try:
                try:
                    state = Path(f"/proc/{child_pid}/stat").read_text().split()[2]
                except FileNotFoundError:
                    state = None
                self.assertIn(state, (None, "Z"), "timed-out subprocess child is still running")
            finally:
                try:
                    os.kill(child_pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            self.assertLess(elapsed, 1.5)
            self.assertEqual(
                (output / "commands/01-timeout-tree.stdout").read_text(),
                "parent partial\n",
            )

    def test_existing_output_directory_is_never_reused(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fixture = self.make_fixture(root)
            runner = self.load_fixture_runner(fixture)
            output = root / "out"
            output.mkdir()
            sentinel = output / "keep"
            sentinel.write_text("unchanged")
            calls = []
            code = self.invoke(runner,
                runner.Options(fixture / "verus", fixture / "tlc.jar", output),
                process_runner=lambda *args: calls.append(args),
            )
            self.assertEqual(code, 2)
            self.assertEqual(calls, [])
            self.assertEqual(sentinel.read_text(), "unchanged")

    def test_source_changed_during_execution_fails_final_rehash(self):
        changed = False

        def alter(command, cwd, normal):
            nonlocal changed
            result = normal(command, cwd)
            if "tlc-false" in str(cwd) and not changed:
                source = cwd.parents[1] / "repo/verification/bootstrap/admission/src/main.rs"
                source.write_text("changed while running\n")
                changed = True
            return result

        self.assert_case_fails(alter, "source input changed during execution")

    def test_execution_copy_changed_during_execution_fails_final_rehash(self):
        changed = False

        def alter(command, cwd, normal):
            nonlocal changed
            result = normal(command, cwd)
            if "tlc-false" in str(cwd) and not changed:
                (cwd / "ToolControl.tla").write_text("changed execution copy\n")
                changed = True
            return result

        self.assert_case_fails(alter, "execution source changed during execution")

    def test_cli_has_no_manifest_override(self):
        runner = load_runner()
        with self.assertRaises(SystemExit):
            runner._parse_args(
                ["--verus-root", "/v", "--tlc-jar", "/t", "--output", "/o", "--tool-pins", "/p"]
            )

    def test_output_inside_repository_is_refused(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fixture = self.make_fixture(root)
            runner = self.load_fixture_runner(fixture)
            output = fixture / "repo/evidence"
            code = self.invoke(runner,
                runner.Options(fixture / "verus", fixture / "tlc.jar", output),
                process_runner=lambda *args: self.fail("must not execute"),
            )
            self.assertEqual(code, 2)
            self.assertFalse(output.exists())

    def test_output_resolving_inside_tool_or_source_root_is_refused(self):
        for root_name in ("verus", "repo"):
            with self.subTest(root=root_name), tempfile.TemporaryDirectory() as td:
                root = Path(td)
                fixture = self.make_fixture(root)
                runner = self.load_fixture_runner(fixture)
                link = fixture / "linked-root"
                link.symlink_to(fixture / root_name, target_is_directory=True)
                output = link / "evidence"
                code = self.invoke(
                    runner,
                    runner.Options(fixture / "verus", fixture / "tlc.jar", output),
                    process_runner=lambda *args: self.fail("must not execute"),
                )
                self.assertEqual(code, 2)
                self.assertFalse((fixture / root_name / "evidence").exists())

    def assert_case_fails(self, alter, expected_error):
        _, output = self.run_failing_case(alter)
        result = json.loads((output / "result.json").read_text())
        self.assertEqual(result["status"], "failed")
        self.assertIn(expected_error, result["error"])

    def assert_preflight_refused(self, mutate):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            fixture = self.make_fixture(root)
            manifest_path = fixture / "repo/verification/tool-pins.json"
            manifest = json.loads(manifest_path.read_text())
            mutate(fixture, manifest)
            manifest_path.write_text(json.dumps(manifest))
            runner = self.load_fixture_runner(fixture)
            calls = []
            code = self.invoke(
                runner,
                runner.Options(fixture / "verus", fixture / "tlc.jar", root / "out"),
                process_runner=lambda *args: calls.append(args),
            )
            self.assertEqual(code, 1)
            self.assertEqual(calls, [])

    def run_failing_case(self, alter):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        fixture = self.make_fixture(root)
        runner = self.load_fixture_runner(fixture)
        output = root / "out"

        def normal(command, cwd):
            return subprocess.CompletedProcess(
                command,
                self.exit_for(command, cwd),
                self.output_for(command, cwd),
                "",
            )

        def fake(command, cwd, env, timeout):
            return alter(command, cwd, normal)

        code = self.invoke(
            runner,
            runner.Options(fixture / "verus", fixture / "tlc.jar", output),
            process_runner=fake,
        )
        self.assertEqual(code, 1)
        return runner, output

    @staticmethod
    def digest(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def make_fixture(self, root):
        script_dir = root / "repo" / "verification" / "bootstrap"
        admission = script_dir / "admission"
        (admission / "src").mkdir(parents=True)
        (script_dir / "false").mkdir()
        for variant in ("positive", "false"):
            (script_dir / "model" / variant).mkdir(parents=True)
            (script_dir / "model" / variant / "ToolControl.tla").write_text(variant)
            (script_dir / "model" / variant / "ToolControl.cfg").write_text("cfg")
        (admission / "Cargo.toml").write_text(
            '[package]\npublish = false\n[dependencies]\nvstd = "=0.0.0-2026-08-30-0159"\n'
        )
        (admission / "Cargo.lock").write_text(
            '[[package]]\nname = "vstd"\nversion = "0.0.0-2026-08-30-0159"\n'
        )
        (admission / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.97.1"\n')
        (admission / "src" / "main.rs").write_text("fn admit_revision() {}\nfn main() {}\n")
        (script_dir / "false" / "admit_revision_false.rs").write_text(
            "fn admit_revision() {}\nfn main() {}\n"
        )
        verus_root = root / "verus"
        verus_root.mkdir()
        for name in ("verus", "cargo-verus", "rust_verify", "z3"):
            (verus_root / name).write_text(name)
        tlc = root / "tlc.jar"
        tlc.write_text("tlc")
        pins = {
            "status": "bootstrap-only",
            "claims": {
                "formal_check_tool_validation": False,
                "bootstrap_tool_validation": True,
                "production_binding": False,
                "full_package_b_closure": False,
            },
            "rust": {"channel": "1.97.1", "rustc_commit": "8bab26f4f68e0e26f0bb7960be334d5b520ea452"},
            "verus": {
                "verus_sha256": self.digest(verus_root / "verus"),
                "cargo_verus_sha256": self.digest(verus_root / "cargo-verus"),
                "rust_verify_sha256": self.digest(verus_root / "rust_verify"),
                "z3": {"sha256": self.digest(verus_root / "z3")},
            },
            "cargo_dependencies": {
                "vstd": "=0.0.0-2026-08-30-0159",
                "cargo_lock_sha256": self.digest(admission / "Cargo.lock"),
            },
            "tlc": {"jar_sha256": self.digest(tlc)},
            "bootstrap_sources": {
                relative: self.digest(script_dir / relative)
                for relative in self.source_files()
            },
        }
        (script_dir.parent / "tool-pins.json").write_text(json.dumps(pins))
        return root

    @staticmethod
    def source_files():
        return (
            "admission/Cargo.toml",
            "admission/Cargo.lock",
            "admission/rust-toolchain.toml",
            "admission/src/main.rs",
            "false/admit_revision_false.rs",
            "model/positive/ToolControl.cfg",
            "model/positive/ToolControl.tla",
            "model/false/ToolControl.cfg",
            "model/false/ToolControl.tla",
        )

    @staticmethod
    def load_fixture_runner(fixture):
        installed = fixture / "repo/verification/bootstrap/run.py"
        shutil.copyfile(MODULE_PATH, installed)
        return load_runner(installed)

    @staticmethod
    def invoke(runner, options, process_runner):
        with mock.patch.dict(os.environ, {"PATH": os.environ.get("PATH", "")}, clear=True):
            return runner._run_bootstrap(options, process_runner=process_runner)

    @staticmethod
    def exit_for(command, cwd):
        joined = " ".join(map(str, command))
        if "admit_revision_false.rs" in joined:
            return 1
        if "tlc-false" in str(cwd):
            return 12
        return 0

    @staticmethod
    def output_for(command, cwd):
        joined = " ".join(map(str, command))
        if command[-1:] == ["--version"] and Path(command[0]).name == "cargo":
            return "cargo 1.97.1 (fake)\n"
        if command[-1:] == ["-Vv"]:
            return "rustc 1.97.1\nrelease: 1.97.1\ncommit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452\n"
        if Path(command[0]).name == "cargo-verus" and (
            "verify" in command or "build" in command
        ):
            return "verification results:: 2045 verified, 0 errors\nverification results:: 1 verified, 0 errors\n"
        if "admit_revision_false.rs" in joined:
            return "postcondition not satisfied\nverification results:: 0 verified, 1 errors\n"
        if "tlc-positive" in str(cwd):
            return "Model checking completed. No error has been found.\n10 states generated, 5 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 5.\n"
        if "tlc-false" in str(cwd):
            return "Invariant EffectImpliesIntent is violated.\n3 states generated, 3 distinct states found, 1 states left on queue.\n"
        return "ok\n"


if __name__ == "__main__":
    unittest.main()
