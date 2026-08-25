#!/usr/bin/env python3
"""What a qualification lane may leave dirty, in ONE place.

ADR-0020 §1 fixes the SOURCE tree. It does not fix the evidence tree — the
lanes' entire job is to write into `conformance/haqp/evidence/`. Three separate
places had each written their own "is the tree clean" check as
`git status --porcelain` being empty, and every one of them therefore rejected
its own lane's output:

- `haqp_flip_packet.py` would have refused the evidence it was called to commit,
  after a 3.5-hour campaign (found 2026-08-16);
- `haqp_blind_review.py` blocked with "fixed review base is dirty" at the END of
  a completed 4-hour lane (found 2026-08-25);
- `haqp_campaign_clock.sh` recorded `clean=0` on a legitimate run, and
  `verify_campaign_clock` requires `run.clean`.

The first was fixed in place and the other two were left standing, which is how
the second four hours were lost. So the predicate lives here now and the callers
import it. Fixing the instance is what kept this alive; this is the class.

The authority is still `qualification_metadata_path` in
`crates/liminal-xtask/src/haq.rs`, plus AM-17.6's single gate exception. If this
list and that function ever disagree, the verifier is right and this is wrong —
it refuses a lane early with a clearer message, and never permits one the
verifier would reject.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

GATE_FILE = "conformance/tests/phase0.rs"

METADATA_FILES = {
    "conformance/haqp/packet.json",
    "docs/execution/phase1-suite-review.md",
    "docs/execution/m17-5-adversarial-findings.md",
    GATE_FILE,
}

METADATA_PREFIXES = ("conformance/haqp/evidence/",)


def metadata_path(path: str) -> bool:
    """Whether the qualification metadata child may carry `path`."""
    return path in METADATA_FILES or path.startswith(METADATA_PREFIXES)


def source_dirt(root: Path | None = None) -> list[str]:
    """Dirty paths a lane may NOT have produced — empty means source-clean."""
    root = root or ROOT
    status = subprocess.check_output(
        ["git", "status", "--porcelain=v1"], cwd=root, text=True
    )
    dirty = [line[3:].strip() for line in status.splitlines() if line.strip()]
    return [path for path in dirty if not metadata_path(path)]


def main() -> int:
    """`python3 scripts/haqp_paths.py` — exit 0 iff the SOURCE tree is clean.

    Exists so shell lanes get the same answer as the Python ones instead of
    reimplementing it a fourth time.
    """
    trespass = source_dirt()
    for path in trespass:
        print(path)
    return 1 if trespass else 0


if __name__ == "__main__":
    sys.exit(main())
