use super::ClientRepository;
use crate::models::{GrantType, ResponseType};
use anyhow::Result;
use sqlx::{PgConnection, Postgres, Transaction};

pub(super) struct InsertRelatedData<'a> {
    pub client_id: &'a str,
    pub redirect_uris: &'a [String],
    pub grant_types: Option<&'a [String]>,
    pub response_types: Option<&'a [String]>,
    pub scopes: &'a [String],
    pub contacts: Option<&'a [String]>,
}

impl ClientRepository {
    #[inline(never)]
    pub(super) async fn delete_related_data(
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
    pub(super) async fn insert_related_data(
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
}
