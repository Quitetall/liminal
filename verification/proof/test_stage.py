import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("stage.py")
SPEC = importlib.util.spec_from_file_location("proof_stage", MODULE_PATH)
STAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STAGE)
StageFailure = STAGE.StageFailure
stage_source = STAGE.stage_source

ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
SOURCE_COMMIT = "ba9d9ae9c88ceae74307f440657d1ecaf7b81ab9"
SOURCE_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "crates/liminal-safety/Cargo.toml",
    "crates/liminal-safety/src/lib.rs",
    "crates/liminal-safety/examples/acknowledgement_witness.rs",
    "crates/liminal-jurisdiction/Cargo.toml",
    "crates/liminal-jurisdiction/src/ilrp.rs",
)


class StageSourceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repository"
        self.repository.mkdir()
        self.destination = self.root / "staged"
        self._git("init", "-q")
        self._git("config", "user.name", "Fixture")
        self._git("config", "user.email", "fixture@example.invalid")
        for relative in SOURCE_PATHS:
            path = self.repository / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"abc")
        self._write_manifests()
        self._git("add", ".")
        self._git("commit", "-q", "-m", "fixture")
        self.commit = self._git("rev-parse", "HEAD").stdout.strip()

    def tearDown(self):
        self.temporary.cleanup()

    def _git(self, *arguments):
        return subprocess.run(
            ["/usr/bin/git", *arguments], cwd=self.repository,
            env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
            text=True, capture_output=True, check=True,
        )

    def _write_manifests(self, *, inventory_overrides=None):
        inputs = {
            "schema": "liminal-proof-inputs-v1",
            "fragment": "ack-identity-dependency-v1",
            "discharges_obligation": False,
            "source_sha256": {relative: ABC_SHA256 for relative in SOURCE_PATHS},
            "tool_sha256": {name: ABC_SHA256 for name in ("verus", "cargo-verus", "rust_verify", "z3")},
        }
        inventory_files = {
            relative: {"sha256": ABC_SHA256, "mode": "100644"}
            for relative in SOURCE_PATHS
        }
        if inventory_overrides:
            inventory_overrides(inventory_files)
        inventory = {
            "schema": "liminal-proof-source-inventory-v1",
            "source_commit": SOURCE_COMMIT,
            "files": inventory_files,
        }
        proof = self.repository / "verification/proof"
        proof.mkdir(parents=True, exist_ok=True)
        (proof / "inputs.json").write_text(json.dumps(inputs), encoding="utf-8")
        (proof / "source-inventory.json").write_text(json.dumps(inventory), encoding="utf-8")

    def _commit_changes(self, message):
        self._git("add", "-A")
        self._git("commit", "-q", "-m", message)
        return self._git("rev-parse", "HEAD").stdout.strip()

    def test_reconstructs_committed_source_despite_dirty_working_file(self):
        (self.repository / "Cargo.toml").write_bytes(b"dirty")
        result = stage_source(self.repository, self.commit, self.destination)
        self.assertEqual((self.destination / "Cargo.toml").read_bytes(), b"abc")
        self.assertEqual(
            {
                key: result[key]
                for key in ("status", "qualification", "commit", "inventory_origin")
            },
            {
                "status": "source-staged",
                "qualification": False,
                "commit": self.commit,
                "inventory_origin": SOURCE_COMMIT,
            },
        )
        self.assertEqual(result["file_sha256"]["Cargo.toml"], ABC_SHA256)
        self.assertEqual(result["file_mode"]["Cargo.toml"], "100644")
        self.assertEqual(
            (self.destination / "verification/proof/inputs.json").read_bytes(),
            self._git("show", f"{self.commit}:verification/proof/inputs.json").stdout.encode(),
        )
        self.assertEqual(
            set(result["file_sha256"]),
            set(SOURCE_PATHS)
            | {"verification/proof/inputs.json", "verification/proof/source-inventory.json"},
        )

    def test_mutable_or_unknown_commit_is_refused_before_destination_creation(self):
        for commit in ("HEAD", "0" * 40):
            with self.subTest(commit=commit):
                destination = self.root / ("stage-" + commit[:8])
                with self.assertRaises(StageFailure):
                    stage_source(self.repository, commit, destination)
                self.assertFalse(destination.exists())

    def test_committed_inventory_pin_mismatch_is_refused_before_destination_creation(self):
        self._write_manifests(
            inventory_overrides=lambda files: files["Cargo.toml"].update(sha256="0" * 64)
        )
        commit = self._commit_changes("mismatched inventory")
        with self.assertRaises(StageFailure):
            stage_source(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_committed_source_byte_drift_is_refused_before_destination_creation(self):
        (self.repository / "Cargo.toml").write_bytes(b"ab" + b"d")
        commit = self._commit_changes("changed source bytes")
        with self.assertRaisesRegex(StageFailure, "source hash mismatch: Cargo.toml"):
            stage_source(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_committed_executable_mode_drift_is_refused_before_destination_creation(self):
        source = self.repository / "Cargo.toml"
        source.chmod(0o755)
        commit = self._commit_changes("changed source mode")
        self.assertEqual(self._git("ls-tree", commit, "--", "Cargo.toml").stdout[:6], "100755")
        with self.assertRaisesRegex(StageFailure, "source mode mismatch: Cargo.toml"):
            stage_source(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_oversized_committed_inputs_manifest_is_refused_before_destination_creation(self):
        inputs = self.repository / "verification/proof/inputs.json"
        inputs.write_bytes(b" " * (64 * 1024 + 1))
        commit = self._commit_changes("oversized inputs manifest")
        with self.assertRaisesRegex(StageFailure, "Git output exceeds"):
            stage_source(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_unexpected_build_script_is_refused_before_destination_creation(self):
        build = self.repository / "crates/liminal-safety/build.rs"
        build.write_bytes(b"abc")
        commit = self._commit_changes("unexpected build script")
        with self.assertRaises(StageFailure):
            stage_source(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_committed_symlink_is_refused_before_destination_creation(self):
        link = self.repository / "crates/liminal-safety/src/link.rs"
        link.symlink_to("lib.rs")
        commit = self._commit_changes("unexpected symlink")
        with self.assertRaises(StageFailure):
            stage_source(self.repository, commit, self.destination)
        self.assertFalse(self.destination.exists())

    def test_existing_destination_is_refused_and_untouched(self):
        self.destination.mkdir()
        sentinel = self.destination / "sentinel"
        sentinel.write_bytes(b"abc")
        try:
            with self.assertRaises(StageFailure):
                stage_source(self.repository, self.commit, self.destination)
        finally:
            self.assertEqual(sentinel.read_bytes(), b"abc")
            self.assertEqual({path.name for path in self.destination.iterdir()}, {"sentinel"})

    def test_destination_parent_alias_into_repository_is_refused_without_output(self):
        alias = self.root / "repository-alias"
        alias.symlink_to(self.repository, target_is_directory=True)
        destination = alias / "staged"
        with self.assertRaises(StageFailure):
            stage_source(self.repository, self.commit, destination)
        self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
