#!/usr/bin/env python3
"""Flip the HAQP packet from `not-run` to `complete` for stage 1a.

This is the LAST step of a qualification lane and it is deliberately dumb: every
value it writes is copied from a committed artifact or computed from git. It
invents nothing. If an artifact disagrees with the packet, `just haq-verify`
must be the thing that says so — not this script, which would then be grading
its own work.

ADR-0020 §1 wants one fixed clean tree. `verify_provenance` enforces that as:

    provenance.commit == fixed_commit == evidence_parent == HEAD^

with HEAD having exactly one parent, and the diff `HEAD^..HEAD` touching only
qualification-metadata paths. So the shape of a legal flip is:

    <fixed base: all code and all evidence>   <- run every lane here
    └── <metadata-only child>                 <- this script prepares it

Run this on a CLEAN tree at the fixed base, AFTER every lane has written its
artifact. It stages nothing and commits nothing; it edits `packet.json` and
prints the commit to make.

Stage 1b is not handled. Mutants stay `predeclared` because ADR-0021 defers the
mutation requirement to M24, and `verify_qualification_stage` refuses a 1a
packet that claims a kill.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import haqp_paths  # noqa: E402  (sibling module, not a package)

ROOT = Path(__file__).resolve().parents[1]
PACKET = ROOT / "conformance/haqp/packet.json"
GATE_FILE = "conformance/tests/phase0.rs"
GATE_IGNORE = '#[ignore = "Phase 0 M17: HAQP qualification evidence not yet complete"]'
EVIDENCE = ROOT / "conformance/haqp/evidence"


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def fail(message: str) -> None:
    print(f"refusing to flip: {message}", file=sys.stderr)
    raise SystemExit(2)


def blake3_of(path: Path) -> str:
    """Digest via the xtask subcommand, so the packet and the verifier agree."""
    return subprocess.check_output(
        ["cargo", "run", "-q", "-p", "liminal-xtask", "--", "haq", "hash", str(path)],
        cwd=ROOT,
        text=True,
    ).strip()


def load(name: str) -> object:
    path = EVIDENCE / name
    if not path.exists():
        fail(f"{path} is missing; its lane has not run at this base")
    return json.loads(path.read_text())


def main() -> int:
    # §1 wants the SOURCE tree fixed and clean. It does not want the evidence
    # tree clean — this runs immediately after the lanes, whose whole job is to
    # write into `conformance/haqp/evidence/`.
    #
    # This computed its own dirty list, and `git()` strips the WHOLE porcelain
    # output — which ate the leading space of the FIRST line only, shifting it
    # one character. `conformance/...` became `onformance/...`, matched no
    # metadata prefix, and the flip refused after a completed 4-hour lane
    # (2026-08-26). Third variant of one bug, each time from keeping a local
    # copy. There is no local copy now.
    trespass = haqp_paths.source_dirt(ROOT)
    if trespass:
        fail(
            "source or gate code is modified; the fixed base must be clean "
            "(ADR-0020 §1):\n  - " + "\n  - ".join(trespass)
        )

    base = git("rev-parse", "HEAD")
    packet = json.loads(PACKET.read_text())

    if packet.get("qualification_stage") != "1a":
        fail(f"this script handles stage 1a only, packet says {packet.get('qualification_stage')!r}")

    # ---- every artifact must have been produced AT THIS BASE ----------------
    # This is the check the campaign actually failed on 2026-08-15: evidence had
    # accumulated across six different commits, which is six trees, and §1
    # admits one. Catching it here turns a confusing verifier error into a
    # specific instruction about which lane to rerun.
    stale: list[str] = []
    for name, field in (
        ("crash.json", "source_commit"),
        ("mutants.json", "source_commit"),
        ("corpus-access.json", "source_commit"),
        ("concurrency.json", "source_commit"),
    ):
        recorded = load(name)
        if isinstance(recorded, dict) and field in recorded and recorded[field] != base:
            stale.append(f"{name} was produced at {recorded[field][:12]}, not {base[:12]}")
    for review in sorted((EVIDENCE / "reviews").glob("*.json")):
        recorded = json.loads(review.read_text())
        commit = recorded.get("fixed_base", {}).get("commit")
        if commit != base:
            stale.append(f"reviews/{review.name} was produced at {str(commit)[:12]}, not {base[:12]}")
        if not recorded.get("fixed_base", {}).get("clean", False):
            stale.append(f"reviews/{review.name} was produced from a dirty base")
    if stale:
        fail("evidence does not describe one fixed tree:\n  - " + "\n  - ".join(stale))

    # ---- copy claims from the artifacts ------------------------------------
    generated = {row["family"]: row for row in load("generated.json")}
    for family in packet["generated"]:
        row = generated.get(family["family"])
        if row is None:
            fail(f"generated.json has no family {family['family']!r}")
        family["result"] = "pass"
        family["seed"] = row["seed"]
        family["evidence_hash"] = row["evidence_hash"]
        family["accepted"] = row["accepted"]
        family["attempts"] = row["attempts"]
        family["discards"] = row["discards"]

    caught = {row["id"] for row in load("canaries.json") if row.get("caught")}
    for canary in packet["canaries"]:
        if canary["id"] not in caught:
            fail(f"canary {canary['id']} is not recorded as caught by the committed run")
        canary["result"] = "caught"

    crash = load("crash.json")
    recorded_boundaries = {row["boundary"] for row in crash["boundaries"]}
    for boundary in packet["crash_boundaries"]:
        if boundary["boundary"] not in recorded_boundaries:
            fail(f"crash boundary {boundary['boundary']} has no committed evidence")
        boundary["result"] = "pass"

    reviews = sorted((EVIDENCE / "reviews").glob("*.json"))
    if len(reviews) != len(packet["reviews"]):
        fail(f"packet declares {len(packet['reviews'])} reviews, evidence has {len(reviews)}")
    for row, path in zip(packet["reviews"], reviews, strict=True):
        record = json.loads(path.read_text())
        if record.get("unresolved_verified_findings"):
            fail(
                f"{path.name} reports {record['unresolved_verified_findings']} unresolved "
                "verified findings; ADR-0020 §6 requires zero"
            )
        row["result"] = "pass"
        row["attempts"] = len(record["attempts"])
        row["findings"] = [f.get("id") or f.get("finding_id") for f in record.get("findings", [])]
        row["unresolved_verified_findings"] = 0
        row["evidence"] = str(path.relative_to(ROOT))

    # Mutants are NOT touched. ADR-0021 defers the mutation requirement to 1b.
    for mutant in packet["mutants"]:
        if mutant["disposition"] != "predeclared":
            fail(f"{mutant['id']} claims {mutant['disposition']!r}; stage 1a defers mutation")

    packet["qualification_state"] = "complete"
    packet["provenance"] = {
        "commit": base,
        "lockfile_blake3": blake3_of(ROOT / "Cargo.lock"),
        "fixed_commit": base,
        "fixed_tree": git("rev-parse", f"{base}^{{tree}}"),
        "evidence_parent": base,
    }

    PACKET.write_text(json.dumps(packet, indent=1) + "\n")

    # AM-17.6: M17.5's exit gate asserts the packet is complete, so it cannot be
    # live before this flip. The metadata child is the only place it can be
    # lifted — at the fixed base it would fail, and after the flip HEAD moves and
    # breaks `evidence_parent == HEAD^`. verify_gate_unignore_only checks that
    # this is the ONLY thing the child changes in that file.
    gate = ROOT / GATE_FILE
    text = gate.read_text()
    if GATE_IGNORE not in text:
        fail(f"{GATE_FILE} does not carry the expected gate #[ignore]; refusing to guess")
    gate.write_text(text.replace(GATE_IGNORE + "\n", "", 1))

    print(f"packet flipped to complete at fixed base {base[:12]}")
    print(f"lifted the qualification gate's #[ignore] in {GATE_FILE}")
    print()
    print("Next, as ONE metadata-only commit (verify_provenance requires exactly one parent")
    print("and refuses any non-metadata path in the child):")
    print()
    print("  git add conformance/haqp/packet.json conformance/haqp/evidence \\")
    print("          docs/execution/phase1-suite-review.md \\")
    print("          docs/execution/m17-5-adversarial-findings.md \\")
    print(f"          {GATE_FILE}")
    print("  git commit -m 'record HAQP-1a qualification evidence'")
    print("  just haq-verify")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
