//! The gateway does not depend on the agents capability: a composition with
//! the `agent` member (and what depends on it) disabled boots the gateway
//! router, the gateway serves without a context materializer, and the agent
//! surfaces are withheld naming the absent member. With the agent member
//! admitted the gateway binds conversations through its materializer.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, header};
use systemprompt::{CompositionError, reference};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted, fixture_app_context_with_extensions, fixture_config,
    test_boot_profile,
};
use systemprompt_traits::DynContextMaterializer;
use tower::ServiceExt;

fn admit(disabled: &[ExtensionId]) -> Result<systemprompt_extension::Admitted, CompositionError> {
    let mut profile = (*test_boot_profile()).clone();
    profile.extensions.disabled = disabled
        .iter()
        .cloned()
        .map(systemprompt_manifest::DisabledExtension::literal)
        .collect();
    reference().build(&profile)
}

fn without_agents() -> Vec<ExtensionId> {
    let mut disabled = vec![ExtensionId::new("agent")];
    loop {
        match admit(&disabled) {
            Err(
                CompositionError::DisabledDependency { owner, .. }
                | CompositionError::WithheldDependency { owner, .. },
            ) if !disabled.contains(&owner) => {
                disabled.push(owner);
            },
            Err(error) => panic!("admission without agent: {error}"),
            Ok(_) => return disabled,
        }
    }
}

fn repositories(
    ctx: &systemprompt_runtime::AppContext,
) -> anyhow::Result<systemprompt_gateway::GatewayRepositories> {
    Ok(
        systemprompt_gateway::surface::repositories::gateway_repositories(
            &ctx.kernel().resolved(),
            &systemprompt_gateway::GatewayHostHandles::resolve(&ctx.kernel().resolved())?,
            ctx.kernel().resolved().optional::<DynContextMaterializer>(),
            ctx.kernel()
                .state()
                .get::<systemprompt_database::DbPool>()?,
        )?,
    )
}

#[tokio::test]
async fn without_the_agent_member_the_gateway_boots_and_serves() -> anyhow::Result<()> {
    let disabled = without_agents();
    for kept in ["gateway", "gateway-http"] {
        assert!(
            !disabled.contains(&ExtensionId::new(kept)),
            "{kept} stays admitted with agent disabled: {disabled:?}"
        );
    }
    let admitted = Arc::new(admit(&disabled)?);
    let held_back: Vec<&ExtensionId> = admitted
        .withheld()
        .iter()
        .map(|w| &w.owner)
        .chain(disabled.iter().filter(|id| id.as_str() != "agent"))
        .collect();
    assert!(
        held_back.iter().any(|id| id.as_str() == "agent-http"),
        "agent-http is held back with the agent member: {held_back:?}"
    );

    let boot = ensure_test_bootstrap();
    let ctx =
        fixture_app_context_with_extensions(fixture_config(&boot.database_url), admitted).await?;
    assert!(
        ctx.kernel()
            .state()
            .find::<DynContextMaterializer>()
            .is_none(),
        "no member provides a context materializer without the agent member"
    );
    assert!(
        repositories(&ctx)?.context_materializer.is_none(),
        "the gateway serves without a materializer"
    );

    let app = gateway_router(&ctx, ctx.admitted(), ctx.kernel())?;
    let request = Request::builder()
        .method(Method::POST)
        .uri("/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))?;
    let response = app.oneshot(request).await?;
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20).await?;
    let body = String::from_utf8_lossy(&body);
    assert_eq!(status, 404, "{body}");
    assert!(
        body.contains("Gateway not enabled"),
        "the inference route is mounted and answers: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn with_the_agent_member_the_gateway_reads_its_context_materializer() -> anyhow::Result<()> {
    let boot = ensure_test_bootstrap();
    let ctx =
        fixture_app_context_with_extensions(fixture_config(&boot.database_url), fixture_admitted())
            .await?;
    assert!(
        ctx.kernel()
            .state()
            .find::<DynContextMaterializer>()
            .is_some(),
        "the agent member provides the context materializer"
    );
    assert!(
        repositories(&ctx)?.context_materializer.is_some(),
        "the gateway binds conversations through it"
    );
    Ok(())
}
