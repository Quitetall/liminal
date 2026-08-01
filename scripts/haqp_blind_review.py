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
import re
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


SYSTEM = "You are an isolated HAQP adversarial reviewer. Return JSON only."
LOCAL_PREFIX = "local:"


def call_arguments(model: str, prompt: str, session_id: str) -> tuple[str, dict[str, Any]]:
    """Pick the lamu tool for `model` and build its arguments.

    ADR-0020 §6 wants two DIFFERENT model families, and the cloud roster has
    collapsed to one vendor that answers.  A locally-served model is a second
    family that costs nothing and cannot be revoked, so the runner has to be
    able to drive both backends.  `local:` names route to lamu's on-disk
    `query` tool; everything else is a cloud alias.
    """
    if model.startswith(LOCAL_PREFIX):
        return "query", {
            "model": model[len(LOCAL_PREFIX) :],
            "prompt": prompt,
            "system": SYSTEM,
            "max_tokens": 32000,
            "temperature": 0.1,
            "origin": "haqp-blind-review",
        }
    return "cloud_query", {
        "model": model,
        "prompt": prompt,
        "system": SYSTEM,
        "max_tokens": 32000,
        "temperature": 0.1,
        "thinking_enabled": True,
        "ephemeral": True,
        "conversation_id": session_id,
    }


def mcp_call(model: str, prompt: str, session_id: str) -> str:
    """Call lamu's stdio MCP directly; avoids stale outer MCP transports."""
    tool, arguments = call_arguments(model, prompt, session_id)
    # Each `lamu start` is an independent stdio server with its own in-memory
    # model map. A model loaded by some other MCP client is "marked loaded but
    # missing from" this one, so a local pass loads its own before querying.
    # The load result is ignored: already-loaded is success, and a genuine
    # failure surfaces on the query itself with a better message.
    calls = []
    if tool == "query":
        calls.append(("load_model", {"name": arguments["model"]}))
    calls.append((tool, arguments))
    requests = [
        {
            "jsonrpc": "2.0",
            "id": index + 1,
            "method": "tools/call",
            "params": {"name": call_tool, "arguments": call_args},
        }
        for index, (call_tool, call_args) in enumerate(calls)
    ]
    final_id = len(requests)
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
        received = item.get("id")
        if received == final_id:
            result = item
            break
        # id 0 is the initialize reply; any other is an intermediate call whose
        # completion releases the next one.
        if isinstance(received, int) and 0 <= received < final_id:
            proc.stdin.write(json.dumps(requests[received], separators=(",", ":")) + "\n")
            proc.stdin.flush()
    proc.kill()
    if result is None:
        raise RuntimeError(f"lamu returned no {tool} result for {model}")
    if "error" in result:
        raise RuntimeError(str(result["error"]))
    content = result.get("result", {}).get("content", [])
    text = "\n".join(item.get("text", "") for item in content if isinstance(item, dict))
    if not text:
        raise RuntimeError(f"lamu {tool} returned empty text for {model}")
    provider_failure(model, text)
    return text


