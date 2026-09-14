"""Pinned-input preflight for the narrow acknowledgement proof fragment."""

import hashlib
import json
import os
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
MAX_SOURCE_INVENTORY_BYTES = 2 * 1024 * 1024
MAX_SOURCE_FILES = 10_000
MAX_SOURCE_FILE_BYTES = 32 * 1024 * 1024
MAX_SOURCE_BYTES = 128 * 1024 * 1024
MAX_SOURCE_PATH_BYTES = 4_096
MAX_SOURCE_PATH_COMPONENTS = 128
SOURCE_COMMIT = "3bd93a9617ff8705ace37f4a19440c70f867f689"
SOURCE_INVENTORY_KEYS = {"schema", "source_commit", "files"}
SOURCE_ENTRY_KEYS = {"sha256", "mode"}
EXACT_SOURCE_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "conformance/Cargo.toml"}
SOURCE_ROOTS = ("crates", "conformance/src", "conformance/spikes", "benches")
FORBIDDEN_SOURCE_COMPONENTS = {
    ".git",
    "heldout",
    "target",
    "__pycache__",
    "node_modules",
    "liminal-5.3-spark",
}


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


def _safe_source_path(raw) -> str:
    if type(raw) is not str:
        raise InputFailure("unsafe source inventory path")
    try:
        encoded = raw.encode("utf-8")
    except UnicodeError as error:
        raise InputFailure("invalid source inventory path encoding") from error
    if len(encoded) > MAX_SOURCE_PATH_BYTES:
        raise InputFailure("source inventory path limit exceeded")
    if not raw or raw.startswith("/") or "\\" in raw or ":" in raw or "\0" in raw:
        raise InputFailure("unsafe source inventory path")
    parts = raw.split("/")
    if len(parts) > MAX_SOURCE_PATH_COMPONENTS:
        raise InputFailure("source inventory path limit exceeded")
    if any(part in ("", ".", "..") for part in parts):
        raise InputFailure("unsafe source inventory path")
    if any(part in FORBIDDEN_SOURCE_COMPONENTS for part in parts):
        raise InputFailure("forbidden source inventory component")
    allowed = raw in EXACT_SOURCE_FILES or any(raw.startswith(root + "/") for root in SOURCE_ROOTS)
    if not allowed:
        raise InputFailure(f"source inventory path outside closed roots: {raw}")
    return raw


def _load_source_inventory(repository: Path, selected_pins: dict) -> dict:
    relative = Path("verification/proof/source-inventory.json")
    path = _regular_path(repository, relative)
    try:
        if path.stat().st_size > MAX_SOURCE_INVENTORY_BYTES:
            raise InputFailure("source inventory exceeds 2 MiB")
        inventory = json.loads(
            path.read_bytes().decode("utf-8"),
            object_pairs_hook=_unique_object,
            parse_constant=_reject_constant,
        )
    except InputFailure:
        raise
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        raise InputFailure("invalid source inventory") from error
    if type(inventory) is not dict or set(inventory) != SOURCE_INVENTORY_KEYS:
        raise InputFailure("source inventory fields do not match the closed schema")
    if inventory["schema"] != "liminal-proof-source-inventory-v1" or type(inventory["schema"]) is not str:
        raise InputFailure("invalid source inventory schema")
    if inventory["source_commit"] != SOURCE_COMMIT or type(inventory["source_commit"]) is not str:
        raise InputFailure("invalid source inventory commit")
    files = inventory["files"]
    if type(files) is not dict or not files or len(files) > MAX_SOURCE_FILES:
        raise InputFailure("invalid source inventory file count")
    validated = {}
    for raw, entry in files.items():
        relative_name = _safe_source_path(raw)
        if type(entry) is not dict or set(entry) != SOURCE_ENTRY_KEYS:
            raise InputFailure(f"invalid source inventory entry: {relative_name}")
        digest = entry["sha256"]
        mode = entry["mode"]
        if type(digest) is not str or SHA256.fullmatch(digest) is None:
            raise InputFailure(f"invalid source inventory hash: {relative_name}")
        if type(mode) is not str or mode not in ("100644", "100755"):
            raise InputFailure(f"invalid source inventory mode: {relative_name}")
        validated[relative_name] = entry
    for relative_name, digest in selected_pins.items():
        if relative_name not in validated or validated[relative_name]["sha256"] != digest:
            raise InputFailure(f"selected pin missing from source inventory: {relative_name}")
    return validated


