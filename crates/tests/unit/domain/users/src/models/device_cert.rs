//! Unit tests for UserDeviceCert model.

use chrono::Utc;
use systemprompt_identifiers::{DeviceCertId, UserId};
use systemprompt_users::UserDeviceCert;

fn make_cert(revoked: bool) -> UserDeviceCert {
    UserDeviceCert {
        id: DeviceCertId::new("cert-001"),
        user_id: UserId::new("user-001"),
        fingerprint: "a".repeat(64),
        label: "My Device".to_string(),
        enrolled_at: Some(Utc::now()),
        revoked_at: if revoked { Some(Utc::now()) } else { None },
    }
}

mod is_active_tests {
    use super::*;

    #[test]
    fn active_when_revoked_at_is_none() {
        let cert = make_cert(false);
        assert!(cert.is_active());
    }

    #[test]
    fn inactive_when_revoked_at_is_set() {
        let cert = make_cert(true);
        assert!(!cert.is_active());
    }

    #[test]
    fn is_active_is_const_fn() {
        let cert = make_cert(false);
        const fn check_const_fn(c: &UserDeviceCert) -> bool {
            c.is_active()
        }
        assert!(check_const_fn(&cert));
    }
}

mod device_cert_struct_tests {
    use super::*;

    #[test]
    fn debug_includes_label() {
        let cert = make_cert(false);
        let s = format!("{:?}", cert);
        assert!(s.contains("UserDeviceCert"));
        assert!(s.contains("My Device"));
    }

    #[test]
    fn clone_preserves_all_fields() {
        let cert = make_cert(false);
        let cloned = cert.clone();
        assert_eq!(cert.id.to_string(), cloned.id.to_string());
        assert_eq!(cert.fingerprint, cloned.fingerprint);
        assert_eq!(cert.label, cloned.label);
    }

    #[test]
    fn serde_round_trip() {
        let cert = make_cert(false);
        let json = serde_json::to_string(&cert).expect("serialize");
        let decoded: UserDeviceCert = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cert.label, decoded.label);
        assert_eq!(cert.fingerprint, decoded.fingerprint);
    }

    #[test]
    fn serde_revoked_at_null_when_active() {
        let cert = make_cert(false);
        let json = serde_json::to_string(&cert).expect("serialize");
        assert!(json.contains("\"revoked_at\":null"));
    }

    #[test]
    fn serde_revoked_at_present_when_revoked() {
        let cert = make_cert(true);
        let json = serde_json::to_string(&cert).expect("serialize");
        assert!(!json.contains("\"revoked_at\":null"));
    }

    #[test]
    fn enrolled_at_can_be_none() {
        let cert = UserDeviceCert {
            id: DeviceCertId::new("cert-002"),
            user_id: UserId::new("user-002"),
            fingerprint: "b".repeat(64),
            label: "Label".to_string(),
            enrolled_at: None,
            revoked_at: None,
        };
        assert!(cert.enrolled_at.is_none());
        assert!(cert.is_active());
    }

    #[test]
    fn fingerprint_field_stores_raw_string() {
        let fp = "ab".repeat(32);
        let cert = UserDeviceCert {
            id: DeviceCertId::new("cert-003"),
            user_id: UserId::new("user-003"),
            fingerprint: fp.clone(),
            label: "x".to_string(),
            enrolled_at: None,
            revoked_at: None,
        };
        assert_eq!(cert.fingerprint, fp);
    }
}
