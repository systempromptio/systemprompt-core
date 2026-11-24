//! The admin-bootstrap view the CLI host builds before any kernel state.
//!
//! [`admin_bootstrap`] reads the one constructor a core-standing `users`
//! member contributes (`Contributions::admin_bootstrap`) from the admitted
//! set and calls it with an open database handle. A constructor any other
//! member contributes, an install's included, is never called; with no
//! admitted `users` member the flows that need it fail with
//! [`AdminBootstrapUnavailable::NotEnabled`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{Admitted, StandingTag};
use systemprompt_traits::{AdminBootstrapError, DatabaseHandle, DynAdminBootstrap};

pub const ADMIN_BOOTSTRAP_OWNER: &str = "users";

#[derive(Debug, thiserror::Error)]
pub enum AdminBootstrapUnavailable {
    #[error("the users member is not enabled, so no admin can be bootstrapped or signed in")]
    NotEnabled,
    #[error(transparent)]
    Store(#[from] AdminBootstrapError),
}

pub fn admin_bootstrap(
    admitted: &Admitted,
    db: Arc<dyn DatabaseHandle>,
) -> Result<DynAdminBootstrap, AdminBootstrapUnavailable> {
    let constructor = admitted
        .built()
        .members()
        .find(|member| {
            member.standing == StandingTag::Core && member.owner.as_str() == ADMIN_BOOTSTRAP_OWNER
        })
        .and_then(|member| member.sink.admin_bootstrap())
        .ok_or(AdminBootstrapUnavailable::NotEnabled)?;
    Ok(constructor(db)?)
}
