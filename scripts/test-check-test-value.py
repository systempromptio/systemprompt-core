import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-test-value.sh"


class CheckTestValue(unittest.TestCase):
    def run_on(self, source):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "probe.rs"
            path.write_text(textwrap.dedent(source))
            result = subprocess.run(
                ["bash", str(SCRIPT), directory],
                capture_output=True,
                text=True,
                check=False,
            )
            return result.returncode, result.stdout + result.stderr

    def assert_flags(self, source, line):
        code, output = self.run_on(source)
        self.assertEqual(code, 1, output)
        self.assertIn(f"probe.rs:{line}:", output)

    def assert_clean(self, source):
        code, output = self.run_on(source)
        self.assertEqual(code, 0, output)
        self.assertIn("check-test-value: OK", output)

    def test_a_one_line_discarded_unwrap_is_flagged(self):
        self.assert_flags(
            """
            fn t() {
                let _ = call().unwrap();
            }
            """,
            3,
        )

    def test_a_wrapped_discarded_expect_is_flagged_at_its_first_line(self):
        self.assert_flags(
            """
            fn t() {
                let _ = repository
                    .create(&params)
                    .await
                    .expect("create");
            }
            """,
            3,
        )

    def test_a_wrapped_discarded_size_is_flagged(self):
        self.assert_flags(
            """
            fn t() {
                let _ = rows
                    .len();
            }
            """,
            3,
        )

    def test_a_bound_and_asserted_value_is_clean(self):
        self.assert_clean(
            """
            fn t() {
                let created = repository
                    .create(&params)
                    .expect("create");
                assert_eq!(created.slug, "a");
                let _ = sender.send(event);
            }
            """
        )

    def test_an_annotated_side_effect_is_clean(self):
        self.assert_clean(
            """
            fn t() {
                let _ = warm().unwrap(); // lint-ok: no-assert warms the cache
            }
            """
        )

    def test_complementary_disjunctions_are_flagged_across_lines(self):
        self.assert_flags(
            """
            fn t() {
                assert!(
                    missing.is_err()
                        || missing.is_ok(),
                );
            }
            """,
            4,
        )
        self.assert_flags("fn t() { assert!(a.is_some() || a.is_none()); }\n", 1)

    def test_a_disjunction_of_independent_checks_is_clean(self):
        self.assert_clean(
            "fn t() { assert!(body.get(\"a\").is_some() || body.get(\"b\").is_some()); }\n"
        )

    def test_a_count_compared_with_zero_is_flagged_across_lines(self):
        self.assert_flags(
            """
            fn t() {
                assert!(
                    repository
                        .count_public_pages()
                        .await
                        .expect("count")
                        >= 0
                );
            }
            """,
            3,
        )

    def test_a_signed_status_compared_with_zero_is_clean(self):
        self.assert_clean("fn t() { assert!(ready >= 0, \"poll failed\"); }\n")

    def test_assert_true_is_flagged(self):
        self.assert_flags("fn t() {\n    assert!(true);\n}\n", 2)

    def test_a_value_compared_with_itself_is_flagged(self):
        self.assert_flags("fn t() {\n    assert_eq!(Kind::A, Kind::A);\n}\n", 2)

    def test_a_value_compared_with_an_expected_value_is_clean(self):
        self.assert_clean("fn t() {\n    assert_eq!(parse(\"a\"), Kind::A);\n}\n")


if __name__ == "__main__":
    unittest.main()
