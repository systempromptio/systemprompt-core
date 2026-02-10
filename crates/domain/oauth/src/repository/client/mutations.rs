use super::inserts::InsertRelatedData;
use super::{ClientRepository, CreateClientParams, UpdateClientParams};
use crate::models::{OAuthClient, TokenAuthMethod};
use anyhow::Result;
use chrono::Utc;

impl ClientRepository {
    pub async fn create(&self, params: CreateClientParams) -> Result<OAuthClient> {
        let auth_method = params
            .token_endpoint_auth_method
            .as_deref()
            .unwrap_or(TokenAuthMethod::default().as_str());
        let now = Utc::now();

        let mut tx = self.write_pool.as_ref().begin().await?;

        sqlx::query!(
            "INSERT INTO oauth_clients (client_id, client_secret_hash, client_name,
                                       token_endpoint_auth_method, client_uri, logo_uri,
                                       is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7)",
            params.client_id.as_str(),
            params.client_secret_hash,
            params.client_name,
            auth_method,
            params.client_uri,
            params.logo_uri,
            now
        )
        .execute(&mut *tx)
        .await?;

        Self::insert_related_data(
            &mut tx,
            InsertRelatedData {
                client_id: params.client_id.as_str(),
                redirect_uris: &params.redirect_uris,
                grant_types: params.grant_types.as_deref(),
                response_types: params.response_types.as_deref(),
                scopes: &params.scopes,
                contacts: params.contacts.as_deref(),
            },
        )
        .await?;

        tx.commit().await?;

        self.get_by_client_id(params.client_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to load created client"))
    }

    pub async fn update(&self, params: UpdateClientParams) -> Result<Option<OAuthClient>> {
        let auth_method = params
            .token_endpoint_auth_method
            .as_deref()
            .unwrap_or(TokenAuthMethod::default().as_str());
        let now = Utc::now();

        let mut tx = self.write_pool.as_ref().begin().await?;

        let result = sqlx::query!(
            "UPDATE oauth_clients SET client_name = $1, token_endpoint_auth_method = $2,
                                      client_uri = $3, logo_uri = $4, updated_at = $5
             WHERE client_id = $6",
            params.client_name,
            auth_method,
            params.client_uri,
            params.logo_uri,
            now,
            params.client_id.as_str()
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Self::delete_related_data(&mut tx, params.client_id.as_str()).await?;
        Self::insert_related_data(
            &mut tx,
            InsertRelatedData {
                client_id: params.client_id.as_str(),
                redirect_uris: &params.redirect_uris,
                grant_types: params.grant_types.as_deref(),
                response_types: params.response_types.as_deref(),
                scopes: &params.scopes,
                contacts: params.contacts.as_deref(),
            },
        )
        .await?;

        tx.commit().await?;

        self.get_by_client_id(params.client_id.as_str()).await
    }

    pub async fn update_secret(
        &self,
        client_id: &str,
        client_secret_hash: &str,
    ) -> Result<Option<OAuthClient>> {
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE oauth_clients SET client_secret_hash = $1, updated_at = $2 WHERE client_id = \
             $3",
            client_secret_hash,
            now,
            client_id
        )
        .execute(&*self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_by_client_id(client_id).await
    }

    pub async fn delete(&self, client_id: &str) -> Result<u64> {
        let result = sqlx::query!("DELETE FROM oauth_clients WHERE client_id = $1", client_id)
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn deactivate(&self, client_id: &str) -> Result<u64> {
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE oauth_clients SET is_active = false, updated_at = $1 WHERE client_id = $2",
            now,
            client_id
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn activate(&self, client_id: &str) -> Result<u64> {
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE oauth_clients SET is_active = true, updated_at = $1 WHERE client_id = $2",
            now,
            client_id
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
