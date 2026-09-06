#!/usr/bin/env python3
"""Planted-defect tests for lossless source coverage and fail-closed adoption."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import sas_migration as migration


class StructuralTests(unittest.TestCase):
    def test_fenced_headings_do_not_split_and_all_bytes_survive(self):
        source = b"preamble\r\n\r\n# One\r\n```rust\r\n# hidden\r\n```\r\n\r\n## Two\r\nnon-ASCII: \xc3\xa9"
        blocks = migration.structural_blocks(source)
        self.assertEqual([row["heading"] for row in blocks], ["(preamble)", "# One", "## Two"])
        self.assertEqual("".join(row["text"] for row in blocks).encode(), source)
        self.assertFalse(blocks[-1]["ends_newline"])

    def test_long_fences_and_tilde_fences_ignore_embedded_short_fences(self):
        source = b"# One\n````md\n```\n# hidden\n````\n~~~\n# hidden too\n~~~\n# Two\n"
        self.assertEqual([row["heading"] for row in migration.structural_blocks(source)], ["# One", "# Two"])

    def test_insertion_allocates_append_only_without_reindexing(self):
        assignments = {"later original block": "LIM-SAS-RQ-100", "last original block": "LIM-SAS-RQ-102"}
        self.assertEqual(migration.allocate_requirement(assignments, "inserted earlier block"), "LIM-SAS-RQ-103")
        self.assertEqual(migration.allocate_requirement(assignments, "later original block"), "LIM-SAS-RQ-100")
        self.assertEqual(assignments["last original block"], "LIM-SAS-RQ-102")


class CommittedHistoryTests(unittest.TestCase):
    """Real disposable Git objects prove post-commit and merge-topology behavior."""

    def setUp(self):
        self.context = tempfile.TemporaryDirectory(prefix="liminal-sas-history-")
        self.root = Path(self.context.name)
        migration.git(self.root, "init", "--quiet")

    def tearDown(self):
        self.context.cleanup()

    def commit(self, assignments, parents=(), *, policy=False):
        path = migration.POLICY if policy else migration.MAP
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if policy:
            target.write_text("\n".join(f"| {identifier} | {statement} |" for identifier, statement in assignments.items()) + "\n")
        else:
            target.write_bytes(migration.json_bytes({"requirement_assignments": assignments}))
        migration.git(self.root, "add", "--", path)
        tree = migration.git(self.root, "write-tree").decode().strip()
        args = ["-c", "user.name=SAS test fixture", "-c", "user.email=sas-test@example.invalid", "commit-tree", tree]
        for parent in parents:
            args.extend(["-p", parent])
        commit = migration.git(self.root, *args, "-m", "Disposable requirement-history fixture").decode().strip()
        migration.git(self.root, "update-ref", "HEAD", commit)
        return commit

    def test_committed_renumber_cannot_become_its_own_authority(self):
        first = self.commit({"first clause": "LIM-SAS-RQ-100"})
        changed = {"first clause": "LIM-SAS-RQ-101"}
        self.commit(changed, [first])
        with self.assertRaisesRegex(migration.MigrationError, "historical requirement ID.*changed or removed"):
            migration.validate_assignment_history(self.root, changed)

    def test_later_allocation_is_stable_too(self):
        first = self.commit({"first clause": "LIM-SAS-RQ-100"})
        second = self.commit({"first clause": "LIM-SAS-RQ-100", "later clause": "LIM-SAS-RQ-101"}, [first])
        changed = {"first clause": "LIM-SAS-RQ-100", "later clause": "LIM-SAS-RQ-102"}
        self.commit(changed, [second])
        with self.assertRaisesRegex(migration.MigrationError, "historical requirement ID.*changed or removed"):
            migration.validate_assignment_history(self.root, changed)

    def test_committed_deletion_then_restoration_is_detected(self):
        original = {"first clause": "LIM-SAS-RQ-100", "later clause": "LIM-SAS-RQ-101"}
        first = self.commit(original)
        second = self.commit({"first clause": "LIM-SAS-RQ-100"}, [first])
        self.commit(original, [second])
        with self.assertRaisesRegex(migration.MigrationError, "historical requirement ID.*changed or removed"):
            migration.validate_assignment_history(self.root, original)

    def test_deleted_requirement_id_cannot_be_reused(self):
        first = self.commit({"first clause": "LIM-SAS-RQ-100", "later clause": "LIM-SAS-RQ-101"})
        reused = {"replacement clause": "LIM-SAS-RQ-100", "later clause": "LIM-SAS-RQ-101"}
        self.commit(reused, [first])
        with self.assertRaisesRegex(migration.MigrationError, "historical requirement ID.*changed or removed"):
            migration.validate_assignment_history(self.root, reused)

    def test_distinct_sibling_allocations_survive_a_real_merge(self):
        initial = {"first clause": "LIM-SAS-RQ-100"}
        first = self.commit(initial)
        left = self.commit({**initial, "left clause": "LIM-SAS-RQ-101"}, [first])
        right = self.commit({**initial, "right clause": "LIM-SAS-RQ-102"}, [first])
        merged = {**initial, "left clause": "LIM-SAS-RQ-101", "right clause": "LIM-SAS-RQ-102"}
        self.commit(merged, [left, right])
        migration.validate_assignment_history(self.root, merged)

    def test_policy_id_cannot_be_remapped_after_commit(self):
        first = self.commit({"LIM-SAS-RQ-001": "Exact original obligation"}, policy=True)
        changed = {"LIM-SAS-RQ-001": "An unrelated substituted obligation"}
        self.commit(changed, [first], policy=True)
        with self.assertRaisesRegex(migration.MigrationError, "historical requirement ID or policy obligation changed"):
            migration.validate_policy_history(self.root, (self.root / migration.POLICY).read_text())

    def test_shallow_history_is_unavailable(self):
        with patch.object(migration, "git", return_value=b"true\n"):
            with self.assertRaisesRegex(migration.MigrationError, "history unavailable in shallow"):
                migration.validate_assignment_history(self.root, {"first clause": "LIM-SAS-RQ-100"})


class MigrationDefectTests(unittest.TestCase):
    """Use real baseline Git objects and isolated copies for all planted defects."""

    @classmethod
    def setUpClass(cls):
        cls.repository = Path(__file__).resolve().parents[1]
        cls.git = staticmethod(migration.git)
        cls.inventory = migration.load_map(cls.repository)
        cls.template_context = tempfile.TemporaryDirectory(prefix="liminal-sas-fixture-")
        cls.template = Path(cls.template_context.name)
        paths = [source["path"] for source in cls.inventory["sources"]]
        paths += [migration.MAP, migration.POLICY, migration.PACKET, migration.WARRANT_INDEX]
        paths += [str(path.relative_to(cls.repository)) for path in (cls.repository / "docs/warrants").rglob("*") if path.is_file()]
        for path in paths:
            target = cls.template / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(cls.repository / path, target)
        with patch.object(migration, "git", side_effect=lambda _root, *args: cls.git(cls.repository, *args)):
            sas, crosswalk = migration.build(cls.template, cls.inventory)
        (cls.template / migration.SAS).parent.mkdir(parents=True, exist_ok=True)
        (cls.template / migration.SAS).write_bytes(sas)
        (cls.template / migration.CROSSWALK).write_bytes(crosswalk)

    @classmethod
    def tearDownClass(cls):
        cls.template_context.cleanup()

    def setUp(self):
        self.context = tempfile.TemporaryDirectory(prefix="liminal-sas-defect-")
        self.root = Path(self.context.name)
        shutil.copytree(self.template, self.root, dirs_exist_ok=True)
        self.git_patch = patch.object(migration, "git", side_effect=lambda _root, *args: self.git(self.repository, *args))
        self.git_patch.start()

    def tearDown(self):
        self.git_patch.stop()
        self.context.cleanup()

    def save_map(self, change):
        inventory = copy.deepcopy(self.inventory)
        change(inventory)
        (self.root / migration.MAP).write_bytes(migration.json_bytes(inventory))

    def save_index(self, change):
        path = self.root / migration.WARRANT_INDEX
        index = json.loads(path.read_bytes())
        change(index)
        path.write_bytes(migration.json_bytes(index))

    def test_baseline_round_trip_and_declared_evidence_gaps(self):
        coverage = migration.check(self.root)
        self.assertEqual(coverage["source_count"], 65)
        self.assertEqual(coverage["haqp_requirement_count"], 55)
        crosswalk = json.loads((self.root / migration.CROSSWALK).read_bytes())
        self.assertTrue(any(gap["test_id"] == "P1-T12" for gap in crosswalk["evidence_gaps"]))
        self.assertTrue(all(not evidence["qualification_claim"] for block in crosswalk["blocks"] for evidence in block["evidence_links"]))
        self.assertTrue((self.root / migration.SAS).read_bytes().startswith((self.root / migration.POLICY).read_bytes()))

    def test_omitted_source_is_rejected(self):
        self.save_map(lambda inventory: inventory["sources"].pop())
        with self.assertRaisesRegex(migration.MigrationError, "source inventory omitted"):
            migration.check(self.root)

    def test_omitted_block_is_rejected(self):
        self.save_map(lambda inventory: inventory["sources"][0]["blocks"].pop())
        with self.assertRaisesRegex(migration.MigrationError, "source block omission"):
            migration.check(self.root)

    def test_duplicate_ids_are_rejected_even_when_both_mapping_copies_change(self):
        def duplicate(inventory):
            rows = [row for source in inventory["sources"] for row in source["blocks"] if row["requirement_ids"]]
            rows[-1]["requirement_ids"] = rows[0]["requirement_ids"].copy()
            inventory["requirement_assignments"][rows[-1]["key"]] = rows[0]["requirement_ids"][0]
        self.save_map(duplicate)
        with self.assertRaisesRegex(migration.MigrationError, "duplicate requirement IDs"):
            migration.check(self.root)

    def test_protected_source_edit_is_rejected(self):
        source = self.root / self.inventory["sources"][0]["path"]
        source.write_bytes(source.read_bytes() + b"\nSilent historical edit.\n")
        with self.assertRaisesRegex(migration.MigrationError, "protected historical source changed"):
            migration.check(self.root)

    def test_source_digest_drift_is_rejected(self):
        self.save_map(lambda inventory: inventory["sources"][0].update(source_sha256="0" * 64))
        with self.assertRaisesRegex(migration.MigrationError, "source digest drift"):
            migration.check(self.root)

    def test_missing_sas_is_unavailable(self):
        (self.root / migration.SAS).unlink()
        with self.assertRaisesRegex(migration.MigrationError, "missing SAS"):
            migration.check(self.root)

    def test_second_sas_candidate_is_unavailable(self):
        (self.root / "docs/sas/other.md").write_text("# Another purported SAS\n")
        with self.assertRaisesRegex(migration.MigrationError, "multiple candidate SAS"):
            migration.check(self.root)

    def test_sas_digest_drift_is_rejected(self):
        sas = self.root / migration.SAS
        sas.write_bytes(sas.read_bytes() + b"\nUnreviewed change.\n")
        with self.assertRaisesRegex(migration.MigrationError, "SAS digest drift"):
            migration.check(self.root)

    def test_invalid_evidence_reference_is_rejected(self):
        path = self.root / migration.CROSSWALK
        crosswalk = json.loads(path.read_bytes())
        block = next(row for row in crosswalk["blocks"] if row["evidence_links"])
        block["evidence_links"][0]["reference"] = "conformance/haqp/packet.json#/tests/99999/evidence/0"
        path.write_bytes(migration.json_bytes(crosswalk))
        with self.assertRaisesRegex(migration.MigrationError, "crosswalk drift or invalid references"):
            migration.check(self.root)

    def test_invalid_work_order_reference_is_rejected(self):
        self.save_map(lambda inventory: inventory["work_order_warrants"].update(M17=["docs/warrants/LIM-WAR-9999/manifest.toml"]))
        with self.assertRaisesRegex(migration.MigrationError, "Warrant reference missing"):
            migration.check(self.root)

    def test_wrong_program_work_order_reference_is_rejected(self):
        self.save_map(lambda inventory: inventory["work_order_warrants"].update(M17=["docs/warrants/OW-WAR-0001/manifest.toml"]))
        with self.assertRaisesRegex(migration.MigrationError, "wrong-program Warrant reference"):
            migration.check(self.root)

    def test_warrant_manifest_identity_must_match_reference(self):
        path = self.root / "docs/warrants/LIM-WAR-0001/manifest.toml"
        path.write_text(path.read_text().replace('local_alias = "LIM-WAR-0001"', 'local_alias = "OW-WAR-0001"'))
        with self.assertRaisesRegex(migration.MigrationError, "reference/manifest identity mismatch"):
            migration.check(self.root)

    def test_warrant_index_identity_drift_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0].update(uuid="00000000-0000-0000-0000-000000000000"))
        with self.assertRaisesRegex(migration.MigrationError, "Warrant index identity drift"):
            migration.check(self.root)

    def test_warrant_index_cycle_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0].update(dependencies=["LIM-WAR-0012"]))
        with self.assertRaisesRegex(migration.MigrationError, "Warrant dependency cycle"):
            migration.check(self.root)

    def test_warrant_index_unknown_dependency_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0].update(dependencies=["LIM-WAR-9999"]))
        with self.assertRaisesRegex(migration.MigrationError, "unknown Warrant dependency"):
            migration.check(self.root)

    def test_warrant_index_false_completion_field_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0].update(completion="complete", authority="accepted"))
        with self.assertRaisesRegex(migration.MigrationError, "index cannot assert authority or completion"):
            migration.check(self.root)

    def test_warrant_index_phase_drift_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0].update(phase=12))
        with self.assertRaisesRegex(migration.MigrationError, "Warrant index phase/reference drift"):
            migration.check(self.root)

    def test_warrant_index_evidence_heading_drift_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0]["evidence_requirements"][0].update(obligation="OBL-999"))
        with self.assertRaisesRegex(migration.MigrationError, "evidence obligation heading/index drift"):
            migration.check(self.root)

    def test_warrant_index_tier_drift_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0]["stages"][0].update(responsibility_tier="T4"))
        with self.assertRaisesRegex(migration.MigrationError, "Warrant stage/index drift"):
            migration.check(self.root)

    def test_warrant_index_executor_drift_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0]["stages"][0].update(executor_kind="human"))
        with self.assertRaisesRegex(migration.MigrationError, "Warrant stage/index drift"):
            migration.check(self.root)

    def test_warrant_index_invalid_executor_kind_is_rejected(self):
        self.save_index(lambda index: index["warrants"][0]["stages"][0].update(executor_kind="autonomous_authority"))
        with self.assertRaisesRegex(migration.MigrationError, "invalid indexed stage executor kind"):
            migration.check(self.root)

    def test_warrant_yaml_invalid_executor_kind_is_rejected(self):
        path = self.root / "docs/warrants/LIM-WAR-0001/atoms/45-milestones.yaml"
        path.write_text(path.read_text().replace('executor_kind: "agent"', 'executor_kind: "autonomous_authority"', 1))
        with self.assertRaisesRegex(migration.MigrationError, "stage executor kind missing/invalid/ambiguous"):
            migration.check(self.root)

    def test_false_acceptance_is_rejected_at_generation(self):
        path = self.root / migration.POLICY
        path.write_text(path.read_text().replace("State: **proposed; acceptance pending**.", "State: **accepted; all requirements complete**."))
        with self.assertRaisesRegex(migration.MigrationError, "policy falsely asserts acceptance"):
            migration.build(self.root, self.inventory)

    def test_false_crosswalk_completion_is_rejected(self):
        self.save_map(lambda inventory: inventory.update(authority="complete"))
        with self.assertRaisesRegex(migration.MigrationError, "falsely asserts SAS acceptance or completion"):
            migration.check(self.root)

    def test_duplicate_phase_is_rejected(self):
        path = self.root / migration.POLICY
        path.write_text(path.read_text() + "\n### Phase 12 — Duplicate authority\n")
        with self.assertRaisesRegex(migration.MigrationError, "policy phases must declare each"):
            migration.build(self.root, self.inventory)

    def test_missing_phase_is_rejected(self):
        path = self.root / migration.POLICY
        path.write_text(path.read_text().replace("### Phase -1", "### Historical phase -1"))
        with self.assertRaisesRegex(migration.MigrationError, "policy phases must declare each"):
            migration.build(self.root, self.inventory)

    def test_noncanonical_requirement_alias_is_rejected(self):
        def alias(inventory):
            row = next(row for source in inventory["sources"] for row in source["blocks"] if row["requirement_ids"])
            row["requirement_ids"] = ["LIM-SAS-RQ-00100"]
            inventory["requirement_assignments"][row["key"]] = row["requirement_ids"][0]
        self.save_map(alias)
        with self.assertRaisesRegex(migration.MigrationError, "noncanonical requirement ID"):
            migration.check(self.root)

    def test_rendered_omission_is_rejected_even_if_attacker_updates_sas_digest(self):
        path = self.root / migration.SAS
        text = path.read_text()
        row = self.inventory["sources"][0]["blocks"][0]
        marker = f"<!-- source-block:{row['anchor']} -->"
        text = text.replace(marker, "<!-- omitted block -->", 1)
        path.write_text(text)
        crosswalk_path = self.root / migration.CROSSWALK
        crosswalk = json.loads(crosswalk_path.read_bytes())
        crosswalk["sas_sha256"] = migration.sha(path.read_bytes())
        crosswalk_path.write_bytes(migration.json_bytes(crosswalk))
        with self.assertRaisesRegex(migration.MigrationError, "SAS source block omission"):
            migration.check(self.root)

    def test_historical_packet_snapshot_allows_later_result_metadata(self):
        path = self.root / migration.PACKET
        packet = json.loads(path.read_bytes())
        packet["qualification_state"] = "complete"
        packet["provenance"] = {"note": "planted later result; HAQP gates own validity"}
        path.write_bytes(migration.json_bytes(packet))
        self.assertEqual(migration.check(self.root)["haqp_requirement_count"], 55)


if __name__ == "__main__":
    unittest.main()
