#!/usr/bin/env python3
"""Build/check a non-authoritative successor bundle from pinned Git objects."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys


PROPOSAL = Path("docs/migration/successor/proposal.json")
RECONCILIATION = Path("docs/migration/successor/reconciliation.json")
CANDIDATE = Path("docs/migration/successor/candidate.md")
ACCEPTED_SAS = Path("docs/sas/LIMINAL_Software_Architecture_Specification.md")
SOURCE_MAP = Path("docs/migration/source-map.json")
ACCEPTED_PROTECTED = [
    ACCEPTED_SAS,
    Path("docs/migration/sas-acceptance-receipt.md"),
    Path("docs/sas/revisions/0.1.0-proposed.1.toml"),
    SOURCE_MAP,
    Path("docs/migration/source-crosswalk.json"),
    Path("docs/migration/consolidation-policy.md"),
    Path("openwarrant.toml"),
    Path("docs/migration/sas-acceptance-request.toml"),
    Path("docs/migration/adoption-decision.md"),
]


class Refusal(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Refusal(message)


def git(root: Path, *args: str) -> bytes:
    try:
        return subprocess.check_output(["git", *args], cwd=root, stderr=subprocess.PIPE)
    except subprocess.CalledProcessError as exc:
        detail = exc.stderr.decode(errors="replace").strip()
        raise Refusal(f"git metadata unavailable for {' '.join(args)}: {detail}") from exc


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_json(root: Path, path: Path) -> dict:
    ensure_no_symlink(root, path, must_exist=True)
    def reject_duplicates(pairs: list[tuple[str, object]]) -> dict:
        result = {}
        for key, value in pairs:
            require(key not in result, f"duplicate JSON key in {path}: {key}")
            result[key] = value
        return result
    try:
        value = json.loads((root / path).read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates)
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise Refusal(f"invalid JSON {path}: {exc}") from exc
    require(isinstance(value, dict), f"invalid JSON object {path}")
    return value


def safe_path(value: object) -> str:
    require(isinstance(value, str) and value, f"invalid source path: {value!r}")
    path = PurePosixPath(value)
    require(not path.is_absolute() and ".." not in path.parts and str(path) == value,
            f"source path escapes repository: {value}")
    return value


def ensure_no_symlink(root: Path, relative: Path, *, must_exist: bool) -> Path:
    require(not relative.is_absolute() and ".." not in relative.parts, f"path escapes repository: {relative}")
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise Refusal(f"path contains symlink: {relative}")
        if not current.exists():
            require(not must_exist, f"required path missing: {relative}")
            break
    return current


def ensure_output_writable(root: Path, relative: Path) -> Path:
    path = ensure_no_symlink(root, relative, must_exist=False)
    if path.exists():
        require(path.stat().st_nlink == 1, f"generated output is a hardlink: {relative}")
    return path


def object_bytes(root: Path, commit: str, path: str) -> bytes:
    safe_path(path)
    return git(root, "show", f"{commit}:{path}")


def resolve_commit(root: Path, value: object, label: str) -> str:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{7,40}", value) is not None,
            f"invalid {label}")
    resolved = git(root, "rev-parse", f"{value}^{{commit}}").decode().strip()
    require(re.fullmatch(r"[0-9a-f]{40}", resolved) is not None, f"invalid resolved {label}")
    return resolved


def quote_source(data: bytes) -> str:
    text = data.decode("utf-8")
    pieces = text.splitlines(keepends=True)
    if not pieces and text == "":
        return "> [empty file]\n"
    return "".join("> " + piece for piece in pieces)


def changed_hunks(root: Path, old: str, new: str, path: str) -> list[dict]:
    raw = git(root, "-c", "color.ui=false", "diff", "--unified=0", "--no-ext-diff",
              "--no-textconv", "--no-renames", "--diff-algorithm=myers", old, new, "--", path).decode()
    result = []
    for line in raw.splitlines():
        match = re.match(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", line)
        if match:
            result.append({
                "old_start": int(match.group(1)), "old_lines": int(match.group(2) or 1),
                "new_start": int(match.group(3)), "new_lines": int(match.group(4) or 1),
            })
    require(result, f"changed source has no exact Git hunk coordinates: {path}")
    return result


def affected_ids(source: dict, hunks: list[dict]) -> list[str]:
    ids: set[str] = set()
    for block in source.get("blocks", []):
        start, end = block.get("start_line"), block.get("end_line")
        if not isinstance(start, int) or not isinstance(end, int):
            continue
        for hunk in hunks:
            h_start = hunk["old_start"]
            h_end = h_start + max(hunk["old_lines"], 1) - 1
            if start <= h_end and h_start <= end:
                ids.update(block.get("requirement_ids", []))
    return sorted(ids)


def source_stated_statuses(data: bytes) -> list[str]:
    text = data.decode("utf-8")
    found = []
    patterns = [
        r"AM-[0-9.]+ \(user-ratified \d{4}-\d{2}-\d{2}\)",
        r"(?im)^status:\s*(ruled|proposed|ratified|unratified|accepted|rejected)\s*$",
        r"(?im)^\*\*Status:\*\*\s*(ruled|proposed|ratified|unratified|accepted|rejected)\b.*$",
    ]
    for pattern in patterns:
        for match in re.finditer(pattern, text):
            value = match.group(0).strip()
            if value not in found:
                found.append(value)
    return found


def context(root: Path) -> tuple[dict, dict, str, str, list[dict]]:
    proposal = load_json(root, PROPOSAL)
    expected_fields = {
        "schema", "status", "candidate_version", "accepted_commit", "accepted_sas_sha256",
        "target_commit", "expected_modified_sources", "expected_added_candidates", "packet_sha256",
    }
    require(set(proposal) == expected_fields, "proposal fields are missing or unexpected")
    require(all(isinstance(proposal.get(key), str) for key in expected_fields - {
        "expected_modified_sources", "expected_added_candidates"
    }), "proposal scalar fields must be strings")
    require(all(isinstance(proposal.get(key), list) for key in {
        "expected_modified_sources", "expected_added_candidates"
    }), "proposal inventories must be arrays")
    require(proposal.get("schema") == "liminal-sas-successor/v1", "unsupported proposal schema")
    require(proposal.get("status") == "candidate-unregistered-unselected",
            "candidate falsely claims accepted/selected status")
    require(proposal.get("candidate_version") == "0.1.0-proposed.2", "unexpected candidate version")
    accepted = resolve_commit(root, proposal.get("accepted_commit"), "accepted commit")
    require(accepted == "9e76fc99027c55ded5bd0cc61da43b0f2b68b049", "accepted commit pin changed")
    target = resolve_commit(root, proposal.get("target_commit"), "target commit")
    require(target == "7e4ba39d6babb1e91a78aecd75bfa0262095c2c9", "target commit/tree pin changed")
    source_map = json.loads(object_bytes(root, accepted, str(SOURCE_MAP)))
    sources = source_map.get("sources")
    require(isinstance(sources, list) and len(sources) == 65, "accepted source inventory is not exactly 65 paths")
    return proposal, source_map, accepted, target, sources


def protected_paths(root: Path, accepted: str, sources: list[dict]) -> list[str]:
    paths = [str(path) for path in ACCEPTED_PROTECTED]
    paths.extend(safe_path(source.get("path")) for source in sources)
    paths.extend(git(root, "ls-tree", "-r", "--name-only", accepted, "--", "docs/warrants").decode().splitlines())
    return sorted(set(paths))


def verify_protected(root: Path, accepted: str, sources: list[dict]) -> None:
    for path in protected_paths(root, accepted, sources):
        current = ensure_no_symlink(root, Path(path), must_exist=True)
        require(current.is_file(), f"accepted protected file missing: {path}")
        require(current.read_bytes() == object_bytes(root, accepted, path),
                f"accepted protected bytes changed: {path}")


def inventory(root: Path, proposal: dict, source_map: dict, accepted: str, target: str,
              sources: list[dict]) -> list[dict]:
    by_path = {source["path"]: source for source in sources}
    modified = [safe_path(value) for value in proposal.get("expected_modified_sources", [])]
    added = [safe_path(value) for value in proposal.get("expected_added_candidates", [])]
    require(len(modified) == 10 and len(set(modified)) == 10, "modified source inventory must contain exactly 10 unique paths")
    require(len(added) == 2 and len(set(added)) == 2, "added candidate inventory must contain exactly 2 unique paths")
    actual_modified = sorted(path for path in by_path
                             if object_bytes(root, accepted, path) != object_bytes(root, target, path))
    require(sorted(modified) == actual_modified,
            f"modified accepted-source inventory mismatch: expected {modified}, actual {actual_modified}")
    candidates = git(root, "diff", "--name-status", accepted, target, "--", "spec", "docs/adr",
                     "docs/execution/rulings").decode().splitlines()
    actual_added = sorted(line.split("\t", 1)[1] for line in candidates if line.startswith("A\t"))
    require(sorted(added) == actual_added,
            f"new candidate-source inventory mismatch: expected {added}, actual {actual_added}")
    records = []
    for path in modified:
        old_data, new_data = object_bytes(root, accepted, path), object_bytes(root, target, path)
        hunks = changed_hunks(root, accepted, target, path)
        records.append({
            "path": path, "change": "modified", "old_sha256": sha(old_data),
            "new_sha256": sha(new_data),
            "old_blob": git(root, "rev-parse", f"{accepted}:{path}").decode().strip(),
            "new_blob": git(root, "rev-parse", f"{target}:{path}").decode().strip(),
            "old_source_bytes": len(old_data), "new_source_bytes": len(new_data),
            "old_ends_newline": old_data.endswith(b"\n"),
            "new_ends_newline": new_data.endswith(b"\n"),
            "hunks": hunks, "affected_accepted_requirement_ids": affected_ids(by_path[path], hunks),
            "classification": "attributed-source-history",
            "successor_incorporation_status": "pending-t1-disposition",
            "source_stated_statuses": source_stated_statuses(new_data),
            "original_recorded_status_retained": True,
        })
    for path in added:
        data = object_bytes(root, target, path)
        records.append({
            "path": path, "change": "added-candidate", "new_sha256": sha(data),
            "new_blob": git(root, "rev-parse", f"{target}:{path}").decode().strip(),
            "new_start": 1,
            "new_end": data.count(b"\n") + (0 if data.endswith(b"\n") else 1),
            "source_bytes": len(data), "ends_newline": data.endswith(b"\n"),
            "affected_accepted_requirement_ids": [], "classification": "pending-normative-admission",
            "successor_incorporation_status": "pending-t1-disposition",
            "source_stated_statuses": source_stated_statuses(data),
            "original_recorded_status_retained": True,
        })
    return records


def build(root: Path) -> tuple[bytes, bytes]:
    proposal, source_map, accepted, target, sources = context(root)
    verify_protected(root, accepted, sources)
    records = inventory(root, proposal, source_map, accepted, target, sources)
    packet = object_bytes(root, target, "conformance/haqp/packet.json")
    require(sha(packet) == proposal.get("packet_sha256"), "target packet provenance digest changed")
    reconciliation = {
        "schema": "liminal-sas-successor-reconciliation/v1",
        "status": "candidate-history-pending-t1-disposition",
        "accepted_commit": accepted, "accepted_tree": git(root, "rev-parse", f"{accepted}^{{tree}}").decode().strip(),
        "target_commit": target, "target_tree": git(root, "rev-parse", f"{target}^{{tree}}").decode().strip(),
        "accepted_sas_sha256": proposal["accepted_sas_sha256"],
        "packet_sha256_provenance_only": sha(packet),
        "qualification_claim": None,
        "t1_impact_summary": [
            "AM-17.9 remains ratified. Proposed scoped reconciliation: accepted SAS section 3 and Class C coexist only for a valid exact-claim-scoped signed ruling; unruled or invalidly ruled C remains unresolved, C is not a blanket waiver, and A or B still requires fix plus full rerun. Successor acceptance is pending; no suite budget, 64-mutant floor, or full M19 threshold changes.",
            "AM-24.1 and AM-24.2 restate the accepted GAP-002 sequencing; classify the source wording without inventing new authority.",
            "R-001 and R-002 retain their exact claim bounds, ruled status, attribution, and signature condition; successor incorporation remains pending.",
            "Phase 1 suite review hashes and counts are attributed evidence history, not a gate milestone or qualification claim.",
        ],
        "records": records,
    }
    accepted_sas = object_bytes(root, accepted, str(ACCEPTED_SAS))
    require(sha(accepted_sas) == proposal["accepted_sas_sha256"] ==
            "53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73",
            "accepted SAS digest pin changed")
    accepted_rows = re.findall(rb"^\|\s*(LIM-SAS-RQ-\d{3})\s*\|", accepted_sas, re.MULTILINE)
    require(len(accepted_rows) == 777 and len(set(accepted_rows)) == 777,
            "accepted SAS does not retain exactly 777 unique requirement rows")
    header = (
        "# Liminal SAS successor candidate — not registered, selected, or accepted\n\n"
        "Candidate `0.1.0-proposed.2` preserves the following accepted SAS bytes exactly. "
        "Its historic proposal header is retained because acceptance was recorded later in "
        "`docs/migration/sas-acceptance-receipt.md`; it is not a new acceptance claim.\n\n"
        "<!-- BEGIN EXACT ACCEPTED SAS BYTES -->\n"
    ).encode()
    candidate = bytearray(header)
    candidate.extend(accepted_sas)
    candidate.extend(b"<!-- END EXACT ACCEPTED SAS BYTES -->\n\n## Non-normative attributed source-history annex\n\n")
    for record in records:
        candidate.extend(f"### Candidate source history: `{record['path']}`\n\n".encode())
        candidate.extend(b"Successor incorporation: **pending T1 disposition**. Existing attributed status is retained.\n\n")
        if record["source_stated_statuses"]:
            candidate.extend(("Source-stated status markers: " +
                              "; ".join(f"`{value}`" for value in record["source_stated_statuses"]) +
                              ". These are attributed source facts, not successor acceptance.\n\n").encode())
        candidate.extend(quote_source(object_bytes(root, target, record["path"])).encode())
        if not candidate.endswith(b"\n"):
            candidate.extend(b"\n")
        candidate.extend(b"\n")
    return (json.dumps(reconciliation, indent=2, ensure_ascii=False).encode() + b"\n", bytes(candidate))


def command(root: Path, mode: str) -> None:
    reconciliation, candidate = build(root)
    if mode == "generate":
        reconciliation_path = ensure_output_writable(root, RECONCILIATION)
        candidate_path = ensure_output_writable(root, CANDIDATE)
        reconciliation_path.write_bytes(reconciliation)
        candidate_path.write_bytes(candidate)
    else:
        reconciliation_path = ensure_no_symlink(root, RECONCILIATION, must_exist=True)
        candidate_path = ensure_no_symlink(root, CANDIDATE, must_exist=True)
        require(reconciliation_path.is_file(), "generated reconciliation missing")
        require(candidate_path.is_file(), "generated candidate missing")
        require(reconciliation_path.read_bytes() == reconciliation, "generated reconciliation drift")
        require(candidate_path.read_bytes() == candidate, "generated candidate drift")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("generate", "check"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        command(args.root.resolve(), args.mode)
    except (Refusal, OSError, UnicodeError) as exc:
        print(f"successor: REFUSED: {exc}", file=sys.stderr)
        return 1
    print(f"successor: {'generated' if args.mode == 'generate' else 'PASS'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
