use super::{ClientRepository, CreateClientParams, UpdateClientParams};
use crate::models::{GrantType, OAuthClient, ResponseType, TokenAuthMethod};
use anyhow::Result;
use chrono::Utc;
use sqlx::{PgConnection, Postgres, Transaction};

struct InsertRelatedData<'a> {
    client_id: &'a str,
    redirect_uris: &'a [String],
    grant_types: Option<&'a [String]>,
    response_types: Option<&'a [String]>,
    scopes: &'a [String],
    contacts: Option<&'a [String]>,
}

impl ClientRepository {
    pub async fn create(&self, params: CreateClientParams) -> Result<OAuthClient> {
        let auth_method = params
            .token_endpoint_auth_method
            .as_deref()
            .unwrap_or(TokenAuthMethod::default().as_str());
        let now = Utc::now();

        let mut tx = self.pool.as_ref().begin().await?;

        sqlx::query!(
            "INSERT INTO oauth_clients (client_id, client_secret_hash, client_name,
                                       token_endpoint_auth_method, client_uri, logo_uri,
                                       is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7)",
            params.client_id,
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
                client_id: &params.client_id,
                redirect_uris: &params.redirect_uris,
                grant_types: params.grant_types.as_deref(),
                response_types: params.response_types.as_deref(),
                scopes: &params.scopes,
                contacts: params.contacts.as_deref(),
            },
        )
        .await?;

        tx.commit().await?;

        self.get_by_client_id(&params.client_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to load created client"))
    }

    pub async fn update(&self, params: UpdateClientParams) -> Result<Option<OAuthClient>> {
        let auth_method = params
            .token_endpoint_auth_method
            .as_deref()
            .unwrap_or(TokenAuthMethod::default().as_str());
        let now = Utc::now();

        let mut tx = self.pool.as_ref().begin().await?;

        let result = sqlx::query!(
            "UPDATE oauth_clients SET client_name = $1, token_endpoint_auth_method = $2,
                                      client_uri = $3, logo_uri = $4, updated_at = $5
             WHERE client_id = $6",
            params.client_name,
            auth_method,
            params.client_uri,
            params.logo_uri,
            now,
            params.client_id
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Self::delete_related_data(&mut tx, &params.client_id).await?;
        Self::insert_related_data(
            &mut tx,
            InsertRelatedData {
                client_id: &params.client_id,
                redirect_uris: &params.redirect_uris,
                grant_types: params.grant_types.as_deref(),
                response_types: params.response_types.as_deref(),
                scopes: &params.scopes,
                contacts: params.contacts.as_deref(),
            },
        )
        .await?;

        tx.commit().await?;

        self.get_by_client_id(&params.client_id).await
    }

    #[inline(never)]
    async fn delete_related_data(
        tx: &mut Transaction<'_, Postgres>,
        client_id: &str,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM oauth_client_redirect_uris WHERE client_id = $1",
            client_id
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM oauth_client_grant_types WHERE client_id = $1",
            client_id
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM oauth_client_response_types WHERE client_id = $1",
            client_id
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM oauth_client_scopes WHERE client_id = $1",
            client_id
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM oauth_client_contacts WHERE client_id = $1",
            client_id
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    #[inline(never)]
    async fn insert_related_data(
        tx: &mut Transaction<'_, Postgres>,
        data: InsertRelatedData<'_>,
    ) -> Result<()> {
        Self::insert_redirect_uris(tx, data.client_id, data.redirect_uris).await?;
        Self::insert_grant_types(tx, data.client_id, data.grant_types).await?;
        Self::insert_response_types(tx, data.client_id, data.response_types).await?;
        Self::insert_scopes(tx, data.client_id, data.scopes).await?;
        Self::insert_contacts(tx, data.client_id, data.contacts).await?;
        Ok(())
    }

    #[inline(never)]
    async fn insert_redirect_uris(
        conn: &mut PgConnection,
        client_id: &str,
        redirect_uris: &[String],
    ) -> Result<()> {
        if redirect_uris.is_empty() {
            return Ok(());
        }
        sqlx::query!(
            "INSERT INTO oauth_client_redirect_uris (client_id, redirect_uri, is_primary)
             SELECT $1, u.uri, u.ord = 1
             FROM unnest($2::text[]) WITH ORDINALITY AS u(uri, ord)",
            client_id,
            redirect_uris
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    #[inline(never)]
    async fn insert_grant_types(
        conn: &mut PgConnection,
        client_id: &str,
        grant_types: Option<&[String]>,
    ) -> Result<()> {
        let default_grant_types: Vec<String> = GrantType::default_grant_types()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let grant_types_list = grant_types.unwrap_or(&default_grant_types);
        if grant_types_list.is_empty() {
            return Ok(());
        }
        sqlx::query!(
            "INSERT INTO oauth_client_grant_types (client_id, grant_type)
             SELECT $1, unnest($2::text[])",
            client_id,
            grant_types_list
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    #[inline(never)]
    async fn insert_response_types(
        conn: &mut PgConnection,
        client_id: &str,
        response_types: Option<&[String]>,
    ) -> Result<()> {
        let default_response_types = vec![ResponseType::Code.to_string()];
        let response_types_list = response_types.unwrap_or(&default_response_types);
        if response_types_list.is_empty() {
            return Ok(());
        }
        sqlx::query!(
            "INSERT INTO oauth_client_response_types (client_id, response_type)
             SELECT $1, unnest($2::text[])",
            client_id,
            response_types_list
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    #[inline(never)]
    async fn insert_scopes(
        conn: &mut PgConnection,
        client_id: &str,
        scopes: &[String],
    ) -> Result<()> {
        if scopes.is_empty() {
            return Ok(());
        }
        sqlx::query!(
            "INSERT INTO oauth_client_scopes (client_id, scope)
             SELECT $1, unnest($2::text[])",
            client_id,
            scopes
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    #[inline(never)]
    async fn insert_contacts(
        conn: &mut PgConnection,
        client_id: &str,
        contacts: Option<&[String]>,
    ) -> Result<()> {
        let Some(contact_list) = contacts else {
            return Ok(());
        };
        if contact_list.is_empty() {
            return Ok(());
        }
        sqlx::query!(
            "INSERT INTO oauth_client_contacts (client_id, contact_email)
             SELECT $1, unnest($2::text[])",
            client_id,
            contact_list
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
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
        .execute(&*self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_by_client_id(client_id).await
    }

    pub async fn delete(&self, client_id: &str) -> Result<u64> {
        let result = sqlx::query!("DELETE FROM oauth_clients WHERE client_id = $1", client_id)
            .execute(&*self.pool)
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
        .execute(&*self.pool)
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
        .execute(&*self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
