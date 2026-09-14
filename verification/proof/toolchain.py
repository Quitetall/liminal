"""Construct a bounded Rust payload; callers own manifest-pin authority.

This does not install rustup metadata, execute installers, select a compiler,
or qualify a proof. Failed writes retain the new partial destination as evidence.
"""

import hashlib
from pathlib import Path
import re
import stat
import tarfile
import tomllib


TARGET = "x86_64-unknown-linux-gnu"
VERSION = "1.97.1"
PACKAGES = {
    "rustc": (f"rustc-{VERSION}-{TARGET}", "rustc"),
    "cargo": (f"cargo-{VERSION}-{TARGET}", "cargo"),
    "rust-std": (f"rust-std-{VERSION}-{TARGET}", f"rust-std-{TARGET}"),
    "rustc-dev": (f"rustc-dev-{VERSION}-{TARGET}", "rustc-dev"),
    "llvm-tools-preview": (f"llvm-tools-{VERSION}-{TARGET}", "llvm-tools-preview"),
    "rust-src": (f"rust-src-{VERSION}", "rust-src"),
}
SHA256 = re.compile(r"[0-9a-f]{64}")
MANIFEST_LIMIT = 2 * 1024 * 1024
ARCHIVE_LIMIT = 512 * 1024 * 1024
ARCHIVES_LIMIT = 1536 * 1024 * 1024
FILE_LIMIT = 256 * 1024 * 1024
TOTAL_LIMIT = 2 * 1024 * 1024 * 1024
MEMBER_LIMIT = 20_000
METADATA = frozenset({
    "LICENSE-APACHE", "LICENSE-MIT", "LICENSE-THIRD-PARTY", "LICENSE.TXT",
    "COPYRIGHT", "README.md", "README.txt", "builder-config", "install.sh",
    "git-commit-hash", "rust-installer-version", "version", "git-commit-info",
    "components",
})


class ToolchainFailure(Exception):
    """Rust construction refused or failed; no qualification is implied."""


def _digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _regular(path, limit):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
        raise ToolchainFailure("input must be a bounded regular file")
    return info.st_size


def _path(raw, directory=False):
    if not raw or raw.startswith("/") or any(c in raw for c in ("\\", "\0", ":")):
        raise ToolchainFailure("unsafe archive path")
    name = raw[:-1] if directory and raw.endswith("/") else raw
    if any(part in ("", ".", "..") for part in name.split("/")):
        raise ToolchainFailure("unsafe archive path segments")
    if len(name.encode("utf-8")) > 4096:
        raise ToolchainFailure("archive path exceeds limit")
    return name


