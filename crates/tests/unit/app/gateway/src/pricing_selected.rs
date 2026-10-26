//! A hop is billed at its own deployment's price: the same model deployed on
//! two providers bills each hop at that hop's rate, never another's.

use systemprompt_catalog_fixtures::{CatalogFixture, model, platform, priced};
use systemprompt_gateway::pricing::resolve_hop;
use systemprompt_manifest::services::catalog::{
    DeploymentDetail, DeploymentTarget, EndpointConfig, EndpointRef, PricingOverride,
    ProviderConfig,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

#[test]
fn each_hop_bills_its_own_deployment_rate() {
    let catalog = CatalogFixture::new()
        .credential("native", platform("native"))
        .credential("ordinary", platform("ordinary"))
        .provider("native", ProviderConfig::new("native"))
        .provider("ordinary", ProviderConfig::new("ordinary"))
        .endpoint(
            "native",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://example.test/v1"),
        )
        .endpoint(
            "ordinary",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://example.test/v1"),
        )
        .model(
            "claude-sonnet-5",
            model(&[ClientFamily::Claude], 0, priced(2.0, 9.0, 0.0)),
        )
        .deploy(
            "claude-sonnet-5",
            [
                DeploymentTarget::Detailed(DeploymentDetail {
                    endpoint: EndpointRef::new("native", "api"),
                    upstream_model: None,
                    pricing: Some(PricingOverride {
                        input_per_million: Some(7.0),
                        ..PricingOverride::default()
                    }),
                    clients: None,
                }),
                DeploymentTarget::Endpoint(EndpointRef::new("ordinary", "api")),
            ],
        )
        .build();
    let hops = catalog.deployments("claude-sonnet-5");
    assert!((resolve_hop(&hops[0]).input_per_million - 7.0).abs() < f64::EPSILON);
    assert!((resolve_hop(&hops[1]).input_per_million - 2.0).abs() < f64::EPSILON);
}
