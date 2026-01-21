use super::ClientRepository;
use crate::models::{OAuthClient, OAuthClientRow};
use anyhow::Result;

impl ClientRepository {
    pub async fn get_by_client_id(&self, client_id: &str) -> Result<Option<OAuthClient>> {
        let row = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    client_uri, logo_uri, is_active, created_at, updated_at, last_used_at
             FROM oauth_clients WHERE client_id = $1 AND is_active = true",
            client_id
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

    pub async fn get_by_client_id_any(&self, client_id: &str) -> Result<Option<OAuthClient>> {
        let row = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    client_uri, logo_uri, is_active, created_at, updated_at, last_used_at
             FROM oauth_clients WHERE client_id = $1",
            client_id
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
                    client_uri, logo_uri, is_active, created_at, updated_at, last_used_at
             FROM oauth_clients WHERE is_active = true ORDER BY created_at DESC"
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut clients = Vec::new();
        for row in rows {
            let client = self.load_client_with_relations(row).await?;
            clients.push(client);
        }

        Ok(clients)
    }

    pub async fn list_paginated(&self, limit: i32, offset: i32) -> Result<Vec<OAuthClient>> {
        let limit_i64 = i64::from(limit);
        let offset_i64 = i64::from(offset);
        let rows = sqlx::query_as!(
            OAuthClientRow,
            "SELECT client_id, client_secret_hash, client_name, name, token_endpoint_auth_method,
                    client_uri, logo_uri, is_active, created_at, updated_at, last_used_at
             FROM oauth_clients WHERE is_active = true ORDER BY created_at DESC
             LIMIT $1 OFFSET $2",
            limit_i64,
            offset_i64
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut clients = Vec::new();
        for row in rows {
            let client = self.load_client_with_relations(row).await?;
            clients.push(client);
        }

        Ok(clients)
    }

    pub async fn count(&self) -> Result<i64> {
        let result =
            sqlx::query_scalar!("SELECT COUNT(*) FROM oauth_clients WHERE is_active = true")
                .fetch_one(&*self.pool)
                .await?;
        Ok(result.unwrap_or(0))
    }

    pub async fn find_by_redirect_uri(&self, redirect_uri: &str) -> Result<Option<OAuthClient>> {
        let row = sqlx::query_as!(
            OAuthClientRow,
            r#"SELECT c.client_id, c.client_secret_hash, c.client_name, c.name,
                      c.token_endpoint_auth_method, c.client_uri, c.logo_uri,
                      c.is_active, c.created_at, c.updated_at, c.last_used_at
             FROM oauth_clients c
             INNER JOIN oauth_client_redirect_uris r ON c.client_id = r.client_id
             WHERE r.redirect_uri = $1 AND c.is_active = true
             LIMIT 1"#,
            redirect_uri
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

    pub async fn find_by_redirect_uri_with_scope(
        &self,
        redirect_uri: &str,
        required_scopes: &[&str],
    ) -> Result<Option<OAuthClient>> {
        let client = self.find_by_redirect_uri(redirect_uri).await?;

        match client {
            Some(c)
                if required_scopes
                    .iter()
                    .any(|s| c.scopes.contains(&s.to_string())) =>
            {
                Ok(Some(c))
            },
            _ => Ok(None),
        }
    }
}
