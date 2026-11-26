//! The site fallback sits behind the IP-ban layer: an install's dynamic
//! service takes every method, so a write to it passes the ban check first,
//! exactly as an API route does.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use systemprompt::reference;
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_boot_profile,
};
use tower::ServiceExt;

struct SiteClaimant;

impl systemprompt_extension::Extension for SiteClaimant {
    fn metadata(&self) -> systemprompt_extension::ExtensionMetadata {
        systemprompt_extension::ExtensionMetadata {
            id: "probe-site",
            name: "probe-site",
            version: "0.0.0",
        }
    }

    fn contribute(
        &self,
        c: &mut systemprompt_extension::Contributions<'_>,
    ) -> Result<(), systemprompt_extension::ContributionError> {
        c.site_fallback(systemprompt_extension::router::SiteFallback::service(
            axum::Router::new().fallback(|| async { "site" }),
        ));
        Ok(())
    }
}

#[tokio::test]
async fn a_write_to_the_site_fallback_passes_the_ban_check_first() {
    let admitted = reference()
        .with(SiteClaimant)
        .build(&test_boot_profile())
        .expect("one site fallback claim admits");
    let bootstrap = ensure_test_bootstrap();
    let ctx = fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        Arc::new(admitted),
    )
    .await
    .expect("the context builds");
    let router = setup_api_server(
        &ctx,
        Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .expect("the API application builds");
    let request = Request::builder()
        .method(Method::POST)
        .uri("/forms/contact")
        .body(Body::empty())
        .expect("valid request");
    let response = router.oneshot(request).await.expect("infallible");
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "a request with no resolvable address is refused before the site answers"
    );
    assert_eq!(
        response
            .headers()
            .get("X-Blocked-Reason")
            .and_then(|value| value.to_str().ok()),
        Some("ip-unresolvable")
    );
}
