"""Offline construction of checksum-closed Cargo dependency sources."""

import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import tarfile
import tomllib


REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
SHA256 = re.compile(r"[0-9a-f]{64}")
ARCHIVE_LIMIT = 512 * 1024 * 1024
FILE_LIMIT = 512 * 1024 * 1024
TOTAL_LIMIT = 2 * 1024 * 1024 * 1024
PACKAGE_LIMIT = 1_000
MEMBER_LIMIT = 100_000
LOCKFILE_LIMIT = 4 * 1024 * 1024


class DependencyFailure(Exception):
    """Dependency inputs or construction failed."""


def _hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise DependencyFailure(f"cannot read input: {path.name}") from error
    return digest.hexdigest()


def _regular_input(path: Path, label: str, size_limit=None) -> None:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise DependencyFailure(f"{label} unavailable: {path.name}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise DependencyFailure(f"{label} must be a regular file: {path.name}")
    if size_limit is not None and metadata.st_size > size_limit:
        raise DependencyFailure(f"{label} exceeds size limit: {path.name}")


def _safe_component(value, label: str) -> str:
    if (
        type(value) is not str
        or value in ("", ".", "..")
        or any(character in value for character in ("/", "\\", ":", "\0"))
    ):
        raise DependencyFailure(f"unsafe package {label}")
    return value


def _read_lockfile(lockfile: Path):
    _regular_input(lockfile, "lockfile", LOCKFILE_LIMIT)
    try:
        raw = lockfile.read_bytes()
        document = tomllib.loads(raw.decode("utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise DependencyFailure("invalid Cargo.lock") from error
    if type(document) is not dict or document.get("version") != 4 or type(document.get("version")) is not int:
        raise DependencyFailure("Cargo.lock version must be exactly 4")
    packages = document.get("package")
    if type(packages) is not list:
        raise DependencyFailure("Cargo.lock package list is missing")
    seen = set()
    registry = {}
    for package in packages:
        if type(package) is not dict:
            raise DependencyFailure("invalid Cargo.lock package")
        name = _safe_component(package.get("name"), "name")
        version = _safe_component(package.get("version"), "version")
        key = (name, version)
        if key in seen:
            raise DependencyFailure(f"duplicate locked package: {name}-{version}")
        seen.add(key)
        source = package.get("source")
        if source is None:
            continue
        if source != REGISTRY or type(source) is not str:
            raise DependencyFailure(f"unsupported package source: {name}-{version}")
        checksum = package.get("checksum")
        if type(checksum) is not str or SHA256.fullmatch(checksum) is None:
            raise DependencyFailure(f"invalid locked checksum: {name}-{version}")
        registry[key] = checksum
    if not registry:
        raise DependencyFailure("Cargo.lock has no registry packages")
    return hashlib.sha256(raw).hexdigest(), registry


def _raw_member_path(name: str, expected_root: str, directory: bool) -> str:
    if not name or name.startswith("/") or "\\" in name or ":" in name or "\0" in name:
        raise DependencyFailure(f"unsafe archive member: {name}")
    raw = name[:-1] if directory and name.endswith("/") else name
    parts = raw.split("/")
    if any(part in ("", ".", "..") for part in parts):
        raise DependencyFailure(f"unsafe archive member: {name}")
    if parts[0] != expected_root:
        raise DependencyFailure(f"archive member outside expected root: {name}")
    if len(parts) == 1 and not directory:
        raise DependencyFailure(f"archive root is not a directory: {name}")
    relative = "/".join(parts[1:])
    if relative and PurePosixPath(relative).name == ".cargo-checksum.json":
        raise DependencyFailure("embedded .cargo-checksum.json is forbidden")
    return relative


def _describe_member(member, expected_root: str):
    if member.isdir():
        kind = "directory"
    elif member.isreg():
        kind = "file"
        if member.size < 0 or member.size > FILE_LIMIT:
            raise DependencyFailure(f"archive member exceeds size limit: {member.name}")
    else:
        raise DependencyFailure(f"link or special archive member: {member.name}")
    mode = member.mode
    if type(mode) is not int or mode < 0 or mode & ~(0o170000 | 0o7777):
        raise DependencyFailure(f"unknown archive mode bits: {member.name}")
    if mode & 0o7000:
        raise DependencyFailure(f"special permission bits: {member.name}")
    expected_type = stat.S_IFDIR if kind == "directory" else stat.S_IFREG
    if mode & 0o170000 not in (0, expected_type):
        raise DependencyFailure(f"conflicting file type bits: {member.name}")
    relative = _raw_member_path(member.name, expected_root, member.isdir())
    return member.name, relative, kind, member.size, member.mode


def _inspect_archive(path: Path, expected_root: str, member_budget: int, byte_budget: int):
    members = []
    paths = {}
    regular_bytes = 0
    try:
        with tarfile.open(path, mode="r:gz") as bundle:
            for member in bundle:
                if len(members) >= member_budget:
                    raise DependencyFailure("dependency archive member limit exceeded")
                descriptor = _describe_member(member, expected_root)
                name, relative, kind, size, _ = descriptor
                if kind == "file":
                    if regular_bytes + size > byte_budget:
                        raise DependencyFailure("dependency archive byte limit exceeded")
                    regular_bytes += size
                normalized = expected_root if not relative else f"{expected_root}/{relative}"
                if normalized in paths:
                    raise DependencyFailure(f"duplicate archive member: {name}")
                parents = (str(parent) for parent in PurePosixPath(normalized).parents)
                if any(parent in paths and paths[parent] == "file" for parent in parents):
                    raise DependencyFailure(f"archive file/parent conflict: {name}")
                if kind == "file" and any(existing.startswith(normalized + "/") for existing in paths):
                    raise DependencyFailure(f"archive file/parent conflict: {name}")
                paths[normalized] = kind
                members.append(descriptor)
    except DependencyFailure:
        raise
    except (OSError, tarfile.TarError, EOFError) as error:
        raise DependencyFailure(f"invalid crate archive: {path.name}") from error
    return members, regular_bytes


def _preflight(lockfile: Path, archives: list[Path], destination: Path):
    if destination.exists() or destination.is_symlink():
        raise DependencyFailure("destination already exists")
    repository = lockfile.parent.resolve()
    target = destination.resolve(strict=False)
    if target == repository or repository in target.parents:
        raise DependencyFailure("destination must be outside the repository")
    lock_digest, packages = _read_lockfile(lockfile)
    if len(packages) > PACKAGE_LIMIT or len(archives) > PACKAGE_LIMIT:
        raise DependencyFailure("dependency package limit exceeded")
    basenames = {}
    for key in packages:
        basename = f"{key[0]}-{key[1]}.crate"
        if basename in basenames:
            raise DependencyFailure(f"ambiguous archive basename: {basename}")
        basenames[basename] = key
    candidates = {key: [] for key in packages}
    total_members = 0
    total_bytes = 0
    archive_hashes = {}
    for archive in archives:
        _regular_input(archive, "archive", ARCHIVE_LIMIT)
        key = basenames.get(archive.name)
        if key is None:
            raise DependencyFailure(f"unknown archive: {archive.name}")
        actual = _hash_file(archive)
        if actual != packages[key]:
            raise DependencyFailure(f"archive checksum mismatch: {archive.name}")
        root = f"{key[0]}-{key[1]}"
        members, regular_bytes = _inspect_archive(
            archive, root, MEMBER_LIMIT - total_members, TOTAL_LIMIT - total_bytes
        )
        total_members += len(members)
        total_bytes += regular_bytes
        candidates[key].append((archive, actual, members))
        archive_hashes[root] = actual
    missing = [f"{name}-{version}" for (name, version), values in candidates.items() if not values]
    if missing:
        raise DependencyFailure(f"missing archive: {missing[0]}")
    return lock_digest, packages, candidates, archive_hashes


def _construct(destination: Path, packages, candidates):
    destination.mkdir(mode=0o755)
    file_hashes = {}
    remaining_members = MEMBER_LIMIT
    remaining_bytes = TOTAL_LIMIT
    for key, checksum in packages.items():
        root = f"{key[0]}-{key[1]}"
        archive, _, expected_members = candidates[key][0]
        package_root = destination / root
        package_root.mkdir(mode=0o755)
        package_root.chmod(0o755)
        generated_hashes = {}
        try:
            with tarfile.open(archive, mode="r|gz") as bundle:
                seen_members = 0
                seen_bytes = 0
                for member in bundle:
                    if seen_members >= remaining_members:
                        raise DependencyFailure("dependency archive member limit exceeded")
                    descriptor = _describe_member(member, root)
                    if seen_members >= len(expected_members) or descriptor != expected_members[seen_members]:
                        raise DependencyFailure(f"archive drift detected: {archive.name}")
                    name, relative, kind, size, mode = descriptor
                    if kind == "file":
                        if seen_bytes + size > remaining_bytes:
                            raise DependencyFailure("dependency archive byte limit exceeded")
                        seen_bytes += size
                    seen_members += 1
                    if not relative:
                        continue
                    output = package_root / relative
                    if kind == "directory":
                        output.mkdir(mode=0o755, parents=True, exist_ok=True)
                        output.chmod(0o755)
                        continue
                    output.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
                    parent = output.parent
                    while parent != destination:
                        parent.chmod(0o755)
                        parent = parent.parent
                    source = bundle.extractfile(member)
                    if source is None:
                        raise DependencyFailure(f"cannot read archive member: {name}")
                    digest = hashlib.sha256()
                    length = 0
                    with output.open("xb") as target:
                        for chunk in iter(lambda: source.read(1024 * 1024), b""):
                            length += len(chunk)
                            digest.update(chunk)
                            target.write(chunk)
                    if length != size:
                        raise DependencyFailure(f"archive member length mismatch: {name}")
                    output.chmod(0o755 if mode & 0o111 else 0o644)
                    actual = digest.hexdigest()
                    generated_hashes[relative] = actual
                    file_hashes[f"{root}/{relative}"] = actual
                if seen_members != len(expected_members):
                    raise DependencyFailure(f"archive drift detected: {archive.name}")
                remaining_members -= seen_members
                remaining_bytes -= seen_bytes
        except DependencyFailure:
            raise
        except (OSError, tarfile.TarError, EOFError) as error:
            raise DependencyFailure(f"dependency construction failed: {root}") from error
        metadata = json.dumps(
            {"files": dict(sorted(generated_hashes.items())), "package": checksum},
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        metadata_path = package_root / ".cargo-checksum.json"
        with metadata_path.open("xb") as target:
            target.write(metadata)
        metadata_path.chmod(0o644)
    return file_hashes


def prepare_dependencies(lockfile: Path, archives: list[Path], destination: Path) -> dict:
    """Construct checksum-closed dependency sources without builds or network access."""
    lock_digest, packages, candidates, archive_hashes = _preflight(lockfile, archives, destination)
    try:
        file_hashes = _construct(destination, packages, candidates)
    except DependencyFailure:
        raise
    except OSError as error:
        raise DependencyFailure("dependency construction failed") from error
    if _hash_file(lockfile) != lock_digest:
        raise DependencyFailure("lockfile drift detected")
    for values in candidates.values():
        for archive, digest, _ in values:
            if _hash_file(archive) != digest:
                raise DependencyFailure(f"archive drift detected: {archive.name}")
    return {
        "status": "dependencies-prepared",
        "qualification": False,
        "package_count": len(packages),
        "archive_sha256": archive_hashes,
        "file_sha256": file_hashes,
    }
