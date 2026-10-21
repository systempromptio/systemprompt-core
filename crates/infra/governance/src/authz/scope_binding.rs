//! Owner-membership verification for API-key scope bindings.
//!
//! A key may only be bound to a dimension value its owner actually holds, as
//! reported by the registered subject-attribute provider for that dimension.
//! The admin HTTP route and the CLI both issue keys through this one check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ScopeDimension, UserId};
use systemprompt_models::attribution::ScopeBinding;
use thiserror::Error;

use super::{AuthzError, SubjectProviderSet};

/// Why a requested scope binding was refused.
#[derive(Debug, Error)]
pub enum ScopeBindingError {
    #[error("unknown scope dimension '{0}': no subject attribute provider registers it")]
    UnknownDimension(ScopeDimension),

    #[error("the key owner is not a member of {dimension} '{value}'")]
    NotAMember {
        dimension: ScopeDimension,
        value: String,
    },

    #[error("subject attribute lookup failed: {0}")]
    Lookup(#[from] AuthzError),
}

impl SubjectProviderSet {
    pub async fn verify_scope_bindings(
        &self,
        owner: &UserId,
        scopes: &[ScopeBinding],
    ) -> Result<(), ScopeBindingError> {
        for scope in scopes {
            let Some(provider) = self.find(scope.dimension.as_str()) else {
                return Err(ScopeBindingError::UnknownDimension(scope.dimension.clone()));
            };
            let held = provider.values_for(owner).await?;
            if !held.iter().any(|value| value == &scope.value) {
                return Err(ScopeBindingError::NotAMember {
                    dimension: scope.dimension.clone(),
                    value: scope.value.clone(),
                });
            }
        }
        Ok(())
    }
}
