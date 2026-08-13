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
import select
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "haqp" / "blind-review"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.STDOUT)


def base_context() -> tuple[str, bool, dict[str, Any]]:
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
    fixed_base = {"commit": commit, "tree": tree, "clean": clean}
    chunks = [f"fixed_commit={commit}\nfixed_tree={tree}\nclean={clean}\n"]
    for rel in files:
        path = ROOT / rel
        if path.exists():
            chunks.append(f"\n--- {rel} ---\n{path.read_text(encoding='utf-8')}")
    return "".join(chunks), clean, fixed_base


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
CODEX_PREFIX = "codex:"
MIMO_DIRECT_PREFIX = "mimo-direct:"


def backend_of(model: str) -> str:
    """Classify provider route used by isolated review process."""
    if model.startswith(MIMO_DIRECT_PREFIX):
        return "mimo-direct"
    return "codex" if model.startswith(CODEX_PREFIX) else "lamu"


def codex_call(model: str, prompt: str) -> str:
    """Run one non-interactive Codex session and return its final message.

    ADR-0020 §6 wants two DIFFERENT model families.  Of lamu's cloud roster only
    MiMo still answers, so the second family comes from the Codex CLI, which
    authenticates against a ChatGPT account rather than an API key and is
    therefore independent of every dead key in `api-keys.env`.

    `--output-last-message` is used rather than parsing stdout: Codex interleaves
    hook lines, tool traces and a token summary with the model's answer, and a
    runner that scraped that stream would eventually mistake a trace line for a
    review.

    The session runs `--sandbox read-only` in a throwaway directory, never
    `--cd` into the repo (`codex exec` is non-interactive, so nothing can prompt
    for an approval it would then wait on).  The review context is supplied
    entirely in the prompt, so both passes see byte-identical material; a
    reviewer that could also wander the working tree would not be reviewing the
    same fixed base as its counterpart.
    """
    with tempfile.TemporaryDirectory(prefix="haqp-codex-") as scratch:
        last = Path(scratch) / "last-message.txt"
        name = model[len(CODEX_PREFIX) :]
        # An empty `codex:` alias would omit --model and silently review with
        # whatever CODEX_HOME/config.toml defaults to, while the record hashes
        # the literal string "codex:" as the reviewer identity — valid-looking
        # evidence for a reviewer that never existed as named.
        if not name:
            raise RuntimeError(f"{model!r} names no model; use codex:<model>")
        argv = [
            "codex",
            "exec",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            # Codex persists session transcripts by default. This machine's
            # config appears not to, but the runner must not depend on an
            # unstated default to keep an untrusted model response off disk.
            "--ephemeral",
            "--color",
            "never",
            "--output-last-message",
            str(last),
            "--model",
            name,
            "-",
        ]
        completed = subprocess.run(
            argv,
            input=f"{SYSTEM}\n\n{prompt}",
            capture_output=True,
            text=True,
            cwd=scratch,
            timeout=3600,
            check=False,
        )
        if not last.exists():
            tail = redact((completed.stderr or completed.stdout or "").strip()[-400:])
            raise RuntimeError(f"{model}: codex wrote no final message (exit {completed.returncode}): {tail}")
        return last.read_text()


