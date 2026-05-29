//! Unit tests for UserApiKey and NewApiKey models.

use chrono::{Duration, Utc};
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_users::{NewApiKey, UserApiKey};

fn make_api_key(revoked: bool, expires_at: Option<chrono::DateTime<Utc>>) -> UserApiKey {
    UserApiKey {
        id: ApiKeyId::new("key-001"),
        user_id: UserId::new("user-001"),
        name: "my-key".to_string(),
        key_prefix: "sp-live-aabbcc".to_string(),
        key_hash: "deadbeef".repeat(8),
        created_at: Some(Utc::now()),
        last_used_at: None,
        expires_at,
        revoked_at: if revoked { Some(Utc::now()) } else { None },
    }
}

mod is_active_tests {
    use super::*;

    #[test]
    fn active_when_not_revoked_and_no_expiry() {
        let key = make_api_key(false, None);
        assert!(key.is_active(Utc::now()));
    }

    #[test]
    fn active_when_not_revoked_and_future_expiry() {
        let key = make_api_key(false, Some(Utc::now() + Duration::hours(1)));
        assert!(key.is_active(Utc::now()));
    }

    #[test]
    fn inactive_when_revoked() {
        let key = make_api_key(true, None);
        assert!(!key.is_active(Utc::now()));
    }

    #[test]
    fn inactive_when_revoked_even_with_future_expiry() {
        let key = make_api_key(true, Some(Utc::now() + Duration::hours(1)));
        assert!(!key.is_active(Utc::now()));
    }

    #[test]
    fn inactive_when_expiry_in_past() {
        let key = make_api_key(false, Some(Utc::now() - Duration::seconds(1)));
        assert!(!key.is_active(Utc::now()));
    }

    #[test]
    fn inactive_at_exact_expiry_time() {
        let expiry = Utc::now();
        let key = make_api_key(false, Some(expiry));
        assert!(!key.is_active(expiry));
    }

    #[test]
    fn active_one_second_before_expiry() {
        let expiry = Utc::now() + Duration::seconds(1);
        let key = make_api_key(false, Some(expiry));
        let check_at = expiry - Duration::milliseconds(500);
        assert!(key.is_active(check_at));
    }

    #[test]
    fn expired_key_that_was_also_revoked() {
        let key = make_api_key(true, Some(Utc::now() - Duration::hours(24)));
        assert!(!key.is_active(Utc::now()));
    }
}

mod api_key_struct_tests {
    use super::*;

    #[test]
    fn debug_includes_name() {
        let key = make_api_key(false, None);
        let s = format!("{:?}", key);
        assert!(s.contains("UserApiKey"));
        assert!(s.contains("my-key"));
    }

    #[test]
    fn clone_preserves_all_fields() {
        let key = make_api_key(false, Some(Utc::now() + Duration::days(30)));
        let cloned = key.clone();
        assert_eq!(key.id.to_string(), cloned.id.to_string());
        assert_eq!(key.user_id.to_string(), cloned.user_id.to_string());
        assert_eq!(key.name, cloned.name);
        assert_eq!(key.key_prefix, cloned.key_prefix);
        assert_eq!(key.key_hash, cloned.key_hash);
    }

    #[test]
    fn serde_round_trip() {
        let key = make_api_key(false, Some(Utc::now() + Duration::days(1)));
        let json = serde_json::to_string(&key).expect("serialize");
        let decoded: UserApiKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(key.name, decoded.name);
        assert_eq!(key.key_prefix, decoded.key_prefix);
    }

    #[test]
    fn serde_revoked_at_null_when_active() {
        let key = make_api_key(false, None);
        let json = serde_json::to_string(&key).expect("serialize");
        assert!(json.contains("\"revoked_at\":null"));
    }

    #[test]
    fn last_used_at_none_by_default() {
        let key = make_api_key(false, None);
        assert!(key.last_used_at.is_none());
    }
}

mod new_api_key_tests {
    use super::*;

    #[test]
    fn new_api_key_exposes_record_and_secret() {
        let record = make_api_key(false, None);
        let secret = "sp-live-aabbcc.supersecretvalue".to_string();
        let new_key = NewApiKey {
            record: record.clone(),
            secret: secret.clone(),
        };
        assert_eq!(new_key.secret, secret);
        assert_eq!(new_key.record.name, record.name);
    }

    #[test]
    fn new_api_key_debug() {
        let record = make_api_key(false, None);
        let new_key = NewApiKey {
            record,
            secret: "secret123".to_string(),
        };
        let s = format!("{:?}", new_key);
        assert!(s.contains("NewApiKey"));
    }

    #[test]
    fn new_api_key_clone() {
        let record = make_api_key(false, None);
        let new_key = NewApiKey {
            record,
            secret: "my-secret".to_string(),
        };
        let cloned = new_key.clone();
        assert_eq!(new_key.secret, cloned.secret);
        assert_eq!(new_key.record.name, cloned.record.name);
    }
}
