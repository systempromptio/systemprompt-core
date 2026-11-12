//! The billing rules `ResolvedCatalog::build` enforces on member-paid
//! credentials: one credential per reachable chain, and a caller credential
//! only on a wire some client request format speaks.

use systemprompt_catalog_fixtures::{CatalogFixture, caller, claude_model, personal, platform};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{
    BillingRuleError, CatalogError, CredentialEntry, EndpointConfig, ProviderConfig,
    is_spoken_by_a_client,
};
use systemprompt_manifest::services::{GatewayRoute, ModelGlobs, RouteAlias};
use systemprompt_wire::WireProtocol;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1";

fn two_providers(own: CredentialEntry) -> CatalogFixture {
    CatalogFixture::new()
        .credential("deployment", platform("anthropic"))
        .credential("own", own)
        .provider("platform", ProviderConfig::new("deployment"))
        .provider("member", ProviderConfig::new("own"))
        .endpoint(
            "platform",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, ANTHROPIC_URL),
        )
        .endpoint(
            "member",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, ANTHROPIC_URL),
        )
}

fn billing(error: CatalogError) -> BillingRuleError {
    match error {
        CatalogError::Billing(rule) => *rule,
        other => panic!("expected a billing rule, got: {other}"),
    }
}

#[test]
fn a_model_deployed_on_a_caller_and_a_platform_credential_is_refused() {
    let error = two_providers(caller("Own account"))
        .model("m", claude_model(200_000))
        .deploy("m", ["member/api", "platform/api"])
        .try_build()
        .expect_err("refused");
    match billing(error) {
        BillingRuleError::MixedChain {
            model,
            first_credential,
            second_credential,
            ..
        } => {
            assert_eq!(model.as_str(), "m");
            assert_eq!(first_credential.as_str(), "own");
            assert_eq!(second_credential.as_str(), "deployment");
        },
        other => panic!("{other}"),
    }
}

#[test]
fn a_platform_primary_with_a_personal_fallback_is_refused() {
    let error = two_providers(personal("Own key"))
        .model("m", claude_model(200_000))
        .deploy("m", ["platform/api", "member/api"])
        .try_build()
        .expect_err("refused");
    assert!(
        matches!(billing(error), BillingRuleError::MixedChain { .. }),
        "a personal credential is member-paid too"
    );
}

#[test]
fn a_disabled_provider_is_not_part_of_the_chain() {
    let catalog = two_providers(caller("Own account"))
        .with_provider("platform", |provider| provider.enabled = false)
        .model("m", claude_model(200_000))
        .deploy("m", ["member/api", "platform/api"])
        .try_build();
    assert!(
        catalog.is_ok(),
        "{:?}",
        catalog.err().map(|e| e.to_string())
    );
}

#[test]
fn two_platform_credentials_share_a_chain() {
    let catalog = two_providers(platform("second"))
        .model("m", claude_model(200_000))
        .deploy("m", ["platform/api", "member/api"])
        .try_build();
    assert!(
        catalog.is_ok(),
        "{:?}",
        catalog.err().map(|e| e.to_string())
    );
}

fn split_models() -> CatalogFixture {
    two_providers(caller("Own account"))
        .model("own-m", claude_model(200_000))
        .model("platform-m", claude_model(1_000_000))
        .deploy("own-m", ["member/api"])
        .deploy("platform-m", ["platform/api"])
}

#[test]
fn a_context_fallback_onto_another_credential_is_refused() {
    let route = GatewayRoute {
        models: ModelGlobs(vec!["own-m".to_owned()]),
        context_fallbacks: vec![ModelId::new("platform-m")],
        ..GatewayRoute::new("long-context")
    };
    let error = split_models()
        .route(route)
        .try_build()
        .expect_err("refused");
    match billing(error) {
        BillingRuleError::MixedContextFallback {
            route,
            model,
            fallback,
            credential,
            fallback_credential,
        } => {
            assert_eq!(route.as_str(), "long-context");
            assert_eq!(model.as_str(), "own-m");
            assert_eq!(fallback.as_str(), "platform-m");
            assert_eq!(credential.as_str(), "own");
            assert_eq!(fallback_credential.as_str(), "deployment");
        },
        other => panic!("{other}"),
    }
}

#[test]
fn a_platform_model_does_not_fall_back_onto_a_members_bill() {
    let route = GatewayRoute {
        models: ModelGlobs(vec!["platform-m".to_owned()]),
        context_fallbacks: vec![ModelId::new("own-m")],
        ..GatewayRoute::new("long-context")
    };
    let error = split_models()
        .route(route)
        .try_build()
        .expect_err("refused");
    assert!(matches!(
        billing(error),
        BillingRuleError::MixedContextFallback { .. }
    ));
}

#[test]
fn an_alias_reaches_only_a_single_credential_chain() {
    let alias = GatewayRoute {
        alias: Some(RouteAlias {
            requested: ModelId::new("fast"),
            model: ModelId::new("m"),
        }),
        ..GatewayRoute::new("fast-alias")
    };
    let error = two_providers(caller("Own account"))
        .model("m", claude_model(200_000))
        .deploy("m", ["member/api", "platform/api"])
        .route(alias.clone())
        .try_build()
        .expect_err("refused");
    assert!(matches!(
        billing(error),
        BillingRuleError::MixedChain { .. }
    ));

    let served = split_models()
        .route(GatewayRoute {
            alias: Some(RouteAlias {
                requested: ModelId::new("fast"),
                model: ModelId::new("own-m"),
            }),
            ..alias
        })
        .try_build();
    assert!(served.is_ok(), "{:?}", served.err().map(|e| e.to_string()));
}

#[test]
fn a_caller_credential_is_refused_on_a_wire_no_client_speaks() {
    let on = |wire| {
        CatalogFixture::new()
            .credential("own", caller("Own account"))
            .provider("member", ProviderConfig::new("own"))
            .endpoint(
                "member",
                "api",
                EndpointConfig::new(wire, "https://api.openai.com/v1"),
            )
            .model("m", claude_model(200_000))
            .deploy("m", ["member/api"])
            .try_build()
    };
    match billing(on(WireProtocol::Gemini).expect_err("refused")) {
        BillingRuleError::CallerWireRewritesRequest { wire, .. } => {
            assert_eq!(wire, WireProtocol::Gemini);
        },
        other => panic!("{other}"),
    }
    for wire in [
        WireProtocol::Anthropic,
        WireProtocol::OpenAiChat,
        WireProtocol::OpenAiResponses,
    ] {
        assert!(is_spoken_by_a_client(wire), "{wire}");
        let built = on(wire);
        assert!(
            built.is_ok(),
            "{wire}: {:?}",
            built.err().map(|e| e.to_string())
        );
    }
}

#[test]
fn the_failover_rule_is_the_catalog_rule() {
    let catalog = split_models().build();
    let own = catalog.deployments("own-m")[0].clone();
    let deployment = catalog.deployments("platform-m")[0].clone();
    assert!(own.may_fail_over_to(&own));
    assert!(deployment.may_fail_over_to(&deployment));
    assert!(!own.may_fail_over_to(&deployment));
    assert!(!deployment.may_fail_over_to(&own));
}