def mimo_direct_call(model: str, prompt: str, *, liveness: bool = False) -> str:
    """Run MiMo through Codex's direct token-plan provider profile.

    LAMU's MCP transport can stall after liveness while the direct provider
    remains healthy. Keep this route isolated, read-only, ephemeral, and
    secret-silent; the API key is sourced inside the child shell and never
    appears in captured output.
    """
    with tempfile.TemporaryDirectory(prefix="haqp-mimo-") as scratch:
        last = Path(scratch) / "last-message.txt"
        name = model[len(MIMO_DIRECT_PREFIX) :]
        argv = [
            "codex",
            "exec",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ephemeral",
            "--color",
            "never",
            "--output-last-message",
            str(last),
            "--model",
            name,
            "-c",
            "model_provider='mimo'",
            "-c",
            "model_providers.mimo.name='mimo'",
            "-c",
            "model_providers.mimo.base_url='https://token-plan-sgp.xiaomimimo.com/v1'",
            "-c",
            "model_providers.mimo.env_key='MIMO_API_KEY'",
            "-c",
            "model_providers.mimo.wire_api='responses'",
            "-c",
            "approval_policy='never'",
            "-c",
            "web_search='disabled'",
            "-",
        ]
        command = [
            "bash",
            "-lc",
            'source /home/brianklam/.config/lamu/api-keys.env; exec "$@"',
            "--",
            *argv,
        ]
        completed = subprocess.run(
            command,
            input=f"{SYSTEM}\n\n{prompt}",
            capture_output=True,
            text=True,
            cwd=scratch,
            timeout=60 if liveness else 900,
            check=False,
        )
        if not last.exists():
            tail = redact((completed.stderr or completed.stdout or "").strip()[-400:])
            raise RuntimeError(
                f"{model}: direct Mimo wrote no final message (exit {completed.returncode}): {tail}"
            )
        return last.read_text()


def call_arguments(
    model: str, prompt: str, session_id: str, *, liveness: bool = False
) -> tuple[str, dict[str, Any]]:
    """Build the lamu `cloud_query` arguments for `model`."""
    return "cloud_query", {
        "model": model,
        "prompt": prompt,
        "system": SYSTEM,
        "max_tokens": 8 if liveness else 32000,
        "temperature": 0.0 if liveness else 0.1,
        "thinking_enabled": not liveness,
        "ephemeral": True,
        "conversation_id": session_id,
    }


def mcp_call(model: str, prompt: str, session_id: str, *, liveness: bool = False) -> str:
    """Send `prompt` to `model` on whichever backend can reach it."""
    if backend_of(model) == "codex":
        text = codex_call(model, prompt)
        provider_failure(model, text)
        return text
    if backend_of(model) == "mimo-direct":
        text = mimo_direct_call(model, prompt, liveness=liveness)
        provider_failure(model, text)
        return text
    tool, arguments = call_arguments(model, prompt, session_id, liveness=liveness)
    calls = [(tool, arguments)]
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
        start_new_session=True,
    )
    assert proc.stdin is not None and proc.stdout is not None
    proc.stdin.write(json.dumps(init, separators=(",", ":")) + "\n")
    proc.stdin.flush()
    result: dict[str, Any] | None = None
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        # `readline()` blocks until the provider emits a newline, so checking
        # the deadline only after calling it is ineffective when a cloud
        # request stalls. Poll the pipe first; this keeps provider hangs on the
        # explicit fail-closed timeout path instead of leaving the runner
        # immortal.
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        ready, _, _ = select.select([proc.stdout], [], [], min(1.0, remaining))
        if not ready:
            continue
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
    if proc.poll() is None:
        # `lamu` is a launcher; killing only its parent can leave the MCP
        # child alive and make `wait()` raise after a valid response. Kill the
        # isolated process group so cleanup cannot rewrite success as provider
        # failure or leak a server into later blind passes.
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)
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


