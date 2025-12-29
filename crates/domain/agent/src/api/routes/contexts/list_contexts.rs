use axum::extract::{Extension, State};
use axum::response::IntoResponse;

use crate::repository::context::ContextRepository;
use systemprompt_models::{ApiError, CollectionResponse};

pub async fn list_contexts(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<systemprompt_runtime::AppContext>,
) -> impl IntoResponse {
    let db_pool = ctx.db_pool().clone();
    let context_repo = ContextRepository::new(db_pool.clone());
    let user_id = &req_ctx.auth.user_id;

    match context_repo.list_contexts_with_stats(user_id).await {
        Ok(contexts) => {
            tracing::debug!(
                user_id = %user_id,
                count = contexts.len(),
                "Contexts listed"
            );
            CollectionResponse::new(contexts).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list contexts");
            ApiError::internal_error(format!("Failed to list contexts: {e}")).into_response()
        },
    }
}
