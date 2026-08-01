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
    provider_failure(model, text)
    return text


def provider_failure(model: str, text: str) -> None:
    """Raise if `text` is a transport/provider error rather than a review.

    lamu reports provider failures — dead API key, exhausted credits, model
    unavailable — as ordinary MCP *content*, not as an MCP-level `error`.  The
    runner therefore used to hand an authentication failure straight to
    `parse_json`, which rejected it with "reviewer JSON lacks attempts array".
    That reason is true and useless: it accuses the model of a bad response
    when nothing was ever asked of it.  M17.5 burned two full runs chasing
    `max_tokens` on the strength of it (F-23).

    Fail-closed is correct here — it stays fail-closed.  What changes is that
    the recorded reason now names the actual cause.
    """
    head = text.lstrip()[:2000]
    if head.startswith("error:"):
        raise RuntimeError(f"{model}: provider call failed: {head.splitlines()[0][:400]}")
    # Some providers return a bare error object with no `error:` prefix.
    try:
        value = json.loads(head)
    except json.JSONDecodeError:
        return
    if isinstance(value, dict) and "attempts" not in value:
        for field in ("error", "code", "type"):
            if field in value:
                raise RuntimeError(f"{model}: provider returned an error object: {head[:400]}")


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


PASSES = (
    ("pass1", os.environ.get("HAQP_BLIND_PASS1", "deepseek-v4-pro"), False),
    ("pass2", os.environ.get("HAQP_BLIND_PASS2", "mimo-v2.5-pro"), True),
)


def vendor_liveness(models: list[str]) -> str | None:
    """Ping each reviewer with a trivial prompt; return the first failure.

    Sending the ~200 KB review context to a vendor whose key is dead costs a
    quarter-hour and produces a misleading record.  A one-token ping costs
    three seconds and names the vendor.
    """
    for model in models:
        try:
            mcp_call(model, "Reply with the single word OK.", f"haqp-liveness-{model}")
        except RuntimeError as exc:
            return f"reviewer {model} is unreachable: {exc}"
    return None


def self_test() -> int:
    """Prove the guards reject the degenerate inputs they exist to reject.

    A fail-closed runner whose guards silently stopped rejecting would report
    a clean review over garbage, so each guard is paired with the case that
    must trip it.
    """
    failures: list[str] = []

    def rejects(label: str, fn: Any, *args: Any) -> None:
        try:
            fn(*args)
        except (RuntimeError, ValueError):
            return
        failures.append(label)

    rejects(
        "provider_failure accepted a lamu error string",
        provider_failure,
        "m",
        'error: provider API: {"code":"invalid_request_error","message":"Authentication Fails"}',
    )
    rejects(
        "provider_failure accepted a bare provider error object",
        provider_failure,
        "m",
        '{"code":402,"message":"This request requires more credits"}',
    )
    # ...and must NOT reject a real review, or every pass blocks forever.
    good = {
        "attempts": [
            {
                "id": f"A{i}",
                "attack_class": "c",
                "target": "t",
                "attempt": "a",
                "observed_result": "o",
                "independently_reproduced": True,
                "classification": "caught_violation",
                "resolved": "yes",
            }
            for i in range(12)
        ],
        "findings": [],
        "unresolved_verified_findings": 0,
        "result": "pass",
    }
    body = json.dumps(good)
    try:
        provider_failure("m", body)
        parse_json(body)
    except (RuntimeError, ValueError) as exc:
        failures.append(f"a well-formed review was rejected: {exc}")
    rejects("parse_json accepted a short attempts array", parse_json, json.dumps({**good, "attempts": good["attempts"][:11]}))
    rejects(
        "parse_json accepted a review with no caught violation",
        parse_json,
        json.dumps({**good, "attempts": [{**a, "classification": "false_positive"} for a in good["attempts"]]}),
    )
    for problem in failures:
        print(f"SELF-TEST FAILED: {problem}", file=sys.stderr)
    if failures:
        return 1
    print(json.dumps({"result": "self-test-ok", "guards": 5}))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", action="store_true", help="run model calls; default records preflight only")
    parser.add_argument("--self-test", action="store_true", help="exercise the guards; no model calls")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    context, clean, commit = base_context()
    packet = packet_state()
    if not clean:
        return blocked("fixed review base is dirty", commit=commit, clean=False)
    if packet.get("status") != "proposed" or packet.get("qualification_state") != "not-run":
        return blocked("packet is not proposed/not-run", commit=commit, clean=True)
    if not args.run:
        return blocked("preflight only; --run required", commit=commit, clean=True)
    OUT.mkdir(parents=True, exist_ok=True)
    models = [model for _, model, _ in PASSES]
    if len(set(models)) != len(models):
        return blocked(
            f"ADR-0020 §6 requires distinct model families; both passes name {models[0]}",
            commit=commit,
            clean=True,
        )
    dead = vendor_liveness(models)
    if dead is not None:
        return blocked(dead, commit=commit, clean=True)
    try:
        records = [
            run_pass(f"{name}-{model}", model, context, pass_two=pass_two) for name, model, pass_two in PASSES
        ]
        first, second = records
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
