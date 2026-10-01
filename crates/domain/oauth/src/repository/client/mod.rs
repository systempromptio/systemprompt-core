//! OAuth client repository: queries, mutations, relations, last-use stamping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod inserts;
mod last_used;
mod mutations;
mod queries;
mod relations;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, UserId};

#[derive(Clone, Debug)]
pub struct ClientRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ClientRepository {
    pub fn new(db: &DbPool) -> crate::error::OauthResult<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}

#[derive(Debug, Clone)]
pub struct CreateClientParams {
    pub client_id: ClientId,
    pub owner_user_id: UserId,
    pub client_secret_hash: Option<String>,
    pub registration_token_hash: Option<String>,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
    pub scopes: Vec<String>,
    pub token_endpoint_auth_method: Option<String>,
    pub application_type: String,
    pub client_uri: Option<String>,
    pub logo_uri: Option<String>,
    pub contacts: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct UpdateClientParams {
    pub client_id: ClientId,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
    pub scopes: Vec<String>,
    pub token_endpoint_auth_method: Option<String>,
    pub client_uri: Option<String>,
    pub logo_uri: Option<String>,
    pub contacts: Option<Vec<String>>,
}
