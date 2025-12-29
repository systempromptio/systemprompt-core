use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;
use systemprompt_core_events::EventRouter;
use systemprompt_identifiers::ContextId;
use systemprompt_models::{ContextEvent, RequestContext};
use systemprompt_runtime::AppContext;

use crate::repository::context::ContextRepository;

pub async fn forward_event(
    Extension(request_context): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(context_id): Path<String>,
    Json(event): Json<ContextEvent>,
) -> Response {
    let db = app_context.db_pool();
    let user_id = request_context.user_id();
    let context_id_typed = ContextId::new(&context_id);

    let context_repo = ContextRepository::new(db.clone());
    if let Err(e) = context_repo
        .validate_context_ownership(&context_id_typed, user_id)
        .await
    {
        tracing::error!(error = %e, "Context ownership validation failed");

        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Context ownership validation failed",
                "message": format!("User does not own context: {e}")
            })),
        )
            .into_response();
    }

    let (protocol, broadcast_count) = match event {
        ContextEvent::AgUi(e) => {
            let event_type = e.event_type();
            let (agui, ctx) = EventRouter::route_agui(user_id, e).await;
            tracing::debug!(event_type = ?event_type, agui = %agui, ctx = %ctx, "AG-UI event routed");
            ("agui", agui + ctx)
        },
        ContextEvent::A2A(e) => {
            let event_type = e.event_type();
            let (a2a, ctx) = EventRouter::route_a2a(user_id, *e).await;
            tracing::debug!(event_type = ?event_type, a2a = %a2a, ctx = %ctx, "A2A event routed");
            ("a2a", a2a + ctx)
        },
        ContextEvent::System(e) => {
            let event_type = e.event_type();
            let ctx = EventRouter::route_system(user_id, e).await;
            tracing::debug!(event_type = ?event_type, ctx = %ctx, "System event routed");
            ("system", ctx)
        },
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "protocol": protocol,
            "broadcast_count": broadcast_count,
            "context_id": context_id
        })),
    )
        .into_response()
}
