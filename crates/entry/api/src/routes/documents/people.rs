//! `GET /api/v1/documents/people`: the people a caller can mention or share
//! with, from the same directory and with the same fields as the
//! `documents_people` MCP tool.
//!
//! Exactly one mode per request: `query` searches active people whose name,
//! email or id has a word starting with each query word, accents ignored (up to
//! `limit`), and `ids` names the active people
//! among a comma-separated list of user ids, leaving out unknown and inactive
//! ones. Either mode takes `document_id` to report each person's level there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Query, State};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_documents::views::{
    DEFAULT_PEOPLE, PeopleOutput, PeopleSearch, people_lookup_view, people_search_view,
};
use systemprompt_files::documents::DocumentsError;
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::RequestContext;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;

#[derive(Debug, Deserialize)]
pub(super) struct PeopleQuery {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    ids: Option<String>,
    #[serde(default)]
    document_id: Option<DocumentId>,
}

fn user_ids(ids: &str) -> Result<Vec<UserId>, DocumentsError> {
    let users: Vec<UserId> = ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(UserId::new)
        .collect();
    if users.is_empty() {
        return Err(DocumentsError::InvalidParams {
            detail: "ids needs at least one user id",
        });
    }
    Ok(users)
}

pub(super) async fn people(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<PeopleQuery>,
) -> DocumentsResult<Json<PeopleOutput>> {
    let who = caller(&state, &req_ctx).await?;
    let PeopleQuery {
        query,
        limit,
        ids,
        document_id,
    } = params;
    let document = document_id.as_ref();
    let found = match (query, ids) {
        (Some(query), None) => {
            let search = PeopleSearch {
                query: &query,
                limit: limit.unwrap_or(DEFAULT_PEOPLE),
                document,
            };
            people_search_view(&state.service, &who.user, search, &who.trace).await?
        },
        (None, Some(ids)) => {
            let users = user_ids(&ids)?;
            people_lookup_view(&state.service, &who.user, &users, document, &who.trace).await?
        },
        _ => {
            return Err(DocumentsError::InvalidParams {
                detail: "give exactly one of query or ids",
            }
            .into());
        },
    };
    Ok(Json(found))
}
