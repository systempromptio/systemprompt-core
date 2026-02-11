use super::inserts::InsertRelatedData;
use super::{ClientRepository, CreateClientParams, UpdateClientParams};
use crate::models::{ClientRelations, OAuthClient, OAuthClientRow, TokenAuthMethod};
use anyhow::Result;
use chrono::Utc;
use systemprompt_identifiers::ClientId;

impl ClientRepository {
    pub async fn create(&self, params: CreateClientParams) -> Result<OAuthClient> {
        let auth_method = params
            .token_endpoint_auth_method
            .as_deref()
            .unwrap_or(TokenAuthMethod::default().as_str());
        let now = Utc::now();

        let default_grant_types: Vec<String> = crate::models::GrantType::default_grant_types()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let default_response_types = vec![crate::models::ResponseType::Code.to_string()];

        let grant_types_list = params
            .grant_types
            .as_deref()
            .unwrap_or(&default_grant_types);
        let response_types_list = params
            .response_types
            .as_deref()
            .unwrap_or(&default_response_types);
        let contacts_list = params.contacts.as_deref().unwrap_or(&[]);

        sqlx::query!(
            r#"
            WITH new_client AS (
                INSERT INTO oauth_clients (client_id, client_secret_hash, client_name,
                                           token_endpoint_auth_method, client_uri, logo_uri,
                                           is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7)
            ),
            new_uris AS (
                INSERT INTO oauth_client_redirect_uris (client_id, redirect_uri, is_primary)
                SELECT $1, u.uri, u.ord = 1
                FROM unnest($8::text[]) WITH ORDINALITY AS u(uri, ord)
            ),
            new_grants AS (
                INSERT INTO oauth_client_grant_types (client_id, grant_type)
                SELECT $1, unnest($9::text[])
            ),
            new_responses AS (
                INSERT INTO oauth_client_response_types (client_id, response_type)
                SELECT $1, unnest($10::text[])
            ),
            new_scopes AS (
                INSERT INTO oauth_client_scopes (client_id, scope)
                SELECT $1, unnest($11::text[])
            )
            INSERT INTO oauth_client_contacts (client_id, contact_email)
            SELECT $1, unnest($12::text[])
            WHERE cardinality($12::text[]) > 0
            "#,
            params.client_id.as_str(),
            params.client_secret_hash,
            params.client_name,
            auth_method,
            params.client_uri,
            params.logo_uri,
            now,
            &params.redirect_uris,
            grant_types_list,
            response_types_list,
            &params.scopes,
            contacts_list,
        )
        .execute(&*self.write_pool)
        .await?;

        let row = OAuthClientRow {
            client_id: ClientId::new(params.client_id.as_str()),
            client_secret_hash: Some(params.client_secret_hash),
            client_name: params.client_name,
            name: None,
            token_endpoint_auth_method: Some(auth_method.to_string()),
            client_uri: params.client_uri,
            logo_uri: params.logo_uri,
            is_active: Some(true),
            created_at: Some(now),
            updated_at: Some(now),
            last_used_at: None,
        };

        let relations = ClientRelations {
            redirect_uris: params.redirect_uris,
            grant_types: params.grant_types.unwrap_or(default_grant_types),
            response_types: params.response_types.unwrap_or(default_response_types),
            scopes: params.scopes,
            contacts: params.contacts,
        };

        Ok(OAuthClient::from_row_with_relations(row, relations))
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
