mod auth_code;
mod refresh_token;
mod scopes;
mod user;

pub use auth_code::{AuthCodeParams, AuthCodeValidationResult};
pub use refresh_token::RefreshTokenParams;

use super::{ClientRepository, CreateClientParams, UpdateClientParams};
use crate::models::OAuthClient;
use crate::services::generate_client_secret;
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_database::DbPool;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct OAuthRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    client_repo: ClientRepository,
}

impl OAuthRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
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
    pub async fn create_client(&self, params: CreateClientParams) -> Result<OAuthClient> {
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

    pub async fn list_clients(&self) -> Result<Vec<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.list().await
    }

    pub async fn list_clients_paginated(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.list_paginated(limit, offset).await
    }

    pub async fn count_clients(&self) -> Result<i64> {
        let client_repo = &self.client_repo;
        client_repo.count().await
    }

    pub async fn find_client_by_id(&self, client_id: &str) -> Result<Option<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.get_by_client_id(client_id).await
    }

    pub async fn find_client_by_redirect_uri(
        &self,
        redirect_uri: &str,
    ) -> Result<Option<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo.find_by_redirect_uri(redirect_uri).await
    }

    pub async fn find_client_by_redirect_uri_with_scope(
        &self,
        redirect_uri: &str,
        required_scopes: &[&str],
    ) -> Result<Option<OAuthClient>> {
        let client_repo = &self.client_repo;
        client_repo
            .find_by_redirect_uri_with_scope(redirect_uri, required_scopes)
            .await
    }


    pub async fn update_client(
        &self,
        client_id: &str,
        name: Option<&str>,
        redirect_uris: Option<&[String]>,
        scopes: Option<&[String]>,
    ) -> Result<OAuthClient> {
        let updated_name = match name {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                return Err(anyhow::anyhow!("Client name is required for update"));
            },
        };

        let updated_redirect_uris = redirect_uris
            .filter(|uris| !uris.is_empty())
            .ok_or_else(|| anyhow::anyhow!("At least one redirect URI required"))?;

        let updated_scopes = scopes
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("At least one scope required"))?;

        let client = self
            .find_client_by_id(client_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

        let client_repo = &self.client_repo;
        let params = UpdateClientParams {
            client_id: client_id.into(),
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

        updated.ok_or_else(|| anyhow::anyhow!("Client not found"))
    }

    pub async fn update_client_full(&self, client: &OAuthClient) -> Result<OAuthClient> {
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

        updated.ok_or_else(|| anyhow::anyhow!("Client not found"))
    }

    pub async fn delete_client(&self, client_id: &str) -> Result<bool> {
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

    pub async fn cleanup_inactive_clients(&self) -> Result<u64> {
        let client_repo = &self.client_repo;
        client_repo.cleanup_inactive().await
    }

    pub async fn cleanup_old_test_clients(&self, days_old: u32) -> Result<u64> {
        let client_repo = &self.client_repo;
        client_repo.cleanup_old_test(days_old).await
    }

    pub async fn cleanup_unused_clients(&self, days_old: u32) -> Result<u64> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_old) * 24 * 60 * 60);
        let client_repo = &self.client_repo;
        client_repo.delete_unused(cutoff_timestamp).await
    }

    pub async fn cleanup_stale_clients(&self, days_unused: u32) -> Result<u64> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_unused) * 24 * 60 * 60);
        let client_repo = &self.client_repo;
        client_repo.delete_stale(cutoff_timestamp).await
    }

    pub async fn list_unused_clients(
        &self,
        days_old: u32,
    ) -> Result<Vec<super::ClientUsageSummary>> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_old) * 24 * 60 * 60);
        let client_repo = &self.client_repo;
        client_repo.list_unused(cutoff_timestamp).await
    }

    pub async fn list_stale_clients(
        &self,
        days_unused: u32,
    ) -> Result<Vec<super::ClientUsageSummary>> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_unused) * 24 * 60 * 60);
        let client_repo = &self.client_repo;
        client_repo.list_stale(cutoff_timestamp).await
    }

    pub async fn deactivate_old_test_clients(&self, days_old: u32) -> Result<u64> {
        let client_repo = &self.client_repo;
        client_repo.deactivate_old_test(days_old).await
    }

    pub async fn list_inactive_clients(&self) -> Result<Vec<super::ClientSummary>> {
        let client_repo = &self.client_repo;
        client_repo.list_inactive().await
    }

    pub async fn list_old_clients(&self, days_old: u32) -> Result<Vec<super::ClientSummary>> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_old) * 24 * 60 * 60);
        let client_repo = &self.client_repo;
        client_repo.list_old(cutoff_timestamp).await
    }

    pub async fn update_client_last_used(&self, client_id: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        let client_repo = &self.client_repo;
        client_repo.update_last_used(client_id, now).await
    }
}
