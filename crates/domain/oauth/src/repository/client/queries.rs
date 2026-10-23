//! Read-side OAuth client queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ClientRepository;
use crate::error::OauthResult as Result;
use crate::models::{OAuthClient, OAuthClientRow};
use systemprompt_identifiers::{ClientId, UserId};

impl ClientRepository {
    pub async fn find_by_client_id(&self, client_id: &ClientId) -> Result<Option<OAuthClient>> {
        let client_id_str = client_id.as_str();
        let row = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    application_type, client_uri, logo_uri, is_active, created_at, updated_at, \
             last_used_at, owner_user_id as \"owner_user_id: UserId\"
             FROM oauth_clients WHERE client_id = $1 AND is_active = true",
            client_id_str
        )
        .fetch_optional(&*self.pool)
        .await?;

        match row {
            Some(row) => {
                let client = self.load_client_with_relations(row).await?;
                Ok(Some(client))
            },
            None => Ok(None),
        }
    }

    pub async fn find_by_client_id_any(&self, client_id: &ClientId) -> Result<Option<OAuthClient>> {
        let client_id_str = client_id.as_str();
        let row = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    application_type, client_uri, logo_uri, is_active, created_at, updated_at, \
             last_used_at, owner_user_id as \"owner_user_id: UserId\"
             FROM oauth_clients WHERE client_id = $1",
            client_id_str
        )
        .fetch_optional(&*self.pool)
        .await?;

        match row {
            Some(row) => {
                let client = self.load_client_with_relations(row).await?;
                Ok(Some(client))
            },
            None => Ok(None),
        }
    }

    pub async fn list(&self) -> Result<Vec<OAuthClient>> {
        let rows = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    application_type, client_uri, logo_uri, is_active, created_at, updated_at, \
             last_used_at, owner_user_id as \"owner_user_id: UserId\"
             FROM oauth_clients WHERE is_active = true ORDER BY created_at DESC"
        )
        .fetch_all(&*self.pool)
        .await?;

        self.load_clients_with_relations_batch(rows).await
    }

    pub async fn list_paginated(&self, limit: i32, offset: i32) -> Result<Vec<OAuthClient>> {
        let limit_i64 = i64::from(limit);
        let offset_i64 = i64::from(offset);
        let rows = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    application_type, client_uri, logo_uri, is_active, created_at, updated_at, \
             last_used_at, owner_user_id as \"owner_user_id: UserId\"
             FROM oauth_clients WHERE is_active = true ORDER BY created_at DESC
             LIMIT $1 OFFSET $2",
            limit_i64,
            offset_i64
        )
        .fetch_all(&*self.pool)
        .await?;

        self.load_clients_with_relations_batch(rows).await
    }

    pub async fn count(&self) -> Result<i64> {
        let result =
            sqlx::query_scalar!("SELECT COUNT(*) FROM oauth_clients WHERE is_active = true")
                .fetch_one(&*self.pool)
                .await?;
        Ok(result.unwrap_or(0))
    }
}

impl ClientRepository {
    pub async fn find_registration_token_hash(
        &self,
        client_id: &ClientId,
    ) -> Result<Option<String>> {
        let client_id_str = client_id.as_str();
        let hash = sqlx::query_scalar!(
            "SELECT registration_token_hash FROM oauth_clients WHERE client_id = $1",
            client_id_str
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(hash.flatten())
    }
}
