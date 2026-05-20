//! Aggregated OAuth repository orchestrating client, code, and token
//! operations.

mod at_rest;
mod auth_code;
mod cleanup;
mod refresh_token;
mod scopes;
mod user;

pub use auth_code::{AuthCodeParams, AuthCodeValidationResult};
pub use refresh_token::RefreshTokenParams;

pub(super) use at_rest::hash_at_rest;

use super::{ClientRepository, CreateClientParams, UpdateClientParams};
use crate::error::{OauthError, OauthResult};
use crate::models::OAuthClient;
use crate::services::generate_client_secret;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ClientId;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct OAuthRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    client_repo: ClientRepository,
}

impl OAuthRepository {
    pub fn new(db: &DbPool) -> OauthResult<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        let client_repo = ClientRepository::new(db)?;
        Ok(Self {
            pool,
            write_pool,
            client_repo,
        })
    }

    pub fn pool_ref(&self) -> &PgPool {
        &self.pool
    }

    pub fn write_pool_ref(&self) -> &PgPool {
        &self.write_pool
    }

    #[instrument(skip(self, params), fields(client_id = %params.client_id, client_name = %params.client_name))]
    pub async fn create_client(&self, params: CreateClientParams) -> OauthResult<OAuthClient> {
        let start_time = Instant::now();

        tracing::info!("Creating OAuth client");

        let client_repo = &self.client_repo;
        let client_id = params.client_id.clone();
        let client_name = params.client_name.clone();
        let scopes = params.scopes.clone();
        let redirect_uris = params.redirect_uris.clone();

        match client_repo.create(params).await {
            Ok(client) => {
                let duration = start_time.elapsed();

                tracing::info!(
                    client_id = %client_id,
                    client_name = %client_name,
                    scopes = ?scopes,
                    redirect_uris = ?redirect_uris,
                    created_in_ms = duration.as_millis(),
                    "OAuth client created"
                );

                if duration.as_millis() > 500 {
                    tracing::warn!(
                        client_id = %client_id,
                        duration_ms = duration.as_millis(),
                        "Slow OAuth client creation"
                    );
                }

                Ok(client)
            },
            Err(e) => {
                let duration = start_time.elapsed();
                tracing::error!(
                    error = %e,
                    client_id = %client_id,
                    duration_ms = duration.as_millis(),
                    "OAuth client creation failed"
                );
                Err(e)
            },
        }
    }

    pub async fn list_clients(&self) -> OauthResult<Vec<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.list().await
    }

    pub async fn list_clients_paginated(
        &self,
        limit: i32,
        offset: i32,
    ) -> OauthResult<Vec<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.list_paginated(limit, offset).await
    }

    pub async fn count_clients(&self) -> OauthResult<i64> {
        let client_repo = &self.client_repo;
        client_repo.count().await
    }

    pub async fn find_client_by_id(
        &self,
        client_id: &ClientId,
    ) -> OauthResult<Option<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.get_by_client_id(client_id).await
    }

    pub async fn find_client_by_redirect_uri(
        &self,
        redirect_uri: &str,
    ) -> OauthResult<Option<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.find_by_redirect_uri(redirect_uri).await
    }

    pub async fn find_client_by_redirect_uri_with_scope(
        &self,
        redirect_uri: &str,
        required_scopes: &[&str],
    ) -> OauthResult<Option<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo
            .find_by_redirect_uri_with_scope(redirect_uri, required_scopes)
            .await
    }

    pub async fn update_client(
        &self,
        client_id: &ClientId,
        name: Option<&str>,
        redirect_uris: Option<&[String]>,
        scopes: Option<&[String]>,
    ) -> OauthResult<OAuthClient> {
        let updated_name = match name {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                return Err(OauthError::Validation(
                    "Client name is required for update".to_string(),
                ));
            },
        };

        let updated_redirect_uris =
            redirect_uris
                .filter(|uris| !uris.is_empty())
                .ok_or_else(|| {
                    OauthError::Validation("At least one redirect URI required".to_string())
                })?;

        let updated_scopes = scopes
            .filter(|s| !s.is_empty())
            .ok_or_else(|| OauthError::Validation("At least one scope required".to_string()))?;

        let client = self
            .find_client_by_id(client_id)
            .await?
            .ok_or_else(|| OauthError::Validation("Client not found".to_string()))?;

        let client_repo = &self.client_repo;
        let params = UpdateClientParams {
            client_id: client_id.clone(),
            client_name: updated_name,
            redirect_uris: updated_redirect_uris.to_vec(),
            grant_types: Some(client.grant_types.clone()),
            response_types: Some(client.response_types.clone()),
            scopes: updated_scopes.to_vec(),
            token_endpoint_auth_method: Some(client.token_endpoint_auth_method.clone()),
            client_uri: client.client_uri.clone(),
            logo_uri: client.logo_uri.clone(),
            contacts: client.contacts.clone(),
        };
        let updated = client_repo.update(params).await?;

        updated.ok_or_else(|| OauthError::Validation("Client not found".to_string()))
    }

    pub async fn update_client_secret(
        &self,
        client_id: &ClientId,
        client_secret_hash: &str,
    ) -> OauthResult<Option<OAuthClient>> {
        self.client_repo
            .update_secret(client_id, client_secret_hash)
            .await
    }

    pub async fn update_client_full(&self, client: &OAuthClient) -> OauthResult<OAuthClient> {
        let client_repo = &self.client_repo;
        let params = UpdateClientParams {
            client_id: client.client_id.clone(),
            client_name: client.client_name.clone(),
            redirect_uris: client.redirect_uris.clone(),
            grant_types: Some(client.grant_types.clone()),
            response_types: Some(client.response_types.clone()),
            scopes: client.scopes.clone(),
            token_endpoint_auth_method: Some(client.token_endpoint_auth_method.clone()),
            client_uri: client.client_uri.clone(),
            logo_uri: client.logo_uri.clone(),
            contacts: client.contacts.clone(),
        };
        let updated = client_repo.update(params).await?;

        updated.ok_or_else(|| OauthError::Validation("Client not found".to_string()))
    }

    pub async fn delete_client(&self, client_id: &ClientId) -> OauthResult<bool> {
        let client_repo = &self.client_repo;
        let rows_affected = client_repo.delete(client_id).await?;
        Ok(rows_affected > 0)
    }

    #[must_use]
    pub fn generate_client_secret() -> String {
        generate_client_secret()
    }

    #[must_use]
    pub fn generate_client_id() -> String {
        format!("client_{}", Uuid::new_v4().simple())
    }
}
