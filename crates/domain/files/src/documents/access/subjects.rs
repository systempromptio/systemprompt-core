//! Subject pairs: the `(kind, value)` tuples a grant can name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::AccessLevel;

use super::resolve::granted_ceiling;
use super::service::DocumentAccessService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, SubjectPairs};

const MAX_SUBJECT_MEMBERS: usize = 500;

pub(super) const USER_KIND: &str = "user";
pub(super) const ROLE_KIND: &str = "role";

impl DocumentAccessService {
    pub async fn subject_pairs(
        &self,
        who: &AuthenticatedUser,
    ) -> Result<SubjectPairs, DocumentsError> {
        let mut pairs = SubjectPairs::default().with(USER_KIND, who.id.as_str());
        for role in &who.roles {
            pairs.push(ROLE_KIND, role.as_str());
        }
        let attributes = self.subjects.resolve_all(&who.id).await?;
        for dimension in self.subjects.dimensions() {
            for value in attributes.values(&dimension.rule_type) {
                pairs.push(dimension.rule_type.as_str(), value.as_str());
            }
        }
        Ok(pairs)
    }

    async fn pairs_of(&self, user: &UserId) -> Result<Option<SubjectPairs>, DocumentsError> {
        let Some(roles) = self.directory.roles_of(user).await? else {
            return Ok(None);
        };
        Ok(Some(self.pairs_with_roles(user, &roles).await?))
    }

    async fn pairs_with_roles(
        &self,
        user: &UserId,
        roles: &[String],
    ) -> Result<SubjectPairs, DocumentsError> {
        let mut pairs = SubjectPairs::default().with(USER_KIND, user.as_str());
        for role in roles {
            pairs.push(ROLE_KIND, role.as_str());
        }
        let attributes = self.subjects.resolve_all(user).await?;
        for dimension in self.subjects.dimensions() {
            for value in attributes.values(&dimension.rule_type) {
                pairs.push(dimension.rule_type.as_str(), value.as_str());
            }
        }
        Ok(pairs)
    }

    pub async fn level_of(
        &self,
        document: &Document,
        user: &UserId,
    ) -> Result<Option<AccessLevel>, DocumentsError> {
        if document.owner_user_id == *user {
            return Ok(Some(AccessLevel::Own));
        }
        let Some(pairs) = self.pairs_of(user).await? else {
            return Ok(None);
        };
        let mut chain = Vec::new();
        if let Some(folder) = &document.folder_id {
            chain.push(folder.clone());
            chain.extend(self.folders.ancestors(folder).await?);
        }
        Ok(self
            .grants
            .effective_level(&document.id, &chain, &pairs)
            .await?
            .map(granted_ceiling))
    }

    pub async fn levels_of(
        &self,
        document: &Document,
        users: &[UserId],
    ) -> Result<Vec<Option<AccessLevel>>, DocumentsError> {
        let roles = self.directory.roles_of_many(users).await?;
        let mut pairs = Vec::with_capacity(users.len());
        for (user, roles) in users.iter().zip(roles) {
            pairs.push(match roles {
                Some(roles) => self.pairs_with_roles(user, &roles).await?,
                None => SubjectPairs::default(),
            });
        }
        let mut chain = Vec::new();
        if let Some(folder) = &document.folder_id {
            chain.push(folder.clone());
            chain.extend(self.folders.ancestors(folder).await?);
        }
        let levels = self
            .grants
            .effective_levels(&document.id, &chain, &pairs)
            .await?;
        Ok(users
            .iter()
            .zip(levels)
            .map(|(user, level)| {
                if document.owner_user_id == *user {
                    Some(AccessLevel::Own)
                } else {
                    level.map(granted_ceiling)
                }
            })
            .collect())
    }

    pub async fn subject_members(
        &self,
        kind: &str,
        value: &str,
    ) -> Result<Vec<UserId>, DocumentsError> {
        let mut members = self
            .directory
            .members(kind, value, MAX_SUBJECT_MEMBERS + 1)
            .await?;
        if members.len() > MAX_SUBJECT_MEMBERS {
            tracing::warn!(
                subject_kind = kind,
                subject_value = value,
                limit = MAX_SUBJECT_MEMBERS,
                "Subject has more members than the fan-out limit; the rest are not addressed"
            );
            members.truncate(MAX_SUBJECT_MEMBERS);
        }
        Ok(members)
    }

    #[must_use]
    pub fn subject_kinds(&self) -> Vec<String> {
        [USER_KIND, ROLE_KIND]
            .into_iter()
            .map(str::to_owned)
            .chain(
                self.subjects
                    .dimensions()
                    .iter()
                    .map(|dimension| dimension.rule_type.as_str().to_owned()),
            )
            .collect()
    }

    #[must_use]
    pub fn is_known_subject_kind(&self, kind: &str) -> bool {
        kind == USER_KIND
            || kind == ROLE_KIND
            || self
                .subjects
                .dimensions()
                .iter()
                .any(|dimension| dimension.rule_type.as_str() == kind)
    }
}