def _scan_source_root(
    repository: Path,
    relative_root: str,
    expected_files: set[str],
    expected_directories: set[str],
):
    def affected_path(directory):
        return next(
            (path for path in sorted(expected_files) if path.startswith(directory + "/")),
            directory,
        )

    root = repository
    parts = relative_root.split("/")
    for index, part in enumerate(parts):
        root = root / part
        try:
            mode = root.lstat().st_mode
        except FileNotFoundError:
            return set(), {}
        except OSError as error:
            raise InputFailure(f"source inventory scan failed: {'/'.join(parts[:index + 1])}") from error
        if stat.S_ISLNK(mode):
            directory = "/".join(parts[: index + 1])
            raise InputFailure(f"source inventory symlink rejected: {affected_path(directory)}")
        if not stat.S_ISDIR(mode):
            raise InputFailure(f"source inventory root is not a directory: {relative_root}")
    if relative_root not in expected_directories:
        raise InputFailure(
            f"source inventory directory set mismatch: unexpected directory {relative_root}"
        )
    directories = {relative_root}
    files = {}
    pending = [(root, relative_root)]
    try:
        while pending:
            directory, relative_directory = pending.pop()
            with os.scandir(directory) as entries:
                for entry in entries:
                    relative_name = f"{relative_directory}/{entry.name}"
                    mode = entry.stat(follow_symlinks=False).st_mode
                    if stat.S_ISLNK(mode):
                        raise InputFailure(
                            f"source inventory symlink rejected: {affected_path(relative_name)}"
                        )
                    if stat.S_ISDIR(mode):
                        if relative_name not in expected_directories:
                            raise InputFailure(
                                "source inventory directory set mismatch: "
                                f"unexpected directory {relative_name}"
                            )
                        directories.add(relative_name)
                        pending.append((Path(entry.path), relative_name))
                    elif stat.S_ISREG(mode):
                        if relative_name not in expected_files:
                            raise InputFailure(
                                f"source inventory unexpected file: {relative_name}"
                            )
                        files[relative_name] = (Path(entry.path), mode, entry.stat(follow_symlinks=False).st_size)
                    else:
                        raise InputFailure(f"source inventory special file rejected: {relative_name}")
    except InputFailure:
        raise
    except OSError as error:
        raise InputFailure(f"source inventory scan failed: {relative_root}") from error
    return directories, files


def _reject_repository_cargo(repository: Path) -> None:
    cargo_config = repository / ".cargo"
    try:
        cargo_config.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise InputFailure("repository .cargo lstat failed") from error
    else:
        raise InputFailure("repository .cargo must be absent")


def _observe_exact_file(repository: Path, relative_name: str):
    current = repository
    parts = relative_name.split("/")
    for index, part in enumerate(parts):
        current = current / part
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            return None
        except OSError as error:
            raise InputFailure(f"source inventory scan failed: {relative_name}") from error
        if stat.S_ISLNK(metadata.st_mode):
            raise InputFailure(f"source inventory symlink rejected: {relative_name}")
        if index < len(parts) - 1 and not stat.S_ISDIR(metadata.st_mode):
            raise InputFailure(f"source inventory special file rejected: {relative_name}")
    if not stat.S_ISREG(metadata.st_mode):
        raise InputFailure(f"source inventory special file rejected: {relative_name}")
    return current, metadata.st_mode, metadata.st_size


def _validate_source_closure(repository: Path, inventory: dict) -> None:
    expected_files = set(inventory)
    expected_directories = set()
    for relative_name in expected_files:
        expected_directories.update(
            str(parent) for parent in Path(relative_name).parents if str(parent) != "."
        )
    relevant_expected_directories = {
        directory
        for directory in expected_directories
        if directory in SOURCE_ROOTS or any(directory.startswith(root + "/") for root in SOURCE_ROOTS)
    }
    actual_files = {}
    actual_directories = set()
    for relative_name in EXACT_SOURCE_FILES:
        observed = _observe_exact_file(repository, relative_name)
        if observed is not None:
            actual_files[relative_name] = observed
    for relative_root in SOURCE_ROOTS:
        directories, files = _scan_source_root(
            repository, relative_root, expected_files, relevant_expected_directories
        )
        actual_directories.update(directories)
        actual_files.update(files)
    if set(actual_files) != expected_files or actual_directories != relevant_expected_directories:
        raise InputFailure("source inventory file or directory set mismatch")
    total_bytes = 0
    for relative_name, (path, mode, size) in actual_files.items():
        if mode & 0o7000:
            raise InputFailure(f"source special permission bits rejected: {relative_name}")
        expected_mode = "100755" if mode & 0o111 else "100644"
        if expected_mode != inventory[relative_name]["mode"]:
            raise InputFailure(f"source mode mismatch: {relative_name}")
        if size > MAX_SOURCE_FILE_BYTES:
            raise InputFailure(f"source file exceeds 32 MiB: {relative_name}")
        total_bytes += size
        if total_bytes > MAX_SOURCE_BYTES:
            raise InputFailure("source inventory exceeds 128 MiB")
        digest = hashlib.sha256()
        try:
            with path.open("rb") as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(chunk)
        except OSError as error:
            raise InputFailure(f"source inventory read failed: {relative_name}") from error
        if digest.hexdigest() != inventory[relative_name]["sha256"]:
            raise InputFailure(f"source inventory hash mismatch: {relative_name}")


def check_inputs(repository: Path, verus_root: Path) -> dict:
    """Validate pinned proof inputs without executing or qualifying a proof."""
    _reject_repository_cargo(repository)
    manifest = _load_manifest(repository)
    _validate_hash_map(manifest["source_sha256"], SOURCE_PATHS, "source_sha256")
    _validate_hash_map(manifest["tool_sha256"], TOOL_PATHS, "tool_sha256")
    source_inventory = _load_source_inventory(repository, manifest["source_sha256"])
    _validate_source_closure(repository, source_inventory)
    actual_sources = _hash_inputs(repository, manifest["source_sha256"], "source")
    actual_tools = _hash_inputs(verus_root, manifest["tool_sha256"], "tool")
    return {
        "status": "inputs-valid",
        "qualification": False,
        "source_sha256": actual_sources,
        "tool_sha256": actual_tools,
    }
