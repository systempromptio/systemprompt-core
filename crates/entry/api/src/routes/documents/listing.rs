//! Document listings: one folder, documents shared with the caller, the
//! caller's own, or the ones the caller touched last, filtered by name and
//! sorted by name, last update or last touch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Query, State};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::models::{ListScope, ListSort, ListingCursor};
use systemprompt_files::documents::service::ListFolderRequest;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

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

pub(super) async fn list(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(query): Query<ListQuery>,
) -> DocumentsResult<Json<ListingJson>> {
    let who = caller(&state, &req_ctx).await?;
    let request = ListFolderRequest {
        scope: list_scope(query.scope),
        folder: folder_id(query.folder.as_deref())?,
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
