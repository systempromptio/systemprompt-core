use systemprompt_identifiers::{
    ChallengeId, ClientId, PolicyId, QuotaSubjectId, RefreshTokenFamilyId, SessionId,
};

macro_rules! external_id_cases {
    ($module:ident, $ty:ty, [$($valid:expr),+ $(,)?]) => {
        mod $module {
            use super::*;

            #[test]
            fn accepts_every_minted_format() {
                for value in [$($valid),+] {
                    assert!(<$ty>::try_new(value).is_ok(), "{value} must be accepted");
                    let json = serde_json::to_string(value).unwrap();
                    assert!(serde_json::from_str::<$ty>(&json).is_ok());
                }
            }

            #[test]
            fn rejects_empty() {
                assert!(<$ty>::try_new("").is_err());
                assert!(serde_json::from_str::<$ty>("\"\"").is_err());
            }

            #[test]
            fn rejects_oversized() {
                let long = "a".repeat(systemprompt_identifiers::macros::MAX_EXTERNAL_ID_LEN + 1);
                assert!(<$ty>::try_new(long.clone()).is_err());
                let json = serde_json::to_string(&long).unwrap();
                assert!(serde_json::from_str::<$ty>(&json).is_err());
            }

            #[test]
            fn rejects_whitespace_and_control() {
                for bad in [" ", "a b", "a\nb", "a\u{0}b", "\tx"] {
                    assert!(<$ty>::try_new(bad).is_err(), "{bad:?} must be refused");
                }
            }

            #[test]
            fn from_str_validates() {
                assert!("".parse::<$ty>().is_err());
                assert!("ok".parse::<$ty>().is_ok());
            }
        }
    };
}

external_id_cases!(session, SessionId, [
    "sess_3f2b8c4e-1d2a-4b5c-9e8f-0a1b2c3d4e5f",
    "sess_3f2b8c4e1d2a4b5c9e8f0a1b2c3d4e5f",
    "system",
    "degraded_3f2b8c4e-1d2a-4b5c-9e8f-0a1b2c3d4e5f",
    "cli-my-server",
    "mcp-validate-my-server",
]);
external_id_cases!(client, ClientId, [
    "sp_web",
    "client_0123456789abcdef",
    "sys_scheduler",
    "bridge:user_123",
    "https://app.example.com/oauth/client-metadata.json",
]);
external_id_cases!(challenge, ChallengeId, ["3f2b8c4e-1d2a-4b5c-9e8f-0a1b2c3d4e5f"]);
external_id_cases!(refresh_family, RefreshTokenFamilyId, [
    "3f2b8c4e-1d2a-4b5c-9e8f-0a1b2c3d4e5f",
]);
external_id_cases!(policy, PolicyId, ["default", "rate_user:abc", "gw-policy.v2"]);
external_id_cases!(quota_subject, QuotaSubjectId, ["user:abc", "3f2b8c4e-1d2a-4b5c-9e8f-0a1b2c3d4e5f"]);
