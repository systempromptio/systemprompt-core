//! Policy identifiers, and the identity of one governed call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(PolicyVersion);
crate::define_id!(PolicyId);
crate::define_id!(SecretPatternId, non_empty);

impl SecretPatternId {
    #[must_use]
    pub fn high_entropy() -> Self {
        Self("high-entropy-token".to_owned())
    }
}
crate::define_id!(CallId, generate, schema);

impl PolicyVersion {
    pub fn unversioned() -> Self {
        Self("unversioned".to_owned())
    }
}
