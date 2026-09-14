"""Prepare fixed proof sandbox requests without executing them."""

import os
from pathlib import Path
import stat


FORBIDDEN_COMPONENTS = {"heldout", "liminal-5.3-spark"}
COMMANDS = {
    "verify": [
        "/verus/cargo-verus", "verify", "--fwd-verus-args-to", "roots",
        "--config", 'source.crates-io.replace-with="vendored-sources"',
        "--config", 'source.vendored-sources.directory="/vendor"', "--locked",
        "--offline", "-p", "liminal-safety", "--", "--output-json", "--no-cheating",
    ],
    "ordinary-build": [
        "/rust/bin/cargo", "build", "--release", "--locked", "--offline", "-p",
        "liminal-safety", "--example", "acknowledgement_witness", "--config",
        'source.crates-io.replace-with="vendored-sources"', "--config",
        'source.vendored-sources.directory="/vendor"',
    ],
    "verified-build": [
        "/verus/cargo-verus", "build", "--fwd-verus-args-to", "roots", "--release",
        "--locked", "--offline", "-p", "liminal-safety", "--config",
        'source.crates-io.replace-with="vendored-sources"', "--config",
        'source.vendored-sources.directory="/vendor"', "--example",
        "acknowledgement_witness", "--", "--output-json", "--no-cheating",
    ],
}


class SandboxFailure(Exception):
    """The sandbox request cannot be prepared under the closed contract."""


def _reject_runtime_overlap(path: Path, label: str) -> None:
    if path == Path("/") or path == Path("/usr") or Path("/usr") in path.parents:
        raise SandboxFailure(f"{label} overlaps the host runtime tree")


def _check_text_path(path: Path, label: str) -> None:
    if not isinstance(path, Path):
        raise SandboxFailure(f"{label} must be a Path")
    try:
        encoded = os.fsencode(path)
        encoded.decode("utf-8")
    except (UnicodeError, ValueError) as error:
        raise SandboxFailure(f"{label} is not a valid UTF-8 path") from error
    if b"\0" in encoded or not path.is_absolute() or ".." in path.parts:
        raise SandboxFailure(f"{label} is not a canonical absolute path")
    if any(part in FORBIDDEN_COMPONENTS for part in path.parts):
        raise SandboxFailure(f"{label} contains a forbidden component")
    _reject_runtime_overlap(path, label)


def _reject_symlink_components(path: Path, label: str) -> None:
    current = Path(path.anchor)
    try:
        for part in path.parts[1:]:
            current /= part
            if stat.S_ISLNK(current.lstat().st_mode):
                raise SandboxFailure(f"{label} contains a symlink component")
    except FileNotFoundError:
        raise SandboxFailure(f"{label} is missing") from None
    except OSError as error:
        raise SandboxFailure(f"cannot inspect {label}") from error


def _existing_directory(path: Path, label: str) -> Path:
    _check_text_path(path, label)
    _reject_symlink_components(path, label)
    try:
        if not stat.S_ISDIR(path.stat().st_mode):
            raise SandboxFailure(f"{label} must be a directory")
        resolved = path.resolve(strict=True)
        _reject_runtime_overlap(resolved, label)
        return resolved
    except OSError as error:
        raise SandboxFailure(f"cannot inspect {label}") from error


def _contains(left: Path, right: Path) -> bool:
    return left == right or left in right.parents


def _validate_paths(roots: tuple[Path, ...], destination: Path) -> None:
    canonical = [
        _existing_directory(path, label)
        for path, label in zip(roots, ("source", "rust", "verus", "vendor"))
    ]
    for index, left in enumerate(canonical):
        for right in canonical[index + 1:]:
            if _contains(left, right) or _contains(right, left):
                raise SandboxFailure("sandbox input roots must not overlap")

    _check_text_path(destination, "destination")
    if destination.name in {"", ".", ".."}:
        raise SandboxFailure("sandbox destination must be named")
    parent = _existing_directory(destination.parent, "destination parent")
    candidate = parent / destination.name
    try:
        destination.lstat()
    except FileNotFoundError:
        pass
    except OSError as error:
        raise SandboxFailure("cannot inspect sandbox destination") from error
    else:
        raise SandboxFailure("sandbox destination already exists")
    if any(_contains(root, candidate) or _contains(candidate, root) for root in canonical):
        raise SandboxFailure("sandbox destination must not overlap an input")


def prepare_sandbox(
    source: Path,
    rust: Path,
    verus: Path,
    vendor: Path,
    destination: Path,
    operation: str,
) -> dict:
    """Prepare one fixed unqualified sandbox request."""
    if type(operation) is not str or operation not in COMMANDS:
        raise SandboxFailure("unsupported sandbox operation")
    roots = (source, rust, verus, vendor)
    _validate_paths(roots, destination)

    try:
        destination.mkdir(mode=0o700)
        for name in ("target", "cargo-home", "rustup-home", "tmp", "logs", "command-parent"):
            (destination / name).mkdir(mode=0o700)
    except OSError as error:
        raise SandboxFailure("cannot create sandbox destination") from error

    argv = [
        "/usr/bin/bwrap", "--unshare-all", "--unshare-user", "--die-with-parent",
        "--new-session", "--disable-userns", "--clearenv", "--proc", "/proc",
        "--dev", "/dev", "--ro-bind", "/usr", "/usr", "--symlink", "usr/bin",
        "/bin", "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
        "--ro-bind", str(rust), "/rust", "--ro-bind", str(verus), "/verus",
        "--ro-bind", str(source), "/work", "--ro-bind", str(vendor), "/vendor",
        "--bind", str(destination / "target"), "/target",
        "--bind", str(destination / "cargo-home"), "/cargo-home",
        "--bind", str(destination / "rustup-home"), "/rustup-home",
        "--ro-bind", str(rust),
        "/rustup-home/toolchains/1.97.1-x86_64-unknown-linux-gnu",
        "--bind", str(destination / "tmp"), "/tmp",
        "--bind", str(destination / "logs"), "/logs",
        "--setenv", "PATH", "/verus:/rust/bin:/usr/bin:/bin",
        "--setenv", "HOME", "/cargo-home", "--setenv", "CARGO_HOME", "/cargo-home",
        "--setenv", "CARGO_TARGET_DIR", "/target", "--setenv", "CARGO_BUILD_JOBS", "2",
        "--setenv", "CARGO_NET_OFFLINE", "true", "--setenv", "RUSTUP_HOME",
        "/rustup-home", "--setenv", "RUSTUP_TOOLCHAIN", "/rust", "--setenv",
        "TMPDIR", "/tmp", "--setenv", "LANG", "C.UTF-8", "--setenv", "LC_ALL",
        "C.UTF-8", "--setenv", "LD_LIBRARY_PATH", "/rust/lib:/verus", "--chdir",
        "/work", "--", "/usr/bin/strace", "-s", "65536", "-f", "-qq", "-e",
        "trace=execve,execveat,open,openat,openat2,chdir,fchdir", "-o",
        "/logs/trace.log", *COMMANDS[operation],
    ]
    return {
        "schema": "liminal-sandbox-request-v1",
        "status": "sandbox-prepared",
        "qualification": False,
        "operation": operation,
        "argv": argv,
        "cwd": str(destination / "command-parent"),
        "environment": {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
        "output": str(destination / "command-evidence"),
    }
