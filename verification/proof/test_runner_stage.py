import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("stage.py")
SPEC = importlib.util.spec_from_file_location("proof_runner_stage", MODULE_PATH)
STAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STAGE)

RUNNER_FILES = (
    "verification/proof/run.py",
    "verification/proof/stage.py",
    "verification/proof/dependencies.py",
    "verification/proof/toolchain.py",
    "verification/proof/distribution.py",
    "verification/proof/command.py",
    "verification/proof/observation.py",
    "verification/proof/sandbox.py",
)
ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
HELLO_SHA256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"


class RunnerStageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repository"
        self.repository.mkdir()
        self.destination = self.root / "runner"
        self._git("init", "-q")
        self._git("config", "user.name", "Fixture")
        self._git("config", "user.email", "fixture@example.invalid")
        for relative in RUNNER_FILES:
            path = self.repository / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"abc")
        sandbox = self.repository / "verification/proof/sandbox.py"
        sandbox.write_bytes(b"hello")
        sandbox.chmod(0o755)
        self._git("add", ".")
        self._git("commit", "-q", "-m", "runner fixture")
        self.commit = self._git("rev-parse", "HEAD").stdout.strip()

    def _git(self, *arguments):
        return subprocess.run(
            ["/usr/bin/git", *arguments],
            cwd=self.repository,
            env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
            text=True,
            capture_output=True,
            check=True,
        )

    def _commit(self, message):
        self._git("add", "-A")
        self._git("commit", "-q", "-m", message)
        return self._git("rev-parse", "HEAD").stdout.strip()

    def test_stages_exact_committed_runner_despite_dirty_and_untracked_files(self):
        (self.repository / "verification/proof/run.py").write_bytes(b"dirty")
        cache = self.repository / "verification/proof/__pycache__/stage.pyc"
        cache.parent.mkdir()
        cache.write_bytes(b"opaque")

        result = STAGE.stage_runner(self.repository, self.commit, self.destination)

        self.assertEqual(
            {key: result[key] for key in ("status", "qualification", "commit")},
            {"status": "runner-staged", "qualification": False, "commit": self.commit},
        )
        self.assertEqual(set(result["file_sha256"]), set(RUNNER_FILES))
        self.assertEqual(set(result["file_mode"]), set(RUNNER_FILES))
        expected_hashes = {relative: ABC_SHA256 for relative in RUNNER_FILES}
        expected_hashes["verification/proof/sandbox.py"] = HELLO_SHA256
        self.assertEqual(result["file_sha256"], expected_hashes)
        for relative in RUNNER_FILES:
            expected = b"hello" if relative == "verification/proof/sandbox.py" else b"abc"
            self.assertEqual((self.destination / relative).read_bytes(), expected)
        self.assertEqual(result["file_mode"]["verification/proof/sandbox.py"], "100755")
        self.assertEqual((self.destination / "verification/proof/run.py").read_bytes(), b"abc")
        self.assertFalse((self.destination / "verification/proof/__pycache__").exists())
        self.assertEqual(
            oct(os.stat(self.destination / "verification/proof/sandbox.py").st_mode & 0o777),
            "0o755",
        )

    def test_runner_directory_modes_ignore_child_umask(self):
        masks = [0o000, 0o022, 0o077]
        for mask in masks:
            with self.subTest(mask=mask):
                destination = self.root / f"runner-mask-{mask:o}"
                loader_script = (
                    "import importlib.util, sys, pathlib, os\n"
                    "spec = importlib.util.spec_from_file_location('stage', sys.argv[1])\n"
                    "module = importlib.util.module_from_spec(spec)\n"
                    "spec.loader.exec_module(module)\n"
                    "os.umask(int(sys.argv[5]))\n"
                    "module.stage_runner(pathlib.Path(sys.argv[2]), sys.argv[3], pathlib.Path(sys.argv[4]))"
                )
                command = [
                    "/usr/bin/python3", "-I", "-B", "-c", loader_script,
                    str(MODULE_PATH), str(self.repository), self.commit,
                    str(destination), str(mask),
                ]
                result = subprocess.run(
                    command, text=True, capture_output=True, timeout=30,
                    check=False, env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, "")
                self.assertEqual(result.stderr, "")
                self.assertEqual(destination.stat().st_mode & 0o777, 0o700)
                verification = destination / "verification"
                self.assertEqual(verification.stat().st_mode & 0o777, 0o755)
                proof = verification / "proof"
                self.assertEqual(proof.stat().st_mode & 0o777, 0o755)
                for relative in RUNNER_FILES:
                    expected = 0o755 if relative == "verification/proof/sandbox.py" else 0o644
                    self.assertEqual((destination / relative).stat().st_mode & 0o777, expected)

    def test_malformed_missing_and_nonfile_selection_refuse_before_output(self):
        cases = [("malformed", "HEAD"), ("unknown", "0" * 40)]
        for name, commit in cases:
            with self.subTest(name=name):
                destination = self.root / f"runner-{name}"
                with self.assertRaises(STAGE.StageFailure):
                    STAGE.stage_runner(self.repository, commit, destination)
                self.assertFalse(destination.exists())

        selected = self.repository / "verification/proof/run.py"
        selected.unlink()
        missing = self._commit("missing selected runner file")
        with self.assertRaises(STAGE.StageFailure):
            STAGE.stage_runner(self.repository, missing, self.root / "runner-missing")
        self.assertFalse((self.root / "runner-missing").exists())

        selected.mkdir()
        (selected / "child.py").write_bytes(b"abc")
        nonfile = self._commit("selected path is a tree")
        with self.assertRaises(STAGE.StageFailure):
            STAGE.stage_runner(self.repository, nonfile, self.root / "runner-tree")
        self.assertFalse((self.root / "runner-tree").exists())

    def test_committed_symlink_mode_refuses_before_output(self):
        selected = self.repository / "verification/proof/run.py"
        selected.unlink()
        selected.symlink_to("stage.py")
        commit = self._commit("runner symlink")
        with self.assertRaises(STAGE.StageFailure):
            STAGE.stage_runner(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_committed_submodule_mode_refuses_before_output(self):
        relative = "verification/proof/run.py"
        self._git(
            "update-index", "--add", "--cacheinfo",
            f"160000,{self.commit},{relative}",
        )
        self._git("commit", "-q", "-m", "runner gitlink")
        commit = self._git("rev-parse", "HEAD").stdout.strip()
        with self.assertRaises(STAGE.StageFailure):
            STAGE.stage_runner(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_existing_destination_is_refused_and_sentinel_preserved(self):
        self.destination.mkdir()
        sentinel = self.destination / "sentinel"
        sentinel.write_bytes(b"keep")
        with self.assertRaises(STAGE.StageFailure):
            STAGE.stage_runner(self.repository, self.commit, self.destination)
        self.assertEqual(sentinel.read_bytes(), b"keep")
        self.assertEqual(list(self.destination.iterdir()), [sentinel])

    def test_oversize_nul_and_non_utf8_blobs_refuse_before_output(self):
        selected = self.repository / "verification/proof/run.py"
        cases = {
            "oversize": b"a" * (1024 * 1024 + 1),
            "nul": b"abc\0",
            "non-utf8": b"\xff",
        }
        for name, raw in cases.items():
            with self.subTest(name=name):
                selected.write_bytes(raw)
                commit = self._commit(name)
                destination = self.root / f"runner-{name}"
                with self.assertRaises(STAGE.StageFailure):
                    STAGE.stage_runner(self.repository, commit, destination)
                self.assertFalse(destination.exists())

    def test_isolated_execution_uses_committed_source_not_stale_checkout_bytecode(self):
        selected = self.repository / "verification/proof/run.py"
        selected.write_bytes(b'print("stale")\n')
        subprocess.run(
            ["/usr/bin/python3", "-B", "-m", "py_compile", str(selected)],
            cwd=self.repository,
            env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
            check=True,
        )
        source_time = selected.stat().st_mtime_ns
        stale_cache = next((selected.parent / "__pycache__").glob("run.*.pyc"))
        self.assertTrue(stale_cache.is_file())
        selected.write_bytes(b'print("fresh")\n')
        os.utime(selected, ns=(source_time, source_time))
        commit = self._commit("fresh runner source with stale checkout bytecode")

        loader = (
            "import importlib.util,sys;"
            "s=importlib.util.spec_from_file_location('fixture',sys.argv[1]);"
            "m=importlib.util.module_from_spec(s);s.loader.exec_module(m)"
        )
        original = subprocess.run(
            ["/usr/bin/python3", "-I", "-B", "-c", loader, str(selected)],
            cwd=self.repository,
            env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(original.returncode, 0)
        self.assertEqual(original.stdout, "stale\n")
        self.assertEqual(original.stderr, "")

        STAGE.stage_runner(self.repository, commit, self.destination)
        staged = self.destination / "verification/proof/run.py"
        completed = subprocess.run(
            ["/usr/bin/python3", "-I", "-B", "-c", loader, str(staged)],
            cwd=self.destination,
            env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
            text=True,
            capture_output=True,
            check=False,
        )

        self.assertEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, "fresh\n")
        self.assertEqual(completed.stderr, "")
        self.assertEqual(staged.read_bytes(), b'print("fresh")\n')
        self.assertFalse((staged.parent / "__pycache__").exists())


if __name__ == "__main__":
    unittest.main()
