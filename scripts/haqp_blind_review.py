#!/usr/bin/env python3
"""Run two isolated HAQP-1 adversarial reviews.

The runner is deliberately fail-closed.  It never changes the packet and never
marks a review qualified.  A dirty tree, an unexpected packet state, a missing
model response, or a response without exactly twelve concrete attempts writes a
blocked record and exits non-zero.  Generated records belong under target/ and
are not release evidence until a clean rerun is reviewed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "haqp" / "blind-review"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.STDOUT)


def base_context() -> tuple[str, bool, str]:
    commit = run("git", "rev-parse", "HEAD").strip()
    status = run("git", "status", "--porcelain=v1").strip()
    clean = not status
    tree = run("git", "rev-parse", "HEAD^{tree}").strip()
    files = [
        "docs/adr/0020-require-high-assurance-phase-1-suite-qualification.md",
        "docs/execution/phase1-suite-review.md",
        "conformance/haqp/packet.json",
        "crates/liminal-xtask/src/haq.rs",
        "crates/liminal-cst/src/lib.rs",
        "crates/liminal-format/src/lib.rs",
        "crates/liminal-query/src/lib.rs",
    ]
    chunks = [f"fixed_commit={commit}\nfixed_tree={tree}\nclean={clean}\n"]
    for rel in files:
        path = ROOT / rel
        if path.exists():
            chunks.append(f"\n--- {rel} ---\n{path.read_text(encoding='utf-8')}")
    return "".join(chunks), clean, commit


def packet_state() -> dict[str, Any]:
    return json.loads((ROOT / "conformance/haqp/packet.json").read_text())


def blocked(reason: str, *, commit: str, clean: bool) -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    record = {
        "schema_version": "haqp-blind-review-v1",
        "result": "blocked",
        "reason": reason,
        "fixed_base": {"commit": commit, "clean": clean},
        "packet": packet_state(),
        "qualification_claim": False,
        "created_unix": int(time.time()),
    }
    (OUT / "blocked.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"result": "blocked", "reason": reason, "path": str(OUT / "blocked.json")}))
    return 2


def mcp_call(model: str, prompt: str, session_id: str) -> str:
    """Call lamu's stdio MCP directly; avoids stale outer MCP transports."""
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "cloud_query",
            "arguments": {
                "model": model,
                "prompt": prompt,
                "system": "You are an isolated HAQP adversarial reviewer. Return JSON only.",
                "max_tokens": 32000,
                "temperature": 0.1,
                "thinking_enabled": True,
                "ephemeral": True,
                "conversation_id": session_id,
            },
        },
    }
    init = {
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "haqp-blind-review", "version": "1"},
        },
    }
    proc = subprocess.Popen(
        ["lamu", "start"],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdin is not None and proc.stdout is not None
    proc.stdin.write(json.dumps(init, separators=(",", ":")) + "\n")
    proc.stdin.flush()
    result: dict[str, Any] | None = None
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        line = proc.stdout.readline()
        if not line:
            break
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if item.get("id") == 0:
            proc.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
            proc.stdin.flush()
            continue
        if item.get("id") == 1:
            result = item
            break
    proc.kill()
    if result is None:
        raise RuntimeError("lamu returned no cloud_query result")
    if "error" in result:
        raise RuntimeError(str(result["error"]))
    content = result.get("result", {}).get("content", [])
    text = "\n".join(item.get("text", "") for item in content if isinstance(item, dict))
    if not text:
        raise RuntimeError("lamu cloud_query returned empty text")
    return text


def parse_json(text: str) -> dict[str, Any]:
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        start, end = text.find("{"), text.rfind("}")
        if start < 0 or end <= start:
            raise ValueError("reviewer response is not JSON")
        value = json.loads(text[start : end + 1])
    if not isinstance(value, dict) or not isinstance(value.get("attempts"), list):
        raise ValueError("reviewer JSON lacks attempts array")
    if len(value["attempts"]) != 12:
        raise ValueError(f"reviewer returned {len(value['attempts'])} attempts, expected 12")
    ids: set[str] = set()
    caught = 0
    required = {
        "id",
        "attack_class",
        "target",
        "attempt",
        "observed_result",
        "independently_reproduced",
        "classification",
        "resolved",
    }
    allowed = {"verified_defect", "false_positive", "caught_violation"}
    for attempt in value["attempts"]:
        if not isinstance(attempt, dict) or not required.issubset(attempt):
            raise ValueError("every attempt needs complete falsification fields")
        identifier = str(attempt["id"])
        if not identifier or identifier in ids:
            raise ValueError("attempt ids must be non-empty and unique")
        ids.add(identifier)
        if any(not str(attempt[field]).strip() for field in required - {"independently_reproduced"}):
            raise ValueError(f"attempt {identifier} contains an empty field")
        if attempt["classification"] not in allowed:
            raise ValueError(f"attempt {identifier} has unknown classification")
        if attempt["classification"] == "caught_violation":
            caught += 1
    if caught == 0:
        raise ValueError("review must record at least one caught violation")
    if not isinstance(value.get("findings", []), list):
        raise ValueError("findings must be an array")
    if not isinstance(value.get("unresolved_verified_findings"), int):
        raise ValueError("unresolved_verified_findings must be an integer")
    if not isinstance(value.get("result"), str) or not value["result"].strip():
        raise ValueError("review result missing")
    return value


def run_pass(name: str, model: str, context: str, *, pass_two: bool) -> dict[str, Any]:
    session_id = f"haqp-blind-{name}-{int(time.time())}"
    prompt = (
        "Conduct one isolated HAQP-1 falsification pass. Do not infer passing evidence. "
        "Record exactly twelve concrete attempts. Each attempt object must contain id, "
        "attack_class, target, attempt, observed_result, independently_reproduced, "
        "classification (verified_defect|false_positive|caught_violation), and resolved. "
        "Return JSON object with attempts array, findings array, unresolved_verified_findings integer, and result. "
        "No markdown, no credentials, no secrets.\n\n"
        + (
            "Start from original spec; prior-pass records are unavailable.\n"
            if pass_two
            else "Attack implementation and packet directly.\n"
        )
        + context
    )
    identity = digest(f"{name}:{model}:haqp-blind-review-v1".encode())
    prompt_hash = digest(prompt.encode())
    raw = mcp_call(model, prompt, session_id)
    parsed = parse_json(raw)
    record = {
        "schema_version": "haqp-blind-review-v1",
        "pass": 2 if pass_two else 1,
        "reviewer": {"model_family": model, "identity_hash": identity},
        "isolated_session_hash": digest(session_id.encode()),
        "sanitized_prompt_hash": prompt_hash,
        "attempts": parsed["attempts"],
        "findings": parsed.get("findings", []),
        "unresolved_verified_findings": parsed.get("unresolved_verified_findings"),
        "result": parsed.get("result"),
        "blindness_proof": {
            "ephemeral_session": True,
            "prior_pass_artifact_supplied": False,
            "pass_two_original_spec_only": pass_two,
        },
        "fixed_base": {"commit": run("git", "rev-parse", "HEAD").strip()},
        "raw_response_sha256": digest(raw.encode()),
    }
    # Do not persist raw model output; it is untrusted and may contain secrets.
    (OUT / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", action="store_true", help="run model calls; default records preflight only")
    args = parser.parse_args()
    context, clean, commit = base_context()
    packet = packet_state()
    if not clean:
        return blocked("fixed review base is dirty", commit=commit, clean=False)
    if packet.get("status") != "proposed" or packet.get("qualification_state") != "not-run":
        return blocked("packet is not proposed/not-run", commit=commit, clean=True)
    if not args.run:
        return blocked("preflight only; --run required", commit=commit, clean=True)
    OUT.mkdir(parents=True, exist_ok=True)
    try:
        first = run_pass("pass1-deepseek-v4-pro", "deepseek-v4-pro", context, pass_two=False)
        second = run_pass("pass2-mimo-v2.5-pro", "mimo-v2.5-pro", context, pass_two=True)
    except (RuntimeError, ValueError, subprocess.SubprocessError) as exc:
        return blocked(f"review execution failed: {exc}", commit=commit, clean=True)
    manifest = {
        "schema_version": "haqp-blind-review-manifest-v1",
        "passes": [first, second],
        "qualification_claim": False,
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"result": "recorded", "passes": 2, "attempts": 24, "qualification_claim": False}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
