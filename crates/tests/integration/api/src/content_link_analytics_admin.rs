//! The content link analytics routes are admin-only: each route of the
//! authenticated content set is mounted behind `authz_gate` with its declared
//! policy, as the API mounts it, and a signed-in non-admin caller is refused
//! on every link analytics read while an admin reaches the handler.

use axum::middleware::from_fn;
use axum::{Extension, Router};
use systemprompt_api::services::middleware::authz_gate;
use systemprompt_content_entry::http as content;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, request_context, setup_ctx};

const ANALYTICS_READS: [&str; 5] = [
    "/links",
    "/links/journey",
    "/links/{link}/clicks",
    "/links/{link}/performance",
    "/links/campaigns/{link}/performance",
];

fn gated(ctx: &AppContext, caller: RequestContext) -> Router {
    let set = content::authenticated_routes();
    let routers = set
        .materialize(&ctx.kernel().resolved())
        .expect("the content route state resolves");
    set.defs()
        .iter()
        .zip(routers)
        .map(|(def, router)| {
            let policy = def.spec.policy;
            router.layer(from_fn(move |req, next| authz_gate(policy, req, next)))
        })
        .fold(Router::new(), Router::merge)
        .layer(Extension(caller))
}

fn uri(template: &str) -> String {
    template.replace("{link}", &Uuid::new_v4().to_string())
}

#[tokio::test]
async fn a_signed_in_user_is_refused_every_link_analytics_read() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    for template in ANALYTICS_READS {
        let caller = request_context("link_reader").with_user_type(UserType::User);
        let resp = gated(&ctx, caller)
            .oneshot(empty_get(&uri(template)))
            .await?;
        assert_eq!(resp.status().as_u16(), 403, "{template} admits a non-admin");
    }
    Ok(())
}

#[tokio::test]
async fn an_anonymous_caller_is_refused_every_link_analytics_read() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    for template in ANALYTICS_READS {
        let caller = request_context("link_anon").with_user_type(UserType::Anon);
        let resp = gated(&ctx, caller)
            .oneshot(empty_get(&uri(template)))
            .await?;
        assert_eq!(
            resp.status().as_u16(),
            403,
            "{template} admits an anonymous caller"
        );
    }
    Ok(())
}

#[tokio::test]
async fn an_admin_reaches_the_link_analytics_handlers() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    for template in ANALYTICS_READS {
        let caller = request_context("link_admin").with_user_type(UserType::Admin);
        let resp = gated(&ctx, caller)
            .oneshot(empty_get(&uri(template)))
            .await?;
        assert_ne!(resp.status().as_u16(), 403, "{template} refuses an admin");
    }
    Ok(())
}