def redact_deep(value: Any) -> Any:
    """Apply `redact` to every string inside a parsed review.

    `run_pass` persists the reviewer's `attempts` and `findings` verbatim, and
    those are model output — the same untrusted text the raw response is
    deliberately never written for.  "No credentials" in the prompt is a
    request, not a control: a reviewer that quoted a key out of its context
    would put it in an attempt's `observed_result`, and the runner would file it
    under `target/`.
    """
    if isinstance(value, str):
        return redact(value)
    if isinstance(value, list):
        return [redact_deep(item) for item in value]
    if isinstance(value, dict):
        return {key: redact_deep(item) for key, item in value.items()}
    return value


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
        if not isinstance(attempt["independently_reproduced"], bool):
            raise ValueError(f"attempt {identifier} independently_reproduced must be boolean")
        if not isinstance(attempt["resolved"], bool):
            raise ValueError(f"attempt {identifier} resolved must be boolean")
        target = str(attempt["target"])
        target_file, separator, target_line = target.rpartition(":")
        if (
            not separator
            or not target_file.strip()
            or not target_line.isdigit()
            or int(target_line) < 1
            or target_file.startswith("/")
            or ".." in target_file.split("/")
        ):
            raise ValueError(f"attempt {identifier} target must be safe file:line")
        if attempt["classification"] not in allowed:
            raise ValueError(f"attempt {identifier} has unknown classification")
        if attempt["classification"] == "verified_defect" and attempt["resolved"]:
            resolution = attempt.get("resolution")
            if not isinstance(resolution, dict) or any(
                not isinstance(resolution.get(field), str) or not resolution[field].strip()
                for field in ("commit", "coordinate", "evidence_path", "evidence_sha256")
            ):
                raise ValueError(f"resolved verified attempt {identifier} needs resolution proof")
        if attempt["classification"] == "caught_violation":
            caught += 1
    if caught == 0:
        raise ValueError("review must record at least one caught violation")
    findings = value.get("findings", [])
    if not isinstance(findings, list):
        raise ValueError("findings must be an array")
    finding_ids: set[str] = set()
    verified_attempts = {
        str(attempt["id"])
        for attempt in value["attempts"]
        if attempt["classification"] == "verified_defect"
    }
    for finding in findings:
        if not isinstance(finding, dict):
            raise ValueError("every finding must be an object linked to an attempt")
        finding_id = finding.get("id")
        attempt_id = finding.get("attempt_id")
        if not isinstance(finding_id, str) or not finding_id.strip():
            raise ValueError("every finding needs a non-empty id")
        if finding_id in finding_ids:
            raise ValueError("finding ids must be non-empty and unique")
        finding_ids.add(finding_id)
        if attempt_id not in verified_attempts:
            raise ValueError(f"finding {finding_id} must link to a verified_defect attempt")
    reproduced = value.get("independently_reproduced")
    if not isinstance(reproduced, list) or any(not isinstance(item, str) or not item.strip() for item in reproduced):
        raise ValueError("independently_reproduced must be an array of non-empty finding ids")
    if len(set(reproduced)) != len(reproduced):
        raise ValueError("independently_reproduced finding ids must be unique")
    unknown_reproduced = set(reproduced) - finding_ids
    if unknown_reproduced:
        raise ValueError("independently_reproduced names findings absent from this record")
    if not isinstance(value.get("unresolved_verified_findings"), int):
        raise ValueError("unresolved_verified_findings must be an integer")
    if value["unresolved_verified_findings"] < 0 or value["unresolved_verified_findings"] > len(reproduced):
        raise ValueError("unresolved_verified_findings exceeds independently reproduced findings")
    if not isinstance(value.get("result"), str) or not value["result"].strip():
        raise ValueError("review result missing")
    return value


class ProviderFailure(RuntimeError):
    """A reviewer backend did not return a review response."""


class ReviewSchemaFailure(ValueError):
    """A reviewer returned text that cannot serve as a complete review record."""


def bound_record_hash(*, prompt_hash: str, fixed_base: dict[str, Any], parsed: dict[str, Any], raw: str) -> str:
    """Hash every persisted review claim to the exact prompt and fixed checkout."""
    bound = {
        "fixed_base": fixed_base,
        "prompt_sha256": prompt_hash,
        # Bind the redacted values actually persisted, not the untrusted source
        # values that are intentionally discarded after this function returns.
        "attempts": redact_deep(parsed["attempts"]),
        "findings": redact_deep(parsed["findings"]),
        "independently_reproduced": redact_deep(parsed["independently_reproduced"]),
        "unresolved_verified_findings": parsed["unresolved_verified_findings"],
        "result": parsed["result"],
        "raw_response_sha256": digest(raw.encode()),
    }
    return digest(json.dumps(bound, sort_keys=True, separators=(",", ":")).encode())


