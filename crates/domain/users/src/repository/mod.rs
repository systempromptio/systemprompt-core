//! Database access layer for the users domain.
//!
//! [`UserRepository`] holds the read and write pools and implements user CRUD,
//! sessions, and federated identity across the `user` submodule; the API-key,
//! device-cert, and banned-IP repositories live alongside it. Mutating
//! operations take typed parameter structs ([`UpdateUserParams`],
//! [`CreateApiKeyParams`], [`EnrollDeviceCertParams`], [`BanIpParams`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod api_key;
mod banned_ip;
mod device_cert;
mod federated_identity;
mod user;

pub use api_key::CreateApiKeyParams;
pub use banned_ip::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp, BannedIpRepository,
};
pub use device_cert::EnrollDeviceCertParams;
pub use user::{MergeResult, UpdateUserParams};

use crate::error::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone)]
pub struct UserRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl UserRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}
