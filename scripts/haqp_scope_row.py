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


# 90 MiB, under GitHub's 100 MB refusal.
TRACE_STORE_CEILING_BYTES = 94_371_840


def store_trace(raw: Path, stored: Path) -> None:
    """Compress a raw trace losslessly, small enough to push, and prove it fits.

    ``--long=27`` sets a 128 MiB match window, the largest a default zstd
    decoder accepts; ``read_evidence_bytes`` uses that default, and a 2 GiB
    window is refused with "Window size larger than maximum". Measured on the
    2026-09-18 campaign: 5.43 GB raw stored at 795 MiB with ``-3`` and 26.3 MiB
    here, decompressing to identical bytes, so ``trace_blake3`` -- taken over
    the raw bytes -- does not change with the storage format.

    Reducing the trace to distinct lines would be far smaller and is NOT safe:
    ``scan_scope_trace`` maps ``(pid, fd)`` to a path as it streams, so a
    repeated ``openat(7, "x", O_WRONLY)`` after fd 7 is reopened elsewhere is a
    different access wearing identical text (blind pass 1 at f360e90, A07).

    The ceiling is checked, not assumed: ``-3`` quietly stopped clearing
    GitHub's limit as campaigns grew, and nothing failed until a push was
    refused, by which point the blob was already in history.
    """
    subprocess.check_call(
        ["zstd", "-q", "-15", "--long=27", "--rm", "-f", str(raw), "-o", str(stored)]
    )
    size = stored.stat().st_size
    if size > TRACE_STORE_CEILING_BYTES:
        raise SystemExit(
            f"stored trace {stored} is {size} bytes, over the "
            f"{TRACE_STORE_CEILING_BYTES}-byte ceiling. A larger window is not "
            "available: 128 MiB is the default decoder limit. Reduce what is "
            "traced; do not reduce the trace -- the fd map needs every line."
        )


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
        pids = [int(t["trace_pid"]) for t in audit["targets"]]
        if any(pid <= 0 for pid in pids):
            raise SystemExit(f"fuzz scope: a target has no traced pid to carve: {pids}")
        prefixes = tuple(f"{pid} ".encode() for pid in pids)
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
    store_trace(raw, stored)

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
