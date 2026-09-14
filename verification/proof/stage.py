"""Reconstruct a closed proof-source projection from an immutable Git commit.

The caller owns commit/profile approval. Exact identity is not authorization.
"""

import hashlib
import importlib.util
import os
from pathlib import Path
import re
import selectors
import subprocess
import time


RUN_PATH = Path(__file__).with_name("run.py")
RUN_SPEC = importlib.util.spec_from_file_location("proof_input_runner_for_stage", RUN_PATH)
RUN = importlib.util.module_from_spec(RUN_SPEC)
RUN_SPEC.loader.exec_module(RUN)

COMMIT = re.compile(r"[0-9a-f]{40}")
MANIFESTS = ("verification/proof/inputs.json", "verification/proof/source-inventory.json")
TREE_PATHS = (*sorted(RUN.EXACT_SOURCE_FILES), *RUN.SOURCE_ROOTS, *MANIFESTS)
RUNNER_PATHS = tuple(
    "verification/proof/" + name
    for name in (
        "run.py", "stage.py", "dependencies.py", "toolchain.py",
        "distribution.py", "command.py", "observation.py", "sandbox.py",
    )
)
TREE_OUTPUT_LIMIT = 4 * 1024 * 1024
RUNNER_FILE_LIMIT = 1024 * 1024
RUNNER_TOTAL_LIMIT = 8 * 1024 * 1024
GIT_ENV = {
    "PATH": "/usr/bin:/bin",
    "LANG": "C.UTF-8",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "GIT_NO_REPLACE_OBJECTS": "1",
    "GIT_NO_LAZY_FETCH": "1",
    "GIT_TERMINAL_PROMPT": "0",
}


class StageFailure(Exception):
    """The immutable source projection could not be safely reconstructed."""


