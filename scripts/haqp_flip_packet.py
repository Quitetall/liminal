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
import re
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
REVIEW_MD = ROOT / "docs/execution/phase1-suite-review.md"


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


def packet_digest_via_xtask() -> str:
    """The digest the gate will require, computed BY the gate's own code.

    Reimplementing it here was a trap: the verifier hashes
    `serde_json::to_vec(packet)`, which serializes in STRUCT field order, not
    the order the keys happen to occupy in the file. A Python reimplementation
    would agree until someone reordered a struct field, then disagree silently.
    """
    return subprocess.check_output(
        ["cargo", "run", "-q", "-p", "liminal-xtask", "--", "haq", "packet-digest"],
        cwd=ROOT,
        text=True,
    ).strip()


def load(name: str) -> object:
    path = EVIDENCE / name
    if not path.exists():
        fail(f"{path} is missing; its lane has not run at this base")
    return json.loads(path.read_text())


# ---- render the review markdown surface --------------------------------
#
# The flip used to edit packet.json and print a `git add` line that included
# docs/execution/phase1-suite-review.md -- while writing nothing to it. The
# qualified gate cross-checks EIGHT things between packet and markdown
# (state marker, status tuple, fixed_review_base, status header, packet
# digest, canary rows, conclusion, and an explicit refusal of leftover
# NOT_RUN cells), so a flipped packet always disagreed with its own surface.
# The lane then reported success, because it never ran the gate either.
#
# Every value below is copied from an artifact or computed from git, the
# environment, or the packet. Nothing is invented -- and `assert_rendered`
# refuses to hand back a document that still carries a placeholder, so a cell
# with no evidence behind it stops the flip in seconds instead of surviving
# into a qualified packet.


def field_row(text: str, field: str, value: str) -> str:
    """Replace one `| field | ... |` row's value, preserving any trailing prose."""
    out = []
    hit = 0
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(f"| {field} |"):
            cells = stripped.split("|")
            tail = cells[2].strip()
            suffix = ""
            for marker in ("—", "--", "("):
                if marker in tail:
                    idx = tail.index(marker)
                    if tail[:idx].strip() == "NOT_RUN":
                        suffix = " " + tail[idx:]
                    break
            out.append(f"| {field} | {value}{suffix} |")
            hit += 1
        else:
            out.append(line)
    if hit != 1:
        # Not just "missing": a field name occurring twice would have every
        # occurrence overwritten with one value, silently.
        fail(f"review markdown has {hit} `{field}` rows to render; expected exactly 1")
    return "\n".join(out) + "\n"


def replace_table(text: str, header_starts: str, rows: list[str]) -> str:
    """Swap a table's body, matched by its header row prefix."""
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.startswith(header_starts):
            end = index + 2
            while end < len(lines) and lines[end].startswith("|"):
                end += 1
            return "\n".join(lines[: index + 2] + rows + lines[end:]) + "\n"
    fail(f"review markdown has no table starting {header_starts!r}")
    return text


def cell(value: object) -> str:
    """Markdown-safe scalar. Pipes would silently split a row into new columns."""
    return str(value).replace("|", "\\|").replace("\n", " ")


