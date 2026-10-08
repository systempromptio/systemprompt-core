//! Private decode target for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so client rows decode `owner_user_id` as a
//! plain string here and become a typed id through the trusted `new`
//! constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ClientId, UserId};

use super::OAuthClientRow;

#[derive(Debug)]
pub(crate) struct OAuthClientDbRow {
    pub client_id: ClientId,
    pub client_secret_hash: Option<String>,
    pub client_name: String,
    pub name: Option<String>,
    pub token_endpoint_auth_method: Option<String>,
    pub application_type: String,
    pub client_uri: Option<String>,
    pub logo_uri: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub owner_user_id: String,
}

impl From<OAuthClientDbRow> for OAuthClientRow {
    fn from(row: OAuthClientDbRow) -> Self {
        Self {
            client_id: row.client_id,
            client_secret_hash: row.client_secret_hash,
            client_name: row.client_name,
            name: row.name,
            token_endpoint_auth_method: row.token_endpoint_auth_method,
            application_type: row.application_type,
            client_uri: row.client_uri,
            logo_uri: row.logo_uri,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_used_at: row.last_used_at,
            owner_user_id: UserId::new(row.owner_user_id),
        }
    }
}