def _inspect(archive, top, component, byte_budget, member_budget):
    entries = {}
    declarations = None
    total = 0
    boundary = f"{top}/{component}"
    manifest = f"{boundary}/manifest.in"
    with tarfile.open(archive, mode="r|xz") as stream:
        for member in stream:
            name = _path(member.name, member.isdir())
            if name in entries or len(entries) >= member_budget:
                raise ToolchainFailure("duplicate member or member limit exceeded")
            if name != top and not name.startswith(top + "/"):
                raise ToolchainFailure("archive root differs from selected package")
            if member.isdir():
                if member.size != 0 or member.mode != 0o755:
                    raise ToolchainFailure("unsupported directory metadata")
                row = {"kind": "directory", "mode": member.mode}
            elif member.isfile():
                if not 0 <= member.size <= FILE_LIMIT or member.mode not in (0o644, 0o755):
                    raise ToolchainFailure("unsupported file metadata or size")
                total += member.size
                if total > byte_budget:
                    raise ToolchainFailure("uncompressed archive exceeds limit")
                if name == manifest and member.size > MANIFEST_LIMIT:
                    raise ToolchainFailure("component manifest exceeds limit")
                digest = hashlib.sha256()
                captured = bytearray() if name == manifest else None
                size = 0
                with stream.extractfile(member) as source:
                    for chunk in iter(lambda: source.read(1024 * 1024), b""):
                        size += len(chunk)
                        digest.update(chunk)
                        if captured is not None:
                            captured.extend(chunk)
                if size != member.size:
                    raise ToolchainFailure("truncated archive file")
                row = {"kind": "file", "mode": member.mode, "bytes": size,
                       "sha256": digest.hexdigest()}
                if captured is not None:
                    declarations = bytes(captured).decode("utf-8")
            else:
                raise ToolchainFailure("links and special archive members refused")
            if name not in (top, boundary) and not name.startswith(boundary + "/"):
                relative = name[len(top) + 1:]
                if relative not in METADATA or row["kind"] != "file":
                    raise ToolchainFailure("unrecognized top-level package content")
            entries[name] = row
    if declarations is None or not declarations.strip():
        raise ToolchainFailure("component manifest missing or empty")
    for container in (top, boundary):
        if entries.get(container, {}).get("kind") != "directory":
            raise ToolchainFailure("component container missing")
    # Every nested entry must have a real directory parent, not an implicit
    # filesystem-created path that could conceal a conflicting file member.
    for name in entries:
        if name == top:
            continue
        if entries.get(name.rsplit("/", 1)[0], {}).get("kind") != "directory":
            raise ToolchainFailure("archive member parent is not a declared directory")
    payload = {name[len(boundary) + 1:]: row for name, row in entries.items()
               if name.startswith(boundary + "/") and name != manifest}
    seen = set()
    covered = set()
    for line in declarations.splitlines():
        kind, separator, raw = line.partition(":")
        if not separator or kind not in ("file", "dir"):
            raise ToolchainFailure("invalid component manifest directive")
        name = _path(raw)
        if name in seen:
            raise ToolchainFailure("duplicate component manifest directive")
        seen.add(name)
        expected = "file" if kind == "file" else "directory"
        if payload.get(name, {}).get("kind") != expected:
            raise ToolchainFailure("component directive does not name matching payload")
        covered.add(name)
        if kind == "dir":
            covered.update(p for p in payload if p.startswith(name + "/"))
    if any(row["kind"] == "file" and name not in covered for name, row in payload.items()):
        raise ToolchainFailure("undeclared regular component payload")
    # Empty directories are preserved, not exempted from output comparison.
    return entries, payload, total


def construct_rust_toolchain(channel_manifest: Path, channel_sha256: str,
                             archives: dict[str, Path], destination: Path) -> dict:
    """Create a new external root from all six pinned Linux component payloads.

    The supplied channel digest must come from caller-approved authority. This
    function establishes byte identity only. Output is not a registered rustup
    installation; callers must independently bind and verify compiler selection.
    """
    try:
        return _construct(channel_manifest, channel_sha256, archives, destination)
    except ToolchainFailure:
        raise
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError, EOFError) as error:
        raise ToolchainFailure(f"toolchain construction failed: {type(error).__name__}") from error


