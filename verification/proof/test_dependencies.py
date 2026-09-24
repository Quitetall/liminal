import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("dependencies.py")
SPEC = importlib.util.spec_from_file_location("dependency_constructor", MODULE_PATH)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
DependencyFailure = RUNNER.DependencyFailure
prepare_dependencies = RUNNER.prepare_dependencies

ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
# The archives below are built at test time through the host's zlib, and gzip
# output differs between zlib builds (zlib-ng here, stock zlib on Ubuntu CI), so
# a digest pinned from one host names bytes another host never writes. Each
# test's lockfile names the archive that test actually built. The wrong-checksum
# test keeps its deliberately wrong value, so the checksum gate stays covered.


class DependencyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repository"
        self.repository.mkdir()
        self.lockfile = self.repository / "Cargo.lock"
        self.archive = self.root / "demo-1.0.0.crate"
        self.destination = self.root / "dependencies"
        self.write_archive(self.archive)

    def tearDown(self):
        self.temporary.cleanup()

    def archive_sha256(self):
        return hashlib.sha256(self.archive.read_bytes()).hexdigest()

    def write_lockfile(self, checksum):
        self.lockfile.write_text(
            'version = 4\n\n[[package]]\nname = "local"\nversion = "0.1.0"\n\n'
            '[[package]]\nname = "demo"\nversion = "1.0.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
            f'checksum = "{checksum}"\n',
            encoding="utf-8",
        )

    def write_archive(self, path):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w", format=tarfile.GNU_FORMAT) as bundle:
            for name, payload, mode in (
                ("demo-1.0.0/src/lib.rs", b"abc", 0o644),
                ("demo-1.0.0/.gitignore", b"abc", 0o644),
                ("demo-1.0.0/bin/tool", b"abc", 0o755),
            ):
                info = tarfile.TarInfo(name)
                info.size = len(payload)
                info.mode = mode
                info.mtime = 0
                bundle.addfile(info, io.BytesIO(payload))
        with path.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                compressed.write(buffer.getvalue())

    def write_custom_archive(self, kind, name):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w", format=tarfile.GNU_FORMAT) as bundle:
            info = tarfile.TarInfo(name)
            info.mode = 0o644
            info.mtime = 0
            if kind == "symlink":
                info.type = tarfile.SYMTYPE
                info.linkname = "src/lib.rs"
                bundle.addfile(info)
            else:
                info.size = 3
                bundle.addfile(info, io.BytesIO(b"abc"))
        with self.archive.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                compressed.write(buffer.getvalue())

    def write_member_budget_archive(self):
        with self.archive.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(
                    fileobj=compressed, mode="w|", format=tarfile.GNU_FORMAT
                ) as bundle:
                    for index in range(100_001):
                        info = tarfile.TarInfo(f"demo-1.0.0/d{index:06d}/")
                        info.type = tarfile.DIRTYPE
                        info.mode = 0o755
                        info.mtime = 0
                        bundle.addfile(info)
                    invalid = tarfile.TarInfo("demo-1.0.0/../invalid")
                    invalid.mode = 0o644
                    invalid.mtime = 0
                    bundle.addfile(invalid)

    def write_mode_archive(self, mode):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w", format=tarfile.GNU_FORMAT) as bundle:
            info = tarfile.TarInfo("demo-1.0.0/src/lib.rs")
            info.type = tarfile.REGTYPE
            info.size = 3
            info.mode = mode & 0o7777
            info.mtime = 0
            bundle.addfile(info, io.BytesIO(b"abc"))
        archive_bytes = bytearray(buffer.getvalue())
        archive_bytes[100:108] = f"{mode:07o}\0".encode("ascii")
        archive_bytes[148:156] = b"        "
        archive_bytes[148:156] = f"{sum(archive_bytes[:512]):06o}\0 ".encode("ascii")
        with self.archive.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                compressed.write(archive_bytes)

    def test_wrong_archive_checksum_fails_before_destination_exists(self):
        self.write_lockfile("0" * 64)

        with self.assertRaisesRegex(DependencyFailure, "checksum"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_valid_dependencies_preserve_files_git_control_and_modes(self):
        self.write_lockfile(self.archive_sha256())

        result = prepare_dependencies(self.lockfile, [self.archive], self.destination)

        package = self.destination / "demo-1.0.0"
        self.assertEqual((package / "src/lib.rs").read_bytes(), b"abc")
        self.assertEqual((package / ".gitignore").read_bytes(), b"abc")
        self.assertEqual((package / "bin/tool").read_bytes(), b"abc")
        self.assertEqual((package / "src/lib.rs").stat().st_mode & 0o777, 0o644)
        self.assertEqual((package / "bin/tool").stat().st_mode & 0o777, 0o755)
        self.assertEqual(
            json.loads((package / ".cargo-checksum.json").read_text(encoding="utf-8")),
            {
                "files": {
                    ".gitignore": ABC_SHA256,
                    "bin/tool": ABC_SHA256,
                    "src/lib.rs": ABC_SHA256,
                },
                "package": self.archive_sha256(),
            },
        )
        self.assertEqual(
            result,
            {
                "status": "dependencies-prepared",
                "qualification": False,
                "package_count": 1,
                "archive_sha256": {"demo-1.0.0": self.archive_sha256()},
                "file_sha256": {
                    "demo-1.0.0/.gitignore": ABC_SHA256,
                    "demo-1.0.0/bin/tool": ABC_SHA256,
                    "demo-1.0.0/src/lib.rs": ABC_SHA256,
                },
            },
        )

    def test_missing_archive_is_rejected_before_destination_exists(self):
        self.write_lockfile(self.archive_sha256())
        with self.assertRaisesRegex(DependencyFailure, "missing archive"):
            prepare_dependencies(self.lockfile, [], self.destination)
        self.assertFalse(self.destination.exists())

    def test_extra_archive_is_rejected_before_destination_exists(self):
        self.write_lockfile(self.archive_sha256())
        extra = self.root / "extra-1.0.0.crate"
        extra.write_bytes(self.archive.read_bytes())
        with self.assertRaisesRegex(DependencyFailure, "unknown archive"):
            prepare_dependencies(self.lockfile, [self.archive, extra], self.destination)
        self.assertFalse(self.destination.exists())

    def test_traversal_member_is_rejected_before_destination_exists(self):
        self.write_custom_archive("file", "demo-1.0.0/../escape")
        self.write_lockfile(self.archive_sha256())
        with self.assertRaisesRegex(DependencyFailure, "unsafe archive member"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_raw_dot_member_is_rejected_before_destination_exists(self):
        self.write_custom_archive("file", "demo-1.0.0/./src/lib.rs")
        self.write_lockfile(self.archive_sha256())
        with self.assertRaisesRegex(DependencyFailure, "unsafe archive member"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_symlink_member_is_rejected_before_destination_exists(self):
        self.write_custom_archive("symlink", "demo-1.0.0/link")
        self.write_lockfile(self.archive_sha256())
        with self.assertRaisesRegex(DependencyFailure, "link or special"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_existing_destination_is_untouched(self):
        self.write_lockfile(self.archive_sha256())
        self.destination.mkdir()
        marker = self.destination / "keep"
        marker.write_bytes(b"abc")
        with self.assertRaisesRegex(DependencyFailure, "destination already exists"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertEqual(marker.read_bytes(), b"abc")

    def test_oversized_lockfile_is_rejected_before_parsing(self):
        self.lockfile.write_bytes(b" " * (4 * 1024 * 1024 + 1))

        with self.assertRaisesRegex(DependencyFailure, "lockfile exceeds size limit"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_member_budget_refuses_before_later_invalid_member(self):
        self.write_member_budget_archive()
        self.write_lockfile(self.archive_sha256())

        with self.assertRaisesRegex(DependencyFailure, "member limit"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_regular_file_type_bits_are_allowed_and_permissions_normalized(self):
        self.write_mode_archive(0o100664)
        self.write_lockfile(self.archive_sha256())

        result = prepare_dependencies(self.lockfile, [self.archive], self.destination)

        output = self.destination / "demo-1.0.0/src/lib.rs"
        self.assertEqual(output.read_bytes(), b"abc")
        self.assertEqual(output.stat().st_mode & 0o777, 0o644)
        self.assertEqual(result["file_sha256"], {"demo-1.0.0/src/lib.rs": ABC_SHA256})

    def test_regular_member_with_symlink_type_bits_is_rejected(self):
        self.write_mode_archive(0o120644)
        self.write_lockfile(self.archive_sha256())

        with self.assertRaisesRegex(DependencyFailure, "conflicting file type"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_setuid_permission_is_rejected(self):
        self.write_mode_archive(0o104644)
        self.write_lockfile(self.archive_sha256())

        with self.assertRaisesRegex(DependencyFailure, "special permission"):
            prepare_dependencies(self.lockfile, [self.archive], self.destination)
        self.assertFalse(self.destination.exists())

    def test_byte_identical_duplicate_archive_candidates_succeed_per_package(self):
        self.write_lockfile(self.archive_sha256())
        first = self.root / "first/demo-1.0.0.crate"
        second = self.root / "second/demo-1.0.0.crate"
        first.parent.mkdir()
        second.parent.mkdir()
        first.write_bytes(self.archive.read_bytes())
        second.write_bytes(self.archive.read_bytes())

        result = prepare_dependencies(self.lockfile, [first, second], self.destination)

        self.assertEqual(result["package_count"], 1)
        self.assertEqual(result["archive_sha256"], {"demo-1.0.0": self.archive_sha256()})
        self.assertEqual(
            result["file_sha256"],
            {
                "demo-1.0.0/.gitignore": ABC_SHA256,
                "demo-1.0.0/bin/tool": ABC_SHA256,
                "demo-1.0.0/src/lib.rs": ABC_SHA256,
            },
        )

    def test_wrong_duplicate_archive_is_rejected_in_both_input_orders(self):
        self.write_lockfile(self.archive_sha256())
        valid = self.root / "valid/demo-1.0.0.crate"
        invalid = self.root / "invalid/demo-1.0.0.crate"
        valid.parent.mkdir()
        invalid.parent.mkdir()
        valid.write_bytes(self.archive.read_bytes())
        invalid.write_bytes(self.archive.read_bytes() + b"wrong")

        for index, archives in enumerate(((valid, invalid), (invalid, valid))):
            destination = self.root / f"duplicate-destination-{index}"
            with self.subTest(order=index):
                with self.assertRaisesRegex(DependencyFailure, "checksum"):
                    prepare_dependencies(self.lockfile, list(archives), destination)
                self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
