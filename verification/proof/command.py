"""Linux command evidence with an owned, resource-bounded systemd service.

This executes commands, not proofs. The caller must independently bind source,
tools, arguments and expected outcomes before interpreting a receipt.
"""

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import time
import uuid


ENV_KEYS = {
    "PATH", "LANG", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_HOME",
    "CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "TMPDIR",
}
LIMITS = {
    "CPUQuotaPerSecUSec": "2s",
    "MemoryHigh": str(2 * 1024**3),
    "MemoryMax": str(4 * 1024**3),
    "MemorySwapMax": "0",
    "TasksMax": "256",
    "LimitFSIZE": str(64 * 1024**2),
    "RuntimeMaxUSec": "10min",
}
STATE_KEYS = {
    "Id", "ActiveState", "SubState", "Result", "ExecMainCode",
    "ExecMainStatus", "ExecMainPID", "MemoryPeak", "CPUUsageNSec",
    "ExecMainStartTimestampMonotonic", "ExecMainExitTimestampMonotonic",
    *LIMITS,
}


class CommandFailure(Exception):
    """Execution or evidence capture failed; retained output is not a pass."""


def _write(path, value):
    with path.open("x", encoding="utf-8") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def _digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def _client_environment():
    runtime = Path("/run/user") / str(os.getuid())
    for path, predicate in ((runtime, stat.S_ISDIR), (runtime / "bus", stat.S_ISSOCK)):
        metadata = path.lstat()
        if metadata.st_uid != os.getuid() or not predicate(metadata.st_mode):
            raise CommandFailure("user bus path has wrong owner or file type")
    return {
        "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8",
        "XDG_RUNTIME_DIR": str(runtime),
        "DBUS_SESSION_BUS_ADDRESS": f"unix:path={runtime}/bus",
    }


def _control(argv, environment, destination, label):
    _write(destination / f"{label}.command.json", {"argv": argv, "env": environment})
    with (destination / f"{label}.stdout").open("xb") as stdout, (
        destination / f"{label}.stderr"
    ).open("xb") as stderr:
        result = subprocess.run(
            argv, env=environment, stdin=subprocess.DEVNULL,
            stdout=stdout, stderr=stderr, timeout=30, check=False,
        )
    _write(destination / f"{label}.exit.json", {"exit_code": result.returncode})
    return result.returncode


def _state(unit, environment):
    result = subprocess.run(
        ["/usr/bin/systemctl", "--user", "show", unit,
         "--property=" + ",".join(sorted(STATE_KEYS))],
        env=environment, stdin=subprocess.DEVNULL, capture_output=True,
        timeout=30, check=False, text=True,
    )
    if result.returncode != 0:
        raise CommandFailure("cannot observe owned service state")
    fields = {}
    for line in result.stdout.splitlines():
        key, separator, value = line.partition("=")
        if not separator or key not in STATE_KEYS or key in fields:
            raise CommandFailure("malformed service state")
        fields[key] = value
    if set(fields) != STATE_KEYS or fields["Id"] != unit:
        raise CommandFailure("incomplete or mismatched service state")
    return fields


