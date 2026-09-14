import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
import warnings
import zipfile


MODULE_PATH = Path(__file__).with_name("distribution.py")
SPEC = importlib.util.spec_from_file_location("distribution_runner", MODULE_PATH)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
DistributionFailure = RUNNER.DistributionFailure
check_distribution = RUNNER.check_distribution

TOP = "verus-x86-linux"
ABC_SHA256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
ARCHIVE_SHA256 = "5a34b41bd100d58806656da66e881ea4e08cf308632d5bd1d800222ab97986ac"
TRAVERSAL_SHA256 = "8882e7245b1a8f3e29d4331764e64a27c5052ae9bbaf43a319d9607c12338563"
DUPLICATE_SHA256 = "8a1e00e7c274759a4a80b2a72e58376601c9c73787747237b0bb2ce870581e7e"
EMPTY_ARCHIVE_SHA256 = "8739c76e681f900923b900c9df0ef75cf421d39cabb54650c4b9ad19b6a76d85"


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.archive = self.root / "verus.zip"
        self.extracted = self.root / "installed"
        self.extracted.mkdir()
        entries = (
            (f"{TOP}/", None),
            (f"{TOP}/bin/", None),
            (f"{TOP}/lib/", None),
            (f"{TOP}/empty/", None),
            (f"{TOP}/bin/verus", b"abc"),
            (f"{TOP}/lib/libvstd.rlib", b"abc"),
        )
        with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_STORED) as bundle:
            for name, payload in entries:
                info = zipfile.ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
                info.external_attr = (0o40755 if payload is None else 0o100644) << 16
                bundle.writestr(info, b"" if payload is None else payload)
        for relative, payload in (("bin/verus", b"abc"), ("lib/libvstd.rlib", b"abc")):
            path = self.extracted / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
        (self.extracted / "empty").mkdir()

    def tearDown(self):
        self.temporary.cleanup()

    def write_archive(self, names):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_STORED) as bundle:
                for name in names:
                    directory = name.endswith("/")
                    info = zipfile.ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
                    info.external_attr = (0o40755 if directory else 0o100644) << 16
                    bundle.writestr(info, b"" if directory else b"abc")

    def test_tampered_library_is_rejected_when_executable_is_unchanged(self):
        (self.extracted / "lib/libvstd.rlib").write_bytes(b"tampered")

        with self.assertRaisesRegex(DistributionFailure, "lib/libvstd.rlib"):
            check_distribution(self.archive, self.extracted, ARCHIVE_SHA256)

    def test_matching_distribution_returns_file_hashes_without_qualification(self):
        self.assertEqual(
            check_distribution(self.archive, self.extracted, ARCHIVE_SHA256),
            {
                "status": "distribution-valid",
                "qualification": False,
                "archive_sha256": ARCHIVE_SHA256,
                "file_sha256": {
                    "bin/verus": ABC_SHA256,
                    "lib/libvstd.rlib": ABC_SHA256,
                },
            },
        )

    def test_extra_extracted_file_is_rejected(self):
        (self.extracted / "extra").write_bytes(b"abc")
        with self.assertRaisesRegex(DistributionFailure, "inventory"):
            check_distribution(self.archive, self.extracted, ARCHIVE_SHA256)

    def test_missing_extracted_file_is_rejected(self):
        (self.extracted / "bin/verus").unlink()
        with self.assertRaisesRegex(DistributionFailure, "inventory"):
            check_distribution(self.archive, self.extracted, ARCHIVE_SHA256)

    def test_extracted_symlink_is_rejected(self):
        path = self.extracted / "bin/verus"
        path.unlink()
        path.symlink_to(self.extracted / "lib/libvstd.rlib")
        with self.assertRaisesRegex(DistributionFailure, "symlink"):
            check_distribution(self.archive, self.extracted, ARCHIVE_SHA256)

    def test_traversal_archive_member_is_rejected(self):
        self.write_archive((f"{TOP}/", f"{TOP}/../escape"))
        with self.assertRaisesRegex(DistributionFailure, "unsafe archive member"):
            check_distribution(self.archive, self.extracted, TRAVERSAL_SHA256)

    def test_duplicate_archive_member_is_rejected(self):
        self.write_archive((f"{TOP}/", f"{TOP}/bin/verus", f"{TOP}/bin/verus"))
        with self.assertRaisesRegex(DistributionFailure, "duplicate archive member"):
            check_distribution(self.archive, self.extracted, DUPLICATE_SHA256)

    def test_wrong_archive_pin_is_rejected(self):
        with self.assertRaisesRegex(DistributionFailure, "archive SHA-256 mismatch"):
            check_distribution(self.archive, self.extracted, "0" * 64)

    def test_missing_empty_directory_is_rejected(self):
        (self.extracted / "empty").rmdir()
        with self.assertRaisesRegex(DistributionFailure, "inventory"):
            check_distribution(self.archive, self.extracted, ARCHIVE_SHA256)

    @unittest.skipIf(os.geteuid() == 0, "root bypasses directory permission checks")
    def test_unreadable_directory_cannot_conceal_extra_file(self):
        directory = self.extracted / "empty"
        (directory / "concealed").write_bytes(b"abc")
        directory.chmod(0)
        try:
            with self.assertRaises(DistributionFailure):
                check_distribution(self.archive, self.extracted, ARCHIVE_SHA256)
        finally:
            directory.chmod(0o755)

    def test_archive_without_regular_files_is_rejected(self):
        self.write_archive(())
        empty_extracted = self.root / "empty-install"
        empty_extracted.mkdir()

        with self.assertRaisesRegex(DistributionFailure, "regular file"):
            check_distribution(self.archive, empty_extracted, EMPTY_ARCHIVE_SHA256)


if __name__ == "__main__":
    unittest.main()