def redact(text: str) -> str:
    """Mask anything key-shaped before it reaches an error message.

    `blocked()` persists its reason to `blocked.json`, and provider errors get
    quoted into that reason verbatim.  DeepSeek masks its own key in the
    authentication failure that motivated F-23 (`****9d40`); nothing obliges
    the next vendor to.  A review runner that writes a live credential into a
    committed-adjacent artifact would be a far worse defect than the one it
    was built to report.

    The pattern is deliberately blunt and WILL over-mask: commit SHAs, UUIDs,
    and long hex error codes all disappear behind `****`.  That is the intended
    trade — a vendor prefix allowlist (`sk-`, `AIza`, ...) only masks the keys
    someone already thought of, and the whole point is the vendor we have not
    used yet.  Do not tighten this to recover post-mortem detail; the untouched
    response is still available in memory to the caller that wants it.
    """
    return re.sub(r"[A-Za-z0-9_\-]{20,}", "****", text)


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
    head = redact(text.lstrip()[:2000])
    if head.startswith("error:"):
        raise RuntimeError(f"{model}: provider call failed: {head.splitlines()[0][:400]}")
    # Some providers return a bare error object with no `error:` prefix.
    try:
        value = json.loads(head)
    except json.JSONDecodeError:
        return
    # A real review always carries `attempts`, and a 12-attempt body is far
    # longer than the 2000-byte prefix above — so it fails the `json.loads`
    # on truncation and never reaches here. The allowlist is therefore only
    # ever consulted for short, whole, non-review payloads.
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
        "reviewer": {
            "model_family": model,
            "identity_hash": identity,
            # The evidence must say which backend answered. A locally served
            # reviewer and a cloud one are isolated in different ways, and a
            # record that hid the difference would let a reader assume the
            # stronger of the two.
            "backend": "local" if model.startswith(LOCAL_PREFIX) else "cloud",
        },
        "isolated_session_hash": digest(session_id.encode()),
        "sanitized_prompt_hash": prompt_hash,
        "attempts": parsed["attempts"],
        "findings": parsed.get("findings", []),
        "unresolved_verified_findings": parsed.get("unresolved_verified_findings"),
        "result": parsed.get("result"),
        "blindness_proof": {
            # Cloud passes ASK for an ephemeral conversation; local passes carry
            # no conversation at all, which is isolation by construction rather
            # than by request. Recording `True` for a local pass would assert a
            # cloud-side guarantee that was never requested, so the field states
            # only what was actually asked for and `session_state` says how the
            # isolation is obtained.
            "ephemeral_session_requested": not model.startswith(LOCAL_PREFIX),
            "session_state": (
                "stateless-local-call" if model.startswith(LOCAL_PREFIX) else "ephemeral-cloud-session"
            ),
            "prior_pass_artifact_supplied": False,
            "pass_two_original_spec_only": pass_two,
        },
        "fixed_base": {"commit": run("git", "rev-parse", "HEAD").strip()},
        "raw_response_sha256": digest(raw.encode()),
    }
    # Do not persist raw model output; it is untrusted and may contain secrets.
    (OUT / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


# Pass 1 is a LOCALLY served Gemma; pass 2 is MiMo in the cloud. ADR-0020 §6
# needs two distinct model families, and as of 2026-08-01 MiMo is the only
# cloud vendor that answers — DeepSeek's key is invalid and OpenRouter, which
# is the route to every other family, can afford 72 tokens. Brian's ruling:
# use the MiMo plan we already pay for, spend nothing on DeepSeek or
# OpenRouter. A local model is therefore the second family: free, on disk,
# and impossible for a provider outage to revoke.
PASSES = (
    ("pass1", os.environ.get("HAQP_BLIND_PASS1", "local:gemma-4-26b-a4b-it-q4_k_m"), False),
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
        # OSError covers a missing `lamu` binary, which is FileNotFoundError and
        # therefore NOT a subprocess.SubprocessError. This runs outside main's
        # try, so letting it escape would crash before any blocked.json existed
        # — a fail-closed runner that leaves no record is not fail-closed.
        except (RuntimeError, subprocess.SubprocessError, OSError) as exc:
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
        # A guard that raises the WRONG type still rejects, but signals a bug
        # in the guard. Record it rather than crashing, so one broken guard
        # does not hide the state of the other four.
        except Exception as exc:  # noqa: BLE001 - self-test reports, never crashes
            failures.append(f"{label}: rejected with an unexpected {type(exc).__name__}: {exc}")
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
    # Backend routing: a `local:` name sent to cloud_query would be looked up in
    # the cloud registry, fail, and be reported as an unreachable vendor — while
    # a cloud alias sent to the local tool would silently review with whatever
    # model happened to be loaded. Neither is visible in the recorded evidence.
    local_tool, local_args = call_arguments("local:some-model", "p", "s")
    if local_tool != "query" or local_args.get("model") != "some-model":
        failures.append(f"a local: model routed to {local_tool!r} as {local_args.get('model')!r}")
    cloud_tool, cloud_args = call_arguments("mimo-v2.5-pro", "p", "s")
    if cloud_tool != "cloud_query" or cloud_args.get("model") != "mimo-v2.5-pro":
        failures.append(f"a cloud model routed to {cloud_tool!r} as {cloud_args.get('model')!r}")

    # Redaction is the one guard whose failure is silent: an unmasked key would
    # still produce a correct-looking blocked.json.
    leaky = 'error: provider API: {"message":"key sk-abcdef0123456789abcdef0123456789 is invalid"}'
    try:
        provider_failure("m", leaky)
        failures.append("provider_failure accepted a leaky error string")
    except RuntimeError as exc:
        if "sk-abcdef0123456789abcdef0123456789" in str(exc):
            failures.append("provider_failure echoed an unmasked credential into its message")
    if "short-key" not in redact("short-key stays"):
        failures.append("redact() mangles ordinary prose")
    # The bare-object path redacts BEFORE json.loads, so a mask that broke JSON
    # structure would make provider_failure fall through and return quietly —
    # the error object would then reach parse_json and be misreported all over
    # again, which is exactly the failure F-23 was about.
    leaky_object = '{"error":"key sk-abcdef0123456789abcdef0123456789 is invalid"}'
    try:
        provider_failure("m", leaky_object)
        failures.append("provider_failure accepted a leaky bare error object")
    except RuntimeError as exc:
        if "sk-abcdef0123456789abcdef0123456789" in str(exc):
            failures.append("provider_failure echoed an unmasked credential from a bare error object")

    for problem in failures:
        print(f"SELF-TEST FAILED: {problem}", file=sys.stderr)
    if failures:
        return 1
    print(json.dumps({"result": "self-test-ok", "checks": 11}))
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
    # Name-level, not family-level: this catches the same model twice, which is
    # the mistake actually available to a caller setting HAQP_BLIND_PASS*. It
    # canNOT catch two siblings of one family (deepseek-v4-pro vs -flash),
    # which would need a vendor taxonomy the runner does not have. The family
    # requirement remains a human precondition of ADR-0020 §6.
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
