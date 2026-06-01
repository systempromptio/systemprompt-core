//! Coverage for identifier types lacking dedicated test modules: cloud,
//! oauth, marketplace, tenant, webhook, hook, plugin, policy, section,
//! funnel, events, connection, and gateway-boot IDs.

use systemprompt_identifiers::{
    AccessTokenId, AuthorizationCode, ChallengeId, CheckoutSessionId, ConnectionId, DbValue,
    DepartmentName, EngagementEventId, EventOutboxId, FunnelId, FunnelProgressId, HookId,
    MarketplaceId, ModelId, PluginId, PolicyId, PolicyVersion, PriceId, ProviderId, RefreshTokenId,
    RouteId, SecretName, SecretPatternId, SectionId, TenantId, ToDbValue, TransactionId,
    WebhookEndpointId,
};

macro_rules! basic_id_tests {
    ($mod:ident, $ty:ty, $sample:expr) => {
        mod $mod {
            use super::*;

            #[test]
            fn new_then_as_str() {
                let id = <$ty>::new($sample);
                assert_eq!(id.as_str(), $sample);
            }

            #[test]
            fn display_matches_inner() {
                let id = <$ty>::new($sample);
                assert_eq!(format!("{id}"), $sample);
            }

            #[test]
            fn serde_is_transparent() {
                let id = <$ty>::new($sample);
                let json = serde_json::to_string(&id).unwrap();
                assert_eq!(json, format!("\"{}\"", $sample));
                let back: $ty = serde_json::from_str(&json).unwrap();
                assert_eq!(back, id);
            }

            #[test]
            fn to_db_value_is_string() {
                let id = <$ty>::new($sample);
                assert!(matches!(id.to_db_value(), DbValue::String(s) if s == $sample));
            }

            #[test]
            fn partial_eq_with_str() {
                let id = <$ty>::new($sample);
                assert_eq!(id, $sample);
                assert_ne!(id, "definitely-not-the-sample");
            }

            #[test]
            fn into_string_via_ref() {
                let id = <$ty>::new($sample);
                let s: String = (&id).into();
                assert_eq!(s, $sample);
            }
        }
    };
}

basic_id_tests!(checkout_session_id, CheckoutSessionId, "cs_test_123");
basic_id_tests!(price_id, PriceId, "price_abc");
basic_id_tests!(transaction_id, TransactionId, "txn_42");
basic_id_tests!(refresh_token_id, RefreshTokenId, "rt_xyz");
basic_id_tests!(access_token_id, AccessTokenId, "at_xyz");
basic_id_tests!(authorization_code, AuthorizationCode, "code_abc");
basic_id_tests!(challenge_id, ChallengeId, "chal_1");
basic_id_tests!(marketplace_id, MarketplaceId, "mkt_default");
basic_id_tests!(tenant_id, TenantId, "tenant_acme");
basic_id_tests!(webhook_endpoint_id, WebhookEndpointId, "whk_1");
basic_id_tests!(hook_id, HookId, "hook_1");
basic_id_tests!(plugin_id, PluginId, "plugin_core");
basic_id_tests!(policy_version, PolicyVersion, "v1");
basic_id_tests!(policy_id, PolicyId, "pol_admin");
basic_id_tests!(section_id, SectionId, "sec_intro");
basic_id_tests!(engagement_event_id, EngagementEventId, "eng_1");
basic_id_tests!(funnel_id, FunnelId, "fun_1");
basic_id_tests!(funnel_progress_id, FunnelProgressId, "fp_1");
basic_id_tests!(event_outbox_id, EventOutboxId, "evt_1");
basic_id_tests!(connection_id, ConnectionId, "conn_1");
basic_id_tests!(provider_id, ProviderId, "anthropic");
basic_id_tests!(model_id, ModelId, "claude-opus-4-8");
basic_id_tests!(route_id, RouteId, "route_default");
basic_id_tests!(department_name, DepartmentName, "engineering");
basic_id_tests!(secret_name, SecretName, "OPENAI_API_KEY");

macro_rules! generate_uniqueness {
    ($mod:ident, $ty:ty) => {
        mod $mod {
            use super::*;

            #[test]
            fn generate_is_uuid_v4_length() {
                let id = <$ty>::generate();
                assert_eq!(id.as_str().len(), 36);
                assert_eq!(id.as_str().matches('-').count(), 4);
            }

            #[test]
            fn generate_is_unique() {
                assert_ne!(<$ty>::generate(), <$ty>::generate());
            }
        }
    };
}

generate_uniqueness!(webhook_generate, WebhookEndpointId);
generate_uniqueness!(hook_generate, HookId);
generate_uniqueness!(engagement_generate, EngagementEventId);
generate_uniqueness!(funnel_generate, FunnelId);
generate_uniqueness!(funnel_progress_generate, FunnelProgressId);
generate_uniqueness!(event_outbox_generate, EventOutboxId);
generate_uniqueness!(connection_generate, ConnectionId);

mod secret_pattern_id {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn try_new_rejects_empty() {
        assert!(SecretPatternId::try_new("").is_err());
    }

    #[test]
    fn try_new_accepts_non_empty() {
        let id = SecretPatternId::try_new("sk-[a-z]+").unwrap();
        assert_eq!(id.as_str(), "sk-[a-z]+");
    }

    #[test]
    fn from_str_validates() {
        assert!(SecretPatternId::from_str("").is_err());
        assert_eq!(
            SecretPatternId::from_str("ghp_.*").unwrap().as_str(),
            "ghp_.*"
        );
    }

    #[test]
    fn new_succeeds_on_non_empty() {
        assert_eq!(SecretPatternId::new("token").as_str(), "token");
    }
}
