"""Pinned-input preflight for the narrow acknowledgement proof fragment."""

import hashlib
import json
from pathlib import Path
import re
import stat

TOP_KEYS = {"schema", "fragment", "discharges_obligation", "source_sha256", "tool_sha256"}
SOURCE_PATHS = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "crates/liminal-safety/Cargo.toml",
    "crates/liminal-safety/src/lib.rs",
    "crates/liminal-safety/examples/acknowledgement_witness.rs",
    "crates/liminal-jurisdiction/Cargo.toml",
    "crates/liminal-jurisdiction/src/ilrp.rs",
}
TOOL_PATHS = {"verus", "cargo-verus", "rust_verify", "z3"}
SHA256 = re.compile(r"[0-9a-f]{64}")
MAX_MANIFEST_BYTES = 64 * 1024


class InputFailure(Exception):
    """The pinned proof inputs are invalid or unavailable."""


def _regular_path(root: Path, relative: Path) -> Path:
    current = root
    for part in relative.parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except OSError as error:
            raise InputFailure(f"input unavailable: {relative}") from error
        if stat.S_ISLNK(mode):
            raise InputFailure(f"symlink input rejected: {relative}")
    if not stat.S_ISREG(mode):
        raise InputFailure(f"non-regular input rejected: {relative}")
    return current


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise InputFailure(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def _reject_constant(value):
    raise InputFailure(f"non-finite JSON number rejected: {value}")


def _load_manifest(repository: Path) -> dict:
    relative = Path("verification/proof/inputs.json")
    manifest_path = _regular_path(repository, relative)
    try:
        if manifest_path.stat().st_size > MAX_MANIFEST_BYTES:
            raise InputFailure("inputs.json exceeds 64 KiB")
        raw = manifest_path.read_bytes().decode("utf-8")
        manifest = json.loads(
            raw,
            object_pairs_hook=_unique_object,
            parse_constant=_reject_constant,
        )
    except InputFailure:
        raise
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        raise InputFailure("invalid inputs.json") from error
    if type(manifest) is not dict or set(manifest) != TOP_KEYS:
        raise InputFailure("inputs.json top-level fields do not match the closed schema")
    if type(manifest["schema"]) is not str or manifest["schema"] != "liminal-proof-inputs-v1":
        raise InputFailure("invalid schema")
    if type(manifest["fragment"]) is not str or manifest["fragment"] != "ack-identity-dependency-v1":
        raise InputFailure("invalid fragment")
    if manifest.get("discharges_obligation") is not False:
        raise InputFailure("discharges_obligation must be exactly false")
    return manifest


def _validate_hash_map(value, expected_keys, field):
    if type(value) is not dict or set(value) != expected_keys:
        raise InputFailure(f"{field} keys do not match the closed input set")
    for relative, digest in value.items():
        if type(relative) is not str or type(digest) is not str or SHA256.fullmatch(digest) is None:
            raise InputFailure(f"{field} values must be lowercase SHA-256 hashes")


def _hash_inputs(root: Path, pins: dict, kind: str) -> dict:
    actual_hashes = {}
    for relative, expected in pins.items():
        path = _regular_path(root, Path(relative))
        digest = hashlib.sha256()
        try:
            with path.open("rb") as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(chunk)
        except OSError as error:
            raise InputFailure(f"input unavailable: {relative}") from error
        actual = digest.hexdigest()
        if actual != expected:
            raise InputFailure(f"{kind} hash mismatch: {relative}")
        actual_hashes[relative] = actual
    return actual_hashes


def check_inputs(repository: Path, verus_root: Path) -> dict:
    """Validate pinned proof inputs without executing or qualifying a proof."""
    manifest = _load_manifest(repository)
    _validate_hash_map(manifest["source_sha256"], SOURCE_PATHS, "source_sha256")
    _validate_hash_map(manifest["tool_sha256"], TOOL_PATHS, "tool_sha256")
    actual_sources = _hash_inputs(repository, manifest["source_sha256"], "source")
    actual_tools = _hash_inputs(verus_root, manifest["tool_sha256"], "tool")
    return {
        "status": "inputs-valid",
        "qualification": False,
        "source_sha256": actual_sources,
        "tool_sha256": actual_tools,
    }