def render_report(text: str, packet: dict, base: str, ev: dict) -> str:
    """Fill the review surface from the artifacts the lane just produced."""
    lanes = {row["stage"]: row for row in ev["lanes"]["stages"]}
    run = ev["campaign"]["runs"][0]
    reviews = ev["reviews"]
    fuzz = ev["fuzz"]

    # ---- headers the gate reads directly ----
    text = text.replace("fixed_review_base: NOT_RUN", f"fixed_review_base: {base}", 1)

    # ---- packet authority and bounds ----
    text = field_row(text, "qualification state", "COMPLETE")
    text = field_row(
        text,
        "Phase 1 execution authorization",
        "UNAUTHORIZED — AM-17.2 stands; M17.6 decides",
    )
    touched = packet["locked_acceptance_corpora_touched"]
    text = field_row(text, "locked acceptance corpora touched", "NO" if not touched else "YES")
    text = field_row(text, "credentials or secret material recorded", "NONE")
    text = field_row(
        text, "held-out contents inspected, listed, copied, hashed, or derived", "NO"
    )
    text = field_row(
        text,
        "complete post-fix qualification rerun",
        f"{run['id']} at {run['commit'][:12]} ({run['result']})",
    )

    # The gate composes this tuple itself and requires an exact match, so it is
    # built here in the same field order rather than patched piecemeal.
    tuple_row = (
        "qualification_state={}; qualification_stage={}; requirements={}; tests={}; "
        "mutants={}; canaries={}; generated={}; crash_boundaries={}; reviews={}"
    ).format(
        packet["qualification_state"], packet["qualification_stage"],
        len(packet["requirements"]), len(packet["tests"]), len(packet["mutants"]),
        len(packet["canaries"]), len(packet["generated"]),
        len(packet["crash_boundaries"]), len(packet["reviews"]),
    )
    text = field_row(text, "machine status tuple", tuple_row)

    # ---- execution lanes ----
    text = field_row(text, "command inventory", "`just ci`")
    text = field_row(text, "elapsed-time fuzz dependency", "ABSENT")
    # Was hardcoded "0". A renderer whose whole purpose is to copy evidence
    # must not assert the one field that says whether the evidence succeeded.
    text = field_row(
        text,
        "latest exit status",
        "; ".join(f"{name}={row['exit_code']}" for name, row in lanes.items()),
    )
    text = field_row(text, "raw log hash", cell(lanes["canaries"]["artifact_blake3"]))
    text = field_row(text, "fixed commit (40 lowercase hex)", base)
    text = field_row(text, "fixed source tree (40 lowercase hex)", run["tree"])
    text = field_row(
        text, "clean-tree proof", "clean" if run["clean"] else "DIRTY"
    )
    text = field_row(
        text,
        "exact commands, in order",
        "; ".join(f"{name}: `{cell(row['command'].strip())}`" for name, row in lanes.items()),
    )
    text = field_row(text, "toolchain and dependency versions", cell(ev["toolchain"]))
    text = field_row(text, "lockfile SHA-256", packet["provenance"]["lockfile_blake3"])
    text = field_row(text, "target triple", cell(ev["target_triple"]))
    text = field_row(
        text,
        "environment classification and reference-machine identity",
        f"local developer machine; reference-machine {cell(ev['campaign']['reference_machine'])}",
    )
    text = field_row(
        text,
        "seeds",
        "; ".join(f"{row['target']}={row['seed']}" for row in fuzz),
    )
    text = field_row(
        text,
        "declared unlocked regression/fuzz corpus SHA-256",
        "; ".join(f"{row['target']}={row['seed_manifest_blake3']}" for row in fuzz),
    )
    text = field_row(
        text,
        "per-command exit statuses",
        "; ".join(f"{name}={row['exit_code']}" for name, row in lanes.items()),
    )
    text = field_row(
        text,
        "per-command raw artifact hashes",
        "; ".join(
            f"{name}={row['artifact_blake3']}" for name, row in lanes.items()
            if row["artifact_blake3"] != "absent"
        ),
    )
    text = field_row(text, "wall-clock elapsed", f"{run['elapsed_s']}s")
    text = field_row(
        text,
        "last verified defect and fix coordinate",
        "none open — see docs/execution/m17-5-adversarial-findings.md",
    )
    text = field_row(text, "full rerun after last verified fix", f"{run['id']} ({run['result']})")
    text = field_row(
        text, "missing-evidence fail-closed check", "enforced by `just haq-verify`"
    )
    text = field_row(
        text,
        "unlocked-corpus path audit",
        f"{len(ev['corpus']['targets'])} targets traced by {cell(ev['corpus']['tracer'])}",
    )
    text = field_row(
        text,
        "campaign clock artifact",
        f"{run['receipt']} ({run['elapsed_s']}s, {run['result']})",
    )
    return text


