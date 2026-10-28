//! Subject pairs: the `(kind, value)` tuples a grant can name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::AuthenticatedUser;

use super::service::DocumentAccessService;
use crate::documents::error::DocumentsError;
use crate::documents::models::SubjectPairs;

const USER_KIND: &str = "user";
const ROLE_KIND: &str = "role";

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
