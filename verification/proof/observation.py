"""Parse complete pinned-Verus stdout without authenticating or qualifying it."""

import json
import re

MAX_OUTPUT_BYTES = 2 * 1024 * 1024
MAX_EVENTS = 64
MAX_COUNT = (1 << 64) - 1
SUMMARY = re.compile(
    r"verification results:: ([0-9]{1,20}) verified, ([0-9]{1,20}) errors"
    r"( \(partial verification with `--verify-\*`\))?\r?\n"
)


class ObservationFailure(Exception):
    """The supplied verifier output cannot be observed under this contract."""


def _decode_u64(value):
    if re.fullmatch(r"0|[1-9][0-9]{0,19}", value) is None:
        raise ObservationFailure("verifier count must be canonical unsigned decimal")
    result = int(value)
    if result > MAX_COUNT:
        raise ObservationFailure("verifier count exceeds u64")
    return result


def _is_scalar_string(value):
    if type(value) is not str:
        return False
    try:
        value.encode("utf-8")
    except UnicodeError:
        return False
    return True


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if not _is_scalar_string(key):
            raise ObservationFailure("verifier JSON keys must contain Unicode scalar values")
        if key in result:
            raise ObservationFailure("duplicate verifier JSON field")
        result[key] = value
    return result


def _validate_function_details(details):
    for notes in details.values():
        if type(notes) is not dict or set(notes) != {
            "obligation_proof_notes", "failed_proof_notes"
        }:
            raise ObservationFailure("function-note fields do not match the closed schema")
        for values in notes.values():
            if type(values) is not list or any(not _is_scalar_string(value) for value in values):
                raise ObservationFailure("function notes must be arrays of strings")
            if len(set(values)) != len(values):
                raise ObservationFailure("function-note sets must not contain duplicate strings")


def _validate_results(results):
    base = {"encountered-error", "encountered-vir-error"}
    if any(type(results.get(key)) is not bool for key in base):
        raise ObservationFailure("result error flags must be booleans")
    if results["encountered-vir-error"]:
        expected = base | ({"success"} if "success" in results else set())
        if set(results) != expected:
            raise ObservationFailure("VIR-error result fields do not match the closed schema")
        if "success" in results and results["success"] is not False:
            raise ObservationFailure("VIR-error success must be false when present")
        return

    entire = results.get("is-verifying-entire-crate")
    if type(entire) is not bool:
        raise ObservationFailure("entire-crate flag must be a boolean")
    expected = base | {"verified", "errors", "is-verifying-entire-crate"}
    if entire:
        expected.add("success")
    if set(results) != expected:
        raise ObservationFailure("result fields do not match verification scope")
    for key in ("verified", "errors"):
        if type(results[key]) is not int or results[key] < 0:
            raise ObservationFailure("result counts must be nonnegative integers")
    if entire:
        expected_success = not results["encountered-error"] and results["errors"] == 0
        if results["success"] is not expected_success:
            raise ObservationFailure("success contradicts result error fields")


def _validate_metadata(metadata):
    scalar_keys = {"profile", "version", "toolchain", "commit"}
    if set(metadata) != scalar_keys | {"platform"}:
        raise ObservationFailure("metadata fields do not match the closed schema")
    if any(not _is_scalar_string(metadata[key]) for key in scalar_keys):
        raise ObservationFailure("metadata values must be strings")
    platform = metadata["platform"]
    if type(platform) is not dict or set(platform) != {"os", "arch"}:
        raise ObservationFailure("platform fields do not match the closed schema")
    if any(not _is_scalar_string(value) for value in platform.values()):
        raise ObservationFailure("platform values must be strings")


def parse_verus_output(raw: bytes) -> dict:
    """Parse complete pinned Verus stdout only.

    This does not establish source, tool, command authenticity or qualification.
    Development observations are bounded to 2 MiB and 64 events; excess refuses,
    rather than returning a truncated observation.
    """
    if type(raw) is not bytes or len(raw) > MAX_OUTPUT_BYTES:
        raise ObservationFailure("verifier output must be bytes within the 2 MiB limit")
    try:
        text = raw.decode("utf-8")
    except UnicodeError as error:
        raise ObservationFailure("verifier output is not UTF-8") from error
    events = []
    decoder = json.JSONDecoder(object_pairs_hook=_unique_object, parse_int=_decode_u64)
    cursor = 0
    while cursor < len(text):
        if text[cursor] in " \t\r\n":
            cursor += 1
            continue
        if len(events) >= MAX_EVENTS:
            raise ObservationFailure("verifier output exceeds the 64-event limit")
        summary = SUMMARY.match(text, cursor)
        if summary is not None:
            event = {
                "kind": "summary",
                "verified": _decode_u64(summary[1]),
                "errors": _decode_u64(summary[2]),
            }
            if summary[3] is not None:
                event["partial"] = True
            events.append(event)
            cursor = summary.end()
        else:
            try:
                report, cursor = decoder.raw_decode(text, cursor)
            except (ValueError, RecursionError) as error:
                raise ObservationFailure("invalid verifier JSON") from error
            if cursor < len(text) and text[cursor] not in " \t\r\n":
                raise ObservationFailure("verifier events must be whitespace separated")
            if type(report) is not dict or set(report) != {
                "func-details", "verification-results", "verus"
            }:
                raise ObservationFailure("report fields do not match the closed schema")
            if any(type(value) is not dict for value in report.values()):
                raise ObservationFailure("report sections must be objects")
            _validate_function_details(report["func-details"])
            _validate_results(report["verification-results"])
            _validate_metadata(report["verus"])
            events.append({"kind": "report", "report": report})
    if not any(event["kind"] == "report" for event in events):
        raise ObservationFailure("verifier output contains no report")
    return {
        "schema": "liminal-verus-observation-v1",
        "status": "output-parsed",
        "qualification": False,
        "events": events,
    }
