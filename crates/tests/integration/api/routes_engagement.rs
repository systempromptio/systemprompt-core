//! Engagement router — single + batch engagement endpoints.

use axum::Extension;
use systemprompt_api::routes::engagement_router;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

#[tokio::test]
async fn record_engagement_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = engagement_router(&ctx)?.layer(Extension(request_context("user_engagement")));
    let body = serde_json::json!({
        "event_type": "scroll",
        "session_id": "00000000-0000-0000-0000-000000000000",
        "url": "https://example.com/",
    });
    let resp = app.oneshot(json_post("/", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn record_engagement_rejects_bad_payload() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = engagement_router(&ctx)?.layer(Extension(request_context("user_engagement")));
    let resp = app
        .oneshot(json_post("/", serde_json::json!({"nope": true})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn record_engagement_batch_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = engagement_router(&ctx)?.layer(Extension(request_context("user_engagement")));
    let body = serde_json::json!({ "events": [] });
    let resp = app.oneshot(json_post("/batch", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

// The tests above post a payload the input struct does not accept (`url`
// instead of `page_url`) and assert `status >= 200`, which cannot fail — so the
// handler body has never run. These drive it with a real session and a real
// payload, and check the side effect rather than the status alone.
mod recorded {
    use anyhow::Result;
    use axum::Extension;
    use systemprompt_api::routes::engagement_router;
    use systemprompt_database::DbPool;
    use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
    use systemprompt_models::RequestContext;
    use systemprompt_test_fixtures::{seed_user_row, seed_user_session};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::common::{body_to_string, json_post, setup_ctx};

    struct Seeded {
        req_ctx: RequestContext,
        session: SessionId,
    }

    async fn seeded(db: &DbPool) -> Result<Seeded> {
        let user = UserId::new(format!("eng-{}", Uuid::new_v4().simple()));
        let session = SessionId::generate();
        seed_user_row(db, &user, &format!("{}@engagement.invalid", user.as_str())).await?;
        seed_user_session(db, &user, &session).await?;

        let req_ctx = RequestContext::new(
            session.clone(),
            TraceId::generate(),
            ContextId::generate(),
            AgentName::new("engagement-test"),
        )
        .with_actor(Actor::user(user));
        Ok(Seeded { req_ctx, session })
    }

    async fn converted(db: &DbPool, session: &SessionId) -> Result<bool> {
        let p = db.pool_arc()?;
        let row: (Option<chrono::DateTime<chrono::Utc>>,) =
            sqlx::query_as("SELECT converted_at FROM user_sessions WHERE session_id = $1")
                .bind(session.as_str())
                .fetch_one(p.as_ref())
                .await?;
        Ok(row.0.is_some())
    }

    fn event(event_type: &str) -> serde_json::Value {
        serde_json::json!({
            "page_url": "https://example.test/pricing",
            "event_type": event_type,
            "time_on_page_ms": 4_200,
            "max_scroll_depth": 80,
            "click_count": 2,
        })
    }

    #[tokio::test]
    async fn a_recorded_event_is_created() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let seeded = seeded(&db).await?;
        let app = engagement_router(&ctx)?.layer(Extension(seeded.req_ctx));

        let (status, body) =
            body_to_string(app.oneshot(json_post("/", event("scroll"))).await?).await?;

        assert_eq!(status.as_u16(), 201, "{body}");
        Ok(())
    }

    #[tokio::test]
    async fn a_conversion_event_marks_the_session_converted() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let seeded = seeded(&db).await?;
        assert!(
            !converted(&db, &seeded.session).await?,
            "the session starts unconverted"
        );
        let app = engagement_router(&ctx)?.layer(Extension(seeded.req_ctx));

        let (status, body) =
            body_to_string(app.oneshot(json_post("/", event("pricing_click"))).await?).await?;
        assert_eq!(status.as_u16(), 201, "{body}");

        assert!(
            converted(&db, &seeded.session).await?,
            "a conversion-type event must flip the session's converted flag"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_non_conversion_event_leaves_the_session_unconverted() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let seeded = seeded(&db).await?;
        let app = engagement_router(&ctx)?.layer(Extension(seeded.req_ctx));

        let (status, _body) =
            body_to_string(app.oneshot(json_post("/", event("scroll"))).await?).await?;
        assert_eq!(status.as_u16(), 201);

        assert!(
            !converted(&db, &seeded.session).await?,
            "scrolling is not a conversion; over-counting it would inflate the funnel"
        );
        Ok(())
    }

    #[tokio::test]
    async fn every_declared_conversion_type_converts() -> Result<()> {
        for event_type in [
            "github_click",
            "evaluate_click",
            "demo_click",
            "demo_site_click",
            "pricing_click",
        ] {
            let (db, ctx) = setup_ctx().await?;
            let seeded = seeded(&db).await?;
            let app = engagement_router(&ctx)?.layer(Extension(seeded.req_ctx));

            let (status, body) =
                body_to_string(app.oneshot(json_post("/", event(event_type))).await?).await?;
            assert_eq!(status.as_u16(), 201, "{event_type}: {body}");
            assert!(
                converted(&db, &seeded.session).await?,
                "{event_type} is declared a conversion and must be counted as one"
            );
        }
        Ok(())
    }
}
