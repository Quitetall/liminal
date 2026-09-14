"""Offline Verus distribution validation seam."""

import hashlib
import os
from pathlib import Path, PurePosixPath
import re
import stat
import zipfile


ARCHIVE_LIMIT = 512 * 1024 * 1024
UNCOMPRESSED_LIMIT = 2 * 1024 * 1024 * 1024
ENTRY_LIMIT = 10_000
TOP = "verus-x86-linux"
SHA256 = re.compile(r"[0-9a-f]{64}")


class DistributionFailure(Exception):
    """The archive or extracted distribution is invalid."""


def _hash_stream(stream, expected_length=None):
    digest = hashlib.sha256()
    length = 0
    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
        length += len(chunk)
        digest.update(chunk)
    if expected_length is not None and length != expected_length:
        raise DistributionFailure("archive member length does not match metadata")
    return digest.hexdigest()


def _archive_pin(archive: Path, expected: str) -> str:
    if type(expected) is not str or SHA256.fullmatch(expected) is None:
        raise DistributionFailure("expected archive pin must be lowercase SHA-256")
    try:
        metadata = archive.lstat()
        if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
            raise DistributionFailure("archive must be a regular file")
        if metadata.st_size > ARCHIVE_LIMIT:
            raise DistributionFailure("archive exceeds 512 MiB")
        with archive.open("rb") as source:
            actual = _hash_stream(source)
    except DistributionFailure:
        raise
    except OSError as error:
        raise DistributionFailure("archive unavailable") from error
    if actual != expected:
        raise DistributionFailure("archive SHA-256 mismatch")
    return actual


def _member_path(info: zipfile.ZipInfo) -> tuple[str, bool]:
    name = info.filename
    if not name or name.startswith("/") or "\\" in name or ":" in name:
        raise DistributionFailure(f"unsafe archive member: {name}")
    directory = info.is_dir()
    components = name[:-1].split("/") if directory else name.split("/")
    if any(component in ("", ".", "..") for component in components):
        raise DistributionFailure(f"unsafe archive member: {name}")
    if components[0] != TOP:
        raise DistributionFailure(f"archive member outside {TOP}: {name}")
    if len(components) == 1 and not directory:
        raise DistributionFailure(f"archive top entry is not a directory: {name}")
    mode = (info.external_attr >> 16) & 0xFFFF
    if mode:
        if directory and not stat.S_ISDIR(mode):
            raise DistributionFailure(f"special archive member: {name}")
        if not directory and not stat.S_ISREG(mode):
            raise DistributionFailure(f"special archive member: {name}")
    return "/".join(components), directory


def _archive_inventory(bundle: zipfile.ZipFile):
    infos = bundle.infolist()
    if len(infos) > ENTRY_LIMIT:
        raise DistributionFailure("archive exceeds 10000 entries")
    if sum(info.file_size for info in infos) > UNCOMPRESSED_LIMIT:
        raise DistributionFailure("archive exceeds 2 GiB declared uncompressed size")
    seen = set()
    directories = set()
    files = {}
    for info in infos:
        normalized, directory = _member_path(info)
        if normalized in seen:
            raise DistributionFailure(f"duplicate archive member: {normalized}")
        seen.add(normalized)
        if normalized == TOP:
            continue
        relative = normalized[len(TOP) + 1 :]
        parents = PurePosixPath(relative).parents
        directories.update(str(parent) for parent in parents if str(parent) != ".")
        if directory:
            directories.add(relative)
        else:
            files[relative] = info
    if not files:
        raise DistributionFailure("distribution archive contains no regular files")
    return directories, files


def _extracted_inventory(root: Path):
    try:
        root_mode = root.lstat().st_mode
    except OSError as error:
        raise DistributionFailure("extracted root unavailable") from error
    if stat.S_ISLNK(root_mode) or not stat.S_ISDIR(root_mode):
        raise DistributionFailure("extracted root must be a directory")
    directories = set()
    files = {}
    try:
        pending = [root]
        while pending:
            directory = pending.pop()
            with os.scandir(directory) as entries:
                for entry in entries:
                    path = Path(entry.path)
                    relative = path.relative_to(root).as_posix()
                    mode = entry.stat(follow_symlinks=False).st_mode
                    if stat.S_ISLNK(mode):
                        raise DistributionFailure(f"symlink extracted member: {relative}")
                    if stat.S_ISDIR(mode):
                        directories.add(relative)
                        pending.append(path)
                    elif stat.S_ISREG(mode):
                        files[relative] = path
                    else:
                        raise DistributionFailure(f"special extracted member: {relative}")
    except DistributionFailure:
        raise
    except OSError as error:
        raise DistributionFailure("cannot inspect extracted distribution") from error
    return directories, files


def check_distribution(archive: Path, extracted_root: Path, expected_sha256: str) -> dict:
    """Validate an extracted distribution without extraction or execution."""
    archive_sha256 = _archive_pin(archive, expected_sha256)
    try:
        with zipfile.ZipFile(archive) as bundle:
            archive_dirs, archive_files = _archive_inventory(bundle)
            extracted_dirs, extracted_files = _extracted_inventory(extracted_root)
            if archive_dirs != extracted_dirs or set(archive_files) != set(extracted_files):
                raise DistributionFailure("extracted inventory does not match archive")
            file_hashes = {}
            for relative, info in archive_files.items():
                with bundle.open(info) as archived:
                    archived_hash = _hash_stream(archived, info.file_size)
                with extracted_files[relative].open("rb") as extracted:
                    extracted_hash = _hash_stream(extracted)
                if archived_hash != extracted_hash:
                    raise DistributionFailure(f"file hash mismatch: {relative}")
                file_hashes[relative] = extracted_hash
    except DistributionFailure:
        raise
    except (OSError, zipfile.BadZipFile, RuntimeError, EOFError) as error:
        raise DistributionFailure("invalid distribution archive") from error
    return {
        "status": "distribution-valid",
        "qualification": False,
        "archive_sha256": archive_sha256,
        "file_sha256": file_hashes,
    }
