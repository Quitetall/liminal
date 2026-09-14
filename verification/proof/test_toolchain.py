import hashlib
import io
import json
import lzma
from pathlib import Path
import stat
import subprocess
import tarfile
import tempfile
import unittest

from toolchain import ToolchainFailure, construct_rust_toolchain


TARGET = "x86_64-unknown-linux-gnu"
ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
COMPONENTS = {
    "rustc": (f"rustc-1.97.1-{TARGET}", "rustc"),
    "cargo": (f"cargo-1.97.1-{TARGET}", "cargo"),
    "rust-std": (f"rust-std-1.97.1-{TARGET}", f"rust-std-{TARGET}"),
    "rustc-dev": (f"rustc-dev-1.97.1-{TARGET}", "rustc-dev"),
    "llvm-tools-preview": (f"llvm-tools-1.97.1-{TARGET}", "llvm-tools-preview"),
    "rust-src": ("rust-src-1.97.1", "rust-src"),
}


class ToolchainConstructorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.destination = self.root / "constructed"
        self.channel = self.root / "channel.toml"
        self.archives = {}
        self.hashes = {}
        for package in COMPONENTS:
            self.write_archive(package)
        self.write_channel()

    def tearDown(self):
        self.temporary.cleanup()

    def write_archive(self, package, *, extra=None, payload=b"abc", manifest=None,
                      directory_count=0):
        top, component = COMPONENTS[package]
        prefix = f"{top}/{component}"
        path = self.root / (package + ".tar.xz")
        with tarfile.open(path, "w:xz", format=tarfile.GNU_FORMAT) as bundle:
            for directory in (top, prefix, prefix + "/bin", prefix + "/empty"):
                item = tarfile.TarInfo(directory)
                item.type = tarfile.DIRTYPE
                item.mode = 0o755
                bundle.addfile(item)
            # Deliberately leave empty/ undeclared: this is the real packaging
            # case, not a manifest declaration that masks missing coverage.
            data = (f"file:bin/{package}\n".encode() if manifest is None else manifest)
            for relative, contents, mode in (("manifest.in", data, 0o644),
                                              (f"bin/{package}", payload, 0o755)):
                item = tarfile.TarInfo(prefix + "/" + relative)
                item.size = len(contents)
                item.mode = mode
                bundle.addfile(item, io.BytesIO(contents))
            for index in range(directory_count):
                item = tarfile.TarInfo(prefix + f"/empty-{index}")
                item.type = tarfile.DIRTYPE
                item.mode = 0o755
                bundle.addfile(item)
            if extra:
                item, data = extra(prefix)
                bundle.addfile(item, io.BytesIO(data) if data is not None else None)
        self.archives[package] = path
        self.hashes[package] = hashlib.sha256(path.read_bytes()).hexdigest()

    def write_channel(self, *, version="1.97.1 (fixture)"):
        lines = ['manifest-version = "2"', '[pkg.rust]', f'version = "{version}"']
        for package in COMPONENTS:
            target = "*" if package == "rust-src" else TARGET
            lines.extend([f'[pkg."{package}".target."{target}"]',
                          'available = true', f'xz_hash = "{self.hashes[package]}"'])
        self.channel.write_text("\n".join(lines), encoding="utf-8")
        self.channel_hash = hashlib.sha256(self.channel.read_bytes()).hexdigest()

    def construct(self):
        return construct_rust_toolchain(self.channel, self.channel_hash,
                                        self.archives, self.destination)

    def refused_without_output(self):
        with self.assertRaises(ToolchainFailure):
            self.construct()
        self.assertFalse(self.destination.exists())

    def test_complete_payload_preserves_undeclared_empty_directory(self):
        result = self.construct()
        self.assertEqual(result["status"], "toolchain-constructed")
        self.assertIs(result["qualification"], False)
        self.assertEqual(list((self.destination / "empty").iterdir()), [])
        self.assertEqual(result["archive_sha256"], self.hashes)
        self.assertEqual(result["channel_sha256"], self.channel_hash)
        expected = {"bin", "empty"} | {f"bin/{package}" for package in COMPONENTS}
        self.assertEqual(set(result["entries"]), expected)
        actual = {str(path.relative_to(self.destination)) for path in self.destination.rglob("*")}
        self.assertEqual(actual, expected)
        for package in COMPONENTS:
            relative = f"bin/{package}"
            self.assertEqual((self.destination / relative).read_bytes(), b"abc")
            self.assertEqual(stat.S_IMODE((self.destination / relative).stat().st_mode), 0o755)
            self.assertEqual(result["entries"][relative]["sha256"], ABC_SHA256)

    def test_archive_hash_drift_refused(self):
        self.archives["rustc"].write_bytes(b"drift")
        self.refused_without_output()

    def test_channel_hash_drift_refused(self):
        self.channel.write_bytes(self.channel.read_bytes() + b"\n")
        self.refused_without_output()

    def test_missing_package_refused(self):
        del self.archives["rustc"]
        self.refused_without_output()

    def test_version_drift_refused(self):
        self.write_channel(version="1.97.2 (fixture)")
        self.refused_without_output()

    def test_existing_destination_untouched(self):
        self.destination.mkdir()
        sentinel = self.destination / "sentinel"
        sentinel.write_bytes(b"keep")
        with self.assertRaises(ToolchainFailure):
            self.construct()
        self.assertEqual(sentinel.read_bytes(), b"keep")

    def test_destination_parent_alias_refused(self):
        alias = self.root / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        self.destination = alias / "constructed"
        self.refused_without_output()

    def test_unsafe_and_duplicate_archive_paths_refused(self):
        for suffix in ("bin/../escape", "bin//escape", "bin/./escape", "bin/rustc"):
            with self.subTest(suffix=suffix):
                def extra(prefix):
                    item = tarfile.TarInfo(prefix + "/" + suffix)
                    item.mode = 0o644
                    item.size = 3
                    return item, b"abc"
                self.write_archive("rustc", extra=extra)
                self.write_channel()
                self.refused_without_output()

    def test_symlink_refused(self):
        def extra(prefix):
            item = tarfile.TarInfo(prefix + "/link")
            item.type = tarfile.SYMTYPE
            item.linkname = "bin/rustc"
            return item, None
        self.write_archive("rustc", extra=extra)
        self.write_channel()
        self.refused_without_output()

    def test_missing_declared_payload_refused(self):
        self.write_archive("rustc", manifest=b"file:bin/missing\n")
        self.write_channel()
        self.refused_without_output()

    def test_unrecognized_top_level_content_is_not_silently_discarded(self):
        def extra(prefix):
            item = tarfile.TarInfo(prefix.rsplit("/", 1)[0] + "/other-component")
            item.type = tarfile.DIRTYPE
            item.mode = 0o755
            return item, None
        self.write_archive("rustc", extra=extra)
        self.write_channel()
        self.refused_without_output()

    def test_aggregate_member_limit_refuses_before_later_invalid_member(self):
        self.write_archive("rustc", directory_count=10_000)
        def extra(prefix):
            item = tarfile.TarInfo(prefix + "/../late-invalid")
            item.type = tarfile.DIRTYPE
            item.mode = 0o755
            return item, None
        self.write_archive("cargo", directory_count=10_000, extra=extra)
        self.write_channel()
        with self.assertRaisesRegex(ToolchainFailure, "member limit exceeded"):
            self.construct()
        self.assertFalse(self.destination.exists())

    def test_conflicting_file_overlap_refused_and_identical_overlap_accepted(self):
        for data in (b"xyz", b"abc"):
            with self.subTest(data=data):
                def extra(prefix):
                    item = tarfile.TarInfo(prefix + "/bin/rustc")
                    item.mode = 0o755
                    item.size = len(data)
                    return item, data
                self.write_archive("cargo", extra=extra,
                                   manifest=b"file:bin/cargo\nfile:bin/rustc\n")
                self.write_channel()
                if data == b"xyz":
                    self.refused_without_output()
                else:
                    self.construct()
                    self.assertEqual((self.destination / "bin/rustc").read_bytes(), b"abc")

    def test_oversized_file_header_refused_without_reading_body(self):
        top, component = COMPONENTS["rustc"]
        headers = []
        for name in (top, f"{top}/{component}"):
            item = tarfile.TarInfo(name)
            item.type = tarfile.DIRTYPE
            item.mode = 0o755
            headers.append(item.tobuf(format=tarfile.GNU_FORMAT))
        item = tarfile.TarInfo(f"{top}/{component}/oversized")
        item.mode = 0o644
        item.size = 256 * 1024 * 1024 + 1
        headers.append(item.tobuf(format=tarfile.GNU_FORMAT))
        # Intentionally no giant body: size must refuse at its header, before
        # attempting a read that would instead diagnose the truncated fixture.
        self.archives["rustc"].write_bytes(lzma.compress(b"".join(headers) + b"\0" * 1024))
        self.hashes["rustc"] = hashlib.sha256(self.archives["rustc"].read_bytes()).hexdigest()
        self.write_channel()
        with self.assertRaisesRegex(ToolchainFailure, "unsupported file metadata or size"):
            self.construct()
        self.assertFalse(self.destination.exists())

    def test_write_failure_retains_partial_destination_without_modifying_inputs(self):
        archive_hashes = {
            package: hashlib.sha256(path.read_bytes()).hexdigest()
            for package, path in self.archives.items()
        }
        module = Path(__file__).with_name("toolchain.py")
        child = r"""
import importlib.util, json, resource, signal, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("toolchain_child", Path(sys.argv[1]))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
archives = {name: Path(path) for name, path in json.loads(sys.argv[4]).items()}
resource.setrlimit(resource.RLIMIT_FSIZE, (2, 2))
signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
try:
    module.construct_rust_toolchain(
        Path(sys.argv[2]), sys.argv[3], archives, Path(sys.argv[5])
    )
except module.ToolchainFailure:
    print("EXPECTED_TOOLCHAIN_FAILURE")
    raise SystemExit(0)
raise SystemExit(3)
"""
        result = subprocess.run(
            ["/usr/bin/python3", "-B", "-c", child, str(module), str(self.channel),
             self.channel_hash, json.dumps({name: str(path) for name, path in self.archives.items()}),
             str(self.destination)],
            cwd=module.parent, env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
            stdin=subprocess.DEVNULL, capture_output=True, text=True, check=False, timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "EXPECTED_TOOLCHAIN_FAILURE")
        self.assertTrue(self.destination.is_dir())
        self.assertEqual((self.destination / "bin/rustc").read_bytes(), b"ab")
        self.assertEqual(
            {package: hashlib.sha256(path.read_bytes()).hexdigest()
             for package, path in self.archives.items()},
            archive_hashes,
        )


if __name__ == "__main__":
    unittest.main()
