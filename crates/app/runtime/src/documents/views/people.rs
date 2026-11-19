//! People a caller can mention or share with, as plain structs.
//!
//! Both surfaces read the store's one directory: [`people_search_view`]
//! matches active people whose name, email or id has a word starting with each
//! query word, accents ignored, and
//! [`people_lookup_view`] names the active people among a list of user ids,
//! leaving out unknown, inactive and service accounts. With a document, each
//! person carries the level they already hold there; reading that needs
//! comment on the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_files::documents::{DocumentService, DocumentsError, SubjectMatch};
use systemprompt_identifiers::{DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::AccessLevel;

use crate::documents::USER_SUBJECT_KIND;

pub const DEFAULT_PEOPLE: usize = 10;

/// A people search: the text, how many matches at most, and the document
/// whose levels to report.
#[derive(Debug, Clone, Copy)]
pub struct PeopleSearch<'a> {
    pub query: &'a str,
    pub limit: usize,
    pub document: Option<&'a DocumentId>,
}

/// `access` is the level held on the asked document, absent for none or
/// when no document was asked.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PersonItem {
    pub user_id: UserId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessLevel>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PeopleOutput {
    pub people: Vec<PersonItem>,
}

pub async fn people_search_view(
    service: &DocumentService,
    who: &AuthenticatedUser,
    search: PeopleSearch<'_>,
    trace: &TraceId,
) -> Result<PeopleOutput, DocumentsError> {
    if search.query.trim().is_empty() {
        return Err(DocumentsError::InvalidParams {
            detail: "query needs a name, email or id",
        });
    }
    let found = service
        .access()
        .search_subjects(USER_SUBJECT_KIND, search.query, search.limit)
        .await?;
    with_levels(service, who, found.matches, search.document, trace).await
}

pub async fn people_lookup_view(
    service: &DocumentService,
    who: &AuthenticatedUser,
    users: &[UserId],
    document: Option<&DocumentId>,
    trace: &TraceId,
) -> Result<PeopleOutput, DocumentsError> {
    let found = service.access().find_people(users).await?;
    with_levels(service, who, found, document, trace).await
}

async fn with_levels(
    service: &DocumentService,
    who: &AuthenticatedUser,
    matches: Vec<SubjectMatch>,
    document: Option<&DocumentId>,
    trace: &TraceId,
) -> Result<PeopleOutput, DocumentsError> {
    let users: Vec<UserId> = matches
        .iter()
        .map(|subject| UserId::new(subject.value.clone()))
        .collect();
    let levels = match document {
        Some(document) => service.levels_on(who, document, &users, trace).await?,
        None => vec![None; users.len()],
    };
    let people = matches
        .into_iter()
        .zip(users)
        .zip(levels)
        .map(|((subject, user_id), access)| PersonItem {
            user_id,
            name: subject.display,
            email: subject.detail,
            access,
        })
        .collect();
    Ok(PeopleOutput { people })
}
