//! The subject source every member factory may hold before the kernel has
//! built the subject provider set.
//!
//! The kernel builds [`SubjectProviderSet`] after the early slots and in the
//! same step as the gateway guards, so a gateway guard, a slot entry (an authz
//! hook, a marketplace filter) or a subject provider that consults other
//! dimensions cannot read `Arc<SubjectProviderSet>` when it is built. It holds
//! [`SubjectSourceCell`] instead: the `authz` member declares the cell
//! (`Contributions::late_cell`), the kernel seeds its read half before any
//! member provides and binds it to the set once the composition has resolved.
//! The write half never enters the state, so no member can bind it. Until it
//! is bound, and on a composition that built no set, every read answers
//! [`SubjectSourceError::NotReady`] and the dimension list is empty: a caller
//! fails closed rather than resolve from an empty set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::LateCell;
use systemprompt_identifiers::UserId;
use systemprompt_models::{AlreadyBound, LateBinder, LateRead, late_pair};

use super::subject::{
    SubjectAttributeSource, SubjectAttributes, SubjectDimension, SubjectProviderSet,
    SubjectSourceError,
};

/// The read half of the subject set cell; see the module docs.
#[derive(Debug, Clone)]
pub struct SubjectSourceCell(LateRead<SubjectProviderSet>);

impl SubjectSourceCell {
    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.0.is_bound()
    }

    fn source(&self) -> Result<&Arc<SubjectProviderSet>, SubjectSourceError> {
        self.0.get().ok_or(SubjectSourceError::NotReady)
    }
}

impl LateCell for SubjectSourceCell {
    type Value = SubjectProviderSet;
    type Binder = LateBinder<SubjectProviderSet>;

    fn unbound() -> (Self, Self::Binder) {
        let (read, binder) = late_pair::<SubjectProviderSet>();
        (Self(read), binder)
    }

    fn bind(binder: Self::Binder, value: Arc<Self::Value>) -> Result<(), AlreadyBound> {
        binder.bind(value)
    }
}

#[async_trait]
impl SubjectAttributeSource for SubjectSourceCell {
    fn dimensions(&self) -> &[SubjectDimension] {
        self.0.get().map_or(&[], |set| set.dimensions())
    }

    async fn values_for(
        &self,
        user: &UserId,
        dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        self.source()?.values_for(user, dimension).await
    }

    async fn resolve_all(&self, user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        self.source()?.resolve_all(user).await
    }

    async fn known_values(
        &self,
        dimension: &SubjectDimension,
    ) -> Result<Option<Vec<String>>, SubjectSourceError> {
        self.source()?.known_values(dimension).await
    }
}