def run_pass(
    name: str, model: str, context: str, fixed_base: dict[str, Any], *, pass_two: bool
) -> dict[str, Any]:
    session_id = f"haqp-blind-{name}-{int(time.time())}"
    prompt = (
        "Conduct one isolated HAQP-1 falsification pass. Do not infer passing evidence. "
        "Record exactly twelve concrete attempts. Each attempt object must contain id, "
        "attack_class, target, attempt, observed_result, independently_reproduced, "
        "classification (verified_defect|false_positive|caught_violation), and resolved. "
        "Every target must be an exact safe repository file:line coordinate, never a symbol-only target. "
        "Each finding must be an object with unique id and attempt_id referencing a verified_defect attempt. "
        "A resolved verified_defect attempt must also carry resolution={commit,coordinate,evidence_path,evidence_sha256}; "
        "leave resolved=false when no fix proof exists. "
        "Return JSON object with attempts array, findings array, independently_reproduced array of finding ids, "
        "unresolved_verified_findings integer, and result. Set unresolved_verified_findings to the count "
        "of independently reproduced findings whose linked attempt has resolved=false; set it to 0 when "
        "none remain unresolved. It must never exceed the length of independently_reproduced. "
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
    try:
        raw = mcp_call(model, prompt, session_id)
    except (RuntimeError, subprocess.SubprocessError, OSError) as exc:
        raise ProviderFailure(f"pass {2 if pass_two else 1} provider failure for {model}: {exc}") from exc
    try:
        parsed = parse_json(raw)
    except (ValueError, TypeError) as exc:
        raise ReviewSchemaFailure(f"pass {2 if pass_two else 1} schema failure for {model}: {exc}") from exc
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
            "backend": backend_of(model),
        },
        "isolated_session_hash": digest(session_id.encode()),
        "sanitized_prompt_hash": prompt_hash,
        "attempts": redact_deep(parsed["attempts"]),
        "findings": redact_deep(parsed.get("findings", [])),
        "independently_reproduced": redact_deep(parsed["independently_reproduced"]),
        "unresolved_verified_findings": parsed.get("unresolved_verified_findings"),
        "result": redact_deep(parsed.get("result")),
        "blindness_proof": {
            # Cloud passes ASK for an ephemeral conversation; local passes carry
            # no conversation at all, which is isolation by construction rather
            # than by request. Recording `True` for a local pass would assert a
            # cloud-side guarantee that was never requested, so the field states
            # only what was actually asked for and `session_state` says how the
            # isolation is obtained.
            "ephemeral_session_requested": backend_of(model) in {"lamu", "mimo-direct"},
            "session_state": (
                "fresh-codex-session" if backend_of(model) == "codex" else "ephemeral-cloud-session"
            ),
            "prior_pass_artifact_supplied": False,
            "pass_two_original_spec_only": pass_two,
        },
        "fixed_base": fixed_base,
        "raw_response_sha256": digest(raw.encode()),
    }
    record["integrity_binding_sha256"] = bound_record_hash(
        prompt_hash=prompt_hash, fixed_base=fixed_base, parsed=parsed, raw=raw
    )
    # Do not persist raw model output; it is untrusted and may contain secrets.
    (OUT / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


# Pass 1 is OpenAI via the Codex CLI; pass 2 is MiMo through direct token-plan
# routing. ADR-0020 §6 needs
# two distinct model families, and as of 2026-08-01 MiMo is the only vendor in
# lamu's cloud roster that answers — DeepSeek's key is invalid and OpenRouter,
# the route to every other family, can afford 72 tokens.
#
# Brian's rulings: use the MiMo plan already paid for, spend nothing on DeepSeek
# or OpenRouter, and review in the CLOUD rather than locally. Codex satisfies all
# three — it authenticates against a ChatGPT account, so it is independent of
# every key in `api-keys.env` without adding a bill.
PASSES = (
    ("pass1", os.environ.get("HAQP_BLIND_PASS1", "codex:gpt-5.6-sol"), False),
    ("pass2", os.environ.get("HAQP_BLIND_PASS2", "mimo-direct:mimo-v2.5-pro"), True),
)


def vendor_liveness(models: list[str]) -> str | None:
    """Ping each reviewer with a trivial prompt; return the first failure.

    Sending the ~200 KB review context to a vendor whose key is dead costs a
    quarter-hour and produces a misleading record.  A one-token ping costs
    three seconds and names the vendor.
    """
    for model in models:
        try:
            mcp_call(
                model,
                "Reply with the single word OK.",
                f"haqp-liveness-{model}",
                liveness=True,
            )
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
                "target": "haq.rs:1",
                "attempt": "a",
                "observed_result": "o",
                "independently_reproduced": True,
                "classification": "caught_violation",
                "resolved": False,
            }
            for i in range(12)
        ],
        "findings": [],
        "independently_reproduced": [],
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
        "parse_json accepted a symbol-only attempt target",
        parse_json,
        json.dumps({
            **good,
            "attempts": [{**good["attempts"][0], "target": "haq.rs:verify_packet_shape"}, *good["attempts"][1:]],
        }),
    )
    rejects(
        "parse_json accepted a review with no caught violation",
        parse_json,
        json.dumps({**good, "attempts": [{**a, "classification": "false_positive"} for a in good["attempts"]]}),
    )
    rejects(
        "parse_json accepted a non-boolean attempt reproduction claim",
        parse_json,
        json.dumps({**good, "attempts": [{**good["attempts"][0], "independently_reproduced": "yes"}, *good["attempts"][1:]]}),
    )
    rejects(
        "parse_json accepted an unlinked finding",
        parse_json,
        json.dumps({
            **good,
            "findings": [{"id": "F1", "attempt_id": "A0"}],
        }),
    )
    linked = {
        **good,
        "attempts": [{**a, "classification": "verified_defect"} if a["id"] == "A0" else a for a in good["attempts"]],
        "findings": [{"id": "F1", "attempt_id": "A0"}],
        "independently_reproduced": ["F1"],
        "unresolved_verified_findings": 1,
    }
    try:
        parse_json(json.dumps(linked))
    except ValueError as exc:
        failures.append(f"a linked finding/reproduction record was rejected: {exc}")
    rejects(
        "parse_json accepted reproduction of an absent finding",
        parse_json,
        json.dumps({**linked, "independently_reproduced": ["F2"]}),
    )
    rejects(
        "parse_json accepted more unresolved findings than reproductions",
        parse_json,
        json.dumps({**linked, "independently_reproduced": [], "unresolved_verified_findings": 1}),
    )
    fixed = {"commit": "a" * 40, "tree": "b" * 40, "clean": True}
    bound = bound_record_hash(prompt_hash="c" * 64, fixed_base=fixed, parsed=linked, raw="review")
    if bound == bound_record_hash(
        prompt_hash="d" * 64, fixed_base=fixed, parsed=linked, raw="review"
    ):
        failures.append("record binding does not include the prompt hash")
    if bound == bound_record_hash(
        prompt_hash="c" * 64, fixed_base={**fixed, "tree": "e" * 40}, parsed=linked, raw="review"
    ):
        failures.append("record binding does not include the fixed base")
    # Pass 2 failures must name the failing boundary. In particular, a provider
    # error is not a malformed review, and a malformed review is not a vendor
    # outage; both paths remain fail-closed rather than continuing to manifest.
    original_mcp_call = mcp_call
    try:
        globals()["mcp_call"] = lambda *_args: (_ for _ in ()).throw(RuntimeError("vendor unavailable"))
        try:
            run_pass("pass2-self-test", "m", "context", fixed, pass_two=True)
            failures.append("pass 2 provider failure was accepted")
        except ProviderFailure as exc:
            if "pass 2 provider failure" not in str(exc):
                failures.append("pass 2 provider failure was not explicit")
        globals()["mcp_call"] = lambda *_args: "{}"
        try:
            run_pass("pass2-self-test", "m", "context", fixed, pass_two=True)
            failures.append("pass 2 schema failure was accepted")
        except ReviewSchemaFailure as exc:
            if "pass 2 schema failure" not in str(exc):
                failures.append("pass 2 schema failure was not explicit")
    finally:
        globals()["mcp_call"] = original_mcp_call
    # Backend routing. A misroute is invisible in the recorded evidence: a
    # `codex:` name handed to lamu is looked up in the cloud registry and
    # reported as an unreachable vendor, while a lamu alias handed to Codex is
    # reviewed by whatever model Codex defaults to. Both would produce a record
    # naming a reviewer that never ran.
    if backend_of("codex:gpt-5.6-sol") != "codex":
        failures.append("a codex: model did not route to the Codex backend")
    if backend_of("mimo-direct:mimo-v2.5-pro") != "mimo-direct":
        failures.append("the direct MiMo alias did not route to the direct provider backend")
    cloud_tool, cloud_args = call_arguments("mimo-v2.5-pro", "p", "s")
    if cloud_tool != "cloud_query" or cloud_args.get("model") != "mimo-v2.5-pro":
        failures.append(f"a lamu alias built {cloud_tool!r} arguments for {cloud_args.get('model')!r}")
    # The two passes must not land on one backend: §6's independence is a claim
    # about families, and two Codex sessions or two lamu models would satisfy the
    # name-distinctness check while sharing a vendor.
    if len({backend_of(model) for _, model, _ in PASSES}) != 2:
        failures.append(f"both passes route to one backend: {[model for _, model, _ in PASSES]}")

    # An empty `codex:` alias must not reach the CLI: it would omit --model,
    # review with an unrecorded default, and file the result under the literal
    # reviewer name "codex:".
    rejects("codex_call accepted an empty model alias", codex_call, "codex:", "p")

    # Persisted review text is model output. A reviewer that quoted a key out of
    # its context would put it in an attempt, and the record would carry it.
    leaked = redact_deep({"attempts": [{"observed_result": "saw sk-abcdef0123456789abcdef0123456789"}]})
    if "sk-abcdef0123456789abcdef0123456789" in json.dumps(leaked):
        failures.append("redact_deep left a credential inside a persisted attempt")
    if redact_deep({"n": 12, "ok": True, "none": None}) != {"n": 12, "ok": True, "none": None}:
        failures.append("redact_deep mangles non-string values")

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
    print(json.dumps({"result": "self-test-ok", "checks": 25}))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", action="store_true", help="run model calls; default records preflight only")
    parser.add_argument("--self-test", action="store_true", help="exercise the guards; no model calls")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    context, clean, fixed_base = base_context()
    commit = fixed_base["commit"]
    packet = packet_state()
    if not clean:
        return blocked("fixed review base is dirty", commit=commit, clean=False)
    if packet.get("status") != "proposed" or packet.get("qualification_state") != "not-run":
        return blocked("packet is not proposed/not-run", commit=commit, clean=True)
    if not args.run:
        return blocked("preflight only; --run required", commit=commit, clean=True)
    OUT.mkdir(parents=True, exist_ok=True)
    models = [model for _, model, _ in PASSES]
    # Name-level, not family-level: this catches the same model twice. It canNOT
    # catch two siblings of one family (gpt-5.6-sol vs gpt-5.6-terra), so the
    # backend check below carries the weight. The family requirement itself
    # remains a human precondition of ADR-0020 §6.
    if len(set(models)) != len(models):
        return blocked(
            f"ADR-0020 §6 requires distinct model families; both passes name {models[0]}",
            commit=commit,
            clean=True,
        )
    # This check lived only in --self-test, so `python3 scripts/haqp_blind_review.py
    # --run` with HAQP_BLIND_PASS1=codex:gpt-5.6-sol HAQP_BLIND_PASS2=codex:gpt-5.6-terra
    # recorded two OpenAI reviews as though they were independent. `just
    # haq-blind-review` happened to catch it because it runs --self-test first,
    # but a guard that only fires on the recommended invocation is not a guard.
    if len({backend_of(model) for model in models}) != len(models):
        return blocked(
            f"ADR-0020 §6 requires distinct model families; {models} share a backend",
            commit=commit,
            clean=True,
        )
    dead = vendor_liveness(models)
    if dead is not None:
        return blocked(dead, commit=commit, clean=True)
    try:
        records = [
            run_pass(f"{name}-{model}", model, context, fixed_base, pass_two=pass_two)
            for name, model, pass_two in PASSES
        ]
        first, second = records
    except (ProviderFailure, ReviewSchemaFailure, RuntimeError, ValueError, subprocess.SubprocessError, OSError) as exc:
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
