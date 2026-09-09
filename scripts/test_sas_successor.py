"""Public-CLI refusal tests for successor generation and verification."""
from __future__ import annotations
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
FILES = ("docs/migration/successor/README.md", "docs/migration/successor/proposal.json", "scripts/sas_successor.py")


class SuccessorCliTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "repo"
        subprocess.run(["git", "clone", "--shared", "--quiet", "--no-checkout", str(ROOT), str(self.root)], check=True)
        subprocess.run(["git", "checkout", "--quiet", "9e76fc99027c55ded5bd0cc61da43b0f2b68b049"], cwd=self.root, check=True)
        for relative in FILES:
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
        result = self.run_cli("generate")
        self.assertEqual(result.returncode, 0, result.stderr)

    def tearDown(self):
        self.temp.cleanup()

    def run_cli(self, mode="check"):
        return subprocess.run([sys.executable, str(self.root / "scripts/sas_successor.py"), mode, "--root", str(self.root)], text=True, capture_output=True)

    def proposal(self):
        return json.loads((self.root / "docs/migration/successor/proposal.json").read_text())

    def write_proposal(self, value):
        (self.root / "docs/migration/successor/proposal.json").write_text(json.dumps(value) + "\n")

    def assert_refused(self, text, mode="check"):
        result = self.run_cli(mode)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(text, result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_checked_in_successor_candidate_passes_public_check(self):
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("successor: PASS", result.stdout)

    def test_edit_to_accepted_sas_is_refused(self):
        path = self.root / "docs/sas/LIMINAL_Software_Architecture_Specification.md"
        path.write_bytes(path.read_bytes() + b"edited\n")
        self.assert_refused("accepted protected bytes changed")

    def test_edit_to_selected_sas_configuration_is_refused(self):
        path = self.root / "openwarrant.toml"
        path.write_text(path.read_text().replace(
            'sas = "docs/sas"',
            'sas = "docs/migration/successor"',
        ))
        self.assert_refused("accepted protected bytes changed")

    def test_forged_accepted_parent_or_digest_is_refused(self):
        value = self.proposal()
        value["accepted_commit"] = value["target_commit"]
        self.write_proposal(value)
        self.assert_refused("accepted commit pin changed")
        value = self.proposal()
        value["accepted_commit"] = "9e76fc99027c55ded5bd0cc61da43b0f2b68b049"
        value["accepted_sas_sha256"] = "0" * 64
        self.write_proposal(value)
        self.assert_refused("accepted SAS digest pin changed")

    def test_swapped_accepted_requirement_id_is_refused(self):
        path = self.root / "docs/migration/source-map.json"
        value = json.loads(path.read_text())
        key = next(iter(value["requirement_assignments"]))
        value["requirement_assignments"][key] = "LIM-SAS-RQ-999"
        path.write_text(json.dumps(value) + "\n")
        self.assert_refused("accepted protected bytes changed")

    def test_changed_target_commit_is_refused(self):
        value = self.proposal()
        value["target_commit"] = value["accepted_commit"]
        self.write_proposal(value)
        self.assert_refused("target commit/tree pin changed")

    def test_omitted_modified_or_added_source_is_refused(self):
        value = self.proposal()
        value["expected_modified_sources"].pop()
        self.write_proposal(value)
        self.assert_refused("exactly 10")
        value = self.proposal()
        value["expected_modified_sources"] = json.loads(
            (ROOT / "docs/migration/successor/proposal.json").read_text()
        )["expected_modified_sources"]
        value["expected_added_candidates"].pop()
        self.write_proposal(value)
        self.assert_refused("exactly 2")

    def test_forged_hunk_coordinate_is_generated_drift(self):
        path = self.root / "docs/migration/successor/reconciliation.json"
        value = json.loads(path.read_text())
        value["records"][0]["hunks"][0]["old_start"] += 1
        path.write_text(json.dumps(value) + "\n")
        self.assert_refused("generated reconciliation drift")

    def test_malformed_or_incomplete_json_is_controlled_refusal(self):
        path = self.root / "docs/migration/successor/proposal.json"
        path.write_text("{bad json")
        self.assert_refused("invalid JSON")
        path.write_text('{}\n')
        self.assert_refused("proposal fields")

    def test_duplicate_json_key_is_refused(self):
        path = self.root / "docs/migration/successor/proposal.json"
        text = path.read_text().replace(
            '"status": "candidate-unregistered-unselected",',
            '"status": "accepted",\n  "status": "candidate-unregistered-unselected",',
        )
        path.write_text(text)
        self.assert_refused("duplicate JSON key")

    def test_path_escape_is_refused(self):
        value = self.proposal()
        value["expected_added_candidates"][0] = "../outside.md"
        self.write_proposal(value)
        self.assert_refused("escapes repository")

    def test_candidate_accepted_status_or_extra_authority_claim_is_refused(self):
        value = self.proposal()
        value["status"] = "accepted"
        self.write_proposal(value)
        self.assert_refused("falsely claims")
        value = self.proposal()
        value["status"] = "candidate-unregistered-unselected"
        value["accepted"] = True
        self.write_proposal(value)
        self.assert_refused("proposal fields")

    def test_generated_candidate_drift_is_refused(self):
        path = self.root / "docs/migration/successor/candidate.md"
        path.write_bytes(path.read_bytes() + b"drift\n")
        self.assert_refused("generated candidate drift")

    def test_omitted_or_swapped_candidate_requirement_row_is_refused(self):
        path = self.root / "docs/migration/successor/candidate.md"
        text = path.read_text()
        requirement = next(
            line for line in text.splitlines(keepends=True)
            if "| LIM-SAS-RQ-" in line and not line.startswith(">")
        )
        path.write_text(text.replace(requirement, "", 1))
        self.assert_refused("generated candidate drift")

        self.run_cli("generate")
        text = path.read_text()
        path.write_text(text.replace("LIM-SAS-RQ-001", "LIM-SAS-RQ-999", 1))
        self.assert_refused("generated candidate drift")

    def test_generation_refuses_symlink_without_touching_sentinel(self):
        candidate = self.root / "docs/migration/successor/candidate.md"
        candidate.unlink()
        sentinel = self.root / "sentinel"
        sentinel.write_text("unchanged")
        candidate.symlink_to(sentinel)
        self.assert_refused("symlink", "generate")
        self.assertEqual(sentinel.read_text(), "unchanged")

    def test_generation_refuses_symlink_ancestor(self):
        successor = self.root / "docs/migration/successor"
        shutil.rmtree(successor)
        sentinel = self.root / "sentinel-dir"
        sentinel.mkdir()
        successor.symlink_to(sentinel, target_is_directory=True)
        self.assert_refused("symlink", "generate")

    def test_generation_refuses_hardlinked_output_without_changing_target(self):
        candidate = self.root / "docs/migration/successor/candidate.md"
        candidate.unlink()
        sentinel = self.root / "hardlink-sentinel"
        sentinel.write_text("unchanged")
        candidate.hardlink_to(sentinel)
        self.assert_refused("hardlink", "generate")
        self.assertEqual(sentinel.read_text(), "unchanged")

    def test_missing_git_metadata_is_controlled_refusal(self):
        shutil.rmtree(self.root / ".git")
        self.assert_refused("git metadata unavailable")

    def test_target_sources_are_quote_prefixed_and_newline_preserved(self):
        candidate = (self.root / "docs/migration/successor/candidate.md").read_bytes()
        self.assertIn(b"> AM-17.9 (user-ratified 2026-09-07)", candidate)
        self.assertTrue(candidate.endswith(b"\n"))

if __name__ == "__main__":
    unittest.main()
