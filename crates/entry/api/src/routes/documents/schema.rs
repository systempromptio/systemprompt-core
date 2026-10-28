//! `GET /api/v1/documents/schema`: the JSON Schema of `ModelViewV2`, the
//! rich text model `documents_read` answers with `view: model` and
//! `documents_create` takes as `content.model`. The documents MCP guide
//! serves the same schema, so both surfaces describe one shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::{Extension, Json};
use systemprompt_files::documents::model::model_view_schema;
use systemprompt_models::RequestContext;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;

pub(super) async fn schema(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> DocumentsResult<Json<serde_json::Value>> {
    caller(&state, &req_ctx).await?;
    Ok(Json(model_view_schema().to_value()))
}
