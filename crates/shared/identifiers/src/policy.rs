//! Policy version identifier.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(PolicyVersion);
crate::define_id!(PolicyId);
crate::define_id!(SecretPatternId, non_empty);

impl PolicyVersion {
    pub fn unversioned() -> Self {
        Self("unversioned".to_owned())
    }
}
