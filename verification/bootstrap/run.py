#!/usr/bin/env python3
"""Fail-closed runner for the bounded formal-tool bootstrap fixtures."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tomllib
from typing import Callable, NamedTuple


TIMEOUT_SECONDS = 300
SOURCE_FILES = (
    "admission/Cargo.toml",
    "admission/Cargo.lock",
    "admission/rust-toolchain.toml",
    "admission/src/main.rs",
    "false/admit_revision_false.rs",
    "model/positive/ToolControl.cfg",
    "model/positive/ToolControl.tla",
    "model/false/ToolControl.cfg",
    "model/false/ToolControl.tla",
)
BLOCKED_ENV = {
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC",
    "CARGO",
    "CARGO_TARGET_DIR",
}
EXPECTED_CLAIMS = {
    "formal_check_tool_validation": False,
    "bootstrap_tool_validation": True,
    "production_binding": False,
    "full_package_b_closure": False,
}


class Options(NamedTuple):
    verus_root: Path
    tlc_jar: Path
    output: Path


class BootstrapFailure(Exception):
    pass


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _default_process_runner(command, cwd, env, timeout):
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = process.communicate()
        raise subprocess.TimeoutExpired(
            command,
            timeout,
            output=stdout if stdout is not None else exc.stdout,
            stderr=stderr if stderr is not None else exc.stderr,
        ) from exc
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


class Evidence:
    def __init__(self, output: Path):
        self.output = output
        self.log_dir = output / "commands"
        self.log_dir.mkdir()
        self.commands = []

    def command(self, label, command, cwd, child_env, process_runner):
        index = len(self.commands) + 1
        record = {
            "index": index,
            "label": label,
            "command": [str(item) for item in command],
            "cwd": str(cwd),
            "env_pin": child_env,
            "timeout_seconds": TIMEOUT_SECONDS,
        }
        stem = self.log_dir / f"{index:02d}-{label}"
        (stem.with_suffix(".json")).write_text(json.dumps(record, indent=2) + "\n")
        try:
            result = process_runner(command, cwd, child_env, TIMEOUT_SECONDS)
            record.update(
                exit_code=result.returncode,
                stdout=result.stdout,
                stderr=result.stderr,
                transport_error=None,
            )
        except subprocess.TimeoutExpired as exc:
            def partial(value):
                if value is None:
                    return ""
                if isinstance(value, bytes):
                    return value.decode(errors="replace")
                return value

            record.update(
                exit_code=None,
                stdout=partial(exc.stdout),
                stderr=partial(exc.stderr),
                transport_error=f"TimeoutExpired: {exc}",
            )
        except Exception as exc:
            record.update(
                exit_code=None,
                stdout="",
                stderr="",
                transport_error=f"{type(exc).__name__}: {exc}",
            )
        (stem.with_suffix(".stdout")).write_text(record["stdout"])
        (stem.with_suffix(".stderr")).write_text(record["stderr"])
        (stem.with_suffix(".json")).write_text(json.dumps(record, indent=2) + "\n")
        self.commands.append(record)
        if record["transport_error"]:
            raise BootstrapFailure(f"{label}: process transport error: {record['transport_error']}")
        return record


def _expect_exit(record, expected):
    if record["exit_code"] != expected:
        raise BootstrapFailure(
            f"{record['label']}: expected exit {expected}, got {record['exit_code']}"
        )


def _combined(record):
    return record["stdout"] + "\n" + record["stderr"]


def _require_verus_positive(record):
    _expect_exit(record, 0)
    output = _combined(record)
    if not re.search(r"\b2045 verified,\s*0 errors\b", output):
        raise BootstrapFailure(f"{record['label']}: missing vstd 2045 verified, 0 errors")
    if not re.search(r"\b1 verified,\s*0 errors\b", output):
        raise BootstrapFailure(f"{record['label']}: missing root 1 verified, 0 errors")


def _require_tlc_positive(record):
    _expect_exit(record, 0)
    output = _combined(record)
    summaries = re.findall(
        r"(?m)^\s*(\d+) states generated, (\d+) distinct states found, "
        r"(\d+) states left on queue\.\s*$",
        output,
    )
    depths = re.findall(
        r"(?m)^The depth of the complete state graph search is (\d+)\.\s*$", output
    )
    if (
        "Model checking completed. No error has been found." not in output
        or not summaries
        or tuple(map(int, summaries[-1])) != (10, 5, 0)
        or not depths
        or int(depths[-1]) != 5
    ):
        raise BootstrapFailure(f"{record['label']}: incomplete positive TLC evidence")
    return {"generated_states": 10, "distinct_states": 5, "queued_states": 0, "depth": 5}


def _require_false_rust(record):
    _expect_exit(record, 1)
    output = _combined(record)
    if "postcondition not satisfied" not in output:
        raise BootstrapFailure("rust-false: missing postcondition diagnostic")
    if not re.search(r"\b0 verified,\s*1 errors\b", output):
        raise BootstrapFailure("rust-false: missing 0 verified, 1 errors")


def _require_false_tlc(record):
    _expect_exit(record, 12)
    output = _combined(record)
    if "Invariant EffectImpliesIntent is violated" not in output:
        raise BootstrapFailure("tlc-false: missing EffectImpliesIntent violation")
    summaries = re.findall(
        r"(?m)^\s*(\d+) states generated, (\d+) distinct states found, "
        r"(\d+) states left on queue\.\s*$",
        output,
    )
    if not summaries or tuple(map(int, summaries[-1][:2])) != (3, 3):
        raise BootstrapFailure("tlc-false: expected 3 generated and 3 distinct states")
    generated, distinct, queued = map(int, summaries[-1])
    return {
        "generated_states": generated,
        "distinct_states": distinct,
        "queued_states": queued,
        "violated_invariant": "EffectImpliesIntent",
    }


def _snapshot(paths):
    values = {}
    for name, path in paths.items():
        if not path.is_file():
            raise BootstrapFailure(f"missing input: {path}")
        values[name] = {"path": str(path), "sha256": _sha256(path)}
    return values


def _load_and_validate(script_dir: Path, options: Options, environ):
    blocked = sorted(
        name for name in environ if name in BLOCKED_ENV or name.startswith("VERUS_")
    )
    if blocked:
        raise BootstrapFailure("refusing ambient override variables: " + ", ".join(blocked))

    manifest_path = script_dir.parent / "tool-pins.json"
    if not manifest_path.is_file():
        raise BootstrapFailure(f"missing installed pin manifest: {manifest_path}")
    try:
        pins = json.loads(manifest_path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise BootstrapFailure(f"invalid pin manifest: {exc}") from exc
    try:
        if pins["status"] != "bootstrap-only":
            raise BootstrapFailure("pin manifest is not bootstrap-only")
        claims = pins["claims"]
        if not isinstance(claims, dict) or set(claims) != set(EXPECTED_CLAIMS):
            raise BootstrapFailure("pin manifest claims are not the exact closed set")
        if any(type(claims[name]) is not bool for name in EXPECTED_CLAIMS):
            raise BootstrapFailure("pin manifest claims must be JSON booleans")
        if claims != EXPECTED_CLAIMS:
            raise BootstrapFailure("pin manifest claims have unexpected values")
        tool_paths = {
            "verus": options.verus_root / "verus",
            "cargo-verus": options.verus_root / "cargo-verus",
            "rust_verify": options.verus_root / "rust_verify",
            "z3": options.verus_root / "z3",
            "tlc": options.tlc_jar,
        }
        expected = {
            "verus": pins["verus"]["verus_sha256"],
            "cargo-verus": pins["verus"]["cargo_verus_sha256"],
            "rust_verify": pins["verus"]["rust_verify_sha256"],
            "z3": pins["verus"]["z3"]["sha256"],
            "tlc": pins["tlc"]["jar_sha256"],
        }
        source_paths = {name: script_dir / name for name in SOURCE_FILES}
        source_pins = pins["bootstrap_sources"]
        if not isinstance(source_pins, dict) or set(source_pins) != set(SOURCE_FILES):
            raise BootstrapFailure("bootstrap source pin map is not the exact closed set")
        if any(
            not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value)
            for value in source_pins.values()
        ):
            raise BootstrapFailure("bootstrap source pins must be lowercase SHA-256 digests")
        tool_hashes = _snapshot(tool_paths)
        source_hashes = _snapshot(source_paths)
        for name, wanted in expected.items():
            if tool_hashes[name]["sha256"] != wanted:
                raise BootstrapFailure(f"tool hash mismatch: {name}")
        for name, wanted in source_pins.items():
            if source_hashes[name]["sha256"] != wanted:
                raise BootstrapFailure(f"bootstrap source hash mismatch: {name}")
        lock_hash = source_hashes["admission/Cargo.lock"]["sha256"]
        if lock_hash != pins["cargo_dependencies"]["cargo_lock_sha256"]:
            raise BootstrapFailure("Cargo.lock hash mismatch")
        vstd_pin = pins["cargo_dependencies"]["vstd"]
        cargo_toml_text = source_paths["admission/Cargo.toml"].read_text()
        cargo_lock_text = source_paths["admission/Cargo.lock"].read_text()
        cargo_toml = tomllib.loads(cargo_toml_text)
        cargo_lock = tomllib.loads(cargo_lock_text)
        if cargo_toml.get("dependencies", {}).get("vstd") != vstd_pin:
            raise BootstrapFailure("Cargo.toml does not contain the exact vstd pin")
        if cargo_toml.get("package", {}).get("publish") is not False:
            raise BootstrapFailure("bootstrap package must set publish = false")
        lock_version = vstd_pin.removeprefix("=")
        if not any(
            package.get("name") == "vstd" and package.get("version") == lock_version
            for package in cargo_lock.get("package", [])
        ):
            raise BootstrapFailure("Cargo.lock does not contain the exact vstd pin")
        for relative in ("admission/src/main.rs", "false/admit_revision_false.rs"):
            rust_source = source_paths[relative].read_text()
            if not re.search(r"(?m)^\s*fn\s+admit_revision\b", rust_source):
                raise BootstrapFailure(f"{relative}: admit_revision helper must be private")
        if pins["rust"]["channel"] != "1.97.1":
            raise BootstrapFailure("unexpected Rust channel pin")
        return pins, manifest_path, tool_paths, source_paths, tool_hashes, source_hashes
    except (KeyError, TypeError, tomllib.TOMLDecodeError) as exc:
        raise BootstrapFailure(f"pin manifest missing required field: {exc}") from exc


def _copy_sources(source_paths, work: Path):
    copied = {}
    for name, source in source_paths.items():
        destination = work / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        copied[name] = destination
    return copied


def _same_hashes(left, right):
    return {name: details["sha256"] for name, details in left.items()} == {
        name: details["sha256"] for name, details in right.items()
    }


def _run_bootstrap(
    options: Options,
    *,
    process_runner: Callable = _default_process_runner,
):
    script_dir = Path(__file__).resolve().parent
    environ = os.environ
    output = options.output.resolve()
    repository = script_dir.resolve().parents[1]
    verus_root = options.verus_root.resolve()
    if output == repository or output.is_relative_to(repository):
        print(f"refusing output directory inside source repository: {output}", file=sys.stderr)
        return 2
    if output == verus_root or output.is_relative_to(verus_root):
        print(f"refusing output directory inside pinned tool root: {output}", file=sys.stderr)
        return 2
    try:
        output.mkdir(parents=True, exist_ok=False)
    except FileExistsError:
        print(f"refusing existing output directory: {output}", file=sys.stderr)
        return 2
    evidence = Evidence(output)
    result = {"outcome": "bootstrap-only", "status": "failed", "error": None}
    try:
        (
            pins,
            manifest_path,
            tool_paths,
            source_paths,
            initial_tool_hashes,
            initial_source_hashes,
        ) = _load_and_validate(script_dir.resolve(), options, environ)
        result.update(
            manifest={"path": str(manifest_path), "sha256": _sha256(manifest_path)},
            runner={"path": str(script_dir / "run.py"), "sha256": _sha256(script_dir / "run.py")},
            tools=initial_tool_hashes,
            sources=initial_source_hashes,
            environment={
                "RUSTUP_TOOLCHAIN": "1.97.1",
                "PATH": str(options.verus_root.resolve()) + os.pathsep + environ.get("PATH", ""),
            },
        )
        work = output / "work"
        copied = _copy_sources(source_paths, work)
        staged_source_hashes = _snapshot(copied)
        if not _same_hashes(staged_source_hashes, initial_source_hashes):
            raise BootstrapFailure("staged source does not match repository source")

        false_work = output / "rust-false"
        false_work.mkdir()
        false_source = false_work / "admit_revision_false.rs"
        shutil.copyfile(copied["false/admit_revision_false.rs"], false_source)
        model_workdirs = {}
        for variant in ("positive", "false"):
            model_work = output / f"tlc-{variant}"
            model_work.mkdir()
            model_workdirs[variant] = model_work
            for suffix in ("cfg", "tla"):
                source = copied[f"model/{variant}/ToolControl.{suffix}"]
                shutil.copyfile(source, model_work / source.name)
        execution_paths = {
            name: path for name, path in copied.items() if name.startswith("admission/")
        }
        execution_paths["false/admit_revision_false.rs"] = false_source
        for variant in ("positive", "false"):
            for suffix in ("cfg", "tla"):
                name = f"model/{variant}/ToolControl.{suffix}"
                execution_paths[name] = model_workdirs[variant] / f"ToolControl.{suffix}"
        execution_source_hashes = _snapshot(execution_paths)
        if not _same_hashes(execution_source_hashes, initial_source_hashes):
            raise BootstrapFailure("execution source does not match repository source")
        result["staged_sources"] = staged_source_hashes
        result["execution_sources"] = execution_source_hashes
        child_env = {
            "PATH": result["environment"]["PATH"],
            "RUSTUP_TOOLCHAIN": "1.97.1",
        }
        cargo = shutil.which("cargo", path=child_env["PATH"])
        rustc = shutil.which("rustc", path=child_env["PATH"])
        java = shutil.which("java", path=child_env["PATH"])
        if not cargo or not rustc or not java:
            raise BootstrapFailure("cargo, rustc, and java must be available on controlled PATH")

        def invoke(label, command, cwd, target=None):
            command_env = dict(child_env)
            if target is not None:
                command_env["CARGO_TARGET_DIR"] = str(target)
            return evidence.command(
                label,
                [str(item) for item in command],
                cwd,
                command_env,
                process_runner,
            )

        cargo_version = invoke("cargo-version", [cargo, "--version"], work)
        _expect_exit(cargo_version, 0)
        if not re.search(r"^cargo 1\.97\.1(?:\s|$)", cargo_version["stdout"]):
            raise BootstrapFailure("cargo version is not exactly 1.97.1")
        rustc_version = invoke("rustc-version", [rustc, "-Vv"], work)
        _expect_exit(rustc_version, 0)
        rustc_text = _combined(rustc_version)
        if "release: 1.97.1" not in rustc_text:
            raise BootstrapFailure("rustc release is not exactly 1.97.1")
        if f'commit-hash: {pins["rust"]["rustc_commit"]}' not in rustc_text:
            raise BootstrapFailure("rustc commit does not match the pin")
        result["versions"] = {
            "cargo": cargo_version["stdout"],
            "rustc": rustc_version["stdout"],
        }
        result["counts"] = {}

        admission = work / "admission"
        cargo_verus = tool_paths["cargo-verus"].resolve()
        for cycle in (1, 2):
            verify = invoke(
                f"cargo-verus-verify-{cycle}",
                [cargo_verus, "verify", "--locked"],
                admission,
                output / f"target-cargo-verify-{cycle}",
            )
            _require_verus_positive(verify)
            result["counts"][f"cargo_verus_verify_{cycle}"] = {
                "vstd_verified": 2045,
                "root_verified": 1,
                "errors": 0,
            }
            verus_target = output / f"target-cargo-verus-build-{cycle}"
            built = invoke(
                f"cargo-verus-build-{cycle}",
                [cargo_verus, "build", "--locked", "--release"],
                admission,
                verus_target,
            )
            _require_verus_positive(built)
            result["counts"][f"cargo_verus_build_{cycle}"] = {
                "vstd_verified": 2045,
                "root_verified": 1,
                "errors": 0,
            }
            ordinary_target = output / f"target-cargo-build-{cycle}"
            ordinary = invoke(
                f"cargo-build-{cycle}",
                [cargo, "build", "--locked", "--release"],
                admission,
                ordinary_target,
            )
            _expect_exit(ordinary, 0)
            _expect_exit(
                invoke(
                    f"cargo-verus-binary-{cycle}",
                    [verus_target / "release" / "cargo-admission"],
                    admission,
                ),
                0,
            )
            _expect_exit(
                invoke(
                    f"cargo-binary-{cycle}",
                    [ordinary_target / "release" / "cargo-admission"],
                    admission,
                ),
                0,
            )

        false_record = invoke(
            "rust-false",
            [tool_paths["verus"].resolve(), "--compile", "admit_revision_false.rs"],
            false_work,
        )
        _require_false_rust(false_record)
        result["counts"]["rust_false"] = {"verified": 0, "errors": 1}

        for variant in ("positive", "false"):
            model_work = model_workdirs[variant]
            tlc_record = invoke(
                f"tlc-{variant}",
                [
                    java,
                    "-cp",
                    options.tlc_jar.resolve(),
                    "tlc2.TLC",
                    "-workers",
                    "1",
                    "-config",
                    "ToolControl.cfg",
                    "ToolControl.tla",
                ],
                model_work,
            )
            if variant == "positive":
                result["counts"]["tlc_positive"] = _require_tlc_positive(tlc_record)
            else:
                result["counts"]["tlc_false"] = _require_false_tlc(tlc_record)

        final_tool_hashes = _snapshot(tool_paths)
        final_source_hashes = _snapshot(source_paths)
        if final_tool_hashes != initial_tool_hashes:
            raise BootstrapFailure("tool input changed during execution")
        if final_source_hashes != initial_source_hashes:
            raise BootstrapFailure("bootstrap source input changed during execution")
        if _snapshot(copied) != staged_source_hashes:
            raise BootstrapFailure("staged source changed during execution")
        if _snapshot(execution_paths) != execution_source_hashes:
            raise BootstrapFailure("execution source changed during execution")
        if _sha256(script_dir / "run.py") != result["runner"]["sha256"]:
            raise BootstrapFailure("runner changed during execution")
        if _sha256(manifest_path) != result["manifest"]["sha256"]:
            raise BootstrapFailure("pin manifest changed during execution")
        result.update(status="passed", commands=evidence.commands)
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        return 0
    except BootstrapFailure as exc:
        result.update(error=str(exc), commands=evidence.commands)
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(f"bootstrap failed: {exc}", file=sys.stderr)
        return 1
    except Exception as exc:
        message = f"internal error: {type(exc).__name__}: {exc}"
        result.update(error=message, commands=evidence.commands)
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(f"bootstrap failed: {message}", file=sys.stderr)
        return 1


def _parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus-root", required=True, type=Path)
    parser.add_argument("--tlc-jar", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    values = parser.parse_args(argv)
    return Options(values.verus_root, values.tlc_jar, values.output)


def main(argv=None):
    return _run_bootstrap(_parse_args(argv))


if __name__ == "__main__":
    raise SystemExit(main())
