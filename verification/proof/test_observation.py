import importlib.util
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("observation.py")
SPEC = importlib.util.spec_from_file_location("proof_observation", MODULE_PATH)
observation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(observation)

FULL_REPORT = (
    b'{"func-details":{"liminal_safety::acknowledgement_matches":'
    b'{"obligation_proof_notes":[],"failed_proof_notes":[]}},'
    b'"verification-results":{"encountered-error":false,'
    b'"encountered-vir-error":false,"success":true,"verified":4,'
    b'"errors":0,"is-verifying-entire-crate":true},'
    b'"verus":{"profile":"release","version":"0.2026.08.30.b432e82",'
    b'"platform":{"os":"linux","arch":"x86_64"},'
    b'"toolchain":"1.97.1-x86_64-unknown-linux-gnu",'
    b'"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"}}'
)


class VerusObservationTests(unittest.TestCase):
    def test_complete_summary_and_root_report_are_preserved_without_qualification(self):
        raw = (
            b"verification results:: 1862 verified, 0 errors\n"
            b'{"func-details":{"liminal_safety::acknowledgement_matches":'
            b'{"obligation_proof_notes":[],"failed_proof_notes":[]}},'
            b'"verification-results":{"encountered-error":false,'
            b'"encountered-vir-error":false,"success":true,"verified":4,'
            b'"errors":0,"is-verifying-entire-crate":true},'
            b'"verus":{"profile":"release","version":"0.2026.08.30.b432e82",'
            b'"platform":{"os":"linux","arch":"x86_64"},'
            b'"toolchain":"1.97.1-x86_64-unknown-linux-gnu",'
            b'"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"}}\n'
        )

        result = observation.parse_verus_output(raw)

        self.assertEqual(result["schema"], "liminal-verus-observation-v1")
        self.assertEqual(result["status"], "output-parsed")
        self.assertIs(result["qualification"], False)
        self.assertEqual(
            result["events"][0], {"kind": "summary", "verified": 1862, "errors": 0}
        )
        self.assertEqual(result["events"][1]["kind"], "report")
        report = result["events"][1]["report"]
        self.assertEqual(report["verification-results"]["verified"], 4)
        self.assertEqual(
            report["func-details"],
            {
                "liminal_safety::acknowledgement_matches": {
                    "obligation_proof_notes": [],
                    "failed_proof_notes": [],
                }
            },
        )

    def test_complete_multi_report_stream_preserves_root_and_empty_example(self):
        raw = (
            b"verification results:: 1862 verified, 0 errors\n"
            b'{"func-details":{"liminal_safety::acknowledgement_matches":'
            b'{"obligation_proof_notes":[],"failed_proof_notes":[]}},'
            b'"verification-results":{"encountered-error":false,'
            b'"encountered-vir-error":false,"success":true,"verified":4,'
            b'"errors":0,"is-verifying-entire-crate":true},'
            b'"verus":{"profile":"release","version":"0.2026.08.30.b432e82",'
            b'"platform":{"os":"linux","arch":"x86_64"},'
            b'"toolchain":"1.97.1-x86_64-unknown-linux-gnu",'
            b'"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"}}\n'
            b'{"func-details":{},"verification-results":{'
            b'"encountered-error":false,"encountered-vir-error":false,'
            b'"success":true,"verified":0,"errors":0,'
            b'"is-verifying-entire-crate":true},'
            b'"verus":{"profile":"release","version":"0.2026.08.30.b432e82",'
            b'"platform":{"os":"linux","arch":"x86_64"},'
            b'"toolchain":"1.97.1-x86_64-unknown-linux-gnu",'
            b'"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"}}\n'
        )

        result = observation.parse_verus_output(raw)

        self.assertIs(result["qualification"], False)
        self.assertNotIn("proof_pass", result)
        self.assertEqual(len(result["events"]), 3)
        self.assertEqual(
            result["events"][0], {"kind": "summary", "verified": 1862, "errors": 0}
        )
        self.assertEqual(result["events"][1]["kind"], "report")
        self.assertEqual(
            result["events"][1]["report"]["verification-results"]["verified"], 4
        )
        self.assertEqual(result["events"][2]["kind"], "report")
        self.assertEqual(
            result["events"][2]["report"]["verification-results"]["verified"], 0
        )
        self.assertEqual(result["events"][2]["report"]["func-details"], {})

    def test_stream_without_any_json_report_is_rejected(self):
        cases = {
            "empty": b"",
            "whitespace": b" \t\r\n",
            "summary-only": b"verification results:: 1862 verified, 0 errors\n",
        }
        for name, raw in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_report_is_object_with_exact_outer_keys(self):
        cases = {
            "null": b"null\n",
            "array": b"[]\n",
            "number": b"7\n",
            "empty-object": b"{}\n",
            "missing-key": b'{"func-details":{},"verification-results":{}}\n',
            "extra-key": (
                b'{"func-details":{},"verification-results":{},"verus":{},'
                b'"unexpected":{}}\n'
            ),
        }
        for name, raw in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_malformed_bytes_and_json_are_normalized_to_observation_failure(self):
        deeply_nested = (
            b'{"func-details":'
            + b"[" * 2000
            + b"]" * 2000
            + b',"verification-results":{},"verus":{}}'
        )
        cases = {
            "invalid-utf8": b"\xff",
            "truncated-object": b'{"func-details":',
            "trailing-junk": FULL_REPORT + b" trailing-junk",
            "excessive-nesting": deeply_nested,
        }
        for name, raw in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_duplicate_json_members_are_rejected_at_any_report_depth(self):
        cases = {
            "duplicate-outer": FULL_REPORT.replace(
                b'{"func-details":', b'{"func-details":{},"func-details":', 1
            ),
            "duplicate-nested-true-then-false": FULL_REPORT.replace(
                b'"success":true,', b'"success":true,"success":false,', 1
            ),
            "duplicate-nested-false-then-true": FULL_REPORT.replace(
                b'"success":true,', b'"success":false,"success":true,', 1
            ),
        }
        for name, raw in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_function_note_entries_have_closed_string_array_schema(self):
        empty_entry = b'{"obligation_proof_notes":[],"failed_proof_notes":[]}'
        populated_entry = (
            b'{"obligation_proof_notes":["obligation note"],'
            b'"failed_proof_notes":["failed note"]}'
        )
        positive = observation.parse_verus_output(
            FULL_REPORT.replace(empty_entry, populated_entry, 1)
        )
        self.assertEqual(
            positive["events"][0]["report"]["func-details"]
            ["liminal_safety::acknowledgement_matches"],
            {
                "obligation_proof_notes": ["obligation note"],
                "failed_proof_notes": ["failed note"],
            },
        )

        cases = {
            "missing-key": FULL_REPORT.replace(
                empty_entry, b'{"obligation_proof_notes":[]}', 1
            ),
            "extra-key": FULL_REPORT.replace(
                empty_entry,
                b'{"obligation_proof_notes":[],"failed_proof_notes":[],'
                b'"unexpected":[]}',
                1,
            ),
            "nonobject-entry": FULL_REPORT.replace(empty_entry, b"7", 1),
            "string-instead-of-array": FULL_REPORT.replace(
                empty_entry,
                b'{"obligation_proof_notes":"note","failed_proof_notes":[]}',
                1,
            ),
            "integer-array-element": FULL_REPORT.replace(
                empty_entry,
                b'{"obligation_proof_notes":[7],"failed_proof_notes":[]}',
                1,
            ),
        }
        for name, raw in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_verification_results_follow_the_conditional_closed_schema(self):
        original = (
            b'{"encountered-error":false,"encountered-vir-error":false,'
            b'"success":true,"verified":4,"errors":0,'
            b'"is-verifying-entire-crate":true}'
        )
        positives = {
            "ordinary-proof-failure": (
                b'{"encountered-error":false,"encountered-vir-error":false,'
                b'"success":false,"verified":3,"errors":1,'
                b'"is-verifying-entire-crate":true}',
                {"encountered-error": False, "encountered-vir-error": False,
                 "success": False, "verified": 3, "errors": 1,
                 "is-verifying-entire-crate": True},
            ),
            "status-error": (
                b'{"encountered-error":true,"encountered-vir-error":false,'
                b'"success":false,"verified":0,"errors":0,'
                b'"is-verifying-entire-crate":true}',
                {"encountered-error": True, "encountered-vir-error": False,
                 "success": False, "verified": 0, "errors": 0,
                 "is-verifying-entire-crate": True},
            ),
            "partial-no-vir": (
                b'{"encountered-error":false,"encountered-vir-error":false,'
                b'"verified":4,"errors":0,"is-verifying-entire-crate":false}',
                {"encountered-error": False, "encountered-vir-error": False,
                 "verified": 4, "errors": 0,
                 "is-verifying-entire-crate": False},
            ),
            "vir-error": (
                b'{"encountered-error":true,"encountered-vir-error":true,'
                b'"success":false}',
                {"encountered-error": True, "encountered-vir-error": True,
                 "success": False},
            ),
            "partial-vir-error": (
                b'{"encountered-error":true,"encountered-vir-error":true}',
                {"encountered-error": True, "encountered-vir-error": True},
            ),
        }
        for name, (encoded, expected) in positives.items():
            with self.subTest(name=name):
                result = observation.parse_verus_output(
                    FULL_REPORT.replace(original, encoded, 1)
                )
                self.assertIs(result["qualification"], False)
                self.assertEqual(
                    result["events"][0]["report"]["verification-results"], expected
                )

        negatives = {
            "missing-boolean": b'{"encountered-error":false}',
            "integer-boolean": original.replace(b"false", b"0", 1),
            "unknown-field": original[:-1] + b',"unexpected":false}',
            "boolean-count": original.replace(b'"verified":4', b'"verified":true', 1),
            "negative-count": original.replace(b'"verified":4', b'"verified":-1', 1),
            "float-count": original.replace(b'"verified":4', b'"verified":4.0', 1),
            "missing-counts-no-vir": (
                b'{"encountered-error":false,"encountered-vir-error":false,'
                b'"success":true,"is-verifying-entire-crate":true}'
            ),
            "unexpected-count-on-vir": (
                b'{"encountered-error":true,"encountered-vir-error":true,'
                b'"success":false,"verified":0}'
            ),
            "unexpected-entire-flag-on-vir": (
                b'{"encountered-error":true,"encountered-vir-error":true,'
                b'"success":false,"is-verifying-entire-crate":true}'
            ),
            "partial-with-success": (
                b'{"encountered-error":false,"encountered-vir-error":false,'
                b'"success":true,"verified":4,"errors":0,'
                b'"is-verifying-entire-crate":false}'
            ),
            "entire-without-success": (
                b'{"encountered-error":false,"encountered-vir-error":false,'
                b'"verified":4,"errors":0,"is-verifying-entire-crate":true}'
            ),
            "contradictory-success": original.replace(b'"errors":0', b'"errors":1', 1),
        }
        for name, encoded in negatives.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(FULL_REPORT.replace(original, encoded, 1))

    def test_verus_metadata_has_closed_typed_shape_without_pin_admission(self):
        original = (
            b'{"profile":"release","version":"0.2026.08.30.b432e82",'
            b'"platform":{"os":"linux","arch":"x86_64"},'
            b'"toolchain":"1.97.1-x86_64-unknown-linux-gnu",'
            b'"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"}'
        )
        changed = (
            b'{"profile":"development-profile","version":"self-reported-version",'
            b'"platform":{"os":"linux","arch":"x86_64"},'
            b'"toolchain":"1.97.1-x86_64-unknown-linux-gnu",'
            b'"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"}'
        )
        positive = observation.parse_verus_output(
            FULL_REPORT.replace(original, changed, 1)
        )
        self.assertIs(positive["qualification"], False)
        self.assertEqual(
            positive["events"][0]["report"]["verus"],
            {
                "profile": "development-profile",
                "version": "self-reported-version",
                "platform": {"os": "linux", "arch": "x86_64"},
                "toolchain": "1.97.1-x86_64-unknown-linux-gnu",
                "commit": "b432e82fed7e05090fd53b5e5fc39020f725aabe",
            },
        )

        cases = {
            "missing-metadata-key": original.replace(
                b',"commit":"b432e82fed7e05090fd53b5e5fc39020f725aabe"', b"", 1
            ),
            "extra-metadata-key": original[:-1] + b',"unexpected":"value"}',
            "wrong-scalar-type": original.replace(b'"profile":"release"', b'"profile":7', 1),
            "nonobject-platform": original.replace(
                b'{"os":"linux","arch":"x86_64"}', b"7", 1
            ),
            "missing-platform-key": original.replace(
                b'{"os":"linux","arch":"x86_64"}', b'{"os":"linux"}', 1
            ),
            "extra-platform-key": original.replace(
                b'{"os":"linux","arch":"x86_64"}',
                b'{"os":"linux","arch":"x86_64","unexpected":"value"}',
                1,
            ),
            "nonstring-platform-value": original.replace(
                b'{"os":"linux","arch":"x86_64"}',
                b'{"os":7,"arch":"x86_64"}',
                1,
            ),
        }
        for name, encoded in cases.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(FULL_REPORT.replace(original, encoded, 1))

    def test_events_require_whitespace_separators(self):
        summary = b"verification results:: 1862 verified, 0 errors\n"

        reports = observation.parse_verus_output(FULL_REPORT + b" \t\r\n" + FULL_REPORT)
        self.assertIs(reports["qualification"], False)
        self.assertEqual([event["kind"] for event in reports["events"]], ["report", "report"])

        interleaved = observation.parse_verus_output(
            FULL_REPORT + b"\n" + summary + FULL_REPORT
        )
        self.assertIs(interleaved["qualification"], False)
        self.assertEqual(
            [event["kind"] for event in interleaved["events"]],
            ["report", "summary", "report"],
        )

        for name, raw in {
            "adjacent-reports": FULL_REPORT + FULL_REPORT,
            "adjacent-report-summary": FULL_REPORT + summary + FULL_REPORT,
        }.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_parser_enforces_byte_and_event_resource_limits(self):
        limit = 2 * 1024 * 1024
        exact_limit = FULL_REPORT + b" " * (limit - len(FULL_REPORT))
        with self.subTest(boundary="bytes"):
            exact = observation.parse_verus_output(exact_limit)
            self.assertIs(exact["qualification"], False)
            self.assertEqual(len(exact["events"]), 1)
            with self.assertRaises(observation.ObservationFailure):
                observation.parse_verus_output(exact_limit + b" ")

        with self.subTest(boundary="reports"):
            reports = observation.parse_verus_output(b"\n".join([FULL_REPORT] * 64))
            self.assertIs(reports["qualification"], False)
            self.assertEqual(len(reports["events"]), 64)
            with self.assertRaises(observation.ObservationFailure):
                observation.parse_verus_output(b"\n".join([FULL_REPORT] * 65))

        summary = b"verification results:: 1862 verified, 0 errors\n"
        with self.subTest(boundary="mixed-events"):
            mixed = observation.parse_verus_output(summary * 63 + FULL_REPORT)
            self.assertIs(mixed["qualification"], False)
            self.assertEqual(len(mixed["events"]), 64)
            with self.assertRaises(observation.ObservationFailure):
                observation.parse_verus_output(summary * 64 + FULL_REPORT)

        for name, raw in {
            "string": "not bytes",
            "none": None,
            "bytearray": bytearray(FULL_REPORT),
        }.items():
            with self.subTest(boundary="input-type", name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_counts_are_canonical_u64_values_in_json_and_summaries(self):
        maximum = 18446744073709551615
        original = (
            b'{"encountered-error":false,"encountered-vir-error":false,'
            b'"success":true,"verified":4,"errors":0,'
            b'"is-verifying-entire-crate":true}'
        )
        max_verified = original.replace(
            b'"verified":4', b'"verified":18446744073709551615', 1
        )
        verified = observation.parse_verus_output(
            FULL_REPORT.replace(original, max_verified, 1)
        )
        self.assertIs(verified["qualification"], False)
        self.assertEqual(
            verified["events"][0]["report"]["verification-results"]["verified"],
            maximum,
        )

        max_errors = original.replace(
            b'"success":true,"verified":4,"errors":0',
            b'"success":false,"verified":4,"errors":18446744073709551615',
            1,
        )
        errors = observation.parse_verus_output(
            FULL_REPORT.replace(original, max_errors, 1)
        )
        self.assertIs(errors["qualification"], False)
        self.assertEqual(
            errors["events"][0]["report"]["verification-results"]["errors"],
            maximum,
        )

        summary = observation.parse_verus_output(
            b"verification results:: 18446744073709551615 verified, "
            b"18446744073709551615 errors\n" + FULL_REPORT
        )
        self.assertIs(summary["qualification"], False)
        self.assertEqual(
            summary["events"][0],
            {"kind": "summary", "verified": maximum, "errors": maximum},
        )

        failing = original.replace(b'"success":true', b'"success":false', 1)
        json_cases = {
            "verified-overflow": original.replace(
                b'"verified":4', b'"verified":18446744073709551616', 1
            ),
            "errors-overflow": failing.replace(
                b'"errors":0', b'"errors":18446744073709551616', 1
            ),
            "verified-negative": original.replace(b'"verified":4', b'"verified":-1', 1),
            "errors-negative": failing.replace(b'"errors":0', b'"errors":-1', 1),
            "verified-bool": original.replace(b'"verified":4', b'"verified":true', 1),
            "errors-bool": failing.replace(b'"errors":0', b'"errors":true', 1),
            "verified-float": original.replace(b'"verified":4', b'"verified":4.0', 1),
            "errors-float": original.replace(b'"errors":0', b'"errors":0.0', 1),
            "verified-nan": original.replace(b'"verified":4', b'"verified":NaN', 1),
            "errors-infinity": failing.replace(b'"errors":0', b'"errors":Infinity', 1),
        }
        for name, encoded in json_cases.items():
            with self.subTest(source="json", name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(FULL_REPORT.replace(original, encoded, 1))

        summary_cases = {
            "verified-overflow": b"18446744073709551616 verified, 0 errors\n",
            "errors-overflow": b"0 verified, 18446744073709551616 errors\n",
            "negative": b"-1 verified, 0 errors\n",
            "leading-zero": b"01 verified, 0 errors\n",
            "five-thousand-digits": b"9" * 5000 + b" verified, 0 errors\n",
            "malformed": b"many verified, 0 errors\n",
            "trailing-suffix": b"4 verified, 0 errors trailing\n",
        }
        for name, suffix in summary_cases.items():
            with self.subTest(source="summary", name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(
                        b"verification results:: " + suffix + FULL_REPORT
                    )

    def test_partial_summary_suffix_is_exact_and_preserved(self):
        partial = observation.parse_verus_output(
            b"verification results:: 1 verified, 0 errors "
            b"(partial verification with `--verify-*`)\n" + FULL_REPORT
        )

        self.assertIs(partial["qualification"], False)
        self.assertEqual(
            partial["events"][0],
            {"kind": "summary", "verified": 1, "errors": 0, "partial": True},
        )
        self.assertEqual(partial["events"][1]["kind"], "report")

        near_misses = {
            "wrong-flag": (
                b"verification results:: 1 verified, 0 errors "
                b"(partial verification with `--verify-module`)\n"
            ),
            "missing-closing-parenthesis": (
                b"verification results:: 1 verified, 0 errors "
                b"(partial verification with `--verify-*`\n"
            ),
            "arbitrary-trailing-bytes": (
                b"verification results:: 1 verified, 0 errors unexpected\n"
            ),
        }
        for name, summary in near_misses.items():
            with self.subTest(name=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(summary + FULL_REPORT)

    def test_json_strings_are_valid_unicode_scalars(self):
        empty_notes = b'"obligation_proof_notes":[]'
        escaped = FULL_REPORT.replace(
            empty_notes, b'"obligation_proof_notes":["\\ud83d\\ude00"]', 1
        )
        encoded = FULL_REPORT.replace(
            empty_notes, b'"obligation_proof_notes":["\xf0\x9f\x98\x80"]', 1
        )

        for name, raw in {"escaped-pair": escaped, "raw-utf8": encoded}.items():
            with self.subTest(positive=name):
                result = observation.parse_verus_output(raw)
                self.assertIs(result["qualification"], False)
                notes = result["events"][0]["report"]["func-details"][
                    "liminal_safety::acknowledgement_matches"
                ]["obligation_proof_notes"]
                self.assertEqual(notes, ["\U0001f600"])

        cases = {}
        for label, surrogate in {
            "high": b"\\ud800",
            "low": b"\\udc00",
        }.items():
            cases[f"note-{label}"] = FULL_REPORT.replace(
                empty_notes,
                b'"obligation_proof_notes":["' + surrogate + b'"]',
                1,
            )
            cases[f"function-name-{label}"] = FULL_REPORT.replace(
                b'"liminal_safety::acknowledgement_matches"',
                b'"' + surrogate + b'"',
                1,
            )
            cases[f"metadata-{label}"] = FULL_REPORT.replace(
                b'"profile":"release"',
                b'"profile":"' + surrogate + b'"',
                1,
            )
            cases[f"platform-{label}"] = FULL_REPORT.replace(
                b'"os":"linux"', b'"os":"' + surrogate + b'"', 1
            )

        for name, raw in cases.items():
            with self.subTest(negative=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(raw)

    def test_note_arrays_preserve_order_and_reject_per_field_duplicates(self):
        empty_entry = b'{"obligation_proof_notes":[],"failed_proof_notes":[]}'
        ordered_cases = {
            "obligation-reverse-order": (
                b'{"obligation_proof_notes":["b","a"],'
                b'"failed_proof_notes":[]}',
                ["b", "a"],
                [],
            ),
            "obligation-ab": (
                b'{"obligation_proof_notes":["a","b"],'
                b'"failed_proof_notes":[]}',
                ["a", "b"],
                [],
            ),
            "failed-reverse-order": (
                b'{"obligation_proof_notes":[],'
                b'"failed_proof_notes":["b","a"]}',
                [],
                ["b", "a"],
            ),
            "failed-ab": (
                b'{"obligation_proof_notes":[],'
                b'"failed_proof_notes":["a","b"]}',
                [],
                ["a", "b"],
            ),
            "same-text-across-fields": (
                b'{"obligation_proof_notes":["same"],'
                b'"failed_proof_notes":["same"]}',
                ["same"],
                ["same"],
            ),
        }
        for name, (entry, obligations, failures) in ordered_cases.items():
            with self.subTest(positive=name):
                result = observation.parse_verus_output(
                    FULL_REPORT.replace(empty_entry, entry, 1)
                )
                self.assertIs(result["qualification"], False)
                notes = result["events"][0]["report"]["func-details"][
                    "liminal_safety::acknowledgement_matches"
                ]
                self.assertEqual(notes["obligation_proof_notes"], obligations)
                self.assertEqual(notes["failed_proof_notes"], failures)

        duplicate_cases = {
            "obligation": (
                b'{"obligation_proof_notes":["same","same"],'
                b'"failed_proof_notes":[]}'
            ),
            "failed": (
                b'{"obligation_proof_notes":[],'
                b'"failed_proof_notes":["same","same"]}'
            ),
            "escaped-and-raw-equivalent": (
                b'{"obligation_proof_notes":["\\ud83d\\ude00","\xf0\x9f\x98\x80"],'
                b'"failed_proof_notes":[]}'
            ),
        }
        for name, entry in duplicate_cases.items():
            with self.subTest(negative=name):
                with self.assertRaises(observation.ObservationFailure):
                    observation.parse_verus_output(
                        FULL_REPORT.replace(empty_entry, entry, 1)
                    )


if __name__ == "__main__":
    unittest.main()