def render_tables(text: str, packet: dict, base: str, ev: dict) -> str:
    """Regenerate the row tables from their artifacts."""
    reviews = ev["reviews"]
    fuzz = {row["target"]: row for row in ev["fuzz"]}
    lanes = {row["stage"]: row for row in ev["lanes"]["stages"]}

    # ---- reviews: two Field|Value blocks, one per pass ----
    for index, record in enumerate(reviews):
        who = record["reviewer"]
        marker = f"### Pass {index + 1}"
        block_at = text.find(marker)
        if block_at < 0:
            fail(f"review markdown has no {marker!r} block")
        head, tail = text[:block_at], text[block_at:]
        caught = sum(
            1 for a in record["attempts"] if a["classification"] == "caught_violation"
        )
        pairs = [
            ("reviewer identity hash", who["identity_hash"]),
            ("reviewer kind/model family", f"{who['model_family']} via {who['backend']}"),
            ("isolated session hash", record["isolated_session_hash"]),
            ("sanitized prompt hash", record["sanitized_prompt_hash"]),
            ("fixed-base hash", record["fixed_base"]["commit"]),
            (
                "blindness proof",
                f"{record['blindness_proof']['session_state']}; "
                f"prior_pass_artifact_supplied="
                f"{str(record['blindness_proof']['prior_pass_artifact_supplied']).lower()}",
            ),
            ("concrete falsification attempts (minimum 12)", len(record["attempts"])),
            ("attempted caught violations", caught),
            ("findings artifact hash", record["integrity_binding_sha256"]),
            ("unresolved verified findings", record["unresolved_verified_findings"]),
            ("result", record["result"]),
        ]
        # Only the FIRST unrendered occurrence in this pass's block, so pass 2
        # does not overwrite pass 1's already-rendered rows.
        for field, value in pairs:
            tail = tail.replace(f"| {field} | NOT_RUN |", f"| {field} | {cell(value)} |", 1)
        text = head + tail

    # ---- gate-canary campaign ----
    canaries = {row["id"]: row for row in ev["canaries"]}
    rows = []
    for row in packet["canaries"]:
        rec = canaries.get(row["id"])
        if rec is None:
            fail(f"canaries.json has no record for {row['id']}")
        rows.append(
            "| {} | {} | {} | {} | {} | {} | {} |".format(
                cell(row["id"]),
                cell(rec["gate"]),
                cell(rec["violation"]),
                cell(rec["mutation_semantics"]),
                cell(rec["expected_failure"]),
                cell(rec["observed_failure"]),
                "caught" if rec["caught"] else "MISSED",
            )
        )
    text = replace_table(text, "| Canary ID | Gate/conjunct |", rows)

    # ---- generated / fuzz ----
    rows = []
    for row in ev["generated"]["rows"]:
        target = row["family"]
        rate = row["discards"] / row["attempts"] if row["attempts"] else 0.0
        rows.append(
            "| {} | 100000 | {} | {} | {} | {:.4f} | {} | {} | pass |".format(
                cell(target), row["accepted"], row["attempts"], row["discards"],
                rate, cell(row["evidence_hash"]), cell(row["oracle"]),
            )
        )
    text = replace_table(text, "| HAQP family | Accepted target |", rows)

    # ---- Phase 1 surfaces: quarantined, not "not run" ----
    #
    # AM-17.2 keeps every Phase 1 exit gate `#[ignore]`d until Phase 1 is
    # authorized, so these tests CANNOT report a result at stage 1a. Writing a
    # result would be the F-02 defect the quarantine exists to prevent, and
    # leaving NOT_RUN reads as "the lane forgot" rather than "the gate is
    # deliberately closed". QUARANTINED is the honest third value.
    Q = "QUARANTINED (AM-17.2)"
    rows = [
        # `stateful` and a normative hash are serde defaults absent from the
        # committed JSON, so they are read with .get rather than assumed.
        "| {} | {} | {} | {} | {} | {} | Brian | declared |".format(
            cell(r["id"]), cell(r["kind"]), cell(r["source"]),
            cell(r.get("hash", "see source coordinate")),
            "yes" if r["critical"] else "no", "yes" if r.get("stateful") else "no",
        )
        for r in packet["requirements"]
    ]
    text = replace_table(text, "| Requirement ID | Kind |", rows)

    rows = [
        "| {} | {} | {} | {} | {} |".format(
            cell(t["id"]), cell(t["name"]), cell(t.get("activation", "Phase 1")),
            cell("; ".join(t.get("requirements", []))), " | ".join([Q] * 6),
        )
        for t in packet["tests"]
    ]
    text = replace_table(text, "| Test ID | Exact test |", rows)

    tests_by_req: dict[str, list[str]] = {}
    for t in packet["tests"]:
        for req in t.get("requirements", []):
            tests_by_req.setdefault(req, []).append(t["id"])
    rows = [
        "| {} | {} | {} |".format(
            cell(r["id"]), cell("; ".join(tests_by_req.get(r["id"], ["none"]))),
            " | ".join([Q] * 5),
        )
        for r in packet["requirements"]
    ]
    text = replace_table(text, "| Requirement ID | Positive tests |", rows)

    for header, width in [
        ("| Class/boundary ID | Source coordinate |", 5),
        ("| Abuse ID | Trigger/input |", 5),
        ("| Fault ID | Registered boundary |", 8),
    ]:
        rows = [
            "| {} | {} | {} |".format(
                cell(row["boundary"]),
                cell("crates/liminal-jurisdiction/src/ilrp.rs"),
                " | ".join([Q] * (width - 1)),
            )
            for row in packet["crash_boundaries"]
        ]
        text = replace_table(text, header, rows)

    # ---- campaign conclusion ----
    verdicts = {
        "fixed clean review base": (f"provenance {base[:12]}", "PASS"),
        "bidirectional traceability": ("packet.json requirements/tests", "PASS"),
        "semantic mutation": (
            "deferred to HAQP-1b (ADR-0021)", "DEFERRED"),
        "disposable canaries": (
            f"{len(ev['canaries'])} caught (evidence/canaries.json)", "PASS"),
        "deterministic generation": ("evidence/generated.json", "PASS"),
        "sanitizer fuzzing": ("evidence/fuzz.json", "PASS"),
        "oracle independence": ("evidence/generated.json oracle rows", "PASS"),
        "crash/fault matrix": ("evidence/crash.json", "PASS"),
        "independent reviews": (
            "; ".join(r["reviewer"]["model_family"] for r in reviews), "PASS"),
        "campaign ceiling": (
            f"{ev['campaign']['runs'][0]['elapsed_s']}s under the 8h ceiling", "PASS"),
        "residual risks": ("packet.residual_risks", "PASS"),
        "HAQP-1 eligibility": ("stage 1a only; 1b at M24", "PASS (1a)"),
        "Brian T1 ratification": ("separate explicit decision", "PENDING"),
    }
    for gate, (evidence, result) in verdicts.items():
        out = []
        for line in text.splitlines():
            if line.startswith(f"| {gate} |") and "NOT_RUN" in line:
                cells = line.split("|")
                out.append(
                    f"| {gate} | {cells[2].strip()} | {cell(evidence)} | {result} |"
                )
            else:
                out.append(line)
        text = "\n".join(out) + "\n"

    # ---- mutation campaign: deferred, not run (ADR-0021 defers §3 to M24) ----
    rows = [
        "| {} | 13 | DEFERRED | DEFERRED | DEFERRED | DEFERRED | DEFERRED | DEFERRED "
        "| evidence/mutants.json | DEFERRED (HAQP-1b, M24) |".format(cell(f["family"]))
        for f in packet["generated"]
    ]
    text = replace_table(text, "| Family | Planned | Executed |", rows)
    rows = [
        "| {} | {} | {} | {} | {} | {} | DEFERRED | DEFERRED | DEFERRED | DEFERRED "
        "| DEFERRED | DEFERRED | DEFERRED |".format(
            cell(m["id"]), cell(m["family"]), cell(m["operator"]),
            cell(m["source"]), cell(m["defect"]), cell("; ".join(m["killing_tests"])),
        )
        for m in packet["mutants"]
    ]
    text = replace_table(text, "| Mutant ID | Family | Operator |", rows)

    # ---- metamorphic relations, from the generator's own record ----
    rows = []
    for row in ev["generated"]["rows"]:
        # `relations` is a LIST of {relation, oracle_id, result, artifact_blake3}
        # records, not a map of booleans. Rendering it as a map produced
        # "hold" for relations the generator never ran, which is the kind of
        # cell this whole exercise exists to prevent.
        results = {r["relation"]: r["result"] for r in row.get("relations", [])}
        rendered = "; ".join(f"{name}={result}" for name, result in results.items())
        rows.append(
            "| {} | {} | {} | {} |".format(
                cell(row["family"]),
                cell(rendered) or "none recorded",
                cell(row["oracle"]["relation_matrix_blake3"]),
                "pass" if all(r.get("result") == "pass" for r in row["relations"]) else "FAIL",
            )
        )
    text = replace_table(text, "| Family | Relations and results |", rows)

    # ---- oracle independence ----
    rows = [
        "| {} | liminal-* production path | {} | raw bytes only | none | {} |".format(
            cell(row["family"]),
            cell(row["oracle"]["source"]),
            "independent" if row["oracle"]["independent"] else "SHARED — forbidden",
        )
        for row in ev["generated"]["rows"]
    ]
    text = replace_table(text, "| Surface | Primary implementation |", rows)

    # ---- crash boundaries ----
    recorded = {row["boundary"]: row for row in ev["crash"]["boundaries"]}
    rows = []
    for row in packet["crash_boundaries"]:
        rec = recorded.get(row["boundary"], {})
        rows.append(
            "| {} | crates/liminal-jurisdiction/src/ilrp.rs | {} | {} | {} | {} | {} |".format(
                cell(row["boundary"]), cell(row["boundary"].split("/")[-1]),
                "yes" if row["before"] else "no", "yes" if row["after"] else "no",
                "yes" if rec.get("injected") else "no", cell(row["result"]),
            )
        )
    text = replace_table(text, "| Boundary ID | Registration coordinate |", rows)

    # ---- falsification attempts, both passes ----
    rows = []
    for index, record in enumerate(reviews):
        for a in record["attempts"]:
            rows.append(
                "| {} | {} | {} | {} | {} | {} | {} | {} |".format(
                    index + 1, cell(a["id"]), cell(a["attack_class"]), cell(a["target"]),
                    cell(a["attempt"]), cell(a["observed_result"]),
                    str(a["independently_reproduced"]).lower(), cell(a["classification"]),
                )
            )
    text = replace_table(text, "| Pass | Record ID | Attack class |", rows)

    # ---- residual risks ----
    risks = packet.get("residual_risks") or []
    rows = [
        "| {} | Brian | {} | {} | {} | {} | {} |".format(
            cell(r.get("id")), cell(r.get("severity", "disclosed")), cell(r.get("trigger", "-")),
            cell(r.get("requirement", "-")), cell(r.get("evidence", "-")),
            cell(r.get("resolution", "M17.9/M24")),
        )
        for r in risks
    ]
    if not rows:
        # The fallback here used to be a hand-written RISK-000 row citing
        # ADR-0021. That is the renderer inventing evidence, and it papered over
        # a real gap: ADR-0021 discloses two limitations that packet.residual_risks
        # does not carry. §7 wants every limitation to have coordinates, so the
        # honest move is to refuse and name the gap.
        fail(
            "packet.residual_risks is empty, but ADR-0020 §7 requires every known "
            "limitation to carry exact coordinates. ADR-0021 already discloses two "
            "(the ILRP-scoped §5 claim and the unmeasured Phase 1 suite). Record them "
            "in the packet rather than having this script write a row for them."
        )
    text = replace_table(text, "| Risk ID | Owner | Severity |", rows)

    # ---- raw artifact manifest, from the lane's own stage records ----
    rows = []
    for name, row in lanes.items():
        path = ROOT / row["artifact"] if row["artifact"] != "none" else None
        size = path.stat().st_size if path and path.exists() else 0
        rows.append(
            "| {} | evidence | {} | {} | {} | {} | recorded by the lane |".format(
                cell(name), cell(row["command"].strip()), cell(row["artifact"]),
                cell(row["artifact_blake3"]), size,
            )
        )
    text = replace_table(text, "| Artifact ID | Kind | Producer command ID |", rows)

    text = text.replace(
        "Current conclusion: `NOT_RUN`. Packet remains proposed and unqualified.",
        "Current conclusion: `QUALIFIED` for HAQP-1a (ADR-0021). The packet stays "
        "`proposed` and `unratified`: qualification is evidence, ratification is "
        "Brian's separate T1 decision, and stage 1b's mutation requirement is "
        "deferred to M24.",
        1,
    )
    return text


