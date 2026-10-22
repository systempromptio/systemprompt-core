//! Canned catalogs: the shapes the 0.73 design exists for.
//!
//! - [`one_platform_provider`]: one API-key credential, one provider, one
//!   endpoint, two models (one advertised to Claude, one not advertised).
//! - [`opencode_go`]: one personal credential behind one provider with two
//!   endpoints on two wires (Chat Completions and Anthropic Messages).
//! - [`vertex_regions`]: one service-account credential, Claude on a global and
//!   an EU endpoint (betas inherited, +10% price multiplier) and Gemini, with a
//!   project scope route pinning the EU endpoint.

use systemprompt_identifiers::ScopeDimension;
use systemprompt_manifest::services::catalog::{
    BetaSpec, DeploymentDetail, DeploymentTarget, EndpointConfig, EndpointRef, PricingOverride,
    ProviderConfig,
};
use systemprompt_manifest::services::{GatewayRoute, RouteDeployment, RouteMatch};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;
use systemprompt_wire::anthropic::AnthropicBeta;

use super::{CatalogFixture, claude_model, model, personal, platform, priced};

pub const OPENCODE_GO_CHAT_URL: &str = "https://opencode.ai/zen/go/v1";
pub const OPENCODE_GO_MESSAGES_URL: &str = "https://opencode.ai/zen/go/anthropic/v1";
const VERTEX_GLOBAL: &str = "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/anthropic/models";
const VERTEX_EU: &str = "https://europe-west4-aiplatform.googleapis.com/v1/projects/{project}/locations/europe-west4/publishers/anthropic/models";
const VERTEX_GEMINI: &str = "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/google/models";

#[must_use]
pub fn one_platform_provider() -> CatalogFixture {
    CatalogFixture::new()
        .credential("anthropic", platform("anthropic"))
        .provider("anthropic", ProviderConfig::new("anthropic"))
        .endpoint(
            "anthropic",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
        )
        .model("claude-sonnet-5", claude_model(1_000_000))
        .model(
            "claude-internal",
            model(&[], 200_000, priced(1.0, 5.0, 0.1)),
        )
        .deploy("claude-sonnet-5", ["anthropic/api"])
        .deploy("claude-internal", ["anthropic/api"])
        .default_model("claude-sonnet-5")
        .gateway_enabled(true)
}

#[must_use]
pub fn opencode_go() -> CatalogFixture {
    let provider = ProviderConfig {
        user_agent: Some("systemprompt-gateway (opencode-go; {install})".to_owned()),
        forward_headers: vec!["x-opencode-session".to_owned()],
        display_name: Some("OpenCode Go".to_owned()),
        ..ProviderConfig::new("opencode-go")
    };
    let messages = EndpointConfig {
        accepted_betas: Some(BetaSpec::List(std::collections::BTreeSet::new())),
        ..EndpointConfig::new(WireProtocol::Anthropic, OPENCODE_GO_MESSAGES_URL)
    };
    CatalogFixture::new()
        .credential("opencode-go", personal("OpenCode Go"))
        .provider("opencode-go", provider)
        .endpoint(
            "opencode-go",
            "chat",
            EndpointConfig::new(WireProtocol::OpenAiChat, OPENCODE_GO_CHAT_URL),
        )
        .endpoint("opencode-go", "messages", messages)
        .model(
            "go-kimi-k3",
            super::ModelEntry {
                upstream_model: Some("kimi-k3".to_owned()),
                ..model(&[ClientFamily::OpenAi], 256_000, priced(0.6, 2.5, 0.15))
            },
        )
        .model(
            "go-minimax-m3",
            super::ModelEntry {
                upstream_model: Some("minimax-m3".to_owned()),
                ..model(
                    &[ClientFamily::Claude, ClientFamily::OpenAi],
                    1_000_000,
                    priced(0.3, 1.2, 0.06),
                )
            },
        )
        .deploy("go-kimi-k3", ["opencode-go/chat"])
        .deploy("go-minimax-m3", ["opencode-go/messages"])
        .default_model("go-minimax-m3")
        .gateway_enabled(true)
}

#[must_use]
pub fn vertex_regions() -> CatalogFixture {
    let betas = [AnthropicBeta::new("context-1m-2025-08-07")]
        .into_iter()
        .collect();
    let global = EndpointConfig {
        accepted_betas: Some(BetaSpec::List(betas)),
        region: Some("global".to_owned()),
        ..EndpointConfig::new(WireProtocol::Anthropic, VERTEX_GLOBAL)
    };
    let eu = EndpointConfig {
        accepted_betas: Some(BetaSpec::Inherit("inherit:claude-global".to_owned())),
        region: Some("europe-west4".to_owned()),
        price_multiplier: Some(1.10),
        ..EndpointConfig::new(WireProtocol::Anthropic, VERTEX_EU)
    };
    let scope = [(
        ScopeDimension::try_new("project").expect("valid dimension"),
        "commerce".to_owned(),
    )]
    .into_iter()
    .collect();
    let eu_only = GatewayRoute {
        label: Some("Commerce stays in the EU".to_owned()),
        models: systemprompt_manifest::services::ModelGlobs(vec!["claude-*".to_owned()]),
        when: Some(RouteMatch {
            scope,
            ..RouteMatch::default()
        }),
        deployments: vec![RouteDeployment::Endpoint(EndpointRef::new(
            "vertex",
            "claude-eu",
        ))],
        ..GatewayRoute::new("project-commerce-eu-only")
    };
    CatalogFixture::new()
        .credential("vertex-sa", platform("vertex"))
        .provider("vertex", ProviderConfig::new("vertex-sa"))
        .endpoint("vertex", "claude-global", global)
        .endpoint("vertex", "claude-eu", eu)
        .endpoint(
            "vertex",
            "gemini",
            EndpointConfig::new(WireProtocol::Gemini, VERTEX_GEMINI),
        )
        .model("claude-sonnet-5", claude_model(1_000_000))
        .model(
            "vertex-gemini-3.8-flash",
            super::ModelEntry {
                upstream_model: Some("gemini-3.8-flash".to_owned()),
                ..model(
                    &[ClientFamily::Gemini],
                    1_000_000,
                    priced(0.75, 3.75, 0.075),
                )
            },
        )
        .deploy(
            "claude-sonnet-5",
            ["vertex/claude-global", "vertex/claude-eu"],
        )
        .deploy(
            "vertex-gemini-3.8-flash",
            [DeploymentTarget::Detailed(DeploymentDetail {
                endpoint: EndpointRef::new("vertex", "gemini"),
                upstream_model: None,
                pricing: Some(PricingOverride {
                    cache_write_per_million: Some(0.0),
                    ..PricingOverride::default()
                }),
                clients: None,
            })],
        )
        .route(eu_only)
        .default_model("claude-sonnet-5")
        .gateway_enabled(true)
}