def _construct(channel_manifest, channel_sha256, archives, destination):
    if type(channel_sha256) is not str or SHA256.fullmatch(channel_sha256) is None:
        raise ToolchainFailure("invalid channel manifest pin")
    _regular(channel_manifest, MANIFEST_LIMIT)
    raw = channel_manifest.read_bytes()
    if len(raw) > MANIFEST_LIMIT or hashlib.sha256(raw).hexdigest() != channel_sha256:
        raise ToolchainFailure("channel manifest pin mismatch")
    channel = tomllib.loads(raw.decode("utf-8"))
    if channel.get("manifest-version") != "2":
        raise ToolchainFailure("unsupported channel manifest version")
    version = channel["pkg"]["rust"]["version"]
    if type(version) is not str or version.split(" ", 1)[0] != VERSION:
        raise ToolchainFailure("Rust version differs from selected profile")
    if type(archives) is not dict or set(archives) != set(PACKAGES):
        raise ToolchainFailure("archive keys must name exactly six selected packages")
    destination = destination.absolute()
    if destination.parent.resolve(strict=True) != destination.parent:
        raise ToolchainFailure("destination parent must be physically canonical")
    if destination.exists() or destination.is_symlink():
        raise ToolchainFailure("destination already exists")
    repository = Path(__file__).resolve().parents[2]
    if destination.is_relative_to(repository):
        raise ToolchainFailure("destination must be external to constructor repository")
    # Never place the result inside an input file, nor overwrite any input.
    if destination == channel_manifest.resolve():
        raise ToolchainFailure("destination conflicts with manifest")
    inspected = {}
    projected = {}
    hashes = {}
    compressed_total = 0
    unpacked_total = 0
    member_total = 0
    for package, (top, component) in PACKAGES.items():
        archive = archives[package]
        compressed_total += _regular(archive, ARCHIVE_LIMIT)
        if compressed_total > ARCHIVES_LIMIT:
            raise ToolchainFailure("aggregate compressed archive limit exceeded")
        target = "*" if package == "rust-src" else TARGET
        record = channel["pkg"][package]["target"][target]
        expected = record["xz_hash"]
        if (record.get("available") is not True or type(expected) is not str
                or SHA256.fullmatch(expected) is None):
            raise ToolchainFailure("selected package unavailable or checksum invalid")
        if _digest(archive) != expected:
            raise ToolchainFailure("archive checksum mismatch")
        hashes[package] = expected
        entries, payload, size = _inspect(
            archive, top, component, TOTAL_LIMIT - unpacked_total, MEMBER_LIMIT - member_total
        )
        unpacked_total += size
        member_total += len(entries)
        if unpacked_total > TOTAL_LIMIT or member_total > MEMBER_LIMIT:
            raise ToolchainFailure("aggregate uncompressed payload limit exceeded")
        for name, row in payload.items():
            if name in projected and projected[name] != row:
                raise ToolchainFailure("conflicting cross-package payload")
            projected[name] = row
        inspected[package] = (entries, payload)
    for name in projected:
        if "/" in name and projected.get(name.rsplit("/", 1)[0], {}).get("kind") != "directory":
            raise ToolchainFailure("conflicting projected parent")
    destination.mkdir(mode=0o755)
    destination.chmod(0o755)
    for name, row in sorted(projected.items(), key=lambda item: (item[0].count("/"), item[0])):
        if row["kind"] == "directory":
            output = destination / name
            output.mkdir(mode=row["mode"])
            output.chmod(row["mode"])
    written = set()
    for package, (top, component) in PACKAGES.items():
        archive = archives[package]
        if _digest(archive) != hashes[package]:
            raise ToolchainFailure("archive changed after inspection")
        entries, payload = inspected[package]
        boundary = f"{top}/{component}/"
        with tarfile.open(archive, mode="r|xz") as stream:
            for member in stream:
                name = _path(member.name, member.isdir())
                if not name.startswith(boundary):
                    continue
                relative = name[len(boundary):]
                if relative not in payload or payload[relative]["kind"] != "file" or relative in written:
                    continue
                expected = entries[name]
                if not member.isfile() or member.size != expected["bytes"] or member.mode != expected["mode"]:
                    raise ToolchainFailure("archive member changed after inspection")
                output = destination / relative
                digest = hashlib.sha256()
                size = 0
                with stream.extractfile(member) as source, output.open("xb") as sink:
                    for chunk in iter(lambda: source.read(1024 * 1024), b""):
                        size += len(chunk)
                        if size > expected["bytes"]:
                            raise ToolchainFailure("archive member grew after inspection")
                        sink.write(chunk)
                        digest.update(chunk)
                if size != expected["bytes"] or digest.hexdigest() != expected["sha256"]:
                    raise ToolchainFailure("constructed file differs from inspected payload")
                output.chmod(expected["mode"])
                written.add(relative)
    if written != {name for name, row in projected.items() if row["kind"] == "file"}:
        raise ToolchainFailure("constructed file set incomplete")
    return {"status": "toolchain-constructed", "qualification": False,
            "channel_sha256": channel_sha256, "archive_sha256": hashes,
            "entries": projected}
