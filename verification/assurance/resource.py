"""Run standalone resource controls through the existing bounded command producer.

Exit 0: child passed with verified limits. Exit 1: observed child failure.
Exit 3: infrastructure/evidence unavailable. No unbounded fallback.
"""

import argparse
import importlib.util
import os
from pathlib import Path
import shutil
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["resource-run", "resource-lint"])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    if sys.platform != "linux":
        print("infrastructure unavailable: resource controls require Linux", file=sys.stderr)
        return 3
    try:
        spec = importlib.util.spec_from_file_location("assurance_bounded_command", root / "verification/proof/command.py")
        producer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(producer)
    except (ImportError, OSError, SyntaxError) as error:
        print(f"infrastructure unavailable: bounded producer: {error}", file=sys.stderr)
        return 3
    cargo = shutil.which("cargo")
    if cargo is None:
        print("infrastructure unavailable: cargo absent", file=sys.stderr)
        return 3
    environment = {key: value for key, value in os.environ.items() if key in producer.ENV_KEYS}
    environment.update(
        CARGO_BUILD_JOBS="2",
        CARGO_HOME=os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")),
        RUSTUP_HOME=os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup")),
    )
    argv = [cargo, "run" if args.command == "resource-run" else "clippy",
            "--locked", "--offline", "--manifest-path",
            str(root / "verification/resource-controls/Cargo.toml"),
            "--target-dir", environment.get("CARGO_TARGET_DIR", str(root / "target"))]
    if args.command == "resource-lint":
        argv.extend(["--all-targets", "--", "-Dwarnings"])
    try:
        record = producer.run_command(argv, root, environment, args.output)
    except (producer.CommandFailure, OSError, ValueError) as error:
        print(f"infrastructure unavailable: {error}", file=sys.stderr)
        return 3
    if record["exit_code"] != 0 or record["signal"] is not None or record["service"]["Result"] != "success":
        print(f"resource control failed; inspect {args.output}", file=sys.stderr)
        return 1
    print(f"bounded resource control passed; qualification: not-established; {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
