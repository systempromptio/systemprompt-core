//! The late-bound [`SubjectSourceHandle`] that breaks the cycle between a
//! subject source and the providers that read it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_traits::authz::SubjectDimension;

use super::{SubjectAttributeSource, SubjectAttributes, SubjectSourceError};
use crate::late_bound::{AlreadyBound, LateBound};

/// A subject source bound after the providers that read it are built.
///
/// Providers are discovered from a context that must already carry a source,
/// and one provider may itself consume the set. The handle breaks that cycle:
/// it is created first, handed to every factory, and bound once the set exists.
/// Before binding every read fails with [`SubjectSourceError::NotReady`]
/// rather than answering from an empty set.
#[derive(Clone, Default)]
pub struct SubjectSourceHandle(LateBound<dyn SubjectAttributeSource>);

impl fmt::Debug for SubjectSourceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubjectSourceHandle")
            .field("bound", &self.0.is_bound())
            .finish()
    }
}

impl SubjectSourceHandle {
    pub fn bind(&self, source: Arc<dyn SubjectAttributeSource>) -> Result<(), AlreadyBound> {
        self.0.bind(source)
    }
}

#[async_trait]
impl SubjectAttributeSource for SubjectSourceHandle {
    fn dimensions(&self) -> &[SubjectDimension] {
        self.0.get().map_or(&[], |source| source.dimensions())
    }

    async fn values_for(
        &self,
        user: &UserId,
        dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        match self.0.get() {
            Some(source) => source.values_for(user, dimension).await,
            None => Err(SubjectSourceError::NotReady),
        }
    }

    async fn resolve_all(&self, user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        match self.0.get() {
            Some(source) => source.resolve_all(user).await,
            None => Err(SubjectSourceError::NotReady),
        }
    }

    async fn known_values(
        &self,
        dimension: &SubjectDimension,
    ) -> Result<Option<Vec<String>>, SubjectSourceError> {
        match self.0.get() {
            Some(source) => source.known_values(dimension).await,
            None => Err(SubjectSourceError::NotReady),
        }
    }
}
