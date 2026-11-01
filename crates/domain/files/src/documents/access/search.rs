//! Searching and validating grant subjects.
//!
//! Users are matched by name, display name or email prefix through the
//! [`SubjectDirectory`](super::SubjectDirectory), people only: a service
//! account is never a user subject. Roles come from the role
//! catalogue the directory holds; a dimension's values come from its
//! provider when the provider can enumerate them. A dimension that cannot is
//! reported as not searchable, and validating a value against it answers
//! [`SubjectValidation::Unenumerable`] for the caller to decide.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

use super::directory::SubjectMatch;
use super::service::DocumentAccessService;
use super::subjects::{ROLE_KIND, USER_KIND};
use crate::documents::error::DocumentsError;

pub const MAX_SUBJECT_SEARCH: usize = 20;

/// The subjects of one kind matching a search. `searchable` is `false` for
/// a dimension whose provider cannot list its values; `matches` is then
/// empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectSearch {
    pub kind: String,
    pub searchable: bool,
    pub matches: Vec<SubjectMatch>,
}

/// The outcome of checking a subject value against its kind.
///
/// `Unenumerable` is a dimension whose provider cannot list its values, so the
/// value can be neither confirmed nor refused; the caller chooses what that
/// means for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectValidation {
    Valid,
    Invalid,
    Unenumerable,
}

fn prefix_matches(values: Vec<String>, query: &str, limit: usize) -> Vec<SubjectMatch> {
    let query = query.trim().to_lowercase();
    values
        .into_iter()
        .filter(|value| value.to_lowercase().starts_with(&query))
        .take(limit)
        .map(|value| SubjectMatch {
            display: value.clone(),
            value,
            detail: None,
        })
        .collect()
}

impl DocumentAccessService {
    pub async fn search_subjects(
        &self,
        kind: &str,
        query: &str,
        limit: usize,
    ) -> Result<SubjectSearch, DocumentsError> {
        let limit = limit.clamp(1, MAX_SUBJECT_SEARCH);
        let matches = match kind {
            USER_KIND => Some(self.directory.search_users(query, limit).await?),
            ROLE_KIND => Some(prefix_matches(self.directory.roles().await?, query, limit)),
            _ => self
                .dimension_values(kind)
                .await?
                .map(|values| prefix_matches(values, query, limit)),
        };
        Ok(SubjectSearch {
            kind: kind.to_owned(),
            searchable: matches.is_some(),
            matches: matches.unwrap_or_default(),
        })
    }

    pub async fn validate_subject(
        &self,
        kind: &str,
        value: &str,
    ) -> Result<SubjectValidation, DocumentsError> {
        let known = match kind {
            USER_KIND => self.directory.is_person(&UserId::new(value)).await?,
            ROLE_KIND => self
                .directory
                .roles()
                .await?
                .iter()
                .any(|role| role == value),
            _ => match self.dimension_values(kind).await? {
                Some(values) => values.iter().any(|known| known == value),
                None => return Ok(SubjectValidation::Unenumerable),
            },
        };
        Ok(if known {
            SubjectValidation::Valid
        } else {
            SubjectValidation::Invalid
        })
    }

    async fn dimension_values(&self, kind: &str) -> Result<Option<Vec<String>>, DocumentsError> {
        let dimension = self
            .subjects
            .dimensions()
            .iter()
            .find(|dimension| dimension.rule_type.as_str() == kind)
            .cloned()
            .ok_or_else(|| DocumentsError::UnknownSubjectKind {
                kind: kind.to_owned(),
            })?;
        Ok(self.subjects.known_values(&dimension).await?)
    }
}
