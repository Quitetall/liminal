import json
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

MODULE_PATH = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("proof_runner", MODULE_PATH)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
InputFailure = RUNNER.InputFailure
check_inputs = RUNNER.check_inputs


ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
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
TOOL_PATHS = ("verus", "cargo-verus", "rust_verify", "z3")


class ProofInputTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repository"
        self.verus_root = self.root / "verus-root"
        for base, paths in ((self.repository, SOURCE_PATHS), (self.verus_root, TOOL_PATHS)):
            for relative in paths:
                path = base / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"abc")
        manifest = {
            "schema": "liminal-proof-inputs-v1",
            "fragment": "ack-identity-dependency-v1",
            "discharges_obligation": False,
            "source_sha256": {path: ABC_SHA256 for path in SOURCE_PATHS},
            "tool_sha256": {path: ABC_SHA256 for path in TOOL_PATHS},
        }
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest_path.parent.mkdir(parents=True)
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        source_inventory = {
            "schema": "liminal-proof-source-inventory-v1",
            "source_commit": "3bd93a9617ff8705ace37f4a19440c70f867f689",
            "files": {
                path: {"sha256": ABC_SHA256, "mode": "100644"}
                for path in SOURCE_PATHS
            },
        }
        (self.repository / "verification/proof/source-inventory.json").write_text(
            json.dumps(source_inventory), encoding="utf-8"
        )

    def tearDown(self):
        self.temporary.cleanup()

    def read_source_inventory(self):
        path = self.repository / "verification/proof/source-inventory.json"
        return path, json.loads(path.read_text(encoding="utf-8"))

    def add_inventory_file(self, relative):
        path, inventory = self.read_source_inventory()
        inventory["files"][relative] = {"sha256": ABC_SHA256, "mode": "100644"}
        path.write_text(json.dumps(inventory), encoding="utf-8")

    def test_changed_source_is_rejected(self):
        (self.repository / "Cargo.toml").write_bytes(b"changed")

        with self.assertRaisesRegex(InputFailure, "Cargo.toml"):
            check_inputs(self.repository, self.verus_root)

    def test_valid_pinned_inputs_return_actual_hashes_without_qualification(self):
        self.assertEqual(
            check_inputs(self.repository, self.verus_root),
            {
                "status": "inputs-valid",
                "qualification": False,
                "source_sha256": {path: ABC_SHA256 for path in SOURCE_PATHS},
                "tool_sha256": {path: ABC_SHA256 for path in TOOL_PATHS},
            },
        )

    def test_changed_tool_is_rejected(self):
        (self.verus_root / "z3").write_bytes(b"changed")

        with self.assertRaisesRegex(InputFailure, "z3"):
            check_inputs(self.repository, self.verus_root)

    def test_missing_source_key_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        del manifest["source_sha256"]["Cargo.toml"]
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "source_sha256"):
            check_inputs(self.repository, self.verus_root)

    def test_extra_source_key_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["source_sha256"]["unapproved.rs"] = ABC_SHA256
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "source_sha256"):
            check_inputs(self.repository, self.verus_root)

    def test_symlink_source_is_rejected(self):
        source = self.repository / "Cargo.toml"
        source.unlink()
        source.symlink_to(self.repository / "Cargo.lock")

        with self.assertRaisesRegex(InputFailure, "Cargo.toml"):
            check_inputs(self.repository, self.verus_root)

    def test_duplicate_json_field_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        text = manifest_path.read_text(encoding="utf-8")
        manifest_path.write_text(
            text.replace(
                '"schema": "liminal-proof-inputs-v1",',
                '"schema": "liminal-proof-inputs-v1", "schema": "liminal-proof-inputs-v1",',
                1,
            ),
            encoding="utf-8",
        )

        with self.assertRaisesRegex(InputFailure, "duplicate"):
            check_inputs(self.repository, self.verus_root)

    def test_integer_zero_is_not_accepted_as_false(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["discharges_obligation"] = 0
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "discharges_obligation"):
            check_inputs(self.repository, self.verus_root)

    def test_malformed_hash_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["tool_sha256"]["z3"] = ABC_SHA256.upper()
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "lowercase SHA-256"):
            check_inputs(self.repository, self.verus_root)

    def test_deeply_nested_json_is_reported_as_invalid_manifest(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest_path.write_text("[" * 2_000 + "0" + "]" * 2_000, encoding="utf-8")

        with self.assertRaises(InputFailure):
            check_inputs(self.repository, self.verus_root)

    def test_nan_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        text = manifest_path.read_text(encoding="utf-8")
        manifest_path.write_text(
            text.replace('"discharges_obligation": false', '"discharges_obligation": NaN'),
            encoding="utf-8",
        )

        with self.assertRaisesRegex(InputFailure, "non-finite JSON"):
            check_inputs(self.repository, self.verus_root)

    def test_oversized_manifest_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest_path.write_bytes(b" " * (64 * 1024 + 1))

        with self.assertRaisesRegex(InputFailure, "64 KiB"):
            check_inputs(self.repository, self.verus_root)

    def test_symlink_source_parent_directory_is_rejected(self):
        source_parent = self.repository / "crates/liminal-safety/src"
        (source_parent / "lib.rs").unlink()
        source_parent.rmdir()
        target = self.root / "outside-source"
        target.mkdir()
        (target / "lib.rs").write_bytes(b"abc")
        source_parent.symlink_to(target, target_is_directory=True)

        with self.assertRaisesRegex(InputFailure, "liminal-safety/src/lib.rs"):
            check_inputs(self.repository, self.verus_root)

    def test_missing_tool_is_rejected(self):
        (self.verus_root / "z3").unlink()

        with self.assertRaisesRegex(InputFailure, "z3"):
            check_inputs(self.repository, self.verus_root)

    def test_nonregular_tool_is_rejected(self):
        tool = self.verus_root / "z3"
        tool.unlink()
        tool.mkdir()

        with self.assertRaisesRegex(InputFailure, "z3"):
            check_inputs(self.repository, self.verus_root)

    def test_unknown_top_level_field_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["unknown"] = "rejected"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "top-level fields"):
            check_inputs(self.repository, self.verus_root)

    def test_missing_tool_key_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        del manifest["tool_sha256"]["z3"]
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "tool_sha256"):
            check_inputs(self.repository, self.verus_root)

    def test_extra_tool_key_is_rejected(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["tool_sha256"]["unapproved-tool"] = ABC_SHA256
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "tool_sha256"):
            check_inputs(self.repository, self.verus_root)

    def test_missing_manifest_is_rejected(self):
        (self.repository / "verification/proof/inputs.json").unlink()

        with self.assertRaisesRegex(InputFailure, "inputs.json"):
            check_inputs(self.repository, self.verus_root)

    def test_oversized_json_integer_is_reported_as_invalid_manifest(self):
        manifest_path = self.repository / "verification/proof/inputs.json"
        manifest_path.write_text('{"value":' + "9" * 10_000 + "}", encoding="utf-8")

        with self.assertRaisesRegex(InputFailure, "invalid inputs.json"):
            check_inputs(self.repository, self.verus_root)

    def test_added_build_script_outside_selected_pins_is_rejected(self):
        (self.repository / "crates/liminal-safety/build.rs").write_bytes(b"abc")

        with self.assertRaisesRegex(InputFailure, "source inventory"):
            check_inputs(self.repository, self.verus_root)

    def test_repository_cargo_configuration_is_rejected(self):
        (self.repository / ".cargo").mkdir()
        with self.assertRaisesRegex(InputFailure, "repository .cargo"):
            check_inputs(self.repository, self.verus_root)

    def test_missing_additional_inventoried_file_is_rejected(self):
        relative = "crates/liminal-safety/src/additional.rs"
        path = self.repository / relative
        path.write_bytes(b"abc")
        self.add_inventory_file(relative)
        path.unlink()
        with self.assertRaisesRegex(InputFailure, "source inventory"):
            check_inputs(self.repository, self.verus_root)

    def test_changed_additional_inventoried_file_is_rejected(self):
        relative = "crates/liminal-safety/src/additional.rs"
        path = self.repository / relative
        path.write_bytes(b"changed")
        self.add_inventory_file(relative)
        with self.assertRaisesRegex(InputFailure, "additional.rs"):
            check_inputs(self.repository, self.verus_root)

    def test_extra_empty_directory_is_rejected(self):
        (self.repository / "crates/liminal-safety/empty").mkdir()
        with self.assertRaisesRegex(InputFailure, "directory set mismatch"):
            check_inputs(self.repository, self.verus_root)

    def test_source_executable_bit_drift_is_rejected(self):
        source = self.repository / "crates/liminal-safety/src/lib.rs"
        source.chmod(0o755)
        with self.assertRaisesRegex(InputFailure, "source mode mismatch"):
            check_inputs(self.repository, self.verus_root)

    def test_malformed_source_inventory_is_rejected(self):
        path = self.repository / "verification/proof/source-inventory.json"
        path.write_text("{", encoding="utf-8")
        with self.assertRaisesRegex(InputFailure, "invalid source inventory"):
            check_inputs(self.repository, self.verus_root)

    def test_extra_source_inventory_map_key_is_rejected(self):
        self.add_inventory_file("crates/liminal-safety/src/not-present.rs")
        with self.assertRaisesRegex(InputFailure, "source inventory"):
            check_inputs(self.repository, self.verus_root)

    def test_missing_source_inventory_map_key_is_rejected(self):
        path, inventory = self.read_source_inventory()
        del inventory["files"]["Cargo.toml"]
        path.write_text(json.dumps(inventory), encoding="utf-8")
        with self.assertRaisesRegex(InputFailure, "selected pin"):
            check_inputs(self.repository, self.verus_root)

    def test_oversized_source_inventory_is_rejected_before_parsing(self):
        path = self.repository / "verification/proof/source-inventory.json"
        path.write_bytes(b" " * (2 * 1024 * 1024 + 1))
        with self.assertRaisesRegex(InputFailure, "source inventory exceeds 2 MiB"):
            check_inputs(self.repository, self.verus_root)

    @unittest.skipIf(os.geteuid() == 0, "root bypasses directory permission checks")
    def test_source_scan_permission_error_is_rejected(self):
        relative = "crates/liminal-safety/private/additional.rs"
        source = self.repository / relative
        source.parent.mkdir()
        source.write_bytes(b"abc")
        self.add_inventory_file(relative)
        source.parent.chmod(0)
        try:
            with self.assertRaisesRegex(InputFailure, "source inventory scan failed"):
                check_inputs(self.repository, self.verus_root)
        finally:
            source.parent.chmod(0o755)

    @unittest.skipIf(os.geteuid() == 0, "root bypasses directory permission checks")
    def test_unreadable_unknown_directory_is_rejected_without_descent(self):
        directory = self.repository / "crates/heldout"
        directory.mkdir()
        directory.chmod(0)
        try:
            with self.assertRaisesRegex(InputFailure, "unexpected directory"):
                check_inputs(self.repository, self.verus_root)
        finally:
            directory.chmod(0o755)

    def test_unlisted_exact_conformance_manifest_is_rejected(self):
        path = self.repository / "conformance/Cargo.toml"
        path.parent.mkdir()
        path.write_bytes(b"abc")
        with self.assertRaisesRegex(InputFailure, "source inventory"):
            check_inputs(self.repository, self.verus_root)

    @unittest.skipIf(os.geteuid() == 0, "root bypasses directory permission checks")
    def test_repository_cargo_lstat_permission_error_is_rejected(self):
        self.repository.chmod(0)
        try:
            with self.assertRaisesRegex(InputFailure, "repository .cargo lstat failed"):
                check_inputs(self.repository, self.verus_root)
        finally:
            self.repository.chmod(0o755)

    def test_source_inventory_path_over_4096_bytes_is_rejected_early(self):
        self.add_inventory_file("crates/" + "a" * 4_090)
        with self.assertRaisesRegex(InputFailure, "source inventory path limit"):
            check_inputs(self.repository, self.verus_root)

    def test_source_inventory_path_over_128_components_is_rejected_early(self):
        self.add_inventory_file("crates/" + "/".join("a" for _ in range(128)))
        with self.assertRaisesRegex(InputFailure, "source inventory path limit"):
            check_inputs(self.repository, self.verus_root)

    def test_artifact_components_are_rejected_from_source_inventory(self):
        path, baseline = self.read_source_inventory()
        for component in ("target", "__pycache__", "node_modules", "liminal-5.3-spark"):
            with self.subTest(component=component):
                inventory = json.loads(json.dumps(baseline))
                inventory["files"][f"crates/{component}/file.rs"] = {
                    "sha256": ABC_SHA256,
                    "mode": "100644",
                }
                path.write_text(json.dumps(inventory), encoding="utf-8")
                with self.assertRaisesRegex(
                    InputFailure, "forbidden source inventory component"
                ):
                    check_inputs(self.repository, self.verus_root)


if __name__ == "__main__":
    unittest.main()