def run_command(argv: list[str], cwd: Path, environment: dict[str, str], output: Path) -> dict:
    """Run one command with fresh output, bounded resources and a retained receipt.

    Nonzero child exits are observations, not transport failures. No success
    result implies proof or qualification. Every child, including witnesses,
    runs in the same kind of capped service, with no inherited environment.
    """
    if (
        type(argv) is not list or not argv or len(argv) > 256
        or any(type(arg) is not str or "\0" in arg for arg in argv)
        or not Path(argv[0]).is_absolute()
    ):
        raise CommandFailure("command requires bounded argv and an absolute executable")
    try:
        argv_bytes = sum(len(arg.encode("utf-8")) for arg in argv)
    except UnicodeError as error:
        raise CommandFailure("command arguments must encode as UTF-8") from error
    if argv_bytes > 65536:
        raise CommandFailure("command arguments exceed 64 KiB")
    try:
        executable = Path(argv[0]).stat()
    except OSError as error:
        raise CommandFailure("command executable is unavailable") from error
    if not stat.S_ISREG(executable.st_mode) or not os.access(argv[0], os.X_OK):
        raise CommandFailure("command executable is not an executable regular file")
    if (
        type(environment) is not dict or not set(environment) <= ENV_KEYS
        or any(type(value) is not str or "\0" in value for value in environment.values())
        or len(json.dumps(environment)) > 65536
    ):
        raise CommandFailure("child environment is not a bounded allow-listed mapping")
    if "CARGO_BUILD_JOBS" in environment and environment["CARGO_BUILD_JOBS"] != "2":
        raise CommandFailure("Cargo build parallelism must be two")
    try:
        cwd = cwd.resolve(strict=True)
        if not cwd.is_dir():
            raise CommandFailure("command cwd must be a directory")
        if output.name in {"", ".", ".."}:
            raise CommandFailure("command evidence requires a new named directory")
        output = output.parent.resolve(strict=True) / output.name
        if output.is_relative_to(cwd):
            raise CommandFailure("command evidence must be outside its source directory")
        output.mkdir(mode=0o700)
    except OSError as error:
        raise CommandFailure("command evidence/source path is unavailable or already exists") from error
    unit = f"liminal-formal-{uuid.uuid4().hex}.service"
    record = {
        "schema": "liminal-command-evidence-v1", "qualification": False,
        "argv": argv, "cwd": str(cwd), "env": dict(environment),
        "unit": unit, "limits": LIMITS, "status": "execution-incomplete",
    }
    _write(output / "request.json", record)
    for name in ("stdout", "stderr"):
        (output / name).touch(exist_ok=False)
    (output / "states.jsonl").touch(exist_ok=False)
    client_environment = None
    launch_attempted = False
    failure = None
    try:
        client_environment = _client_environment()
        launch = [
            "/usr/bin/systemd-run", "--user", "--quiet", "--expand-environment=no",
            "--unit=" + unit,
            "--working-directory=" + str(cwd),
            "--property=RemainAfterExit=yes", "--property=CPUAccounting=yes",
            "--property=MemoryAccounting=yes", "--property=CPUQuota=200%",
            "--property=MemoryHigh=2G", "--property=MemoryMax=4G",
            "--property=MemorySwapMax=0", "--property=TasksMax=256",
            "--property=LimitFSIZE=67108864", "--property=RuntimeMaxSec=600",
            "--property=StandardInput=null",
            "--property=StandardOutput=append:" + str(output / "stdout"),
            "--property=StandardError=append:" + str(output / "stderr"),
            "/usr/bin/env", "-i",
            *(f"{key}={value}" for key, value in sorted(environment.items())), *argv,
        ]
        launch_attempted = True
        if _control(launch, client_environment, output, "launch") != 0:
            raise CommandFailure("service launch failed; no child success may be inferred")
        deadline = time.monotonic() + 630
        while True:
            observed = _state(unit, client_environment)
            with (output / "states.jsonl").open("a", encoding="utf-8") as states:
                states.write(json.dumps(observed, sort_keys=True) + "\n")
            if any(observed[key] != value for key, value in LIMITS.items()):
                raise CommandFailure("observed service limits differ from requested limits")
            if observed["SubState"] in {"exited", "failed", "dead"}:
                break
            if time.monotonic() >= deadline:
                raise CommandFailure("service observation deadline exceeded")
            time.sleep(0.1)
        _write(output / "terminal-state.json", observed)
        if observed["ExecMainCode"] not in {"1", "2", "3"}:
            raise CommandFailure("service did not report a completed child process")
        record.update(
            status="command-completed", service=observed,
            exit_code=int(observed["ExecMainStatus"]) if observed["ExecMainCode"] == "1" else None,
            signal=int(observed["ExecMainStatus"]) if observed["ExecMainCode"] != "1" else None,
        )
    except (CommandFailure, OSError, ValueError, subprocess.SubprocessError) as error:
        failure = str(error)
    finally:
        if launch_attempted and client_environment is not None:
            try:
                stop = _control(
                    ["/usr/bin/systemctl", "--user", "stop", unit],
                    client_environment, output, "stop",
                )
                if stop != 0:
                    failure = failure or "owned service stop failed; inspect retained unit"
                # An already-unloaded unit makes reset-failed return nonzero.
                # Retain that exit, then establish absence independently below.
                reset = _control(
                    ["/usr/bin/systemctl", "--user", "reset-failed", unit],
                    client_environment, output, "reset-failed",
                )
                cleanup = _control(
                    ["/usr/bin/systemctl", "--user", "show", unit,
                     "--property=Id,LoadState,ActiveState,SubState"],
                    client_environment, output, "cleanup-state",
                )
                cleanup_lines = (output / "cleanup-state.stdout").read_text().splitlines()
                absent = cleanup == 0 and len(cleanup_lines) == 4 and set(cleanup_lines) == {
                    f"Id={unit}", "LoadState=not-found", "ActiveState=inactive", "SubState=dead",
                }
                record["cleanup"] = {
                    "stop_exit": stop, "reset_exit": reset,
                    "state_exit": cleanup, "unit_absent": absent,
                }
                if not absent:
                    failure = failure or "owned service absence was not established"
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                failure = failure or f"owned service cleanup failed: {error}"
        if failure is not None:
            record.update(status="execution-incomplete", error=failure)
        record["stdout_sha256"] = _digest(output / "stdout")
        record["stderr_sha256"] = _digest(output / "stderr")
        _write(output / "receipt.json", record)
    if failure is not None:
        raise CommandFailure(failure)
    return record
