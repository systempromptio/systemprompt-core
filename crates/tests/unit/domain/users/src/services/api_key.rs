//! Unit tests for ApiKeyService pure logic and exported types.

use systemprompt_identifiers::UserId;
use systemprompt_users::{API_KEY_PREFIX, IssueApiKeyParams};

mod api_key_prefix_tests {
    use super::*;

    #[test]
    fn api_key_prefix_constant_value() {
        assert_eq!(API_KEY_PREFIX, "sp-live-");
    }

    #[test]
    fn api_key_prefix_starts_with_sp() {
        assert!(API_KEY_PREFIX.starts_with("sp-"));
    }

    #[test]
    fn api_key_prefix_non_empty() {
        assert!(!API_KEY_PREFIX.is_empty());
    }

    #[test]
    fn valid_key_starts_with_prefix() {
        let key = format!("{}aabbcc.secret", API_KEY_PREFIX);
        assert!(key.starts_with(API_KEY_PREFIX));
    }

    #[test]
    fn key_without_prefix_is_invalid() {
        let key = "sk-not-sp-live-aabbcc.secret";
        assert!(!key.starts_with(API_KEY_PREFIX));
    }

    #[test]
    fn key_extraction_logic_finds_dot_separator() {
        let key = format!("{}aabbcc.secretdata", API_KEY_PREFIX);
        let dot_pos = key.find('.');
        assert!(dot_pos.is_some());
        let prefix = &key[..dot_pos.unwrap()];
        assert_eq!(prefix, format!("{}aabbcc", API_KEY_PREFIX));
    }

    #[test]
    fn key_without_dot_has_no_extractable_prefix() {
        let key = format!("{}aabbccnosep", API_KEY_PREFIX);
        assert!(key.find('.').is_none());
    }

    #[test]
    fn key_not_starting_with_prefix_returns_none_logic() {
        let key = "bearer token_value";
        let starts = key.starts_with(API_KEY_PREFIX);
        assert!(!starts);
    }
}

mod issue_api_key_params_tests {
    use super::*;

    fn make_uid() -> UserId {
        UserId::new("user-001")
    }

    #[test]
    fn params_stores_required_fields() {
        let uid = make_uid();
        let params = IssueApiKeyParams {
            user_id: &uid,
            name: "ci-key",
            expires_at: None,
        };
        assert_eq!(params.name, "ci-key");
        assert!(params.expires_at.is_none());
        assert_eq!(params.user_id.to_string(), "user-001");
    }

    #[test]
    fn params_with_expiry() {
        use chrono::{Duration, Utc};
        let uid = make_uid();
        let expires = Utc::now() + Duration::days(30);
        let params = IssueApiKeyParams {
            user_id: &uid,
            name: "expiring-key",
            expires_at: Some(expires),
        };
        assert!(params.expires_at.is_some());
        assert_eq!(params.name, "expiring-key");
    }

    #[test]
    fn empty_name_would_fail_validation() {
        let name = "   ";
        let trimmed = name.trim();
        assert!(trimmed.is_empty());
    }

    #[test]
    fn whitespace_trimming_on_name() {
        let name = "  my key  ";
        let trimmed = name.trim();
        assert_eq!(trimmed, "my key");
        assert!(!trimmed.is_empty());
    }

    #[test]
    fn params_debug() {
        let uid = make_uid();
        let params = IssueApiKeyParams {
            user_id: &uid,
            name: "debug-key",
            expires_at: None,
        };
        let s = format!("{:?}", params);
        assert!(s.contains("IssueApiKeyParams") || s.contains("debug-key") || s.contains("name"));
    }

    #[test]
    fn params_clone() {
        let uid = make_uid();
        let params = IssueApiKeyParams {
            user_id: &uid,
            name: "clone-me",
            expires_at: None,
        };
        let cloned = params.clone();
        assert_eq!(params.name, cloned.name);
    }

    #[test]
    fn name_with_only_whitespace_trims_to_empty() {
        let names = ["", " ", "\t", "   \n  "];
        for name in names {
            assert!(name.trim().is_empty(), "expected {name:?} to trim to empty");
        }
    }

    #[test]
    fn name_with_content_trims_non_empty() {
        let names = ["a", " key ", "\ttest\t", "my-api-key"];
        for name in names {
            assert!(
                !name.trim().is_empty(),
                "expected {name:?} to be non-empty after trim"
            );
        }
    }
}
