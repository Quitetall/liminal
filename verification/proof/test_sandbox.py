import importlib.util
import stat
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("sandbox.py")
SPEC = importlib.util.spec_from_file_location("proof_sandbox", MODULE_PATH)
sandbox = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sandbox)


class SandboxPreparationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.rust = self.root / "rust"
        self.verus = self.root / "verus"
        self.vendor = self.root / "vendor"
        for path in (self.source, self.rust, self.verus, self.vendor):
            path.mkdir()
        self.destination = self.root / "request"

    def test_verify_request_reproduces_the_fixed_successful_sandbox(self):
        result = sandbox.prepare_sandbox(
            self.source,
            self.rust,
            self.verus,
            self.vendor,
            self.destination,
            "verify",
        )

        expected_argv = [
            "/usr/bin/bwrap", "--unshare-all", "--unshare-user",
            "--die-with-parent", "--new-session", "--disable-userns",
            "--clearenv", "--proc", "/proc", "--dev", "/dev",
            "--ro-bind", "/usr", "/usr", "--symlink", "usr/bin", "/bin",
            "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
            "--ro-bind", str(self.rust), "/rust",
            "--ro-bind", str(self.verus), "/verus",
            "--ro-bind", str(self.source), "/work",
            "--ro-bind", str(self.vendor), "/vendor",
            "--bind", str(self.destination / "target"), "/target",
            "--bind", str(self.destination / "cargo-home"), "/cargo-home",
            "--bind", str(self.destination / "rustup-home"), "/rustup-home",
            "--ro-bind", str(self.rust),
            "/rustup-home/toolchains/1.97.1-x86_64-unknown-linux-gnu",
            "--bind", str(self.destination / "tmp"), "/tmp",
            "--bind", str(self.destination / "logs"), "/logs",
            "--setenv", "PATH", "/verus:/rust/bin:/usr/bin:/bin",
            "--setenv", "HOME", "/cargo-home",
            "--setenv", "CARGO_HOME", "/cargo-home",
            "--setenv", "CARGO_TARGET_DIR", "/target",
            "--setenv", "CARGO_BUILD_JOBS", "2",
            "--setenv", "CARGO_NET_OFFLINE", "true",
            "--setenv", "RUSTUP_HOME", "/rustup-home",
            "--setenv", "RUSTUP_TOOLCHAIN", "/rust",
            "--setenv", "TMPDIR", "/tmp",
            "--setenv", "LANG", "C.UTF-8",
            "--setenv", "LC_ALL", "C.UTF-8",
            "--setenv", "LD_LIBRARY_PATH", "/rust/lib:/verus",
            "--chdir", "/work", "--",
            "/usr/bin/strace", "-s", "65536", "-f", "-qq", "-e",
            "trace=execve,execveat,open,openat,openat2,chdir,fchdir",
            "-o", "/logs/trace.log", "/verus/cargo-verus", "verify",
            "--fwd-verus-args-to", "roots", "--config",
            'source.crates-io.replace-with="vendored-sources"', "--config",
            'source.vendored-sources.directory="/vendor"', "--locked",
            "--offline", "-p", "liminal-safety", "--", "--output-json",
            "--no-cheating",
        ]
        self.assertEqual(
            result,
            {
                "schema": "liminal-sandbox-request-v1",
                "status": "sandbox-prepared",
                "qualification": False,
                "operation": "verify",
                "argv": expected_argv,
                "cwd": str(self.destination / "command-parent"),
                "environment": {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
                "output": str(self.destination / "command-evidence"),
            },
        )
        self.assertEqual(stat.S_IMODE(self.destination.stat().st_mode), 0o700)
        for name in ("target", "cargo-home", "rustup-home", "tmp", "logs", "command-parent"):
            path = self.destination / name
            self.assertTrue(path.is_dir())
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o700)
        self.assertFalse((self.destination / "command-evidence").exists())

    def test_unknown_operation_is_refused_before_destination_creation(self):
        for operation in ("execute", None, []):
            with self.subTest(operation=operation):
                with self.assertRaises(sandbox.SandboxFailure):
                    sandbox.prepare_sandbox(
                        self.source, self.rust, self.verus, self.vendor,
                        self.destination, operation,
                    )
                self.assertFalse(self.destination.exists())

    def test_existing_destination_is_refused_and_sentinel_is_untouched(self):
        self.destination.mkdir()
        sentinel = self.destination / "sentinel"
        sentinel.write_bytes(b"keep")
        with self.assertRaises(sandbox.SandboxFailure):
            sandbox.prepare_sandbox(
                self.source, self.rust, self.verus, self.vendor,
                self.destination, "verify",
            )
        self.assertEqual(sentinel.read_bytes(), b"keep")
        self.assertEqual(list(self.destination.iterdir()), [sentinel])

    def test_input_overlap_and_nested_destination_are_refused_before_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            rust = source / "nested-rust"
            verus = root / "verus"
            vendor = root / "vendor"
            for path in (rust, verus, vendor):
                path.mkdir(parents=True)
            destination = root / "request"
            with self.assertRaises(sandbox.SandboxFailure):
                sandbox.prepare_sandbox(
                    source, rust, verus, vendor, destination, "verify"
                )
            self.assertFalse(destination.exists())

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = [root / name for name in ("source", "rust", "verus", "vendor")]
            for path in inputs:
                path.mkdir()
            destination = inputs[0] / "request"
            with self.assertRaises(sandbox.SandboxFailure):
                sandbox.prepare_sandbox(*inputs, destination, "verify")
            self.assertFalse(destination.exists())

    def test_invalid_input_and_destination_paths_refuse_before_creation(self):
        regular_file = self.root / "not-directory"
        regular_file.write_bytes(b"abc")
        symlink_input = self.root / "source-link"
        symlink_input.symlink_to(self.source, target_is_directory=True)
        alias_parent = self.root / "parent-link"
        real_parent = self.root / "real-parent"
        real_parent.mkdir()
        alias_parent.symlink_to(real_parent, target_is_directory=True)
        relative_destination = Path("request")
        self.assertFalse(relative_destination.exists())
        host_destination = Path("/usr/request")
        try:
            host_before = host_destination.lstat()
        except FileNotFoundError:
            host_before = None

        cases = {
            "relative-input": (Path("source"), self.destination),
            "missing-input": (self.root / "missing", self.destination),
            "non-directory-input": (regular_file, self.destination),
            "symlink-input": (symlink_input, self.destination),
            "retained-parent-component": (
                Path(str(self.root) + "/source/../source"), self.destination
            ),
            "nul-input": (Path(str(self.root) + "/bad\0name"), self.destination),
            "surrogate-input": (Path(str(self.root) + "/bad\ud800"), self.destination),
            "forbidden-without-traversal": (
                self.root / "heldout" / "unreadable", self.destination
            ),
            "artifact-without-traversal": (
                self.root / "liminal-5.3-spark" / "unreadable", self.destination
            ),
            "host-runtime-input": (Path("/usr"), self.destination),
            "filesystem-root-input": (Path("/"), self.destination),
            "relative-destination": (self.source, relative_destination),
            "symlink-destination-parent": (
                self.source, alias_parent / "request"
            ),
            "host-runtime-destination": (self.source, host_destination),
        }
        for name, (source, destination) in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(sandbox.SandboxFailure):
                    sandbox.prepare_sandbox(
                        source, self.rust, self.verus, self.vendor,
                        destination, "verify",
                    )
                self.assertFalse(self.destination.exists())
                self.assertFalse((real_parent / "request").exists())
                if name == "relative-destination":
                    self.assertFalse(relative_destination.exists())
                if name == "host-runtime-destination":
                    try:
                        host_after = host_destination.lstat()
                    except FileNotFoundError:
                        host_after = None
                    self.assertEqual(host_after, host_before)

    def test_final_destination_symlinks_are_refused_and_untouched(self):
        for name, dangling in (("existing-target", False), ("dangling", True)):
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    inputs = [root / item for item in ("source", "rust", "verus", "vendor")]
                    for path in inputs:
                        path.mkdir()
                    target = root / "target"
                    if not dangling:
                        target.mkdir()
                        sentinel = target / "sentinel"
                        sentinel.write_bytes(b"keep")
                    destination = root / "request"
                    destination.symlink_to(target, target_is_directory=True)
                    link_text = destination.readlink()

                    with self.assertRaises(sandbox.SandboxFailure):
                        sandbox.prepare_sandbox(*inputs, destination, "verify")

                    self.assertTrue(destination.is_symlink())
                    self.assertEqual(destination.readlink(), link_text)
                    if dangling:
                        self.assertFalse(target.exists())
                    else:
                        self.assertEqual(sentinel.read_bytes(), b"keep")
                        self.assertEqual(list(target.iterdir()), [sentinel])

    def test_double_slash_runtime_aliases_are_refused_before_creation(self):
        for name, source in {
            "usr-alias": Path("//usr"),
            "usr-bin-alias": Path("//usr/bin"),
            "root-alias": Path("//"),
        }.items():
            with self.subTest(name=name):
                destination = self.root / f"request-{name}"
                with self.assertRaises(sandbox.SandboxFailure):
                    sandbox.prepare_sandbox(
                        source, self.rust, self.verus, self.vendor,
                        destination, "verify",
                    )
                self.assertFalse(destination.exists())

    def test_build_operations_use_only_the_fixed_witness_commands(self):
        commands = {
            "ordinary-build": [
                "/rust/bin/cargo", "build", "--release", "--locked", "--offline",
                "-p", "liminal-safety", "--example", "acknowledgement_witness",
                "--config", 'source.crates-io.replace-with="vendored-sources"',
                "--config", 'source.vendored-sources.directory="/vendor"',
            ],
            "verified-build": [
                "/verus/cargo-verus", "build", "--fwd-verus-args-to", "roots",
                "--release", "--locked", "--offline", "-p", "liminal-safety",
                "--config", 'source.crates-io.replace-with="vendored-sources"',
                "--config", 'source.vendored-sources.directory="/vendor"',
                "--example", "acknowledgement_witness", "--", "--output-json",
                "--no-cheating",
            ],
        }
        for operation, command in commands.items():
            with self.subTest(operation=operation):
                destination = self.root / operation
                result = sandbox.prepare_sandbox(
                    self.source, self.rust, self.verus, self.vendor,
                    destination, operation,
                )
                self.assertIs(result["qualification"], False)
                self.assertEqual(result["operation"], operation)
                marker = result["argv"].index("/logs/trace.log")
                self.assertEqual(result["argv"][marker + 1:], command)
                self.assertEqual(result["cwd"], str(destination / "command-parent"))
                self.assertEqual(result["output"], str(destination / "command-evidence"))


if __name__ == "__main__":
    unittest.main()
