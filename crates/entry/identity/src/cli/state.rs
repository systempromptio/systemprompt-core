//! The values the identity commands read beyond the database handle.
//!
//! [`IdentityCliState`] is provided open by `identity-http` from the sealed
//! kernel seeds its provide fn reads (the configuration, the at-rest pepper
//! and the admitted owner reassignments). Its type is private to this crate,
//! so no other member's resolution can name it; a member command resolves it
//! as the member's own value. A host that seeds no pepper or configuration
//! (a `--database-url` run) provides none, and only the commands that need it
//! fail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_extension::AdmittedOwnerReassignments;
use systemprompt_extension::cli::CommandHost;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::TrustedIssuer;
use systemprompt_oauth::OAuthRepository;
use systemprompt_traits::AtRestPepper;

#[derive(Clone)]
pub(crate) struct IdentityCliState {
    pub(crate) oauth: Arc<OAuthRepository>,
    pub(crate) owner_reassignments: Arc<AdmittedOwnerReassignments>,
    pub(crate) api_external_url: String,
    pub(crate) api_key_max_lifetime_days: Option<u32>,
    pub(crate) archived_users_days: u32,
    pub(crate) issuer: String,
    pub(crate) trusted_issuers: Vec<TrustedIssuer>,
}

pub(crate) fn provide(r: &Resolved<'_>) -> Result<IdentityCliState, ProvideError> {
    let (Some(pepper), Some(config)) = (r.optional::<AtRestPepper>(), r.optional::<Arc<Config>>())
    else {
        return Err(ProvideError::Withheld);
    };
    let db = r.get::<DbPool>()?;
    Ok(IdentityCliState {
        oauth: Arc::new(OAuthRepository::new(&db, pepper.shared())),
        owner_reassignments: r.get::<Arc<AdmittedOwnerReassignments>>()?,
        api_external_url: config.api_external_url.clone(),
        api_key_max_lifetime_days: config.api_key_max_lifetime_days,
        archived_users_days: config.retention.archived_users_days,
        issuer: config.jwt_issuer.clone(),
        trusted_issuers: config.trusted_issuers.clone(),
    })
}

pub(crate) async fn identity_state(host: &dyn CommandHost) -> Result<IdentityCliState> {
    Ok(host.resolved().await?.get::<IdentityCliState>()?)
}