def _git(repository: Path, arguments: list[str], limit: int) -> bytes:
    process = None
    try:
        process = subprocess.Popen(
            ["/usr/bin/git", "--no-replace-objects", "-c", "core.hooksPath=/dev/null", *arguments],
            cwd=repository,
            env=GIT_ENV,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        if process.stdout is None:
            raise StageFailure("Git source reconstruction failed")
        deadline = time.monotonic() + 30
        chunks = []
        size = 0
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise subprocess.TimeoutExpired(process.args, 30)
                if not selector.select(min(remaining, 0.1)):
                    continue
                chunk = os.read(process.stdout.fileno(), min(1024 * 1024, limit + 1 - size))
                if not chunk:
                    break
                chunks.append(chunk)
                size += len(chunk)
                if size > limit:
                    raise StageFailure("Git output exceeds the source staging limit")
        remaining = deadline - time.monotonic()
        if remaining <= 0 or process.wait(timeout=remaining) != 0:
            raise StageFailure("Git source reconstruction failed")
        return b"".join(chunks)
    except StageFailure:
        raise
    except (OSError, subprocess.SubprocessError) as error:
        raise StageFailure("Git source reconstruction failed") from error
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait()
        if process is not None and process.stdout is not None:
            process.stdout.close()


def _validate_inputs(raw: bytes) -> dict:
    try:
        value = RUN._parse_manifest(raw)
        RUN._validate_hash_map(value["source_sha256"], RUN.SOURCE_PATHS, "source_sha256")
        RUN._validate_hash_map(value["tool_sha256"], RUN.TOOL_PATHS, "tool_sha256")
    except RUN.InputFailure as error:
        raise StageFailure(f"invalid committed inputs.json: {error}") from error
    return value


def _validate_inventory(raw: bytes, selected: dict) -> dict:
    try:
        return RUN._parse_source_inventory(raw, selected)
    except RUN.InputFailure as error:
        raise StageFailure(f"invalid committed source inventory: {error}") from error


def _tree(
    repository: Path, commit: str, paths: tuple[str, ...] = TREE_PATHS
) -> dict[str, tuple[str, str]]:
    raw = _git(repository, ["ls-tree", "-r", "-z", commit, "--", *paths], TREE_OUTPUT_LIMIT)
    entries = {}
    try:
        records = raw.split(b"\0")
        if records[-1] != b"":
            raise ValueError("unterminated tree output")
        for record in records[:-1]:
            metadata, separator, raw_path = record.partition(b"\t")
            mode, kind, object_id = metadata.decode("ascii").split(" ")
            path = raw_path.decode("utf-8")
            if not separator or path in entries or kind != "blob" or mode not in ("100644", "100755"):
                raise ValueError("invalid tree entry")
            entries[path] = (mode, object_id)
    except (UnicodeError, ValueError) as error:
        raise StageFailure("invalid Git source tree metadata") from error
    return entries


def _destination(repository: Path, destination: Path) -> Path:
    try:
        parent = destination.parent.resolve(strict=True)
        target = parent / destination.name
        if destination.name in ("", ".", "..") or target.is_relative_to(repository):
            raise StageFailure("staging destination must be new and outside the repository")
        try:
            target.lstat()
        except FileNotFoundError:
            return target
        raise StageFailure("staging destination already exists")
    except StageFailure:
        raise
    except OSError as error:
        raise StageFailure("staging destination is unavailable") from error


def stage_source(repository: Path, commit: str, destination: Path) -> dict:
    """Stage the approved source projection from one exact Git commit."""
    if type(commit) is not str or COMMIT.fullmatch(commit) is None:
        raise StageFailure("source staging requires an exact lowercase commit hash")
    try:
        repository = repository.resolve(strict=True)
        if not repository.is_dir():
            raise StageFailure("source repository must be a directory")
    except StageFailure:
        raise
    except OSError as error:
        raise StageFailure("source repository is unavailable") from error
    target = _destination(repository, destination)
    resolved = _git(repository, ["rev-parse", "--verify", commit + "^{commit}"], 128)
    try:
        if resolved.decode("ascii").strip() != commit:
            raise StageFailure("requested Git commit identity does not match")
    except UnicodeError as error:
        raise StageFailure("invalid Git commit identity") from error
    tree = _tree(repository, commit)
    if not all(path in tree for path in MANIFESTS):
        raise StageFailure("committed proof input manifests are missing")
    manifest_blobs = {
        path: _git(repository, ["cat-file", "blob", tree[path][1]], limit)
        for path, limit in zip(MANIFESTS, (RUN.MAX_MANIFEST_BYTES, RUN.MAX_SOURCE_INVENTORY_BYTES))
    }
    inputs = _validate_inputs(manifest_blobs[MANIFESTS[0]])
    inventory = _validate_inventory(manifest_blobs[MANIFESTS[1]], inputs["source_sha256"])
    expected = set(inventory) | set(MANIFESTS)
    if set(tree) != expected:
        raise StageFailure("Git source tree does not match the closed source inventory")
    blobs = dict(manifest_blobs)
    hashes = {path: hashlib.sha256(raw).hexdigest() for path, raw in manifest_blobs.items()}
    modes = {path: tree[path][0] for path in MANIFESTS}
    total = sum(len(raw) for raw in manifest_blobs.values())
    for path, entry in inventory.items():
        mode, object_id = tree[path]
        if mode != entry["mode"]:
            raise StageFailure(f"Git source mode mismatch: {path}")
        raw = _git(repository, ["cat-file", "blob", object_id], RUN.MAX_SOURCE_FILE_BYTES)
        total += len(raw)
        if total > RUN.MAX_SOURCE_BYTES:
            raise StageFailure("Git source projection exceeds 128 MiB")
        digest = hashlib.sha256(raw).hexdigest()
        if digest != entry["sha256"]:
            raise StageFailure(f"Git source hash mismatch: {path}")
        blobs[path] = raw
        hashes[path] = digest
        modes[path] = mode
    try:
        target.mkdir(mode=0o700)
        for relative in sorted(blobs):
            path = target / relative
            path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            with path.open("xb") as output:
                output.write(blobs[relative])
            path.chmod(0o755 if modes[relative] == "100755" else 0o644)
    except OSError as error:
        raise StageFailure("source staging write failed; partial output retained") from error
    return {
        "status": "source-staged",
        "qualification": False,
        "commit": commit,
        "inventory_origin": RUN.SOURCE_COMMIT,
        "file_sha256": hashes,
        "file_mode": modes,
    }


def stage_runner(repository: Path, commit: str, destination: Path) -> dict:
    """Stage the closed proof-runner projection from one exact Git commit."""
    if type(commit) is not str or COMMIT.fullmatch(commit) is None:
        raise StageFailure("runner staging requires an exact lowercase commit hash")
    try:
        repository = repository.resolve(strict=True)
        if not repository.is_dir():
            raise StageFailure("runner repository must be a directory")
    except StageFailure:
        raise
    except OSError as error:
        raise StageFailure("runner repository is unavailable") from error
    target = _destination(repository, destination)
    resolved = _git(repository, ["rev-parse", "--verify", commit + "^{commit}"], 128)
    try:
        if resolved.decode("ascii").strip() != commit:
            raise StageFailure("requested Git commit identity does not match")
    except UnicodeError as error:
        raise StageFailure("invalid Git commit identity") from error
    tree = _tree(repository, commit, RUNNER_PATHS)
    if set(tree) != set(RUNNER_PATHS):
        raise StageFailure("Git runner tree does not match the closed projection")

    blobs = {}
    hashes = {}
    modes = {}
    total = 0
    for path in RUNNER_PATHS:
        mode, object_id = tree[path]
        raw = _git(repository, ["cat-file", "blob", object_id], RUNNER_FILE_LIMIT)
        total += len(raw)
        if total > RUNNER_TOTAL_LIMIT:
            raise StageFailure("Git runner projection exceeds 8 MiB")
        try:
            text = raw.decode("utf-8")
        except UnicodeError as error:
            raise StageFailure(f"Git runner file is not UTF-8: {path}") from error
        if "\0" in text:
            raise StageFailure(f"Git runner file contains NUL: {path}")
        blobs[path] = raw
        hashes[path] = hashlib.sha256(raw).hexdigest()
        modes[path] = mode

    try:
        target.mkdir(mode=0o700)
        for relative in RUNNER_PATHS:
            path = target / relative
            path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            with path.open("xb") as output:
                output.write(blobs[relative])
            path.chmod(0o755 if modes[relative] == "100755" else 0o644)
    except OSError as error:
        raise StageFailure("runner staging write failed; partial output retained") from error
    return {
        "status": "runner-staged",
        "qualification": False,
        "commit": commit,
        "file_sha256": hashes,
        "file_mode": modes,
    }
