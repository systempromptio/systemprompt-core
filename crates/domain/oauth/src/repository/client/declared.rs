//! Static OAuth clients the identity config plane manages: rows whose
//! `declared_source` names the declaration file they come from.
//!
//! The plane reads and writes only these rows on the apply transaction. A
//! declared client stores the secrets-document key its secret came from, never
//! the secret; the secret is stored as its bcrypt hash like every other
//! client's. Releasing a client (its declaration was removed) clears both
//! stamps and `person_ui` and deactivates it. `person_ui` is written only here,
//! and [`ClientRepository::is_person_ui`] honours it only on an active row that
//! still carries its declaration stamp. Changing a client's `person_ui` revokes
//! its outstanding refresh tokens and the consent people remembered for it, so
//! no grant outlives the kind of session it was minted for; releasing it drops
//! that consent too.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use sqlx::{PgConnection, Postgres, Transaction};
use systemprompt_identifiers::{ClientId, UserId};

use super::ClientRepository;
use super::inserts::InsertRelatedData;
use crate::error::OauthResult as Result;
use crate::models::{ResponseType, TokenAuthMethod};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredClientRow {
    pub client_id: ClientId,
    pub client_name: String,
    pub owner_user_id: UserId,
    pub is_active: bool,
    pub secret_key: Option<String>,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub person_ui: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DeclaredClientWrite<'a> {
    pub client_id: &'a ClientId,
    pub client_name: &'a str,
    pub owner_user_id: &'a UserId,
    pub is_active: bool,
    pub secret_key: Option<&'a str>,
    pub secret_hash: Option<&'a str>,
    pub redirect_uris: &'a [String],
    pub grant_types: &'a [String],
    pub scopes: &'a [String],
    pub person_ui: bool,
    pub source: &'a str,
}

impl ClientRepository {
    pub async fn list_declared_on(conn: &mut PgConnection) -> Result<Vec<DeclaredClientRow>> {
        let rows = sqlx::query!(
            r#"
            SELECT c.client_id, c.client_name, c.owner_user_id, c.is_active,
                   c.declared_secret_key, c.person_ui,
                   COALESCE((SELECT array_agg(r.redirect_uri
                                              ORDER BY r.is_primary DESC, r.redirect_uri)
                             FROM oauth_client_redirect_uris r
                             WHERE r.client_id = c.client_id), '{}') AS "redirect_uris!",
                   COALESCE((SELECT array_agg(g.grant_type ORDER BY g.grant_type)
                             FROM oauth_client_grant_types g
                             WHERE g.client_id = c.client_id), '{}') AS "grant_types!",
                   COALESCE((SELECT array_agg(s.scope ORDER BY s.scope)
                             FROM oauth_client_scopes s
                             WHERE s.client_id = c.client_id), '{}') AS "scopes!"
            FROM oauth_clients c
            WHERE c.declared_source IS NOT NULL
            ORDER BY c.client_id
            "#,
        )
        .fetch_all(conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| DeclaredClientRow {
                client_id: ClientId::new(row.client_id),
                client_name: row.client_name,
                owner_user_id: UserId::new(row.owner_user_id),
                is_active: row.is_active,
                secret_key: row.declared_secret_key,
                redirect_uris: row.redirect_uris,
                grant_types: row.grant_types,
                scopes: row.scopes,
                person_ui: row.person_ui,
            })
            .collect())
    }

    async fn clear_grants_on_person_ui_change(
        tx: &mut Transaction<'_, Postgres>,
        client: &DeclaredClientWrite<'_>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM oauth_refresh_tokens
            WHERE client_id = $1
              AND EXISTS (SELECT 1 FROM oauth_clients
                          WHERE client_id = $1 AND person_ui <> $2)
            "#,
            client.client_id.as_str(),
            client.person_ui,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            r#"
            DELETE FROM oauth_consents
            WHERE client_id = $1
              AND EXISTS (SELECT 1 FROM oauth_clients
                          WHERE client_id = $1 AND person_ui <> $2)
            "#,
            client.client_id.as_str(),
            client.person_ui,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn upsert_declared_on(
        tx: &mut Transaction<'_, Postgres>,
        client: &DeclaredClientWrite<'_>,
    ) -> Result<()> {
        let auth_method = if client.secret_hash.is_some() {
            TokenAuthMethod::ClientSecretBasic
        } else {
            TokenAuthMethod::None
        };
        Self::clear_grants_on_person_ui_change(tx, client).await?;
        sqlx::query!(
            r#"
            INSERT INTO oauth_clients (client_id, client_secret_hash, client_name,
                                       token_endpoint_auth_method, application_type,
                                       is_active, created_at, updated_at, owner_user_id,
                                       declared_source, declared_secret_key, person_ui)
            VALUES ($1, $2, $3, $4, 'web', $5, $6, $6, $7, $8, $9, $10)
            ON CONFLICT (client_id) DO UPDATE
            SET client_secret_hash = EXCLUDED.client_secret_hash,
                client_name = EXCLUDED.client_name,
                token_endpoint_auth_method = EXCLUDED.token_endpoint_auth_method,
                is_active = EXCLUDED.is_active,
                owner_user_id = EXCLUDED.owner_user_id,
                declared_source = EXCLUDED.declared_source,
                declared_secret_key = EXCLUDED.declared_secret_key,
                person_ui = EXCLUDED.person_ui,
                updated_at = EXCLUDED.updated_at
            "#,
            client.client_id.as_str(),
            client.secret_hash,
            client.client_name,
            auth_method.as_str(),
            client.is_active,
            Utc::now(),
            client.owner_user_id.as_str(),
            client.source,
            client.secret_key,
            client.person_ui,
        )
        .execute(&mut **tx)
        .await?;
        Self::delete_related_data(tx, client.client_id).await?;
        let response_types = [ResponseType::Code.to_string()];
        Self::insert_related_data(
            tx,
            InsertRelatedData {
                client_id: client.client_id,
                redirect_uris: client.redirect_uris,
                grant_types: Some(client.grant_types),
                response_types: Some(&response_types),
                scopes: client.scopes,
                contacts: None,
            },
        )
        .await
    }

    pub async fn is_person_ui(&self, client_id: &ClientId) -> Result<bool> {
        let person_ui = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM oauth_clients
                WHERE client_id = $1 AND is_active AND person_ui
                  AND declared_source IS NOT NULL
            ) AS "person_ui!"
            "#,
            client_id.as_str()
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(person_ui)
    }

    pub async fn release_declared_on(conn: &mut PgConnection, client_id: &ClientId) -> Result<()> {
        sqlx::query!(
            "DELETE FROM oauth_consents WHERE client_id = $1",
            client_id.as_str()
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            r#"
            UPDATE oauth_clients
            SET declared_source = NULL, declared_secret_key = NULL, person_ui = false,
                is_active = false, updated_at = $2
            WHERE client_id = $1 AND declared_source IS NOT NULL
            "#,
            client_id.as_str(),
            Utc::now()
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}
