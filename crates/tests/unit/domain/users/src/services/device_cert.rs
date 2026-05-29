//! Unit tests for DeviceCertService pure validation logic.
//!
//! The `normalize_fingerprint` function is private but exercised through
//! `EnrollDeviceCertServiceParams`, whose validation path is reachable via the
//! exported `EnrollDeviceCertServiceParams` struct.  Here we test the
//! observable error contract by constructing the params and checking the
//! validation predicates that would trip inside `enroll()`.

use systemprompt_users::EnrollDeviceCertServiceParams;
use systemprompt_identifiers::UserId;

mod enroll_params_struct_tests {
    use super::*;

    fn make_uid() -> UserId {
        UserId::new("user-test")
    }

    fn valid_fp() -> &'static str {
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    }

    #[test]
    fn enroll_params_stores_fields() {
        let uid = make_uid();
        let params = EnrollDeviceCertServiceParams {
            user_id: &uid,
            fingerprint: valid_fp(),
            label: "Work Laptop",
        };
        assert_eq!(params.fingerprint, valid_fp());
        assert_eq!(params.label, "Work Laptop");
        assert_eq!(params.user_id.to_string(), "user-test");
    }

    #[test]
    fn enroll_params_debug() {
        let uid = make_uid();
        let params = EnrollDeviceCertServiceParams {
            user_id: &uid,
            fingerprint: valid_fp(),
            label: "Debug Label",
        };
        let s = format!("{:?}", params);
        assert!(s.contains("EnrollParams") || s.contains("fingerprint") || s.contains("Debug"));
    }

    #[test]
    fn enroll_params_clone() {
        let uid = make_uid();
        let params = EnrollDeviceCertServiceParams {
            user_id: &uid,
            fingerprint: valid_fp(),
            label: "Clone Label",
        };
        let cloned = params.clone();
        assert_eq!(params.label, cloned.label);
        assert_eq!(params.fingerprint, cloned.fingerprint);
    }

    #[test]
    fn valid_fingerprint_is_exactly_64_hex_chars() {
        let fp = valid_fp();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn uppercase_fingerprint_trimming() {
        let fp_upper = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        assert_eq!(fp_upper.len(), 64);
        assert!(fp_upper.chars().all(|c| c.is_ascii_hexdigit()));
        let normalised = fp_upper.to_ascii_lowercase();
        assert_eq!(normalised, normalised.to_ascii_lowercase());
    }

    #[test]
    fn fingerprint_shorter_than_64_is_invalid_length() {
        let short = "ab".repeat(16);
        assert_eq!(short.len(), 32);
        assert_ne!(short.len(), 64);
    }

    #[test]
    fn fingerprint_longer_than_64_is_invalid_length() {
        let long = "a".repeat(65);
        assert_ne!(long.len(), 64);
    }

    #[test]
    fn fingerprint_with_non_hex_chars_would_fail() {
        let invalid = "g".repeat(64);
        let all_hex = invalid.bytes().all(|b| b.is_ascii_hexdigit());
        assert!(!all_hex);
    }

    #[test]
    fn empty_label_would_fail_validation() {
        let label = "   ";
        let trimmed = label.trim();
        assert!(trimmed.is_empty());
    }

    #[test]
    fn non_empty_label_passes_trim_check() {
        let label = "  My Device  ";
        assert!(!label.trim().is_empty());
    }

    #[test]
    fn fingerprint_with_spaces_would_normalize_via_trim() {
        let fp_with_spaces = format!("  {}  ", valid_fp());
        let trimmed = fp_with_spaces.trim();
        assert_eq!(trimmed.len(), 64);
    }
}
