
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use tracing::instrument;

use super::super::responses::{bad_request, internal_error, not_found, single_response};
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::{OAuthClientResponse, UpdateOAuthClientRequest};
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::OAuthState;

fn init_error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "server_error",
            "error_description": format!("Repository initialization failed: {e}")
        })),
    )
        .into_response()
}

#[instrument(skip(state, req_ctx, request), fields(client_id = %client_id))]
pub async fn update_client(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    Path(client_id): Path<String>,
    Json(request): Json<UpdateOAuthClientRequest>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(state.db_pool()) {
        Ok(r) => r,
        Err(e) => return init_error(e),
    };

    match repository.find_client_by_id(&client_id).await {
        Ok(Some(prev_client)) => {
            match repository
                .update_client(
                    &client_id,
                    request.name.as_deref(),
                    request.redirect_uris.as_deref(),
                    request.scopes.as_deref(),
                )
                .await
            {
                Ok(client) => {
                    tracing::info!(
                        client_id = %client_id,
                        client_name = ?client.name,
                        updated_by = %req_ctx.auth.user_id,
                        name_changed = request.name.is_some() && request.name.as_deref() != prev_client.name.as_deref(),
                        redirect_uris_changed = request.redirect_uris.is_some(),
                        scopes_changed = request.scopes.is_some(),
                        "OAuth client updated"
                    );
                    let response: OAuthClientResponse = client.into();
                    single_response(response)
                },
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        client_id = %client_id,
                        updated_by = %req_ctx.auth.user_id,
                        "OAuth client update failed"
                    );
                    bad_request(format!("Failed to update client: {e}"))
                },
            }
        },
        Ok(None) => {
            tracing::info!(
                client_id = %client_id,
                reason = "Client not found",
                updated_by = %req_ctx.auth.user_id,
                "OAuth client update failed"
            );
            not_found(format!("Client with ID '{client_id}' not found"))
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %client_id,
                updated_by = %req_ctx.auth.user_id,
                "OAuth client update failed"
            );
            internal_error(format!("Failed to get client: {e}"))
        },
    }
}