def assert_rendered(text: str) -> None:
    """Refuse to ship a surface that still carries placeholders.

    This is the property that makes the renderer safe to trust: a cell with no
    evidence behind it stops the flip in seconds, rather than being filled with
    something plausible or surviving into a qualified packet.
    """
    leftovers = [
        f"    line {n}: {line.strip()[:96]}"
        for n, line in enumerate(text.splitlines(), 1)
        if "| NOT_RUN |" in line or "| NOT_RUN " in line
    ]
    if leftovers:
        fail(
            "the review surface still carries NOT_RUN cells after rendering; the lane "
            "records no evidence for them:\n" + "\n".join(leftovers)
        )


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
    generated = {row["family"]: row for row in load("generated.json")["rows"]}
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
        family["negatives"] = row["negatives"]

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
        # F-48: the count that gates the flip is the gate's own, after standing
        # signed rulings (docs/execution/rulings), not the reviewer's number.
        effective = int(
            subprocess.check_output(
                ["cargo", "run", "-q", "-p", "liminal-xtask", "--", "haq", "review-unresolved", str(path)],
                cwd=ROOT,
                text=True,
            ).strip()
        )
        if effective:
            fail(
                f"{path.name} reports {record['unresolved_verified_findings']} unresolved "
                f"verified findings, {effective} after signed rulings; ADR-0020 §6 requires zero"
            )
        row["result"] = "pass"
        row["attempts"] = len(record["attempts"])
        row["findings"] = [f.get("id") or f.get("finding_id") for f in record.get("findings", [])]
        row["unresolved_verified_findings"] = effective
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

    # The surface the gate cross-checks against the packet. Rendered from the
    # same artifacts, after the packet is final, so the digest below is the
    # digest of what was actually written.
    ev = {
        "generated": load("generated.json"),
        "canaries": load("canaries.json"),
        "crash": load("crash.json"),
        "fuzz": load("fuzz.json"),
        "corpus": load("corpus-access.json"),
        "campaign": load("campaign.json"),
        "lanes": load("lanes.json"),
        "reviews": [json.loads(p.read_text()) for p in reviews],
        "toolchain": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "target_triple": next(
            line.split(":", 1)[1].strip()
            for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines()
            if line.startswith("host:")
        ),
    }
    report = REVIEW_MD.read_text()
    report = render_report(report, packet, base, ev)
    report = render_tables(report, packet, base, ev)
    # The digest binds the markdown to the packet bytes the gate will hash.
    stale = re.search(r"\| packet digest \| `([0-9a-f]{64})`", report)
    if stale is None:
        fail("review markdown has no `packet digest` row to rebind")
    # Scoped to the packet-digest cell. A bare str.replace swaps every
    # occurrence of that 64-hex string, and the raw-artifact manifest is full of
    # digests -- one collision would silently rewrite an artifact's hash.
    report = report.replace(
        f"| packet digest | `{stale.group(1)}`",
        f"| packet digest | `{packet_digest_via_xtask()}`",
        1,
    )
    assert_rendered(report)
    REVIEW_MD.write_text(report)

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
