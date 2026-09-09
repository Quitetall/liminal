#!/usr/bin/env python3
"""Assemble and audit the proposed SAS without modifying its historical sources.

Source classifications are explicit review inputs, not a claim that a text
classifier can establish semantic equivalence. Every byte is incorporated, even
when a block is contextual or records an unresolved obligation.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib

BASELINE = "0d8c32a4ae1afda9808e55c3c291445aad4e3d60"
MAP = "docs/migration/source-map.json"
CROSSWALK = "docs/migration/source-crosswalk.json"
POLICY = "docs/migration/consolidation-policy.md"
SAS = "docs/sas/LIMINAL_Software_Architecture_Specification.md"
PACKET = "conformance/haqp/packet.json"
WARRANT_INDEX = "docs/migration/warrant-index.json"
CLASSES = {"normative-clause", "mixed-clause", "context", "historical-status", "example", "open-gap"}
REQUIRED_CLASSES = {"normative-clause", "mixed-clause", "open-gap"}
SUPPLEMENTAL_SOURCES = {
    "spec/README.md", "spec/rfc/README.md", "docs/adr/README.md",
    "docs/execution/template.md", "spec/v4/liminal_master_architecture_v3_to_v4.diff",
}
PROVENANCE_ONLY = SUPPLEMENTAL_SOURCES - {"spec/rfc/README.md", "docs/adr/README.md"}
NAVIGATION_INDEXES = {"spec/transforms.md", "spec/protocol.md"}
REQUIREMENT_ROW = re.compile(r"^\|\s*(LIM-SAS-RQ-\d{3,})\s*\|", re.MULTILINE)
HEADING = re.compile(r"^ {0,3}(#{1,6})[ \t]+(.*?)[ \t]*\r?\n?$")
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")


class MigrationError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MigrationError(message)


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE)


def historical(root: Path, path: str) -> bytes:
    return git(root, "show", f"{BASELINE}:{path}")


def expected_paths(root: Path) -> list[str]:
    adr = git(root, "ls-tree", "-r", "--name-only", BASELINE, "docs/adr").decode().splitlines()
    adr = [p for p in adr if re.match(r"docs/adr/00(?:0\d|1\d|2[01])-.*\.md$", p)]
    require(len(adr) == 22, "baseline must contain ADR-0000 through ADR-0021")
    return [
        "spec/v4/liminal_master_architecture_plan_v4.md",
        "spec/v4/liminal_architecture_revision_4.md",
        *[f"spec/{name}.md" for name in ("kernel", "ir", "syntax", "transforms", "protocol")],
        "spec/rfc/0001-add-liminal-jurisdiction-crate.md", *sorted(adr),
        "docs/execution/00-protocol.md",
        *[f"docs/execution/M{n:02d}.md" for n in range(1, 25)],
        "docs/execution/phases.md", "docs/execution/phase0-amendments.md",
        "docs/implementation-plan.md", "docs/security/threat-model.md",
        "docs/execution/phase1-suite-review.md", *sorted(SUPPLEMENTAL_SOURCES),
    ]


def structural_blocks(data: bytes) -> list[dict]:
    """Partition all bytes at ATX headings outside CommonMark fenced code."""
    text = data.decode("utf-8")
    lines = text.splitlines(keepends=True)
    starts: list[tuple[int, str, int, tuple[str, ...]]] = []
    fence: tuple[str, int] | None = None
    hierarchy: list[tuple[int, str]] = []
    for number, line in enumerate(lines):
        match = FENCE.match(line.rstrip("\r\n"))
        if fence:
            if match and match[1][0] == fence[0] and len(match[1]) >= fence[1] and not match[2].strip():
                fence = None
            continue
        if match and (match[1][0] != "`" or "`" not in match[2]):
            fence = (match[1][0], len(match[1]))
            continue
        heading = HEADING.match(line)
        if heading:
            level = len(heading[1])
            hierarchy = [(depth, name) for depth, name in hierarchy if depth < level]
            title = heading[2]
            hierarchy.append((level, title))
            starts.append((number, line.rstrip("\r\n"), level, tuple(name for _, name in hierarchy)))
    if not starts or starts[0][0] != 0:
        starts.insert(0, (0, "(preamble)", 0, ()))
    occurrences: Counter[str] = Counter()
    blocks = []
    for index, (start, heading, level, parents) in enumerate(starts):
        end = starts[index + 1][0] if index + 1 < len(starts) else len(lines)
        value = "".join(lines[start:end])
        occurrences[heading] += 1
        blocks.append({"heading": heading, "heading_level": level,
                       "heading_occurrence": occurrences[heading], "hierarchy": list(parents),
                       "start_line": start + 1, "end_line": end,
                       "text_sha256": sha(value.encode()), "text_bytes": len(value.encode()),
                       "ends_newline": value.endswith("\n"), "text": value})
    require("".join(b["text"] for b in blocks).encode() == data, "partition lost source bytes")
    return blocks


def initial_classification(path: str, block: dict) -> tuple[str, str]:
    """Seed explicit review records once; generate/check never reclassify."""
    title = re.sub(r"^#+\s*", "", block["heading"]).lower()
    body = block["text"].split("\n", 1)[1] if block["heading_level"] else block["text"]
    if path in PROVENANCE_ONLY:
        return "context", "Provenance-only index, template, or historical diff; no new obligation."
    if not body.strip() or body.strip() in {"---", "None.", "None", "—"}:
        return "context", "Structural heading or explicitly empty section."
    if block["heading_level"] == 1 and path not in SUPPLEMENTAL_SOURCES and (path.startswith("docs/adr/") or path.startswith("spec/rfc/")):
        return "historical-status", "Recorded source metadata and decision status; no retrospective acceptance."
    if title in {"status", "campaign conclusion"}:
        return "historical-status", "Historical status or prediction; completion must be derived from current evidence."
    if title in {"spec inputs", "references", "related decisions", "links", "index"}:
        return "context", "Source citations and navigation, retained without separate normative assignment."
    if title in {"context", "motivation", "rationale", "alternatives considered", "alternatives", "rejected alternatives"}:
        return "context", "Decision background or rejected alternatives; operative decision retained separately."
    if any("research questions" in p.lower() or "required adrs and open" in p.lower() for p in block["hierarchy"]):
        return "open-gap", "Source expressly defers a research or architectural decision; not resolved by transcription."
    if title == "discovered gaps":
        return "open-gap", "Source gap ledger retained; historical dispositions apply only as recorded."
    if any(re.match(r"(?:\d+\. )?examples$", p.lower()) for p in block["hierarchy"]):
        return "example", "Illustrative example under an explicit examples heading."
    if title == "decision" or title.startswith("law "):
        return "normative-clause", "Accepted decision or architectural law; applicability remains governed by source authority."
    return "mixed-clause", "Entire substantive section retained; normative clauses, explanation, and historical status keep their original roles."


def allocate_requirement(assignments: dict[str, str], key: str) -> str:
    if key not in assignments:
        next_number = max([99, *[int(value.rsplit("-", 1)[1]) for value in assignments.values()]]) + 1
        assignments[key] = f"LIM-SAS-RQ-{next_number:03d}"
    return assignments[key]


def source_role(path: str) -> str:
    return "provenance-only" if path in PROVENANCE_ONLY else "navigation-index" if path in NAVIGATION_INDEXES else "incorporated"


def initialize(root: Path) -> dict:
    require(not (root / MAP).exists(), "source-map already exists; refuse to reindex stable assignments")
    result = {"schema_version": 1, "baseline_commit": BASELINE,
              "authority": "proposed-unaccepted", "classification_review": "explicit classifications; semantic review required",
              "packet_sha256": sha(historical(root, PACKET)), "requirement_assignments": {},
              "work_order_warrants": {}, "sources": []}
    for path in expected_paths(root):
        data = historical(root, path)
        source = {"path": path, "source_sha256": sha(data), "source_bytes": len(data),
                  "source_lines": len(data.decode().splitlines()),
                  "role": source_role(path),
                  "blocks": []}
        for block in structural_blocks(data):
            key = f"{path}::{block['heading']}::{block['heading_occurrence']}"
            kind, reason = initial_classification(path, block)
            row = {key_: value for key_, value in block.items() if key_ != "text"}
            row.update({"key": key, "anchor": "source-" + sha(key.encode())[:20],
                        "classification": kind, "classification_reason": reason,
                        "requirement_ids": [allocate_requirement(result["requirement_assignments"], key)] if kind in REQUIRED_CLASSES else []})
            source["blocks"].append(row)
        result["sources"].append(source)
    return result


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def load_map(root: Path) -> dict:
    return json.loads((root / MAP).read_bytes())


def document_history(root: Path, path: str) -> list[tuple[str, bytes]]:
    """Read every changing ancestor, including the introduction, before HEAD.

    Comparing only with HEAD makes a committed reassignment its own authority.
    A shallow clone cannot prove that an apparent introduction is the first one.
    """
    require(git(root, "rev-parse", "--is-shallow-repository").strip() == b"false",
            "stable requirement history unavailable in shallow repository; fetch full history")
    commits = git(root, "log", "--full-history", "--reverse", "--topo-order",
                  "--format=%H", "HEAD", "--", path).decode().splitlines()
    records = []
    for commit in commits:
        try:
            records.append((commit, git(root, "show", f"{commit}:{path}")))
        except subprocess.CalledProcessError as error:
            raise MigrationError(f"historical requirement document removed at {commit}: {path}") from error
    return records


def validate_assignment_history(root: Path, assignments: dict[str, str]) -> None:
    validate_requirement_history(root, MAP, assignments,
                                 lambda data: json.loads(data)["requirement_assignments"], source_ids=True)


def policy_assignments(policy: str) -> dict[str, str]:
    rows = re.findall(r"^\|\s*(LIM-SAS-RQ-\d{3,})\s*\|\s*(.*?)\s*\|\s*$", policy, re.MULTILINE)
    require(len(rows) == len({identifier for identifier, _ in rows}), "duplicate policy requirement IDs")
    return {identifier: re.sub(r"\s+", " ", statement.strip()) for identifier, statement in rows}


def validate_policy_history(root: Path, policy: str) -> None:
    validate_requirement_history(root, POLICY, policy_assignments(policy),
                                 lambda data: policy_assignments(data.decode()), source_ids=False)


def validate_requirement_history(root: Path, path: str, current: dict, parse, *, source_ids: bool) -> None:
    records = document_history(root, path)
    snapshots = {commit: parse(data) for commit, data in records}
    historical_union: dict[str, str] = {}
    identifier_owners: dict[str, str] = {}

    def read(commit: str) -> dict:
        if commit not in snapshots:
            exists = git(root, "ls-tree", "-r", "--name-only", commit, "--", path).strip()
            snapshots[commit] = parse(git(root, "show", f"{commit}:{path}")) if exists else {}
        return snapshots[commit]

    def identity(key: str, value: str) -> str:
        return value if source_ids else key

    def transition(snapshot: dict, parents: dict, coordinate: str) -> None:
        for key, value in parents.items():
            require(snapshot.get(key) == value,
                    f"historical requirement ID or policy obligation changed or removed at {coordinate}: {key}")
        maximum = max([99 if source_ids else 0,
                       *[int(identity(key, value).rsplit("-", 1)[1]) for key, value in parents.items()]])
        for key, value in snapshot.items():
            identifier = identity(key, value)
            require(bool(re.fullmatch(r"LIM-SAS-RQ-\d{3,}", identifier))
                    and identifier == f"LIM-SAS-RQ-{int(identifier.rsplit('-', 1)[1]):03d}",
                    f"historical requirement ID is noncanonical at {coordinate}: {identifier}")
            if key not in parents:
                require(int(identifier.rsplit("-", 1)[1]) > maximum,
                        f"historical requirement allocation is not append-only at {coordinate}: {identifier}")
            require(key not in historical_union or historical_union[key] == value,
                    f"historical requirement ID or policy obligation remapped at {coordinate}: {key}")
            require(identifier not in identifier_owners or identifier_owners[identifier] == key,
                    f"historical requirement ID reused at {coordinate}: {identifier}")
            historical_union[key] = value
            identifier_owners[identifier] = key

    for commit, _ in records:
        parents = git(root, "rev-list", "--parents", "-n", "1", commit).decode().split()[1:]
        parent_union = {}
        for parent in parents:
            for key, value in read(parent).items():
                require(key not in parent_union or parent_union[key] == value,
                        f"conflicting historical requirement identities at merge {commit}: {key}")
                parent_union[key] = value
        # Parent edges, not an invented ordering between sibling branches,
        # determine which requirements this commit is required to retain.
        transition(snapshots[commit], parent_union, commit)
    transition(current, read("HEAD"), "working candidate")
    require(all(current.get(key) == value for key, value in historical_union.items()),
            "working candidate omits or reassigns an ancestral requirement")


def validate_inventory(root: Path, inventory: dict) -> dict[str, str]:
    require(inventory["schema_version"] == 1, "unsupported source-map schema")
    require(inventory["baseline_commit"] == BASELINE, "source baseline changed")
    require(inventory["authority"] == "proposed-unaccepted", "migration falsely asserts SAS acceptance or completion")
    require([s["path"] for s in inventory["sources"]] == expected_paths(root), "source inventory omitted, duplicated, or reordered sources")
    packet_bytes = historical(root, PACKET)
    require(sha(packet_bytes) == inventory["packet_sha256"], "HAQP packet digest drift")
    texts, requirement_ids, keys, anchors = {}, [], [], []
    assignments = inventory["requirement_assignments"]
    for source in inventory["sources"]:
        path = source["path"]
        data = historical(root, path)
        require((root / path).read_bytes() == data, f"protected historical source changed: {path}")
        require(sha(data) == source["source_sha256"], f"source digest drift: {path}")
        require(len(data) == source["source_bytes"] and len(data.decode().splitlines()) == source["source_lines"], f"source size drift: {path}")
        require(source["role"] == source_role(path), f"source role changed: {path}")
        original = structural_blocks(data)
        require(len(original) == len(source["blocks"]), f"source block omission or duplication: {path}")
        for actual, row in zip(original, source["blocks"]):
            for field in actual.keys() - {"text"}:
                require(actual[field] == row[field], f"block {field} drift: {path}:{actual['start_line']}")
            key = f"{path}::{actual['heading']}::{actual['heading_occurrence']}"
            require(row["key"] == key, f"block key drift: {key}")
            require(row["anchor"] == "source-" + sha(key.encode())[:20], f"block anchor drift: {key}")
            kind = row["classification"]
            require(kind in CLASSES and bool(row["classification_reason"].strip()), f"unclassified source block: {key}")
            require(path not in PROVENANCE_ONLY or kind not in REQUIRED_CLASSES, f"provenance-only source assigned requirement: {key}")
            if kind in REQUIRED_CLASSES:
                require(len(row["requirement_ids"]) == 1 and assignments.get(key) == row["requirement_ids"][0], f"missing or unstable requirement assignment: {key}")
            else:
                require(not row["requirement_ids"], f"context/status/example falsely assigned requirement: {key}")
            for requirement in row["requirement_ids"]:
                require(bool(re.fullmatch(r"LIM-SAS-RQ-\d{3,}", requirement)) and int(requirement.rsplit("-", 1)[1]) >= 100, f"invalid source requirement ID: {requirement}")
                require(requirement == f"LIM-SAS-RQ-{int(requirement.rsplit('-', 1)[1]):03d}", f"noncanonical requirement ID: {requirement}")
                requirement_ids.append(requirement)
            texts[key] = actual["text"]
            keys.append(key)
            anchors.append(row["anchor"])
    require(len(keys) == len(set(keys)) and len(anchors) == len(set(anchors)), "duplicate source blocks or anchors")
    require(len(requirement_ids) == len(set(requirement_ids)), "duplicate requirement IDs")
    require(set(assignments) == {row["key"] for source in inventory["sources"] for row in source["blocks"] if row["requirement_ids"]}, "orphan requirement assignment")
    require(set(assignments.values()) == set(requirement_ids), "requirement assignment/index mismatch")
    validate_assignment_history(root, assignments)
    return texts


def pointer(document: object, reference: str) -> object:
    current = document
    for part in reference.split("/")[1:]:
        part = part.replace("~1", "/").replace("~0", "~")
        current = current[int(part)] if isinstance(current, list) else current[part]
    return current


def validate_warrant_reference(root: Path, reference: str) -> None:
    match = re.fullmatch(r"docs/warrants/(LIM-WAR-(\d{4,}))/manifest\.toml", reference)
    require(match is not None, f"malformed or wrong-program Warrant reference: {reference}")
    require(match[1] == f"LIM-WAR-{int(match[2]):04d}", f"noncanonical Warrant reference: {reference}")
    require((root / reference).is_file(), f"Warrant reference missing: {reference}")
    manifest = tomllib.loads((root / reference).read_text())
    require(manifest.get("local_alias") == match[1], f"Warrant reference/manifest identity mismatch: {reference}")


def exact_fields(value: dict, fields: set[str], context: str) -> None:
    require(isinstance(value, dict) and set(value) == fields,
            f"{context} has missing/unknown fields; migration index cannot assert authority or completion")


def repo_reference(root: Path, reference: str, *, future_sas: bool = False) -> Path:
    path = Path(reference)
    require(not path.is_absolute() and ".." not in path.parts and str(path) == reference,
            f"noncanonical repository reference: {reference}")
    target = root / path
    require((future_sas and reference == SAS) or target.is_file(), f"repository reference missing: {reference}")
    return target


def validate_dependency_graph(dependencies: dict[str, list[str]]) -> None:
    for alias, prerequisites in dependencies.items():
        require(len(prerequisites) == len(set(prerequisites)), f"duplicate Warrant dependency: {alias}")
        require(set(prerequisites) <= dependencies.keys(), f"unknown Warrant dependency: {alias}")
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(alias: str) -> None:
        require(alias not in visiting, f"Warrant dependency cycle: {alias}")
        if alias in visited:
            return
        visiting.add(alias)
        for prerequisite in dependencies[alias]:
            visit(prerequisite)
        visiting.remove(alias)
        visited.add(alias)

    for alias in dependencies:
        visit(alias)


def validate_warrant_index(root: Path, inventory: dict, policy_ids: list[str]) -> None:
    """Validate a proposed contract index, never an authority/completion view."""
    index = json.loads((root / WARRANT_INDEX).read_bytes())
    exact_fields(index, {"schema", "description", "warrants"}, "Warrant index")
    require(index["schema"] == "liminal.migration.warrant-index/v1", "unsupported Warrant index schema")
    require(index["description"].startswith("Proposed bounded work;"), "Warrant index must explicitly remain proposed")
    rows = index["warrants"]
    fields = {"alias", "uuid", "milestone", "phase", "roadmap_ref", "sas_requirement_refs", "contribution",
              "manifest", "dependencies", "source_coordinates", "evidence_requirements", "pending_decision_roles", "stages"}
    for row in rows:
        exact_fields(row, fields, "Warrant index row")
    require(len({row["alias"] for row in rows}) == len(rows), "duplicate Warrant index alias")
    require(len({row["uuid"] for row in rows}) == len(rows), "duplicate Warrant index UUID")
    expected_manifests = {path for paths in inventory.get("policy_requirement_warrants", {}).values() for path in paths}
    require({row["manifest"] for row in rows} == expected_manifests and len(rows) == len(expected_manifests),
            "Warrant index manifest inventory drift")
    validate_dependency_graph({row["alias"]: row["dependencies"] for row in rows})
    all_ids = set(policy_ids) | set(inventory["requirement_assignments"].values())
    for row in rows:
        alias = row["alias"]
        validate_warrant_reference(root, row["manifest"])
        manifest_path = root / row["manifest"]
        manifest = tomllib.loads(manifest_path.read_text())
        require(manifest["local_alias"] == alias and manifest["uuid"] == row["uuid"], f"Warrant index identity drift: {alias}")
        roadmap = re.fullmatch(r"roadmap://LIM-PHASE-(-?\d+)(?:/[a-z0-9]+(?:-[a-z0-9]+)*)?", row["roadmap_ref"])
        require(roadmap is not None and roadmap[1] == str(row["phase"]) and type(row["phase"]) is int
                and row["phase"] in range(-1, 13), f"Warrant index phase/reference drift: {alias}")
        require([entry["ref"] for entry in manifest.get("roadmap", [])] == [row["roadmap_ref"]],
                f"Warrant index roadmap/manifest drift: {alias}")
        require(row["contribution"] == "partial", f"Warrant index falsely asserts complete contribution: {alias}")
        refs = row["sas_requirement_refs"]
        require(bool(refs) and len(refs) == len(set(refs)), f"missing/duplicate Warrant SAS references: {alias}")
        for reference in refs:
            require(reference.startswith("sas://") and reference[6:] in all_ids, f"unknown Warrant SAS reference: {reference}")
            require(row["manifest"] in inventory.get("policy_requirement_warrants", {}).get(reference[6:], []),
                    f"Warrant index/source-map requirement drift: {alias}")
        require([(entry["ref"], entry["contribution"]) for entry in manifest.get("implements", [])]
                == [(reference, row["contribution"]) for reference in refs], f"Warrant index implements/manifest drift: {alias}")
        if row["milestone"] in inventory["work_order_warrants"]:
            require(row["manifest"] in inventory["work_order_warrants"][row["milestone"]], f"Warrant index milestone drift: {alias}")
        else:
            require(row["milestone"] in {"INTEGRATION", "PROFILE", "PARITY", "RELEASE"}, f"unknown indexed milestone: {alias}")
        for reference in row["source_coordinates"]:
            repo_reference(root, reference, future_sas=True)
        roles = row["pending_decision_roles"]
        require(bool(roles), f"Warrant index omits pending decision roles: {alias}")
        for role in roles:
            exact_fields(role, {"role", "purpose"}, f"pending decision role for {alias}")
            require(bool(role["role"].strip()) and bool(role["purpose"].strip()), f"empty pending decision role: {alias}")
        assurance_atoms = [atom for atom in manifest["atoms"] if atom["role"] == "assurance"]
        require(len(assurance_atoms) == 1, f"missing/ambiguous assurance atom: {alias}")
        assurance_path = str((manifest_path.parent / assurance_atoms[0]["path"]).relative_to(root))
        assurance = repo_reference(root, assurance_path).read_text()
        obligations = re.findall(r"^### (OBL-\d{3}) — (.+)$", assurance, re.MULTILINE)
        require(bool(obligations) and len(obligations) == len({identifier for identifier, _ in obligations}), f"missing/duplicate assurance obligation headings: {alias}")
        evidence = row["evidence_requirements"]
        for entry in evidence:
            exact_fields(entry, {"obligation", "path", "statement"}, f"evidence requirement for {alias}")
            require(entry["path"] == assurance_path, f"evidence obligation path/manifest drift: {alias}")
        require([(entry["obligation"], entry["statement"]) for entry in evidence] == obligations,
                f"evidence obligation heading/index drift: {alias}")
        stage_atoms = [atom for atom in manifest["atoms"] if atom["role"] == "milestones"]
        require(len(stage_atoms) == 1, f"missing/ambiguous milestones atom: {alias}")
        stage_path = str((manifest_path.parent / stage_atoms[0]["path"]).relative_to(root))
        stage_text = repo_reference(root, stage_path).read_text()
        # Generated stage records have a narrow quoted scalar shape. OpenWarrant
        # owns full YAML/schema validation; this compares the indexed coordinates.
        stage_sections = stage_text.split("\nstages:\n")
        require(len(stage_sections) == 2, f"milestone stage section missing/ambiguous: {alias}")
        stage_records = []
        for entry in re.finditer(r'^  - id: "(STAGE-\d{3})"\n(.*?)(?=^  - id:|\Z)', stage_sections[1], re.MULTILINE | re.DOTALL):
            tiers = re.findall(r'^    responsibility_tier: "(T[1-4])"$', entry[2], re.MULTILINE)
            require(len(tiers) == 1, f"stage responsibility tier missing/ambiguous: {alias}")
            executors = re.findall(r'^    executor_kind: "(agent|human)"$', entry[2], re.MULTILINE)
            require(len(executors) == 1, f"stage executor kind missing/invalid/ambiguous: {alias}")
            stage_records.append((entry[1], tiers[0], executors[0]))
        for stage in row["stages"]:
            exact_fields(stage, {"id", "source_coordinate", "responsibility_tier", "executor_kind"}, f"indexed stage for {alias}")
            require(bool(stage["source_coordinate"].strip()), f"indexed stage source coordinate missing: {alias}")
            require(stage["executor_kind"] in ("agent", "human"), f"invalid indexed stage executor kind: {alias}")
        require(bool(stage_records) and len(stage_records) == len({entry[0] for entry in stage_records})
                and stage_records == [(entry["id"], entry["responsibility_tier"], entry["executor_kind"]) for entry in row["stages"]],
                f"Warrant stage/index drift: {alias}")


def haqp_mapping(root: Path, inventory: dict, texts: dict[str, str]) -> dict[str, dict]:
    packet = json.loads(historical(root, PACKET))
    mapping = {key: {"haqp_requirement_ids": [], "haqp_test_links": [], "evidence_links": []} for key in texts}
    requirements = packet["requirements"]
    tests = packet["tests"]
    require(len({r["id"] for r in requirements}) == len(requirements), "duplicate HAQP requirement IDs")
    require(len({t["id"] for t in tests}) == len(tests), "duplicate HAQP test IDs")
    known = {r["id"] for r in requirements}
    rust_files = git(root, "ls-tree", "-r", "--name-only", BASELINE, "conformance/tests").decode().splitlines()
    rust = {path: historical(root, path).decode() for path in rust_files if path.endswith(".rs")}
    test_locations = {}
    for test in tests:
        require(set(test["requirements"]) <= known, f"unknown HAQP test requirement: {test['id']}")
        name = test["name"].split("::")[-1]
        locations = [path for path, content in rust.items() if re.search(r"\bfn\s+" + re.escape(name) + r"\s*\(", content)]
        require(len(locations) <= 1, f"HAQP test function ambiguous: {test['name']}")
        # HAQP declares future Phase 1 tests. A declaration is a real reference,
        # but it is never evidence that a runnable test already exists.
        test_locations[test["id"]] = locations[0] if locations else None
    for requirement_index, requirement in enumerate(requirements):
        path, anchor = requirement["source"].split(":", 1)
        source = next((s for s in inventory["sources"] if s["path"] == path), None)
        require(source is not None, f"HAQP source not inventoried: {path}")
        if anchor.startswith("#"):
            matching = [row for row in source["blocks"] if row["heading"] == anchor]
        else:
            definition = re.compile(r"^\s*-\s+\*\*" + re.escape(anchor) + r"(?=\s|\*|[—–:])", re.MULTILINE)
            matching = [row for row in source["blocks"] if definition.search(texts[row["key"]])]
        require(len(matching) == 1, f"HAQP source anchor missing or ambiguous: {requirement['source']}")
        row = matching[0]
        require(bool(row["requirement_ids"]), f"HAQP obligation classified as non-requirement: {requirement['source']}")
        links = mapping[row["key"]]
        links["haqp_requirement_ids"].append(requirement["id"])
        for test_index, test in enumerate(tests):
            if requirement["id"] not in test["requirements"]:
                continue
            link = {"test_id": test["id"], "name": test["name"], "source_path": test_locations[test["id"]],
                    "availability": "baseline-source-present-unqualified" if test_locations[test["id"]] else "baseline-declared-only-missing",
                    "inspection_commit": BASELINE,
                    "packet_reference": f"{PACKET}#/tests/{test_index}"}
            if link not in links["haqp_test_links"]:
                links["haqp_test_links"].append(link)
            for evidence_index, kind in enumerate(test["evidence"]):
                reference = f"/tests/{test_index}/evidence/{evidence_index}"
                require(pointer(packet, reference) == kind, "invalid packet evidence reference")
                evidence = {"kind": kind, "reference": f"{PACKET}#{reference}",
                            "meaning": "declared evidence category, not a qualification result", "qualification_claim": False}
                if evidence not in links["evidence_links"]:
                    links["evidence_links"].append(evidence)
    return mapping


def build(root: Path, inventory: dict) -> tuple[bytes, bytes]:
    texts = validate_inventory(root, inventory)
    policy = (root / POLICY).read_bytes().decode()
    require(policy.count("State: **proposed; acceptance pending**.") == 1,
            "policy falsely asserts acceptance or omits the explicit proposed state")
    policy_ids = REQUIREMENT_ROW.findall(policy)
    require(bool(policy_ids) and len(policy_ids) == len(set(policy_ids)), "missing or duplicate policy requirement IDs")
    require(all(0 < int(identifier.rsplit("-", 1)[1]) < 100 for identifier in policy_ids), "policy requirements must use reserved IDs 001-099")
    require(all(identifier == f"LIM-SAS-RQ-{int(identifier.rsplit('-', 1)[1]):03d}" for identifier in policy_ids), "noncanonical policy requirement ID")
    phases = re.findall(r"^### Phase ([+-]?\d+)(?=\s|$)", policy, re.MULTILINE)
    require(phases == [str(number) for number in range(-1, 13)], "policy phases must declare each canonical Liminal phase -1 through 12 exactly once in order")
    require(set(inventory["work_order_warrants"]) <= {f"M{number}" for number in range(17, 25)}, "unknown work-order Warrant mapping")
    require(set(inventory.get("policy_requirement_warrants", {})) <= set(policy_ids), "unknown policy requirement Warrant mapping")
    validate_policy_history(root, policy)
    for mapping in (inventory["work_order_warrants"], inventory.get("policy_requirement_warrants", {})):
        for references in mapping.values():
            for reference in references:
                validate_warrant_reference(root, reference)
    validate_warrant_index(root, inventory, policy_ids)
    links = haqp_mapping(root, inventory, texts)
    output = [policy, "" if policy.endswith("\n") else "\n", "\n## Incorporated source requirement registry\n\n",
              "Each row identifies one complete source section. Classification and consolidation policy determine which clauses are normative; context, examples, and recorded status do not become obligations. Source clauses retain all original qualifiers and applicability.\n\n",
              "| Requirement ID | Incorporated clause |\n|---|---|\n"]
    rows = [row for source in inventory["sources"] for row in source["blocks"]]
    for row in rows:
        if row["requirement_ids"]:
            title = re.sub(r"^#+\s*", "", row["heading"]).replace("|", "&#124;")
            output.append(f"| {row['requirement_ids'][0]} | [{title}](#{row['anchor']}) ({row['classification']}) |\n")
    output.append("\n## Complete incorporated source clauses and historical provenance\n\n")
    crosswalk = {"schema_version": 1, "baseline_commit": BASELINE, "authority": "proposed-unaccepted",
                 "policy_sha256": sha(policy.encode()), "source_map_sha256": sha(json_bytes(inventory)),
                 "packet_sha256": inventory["packet_sha256"],
                 "policy_requirements": [{"id": identifier, "provenance": "user-migration-plan", "source": POLICY,
                                          "work_orders": inventory.get("policy_requirement_warrants", {}).get(identifier, [])} for identifier in policy_ids],
                 "sources": [], "blocks": []}
    for entry in crosswalk["policy_requirements"]:
        for warrant in entry["work_orders"]:
            validate_warrant_reference(root, warrant)
    for source in inventory["sources"]:
        crosswalk["sources"].append({key: value for key, value in source.items() if key != "blocks"})
        output.append(f"### Historical source: `{source['path']}`\n\nSource role: `{source['role']}`. Source SHA-256: `{source['source_sha256']}`; baseline: `{BASELINE}`.\n\n")
        if source["role"] == "navigation-index":
            output.append("Navigation index only: statements point to the incorporated v4/R4 clauses; this source does not create a competing constitutional chapter.\n\n")
        milestone = re.fullmatch(r"docs/execution/(M\d{2})\.md", source["path"])
        warrants = inventory["work_order_warrants"].get(milestone[1], []) if milestone else []
        for warrant in warrants:
            validate_warrant_reference(root, warrant)
        for row in source["blocks"]:
            text = texts[row["key"]]
            quoted = "".join("> " + line for line in text.splitlines(keepends=True))
            if quoted and not quoted.endswith("\n"):
                quoted += "\n"
            output.extend([f"<a id=\"{row['anchor']}\"></a>\n\n",
                           f"Classification: `{row['classification']}`. Original lines {row['start_line']}–{row['end_line']}; text SHA-256: `{row['text_sha256']}`.\n\n",
                           f"<!-- source-block:{row['anchor']} -->\n", quoted,
                           f"<!-- /source-block:{row['anchor']} -->\n\n"])
            crosswalk["blocks"].append({**row, "source_path": source["path"], "source_sha256": source["source_sha256"],
                                       "source_reference": f"{source['path']}#L{row['start_line']}",
                                       "sas_reference": f"{SAS}#{row['anchor']}",
                                       "historical_work_orders": [source["path"]] if milestone else [],
                                       "work_orders": warrants, **links[row["key"]]})
    sas = "".join(output).encode()
    gaps = {}
    for block in crosswalk["blocks"]:
        for test in block["haqp_test_links"]:
            if test["availability"] == "baseline-declared-only-missing":
                gap = gaps.setdefault(test["test_id"], {"test_id": test["test_id"], "declared_name": test["name"],
                                      "source": test["packet_reference"], "state": "open",
                                      "severity": "blocks applicable milestone qualification",
                                      "inspection_commit": BASELINE,
                                      "reason": "The baseline packet declares this future test but no matching Rust function exists in baseline conformance/tests. Later Warrant evidence may resolve this gap without rewriting the snapshot.",
                                      "requirement_ids": [], "haqp_requirement_ids": [], "work_orders": []})
                for field in ("requirement_ids", "haqp_requirement_ids", "work_orders"):
                    gap[field] = sorted(set(gap[field]) | set(block[field]))
    crosswalk["evidence_gaps"] = list(gaps.values())
    crosswalk["sas_sha256"] = sha(sas)
    crosswalk["coverage"] = {"source_count": len(inventory["sources"]), "block_count": len(rows),
                             "source_bytes": sum(source["source_bytes"] for source in inventory["sources"]),
                             "source_requirement_count": sum(bool(row["requirement_ids"]) for row in rows),
                             "policy_requirement_count": len(policy_ids),
                             "haqp_requirement_count": sum(len(link["haqp_requirement_ids"]) for link in links.values()),
                             "classifications": dict(sorted(Counter(row["classification"] for row in rows).items()))}
    return sas, json_bytes(crosswalk)


def verify_rendered(root: Path, inventory: dict, sas: bytes, crosswalk_bytes: bytes) -> None:
    """Independently reverse quote wrappers; every original byte must survive."""
    content = sas.decode()
    crosswalk = json.loads(crosswalk_bytes)
    require(crosswalk["authority"] == "proposed-unaccepted", "crosswalk falsely asserts completion or acceptance")
    require(crosswalk["sas_sha256"] == sha(sas), "SAS digest drift")
    index_ids = REQUIREMENT_ROW.findall(content)
    expected_ids = [entry["id"] for entry in crosswalk["policy_requirements"]] + [identifier for source in inventory["sources"] for row in source["blocks"] for identifier in row["requirement_ids"]]
    require(index_ids == expected_ids and len(index_ids) == len(set(index_ids)), "SAS requirement index mismatch or duplicate IDs")
    actual_anchors = re.findall(r"<!-- source-block:([^ ]+) -->", content)
    expected_anchors = [row["anchor"] for source in inventory["sources"] for row in source["blocks"]]
    require(actual_anchors == expected_anchors, "SAS source block omission or duplication")
    for source in inventory["sources"]:
        reconstructed = []
        for row in source["blocks"]:
            anchor = re.escape(row["anchor"])
            matches = re.findall(r"<!-- source-block:" + anchor + r" -->\n(.*?)<!-- /source-block:" + anchor + r" -->", content, re.DOTALL)
            require(len(matches) == 1, f"source wrapper missing or duplicated: {row['key']}")
            lines = matches[0].splitlines(keepends=True)
            require(all(line.startswith("> ") for line in lines), f"source quote corrupt: {row['key']}")
            text = "".join(line[2:] for line in lines)
            if not row["ends_newline"] and text.endswith("\n"):
                text = text[:-1]
            require(sha(text.encode()) == row["text_sha256"], f"incorporated source bytes changed: {row['key']}")
            reconstructed.append(text)
        require("".join(reconstructed).encode() == historical(root, source["path"]), f"source coverage incomplete: {source['path']}")


def check(root: Path) -> dict:
    candidates = sorted((root / "docs/sas").rglob("*.md"))
    require(candidates == [root / SAS], "missing SAS or multiple candidate SAS documents")
    inventory = load_map(root)
    expected_sas, expected_crosswalk = build(root, inventory)
    sas, crosswalk = (root / SAS).read_bytes(), (root / CROSSWALK).read_bytes()
    verify_rendered(root, inventory, sas, crosswalk)
    require(sas == expected_sas, "generated SAS drift or falsely asserted status; run generate after reviewing inputs")
    require(crosswalk == expected_crosswalk, "generated crosswalk drift or invalid references")
    return json.loads(crosswalk)["coverage"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("initialize", "generate", "check"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        if args.command == "initialize":
            inventory = initialize(args.root)
            (args.root / MAP).parent.mkdir(parents=True, exist_ok=True)
            (args.root / MAP).write_bytes(json_bytes(inventory))
            print("Initialized explicit source map. Semantic classifications require review.")
        elif args.command == "generate":
            inventory = load_map(args.root)
            sas, crosswalk = build(args.root, inventory)
            verify_rendered(args.root, inventory, sas, crosswalk)
            (args.root / SAS).parent.mkdir(parents=True, exist_ok=True)
            (args.root / SAS).write_bytes(sas)
            (args.root / CROSSWALK).write_bytes(crosswalk)
            print(json.dumps(check(args.root), sort_keys=True))
        else:
            print(json.dumps(check(args.root), sort_keys=True))
        return 0
    except (MigrationError, KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"SAS migration unavailable: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
