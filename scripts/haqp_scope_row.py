#!/usr/bin/env python3
"""Emit one corpus-scope row for a stage the lane just ran under strace.

Every digest is asked of the xtask, so the row is judged later by the same code
that wrote it (the packet-digest lesson). Usage:

  haqp_scope_row.py <scope> <declared-command> <raw-trace> <exit-code> >> rows.ndjson

Stores the trace compressed at conformance/haqp/evidence/access/scopes/<scope>.trace.zst
and removes the raw file. The digest is over the RAW bytes.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ACCESS = ROOT / "conformance/haqp/evidence/access"


def xtask(*args: str) -> str:
    return subprocess.check_output(
        ["cargo", "run", "-q", "-p", "liminal-xtask", "--", "haq", *args], cwd=ROOT, text=True
    ).strip()


def blake3_text(text: str) -> str:
    with tempfile.NamedTemporaryFile("w", delete=False) as tmp:
        tmp.write(text)
        name = tmp.name
    try:
        return xtask("hash", name)
    finally:
        Path(name).unlink()


def main() -> int:
    scope, declared, raw, exit_code = sys.argv[1], sys.argv[2], Path(sys.argv[3]), int(sys.argv[4])
    command = f"strace -f -q -e trace=%file -o target/haqp/scope-{scope}.trace {declared}"
    parts: list[dict[str, str]] = []
    if scope == "fuzz":
        # F-43: the campaign carved each fuzz binary's lines into its per-target
        # trace; drop those pids' lines here and store the remainder. The row
        # names the parts, and every digest is over the union.
        audit = json.loads((ACCESS.parent / "corpus-access.json").read_text())
        parts = [{"trace": t["trace"], "trace_blake3": t["trace_blake3"]} for t in audit["targets"]]
        prefixes = tuple(f"{int(t['trace_pid'])} ".encode() for t in audit["targets"])
        remainder = raw.with_name(raw.name + ".remainder")
        with raw.open("rb") as src, remainder.open("wb") as dst:
            for line in src:
                if not line.startswith(prefixes):
                    dst.write(line)
        remainder.replace(raw)
    # The traced root process is the last to report exit; children exit first.
    # Streamed: a stage trace is a gigabyte and more.
    exit_line = re.compile(rb"^(\d+) \+\+\+ exited with (\d+) \+\+\+$")
    pid, traced_exit = 0, 1
    with raw.open("rb") as src:
        for line in src:
            match = exit_line.match(line.rstrip(b"\r\n"))
            if match:
                pid, traced_exit = int(match.group(1)), int(match.group(2))

    trace_blake3 = xtask("hash", str(raw))
    part_files = [str(ROOT / part["trace"]) for part in parts]
    observed = xtask("scope-digest", "paths", scope, str(raw), *part_files)
    resolved = xtask("scope-digest", "resolved", scope, str(raw), *part_files)

    (ACCESS / "scopes").mkdir(parents=True, exist_ok=True)
    stored = ACCESS / "scopes" / f"{scope}.trace.zst"
    subprocess.check_call(["zstd", "-q", "-3", "--rm", "-f", str(raw), "-o", str(stored)])

    receipt = ACCESS / "strace.version"
    row = {
        "scope": scope,
        "command": command,
        "trace": str(stored.relative_to(ROOT)),
        "trace_blake3": trace_blake3,
        "exit_code": exit_code,
        "result": "pass" if exit_code == 0 else "fail",
        "trace_pid": pid,
        "trace_exit_code": traced_exit,
        "trace_complete": traced_exit == 0,
        "process_binding": blake3_text(f"{command}\0{pid}\0{traced_exit}\0{trace_blake3}\0{observed}"),
        "tracer_binary": "strace",
        "tracer_version": str(receipt.relative_to(ROOT)),
        "tracer_version_blake3": xtask("hash", str(receipt)) if receipt.exists() else "",
        "observed_paths_blake3": observed,
        "trace_root": str(ROOT),
        "resolved_paths_blake3": resolved,
        "parts": parts,
    }
    print(json.dumps(row))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
