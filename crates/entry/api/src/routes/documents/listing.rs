//! Document listings: one folder, documents shared with the caller, the
//! caller's own, the ones the caller touched last, or the ones a conversation
//! the caller owns touched, filtered by name and sorted by name, last update
//! or last touch. A `context` listing names its conversation by
//! `context_id`; one the caller does not own and one that does not exist are
//! both `unknown_context`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Query, State};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::models::{ListScope, ListSort, ListingCursor};
use systemprompt_files::documents::service::ListFolderRequest;
use systemprompt_identifiers::ContextId;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::documents::attribution::attributed_context;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
use super::items::folder_id;
use super::views::ListingJson;

const DEFAULT_PAGE: i64 = 50;
const MAX_PAGE: i64 = 200;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ScopeParam {
    #[default]
    Folder,
    Shared,
    Mine,
    Recent,
    Context,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SortParam {
    Name,
    #[default]
    Updated,
    Recent,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListQuery {
    #[serde(default)]
    scope: ScopeParam,
    folder: Option<String>,
    context_id: Option<String>,
    query: Option<String>,
    #[serde(default)]
    sort: SortParam,
    cursor: Option<String>,
    limit: Option<i64>,
}

const fn list_scope(scope: ScopeParam) -> ListScope {
    match scope {
        ScopeParam::Folder => ListScope::Folder,
        ScopeParam::Shared => ListScope::Shared,
        ScopeParam::Mine => ListScope::Mine,
        ScopeParam::Recent => ListScope::Recent,
        ScopeParam::Context => ListScope::Context,
    }
}

const fn list_sort(sort: SortParam) -> ListSort {
    match sort {
        SortParam::Name => ListSort::Name,
        SortParam::Updated => ListSort::Updated,
        SortParam::Recent => ListSort::Recent,
    }
}

fn listing_cursor(raw: Option<&str>) -> DocumentsResult<Option<ListingCursor>> {
    raw.map(|raw| {
        raw.parse::<ListingCursor>().map_err(|source| {
            ApiError::bad_request("cursor is not valid")
                .with_source(source)
                .into()
        })
    })
    .transpose()
}

async fn listed_context(
    state: &DocumentsState,
    req_ctx: &RequestContext,
    raw: Option<&str>,
) -> DocumentsResult<ContextId> {
    let Some(raw) = raw.filter(|raw| !raw.is_empty()) else {
        return Err(ApiError::bad_request("scope=context needs a context_id").into());
    };
    let claimed = ContextId::try_new(raw)?;
    let owned = attributed_context(state.contexts.as_ref(), req_ctx, Some(&claimed)).await?;
    Ok(owned.unwrap_or(claimed))
}

pub(super) async fn list(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(query): Query<ListQuery>,
) -> DocumentsResult<Json<ListingJson>> {
    let who = caller(&state, &req_ctx).await?;
    let scope = list_scope(query.scope);
    let context_id = match scope {
        ListScope::Context => {
            Some(listed_context(&state, &req_ctx, query.context_id.as_deref()).await?)
        },
        _ => None,
    };
    let request = ListFolderRequest {
        scope,
        folder: folder_id(query.folder.as_deref())?,
        context_id,
        query: query.query,
        sort: list_sort(query.sort),
        cursor: listing_cursor(query.cursor.as_deref())?,
        limit: query.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE),
    };
    let listing = state
        .service
        .list_folder(&who.user, request, &who.trace)
        .await?;
    Ok(Json(listing.into()))
}
