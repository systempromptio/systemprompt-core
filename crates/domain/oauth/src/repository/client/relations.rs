use super::ClientRepository;
use crate::models::{ClientRelations, OAuthClient, OAuthClientRow};
use anyhow::Result;
use std::collections::HashMap;
use systemprompt_identifiers::ClientId;

impl ClientRepository {
    pub(super) async fn load_client_with_relations(
        &self,
        row: OAuthClientRow,
    ) -> Result<OAuthClient> {
        let relations = ClientRelations {
            redirect_uris: self.load_redirect_uris(&row.client_id).await?,
            grant_types: self.load_grant_types(&row.client_id).await?,
            response_types: self.load_response_types(&row.client_id).await?,
            scopes: self.load_scopes(&row.client_id).await?,
            contacts: self.load_contacts(&row.client_id).await?,
        };

        Ok(OAuthClient::from_row_with_relations(row, relations))
    }

    pub(super) async fn load_clients_with_relations_batch(
        &self,
        rows: Vec<OAuthClientRow>,
    ) -> Result<Vec<OAuthClient>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let client_ids: Vec<String> = rows.iter().map(|r| r.client_id.to_string()).collect();

        let (redirect_uris, grant_types, response_types, scopes, contacts) = tokio::try_join!(
            self.load_redirect_uris_batch(&client_ids),
            self.load_grant_types_batch(&client_ids),
            self.load_response_types_batch(&client_ids),
            self.load_scopes_batch(&client_ids),
            self.load_contacts_batch(&client_ids),
        )?;

        let mut clients = Vec::with_capacity(rows.len());
        for row in rows {
            let cid = row.client_id.to_string();
            let relations = ClientRelations {
                redirect_uris: redirect_uris.get(&cid).cloned().unwrap_or_default(),
                grant_types: grant_types.get(&cid).cloned().unwrap_or_default(),
                response_types: response_types.get(&cid).cloned().unwrap_or_default(),
                scopes: scopes.get(&cid).cloned().unwrap_or_default(),
                contacts: contacts.get(&cid).cloned(),
            };
            clients.push(OAuthClient::from_row_with_relations(row, relations));
        }

        Ok(clients)
    }

    async fn load_redirect_uris_batch(
        &self,
        client_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let rows = sqlx::query!(
            "SELECT client_id, redirect_uri FROM oauth_client_redirect_uris WHERE client_id = ANY($1) ORDER BY is_primary DESC",
            client_ids
        )
        .fetch_all(&*self.pool)
        .await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            map.entry(row.client_id).or_default().push(row.redirect_uri);
        }
        Ok(map)
    }

    async fn load_grant_types_batch(
        &self,
        client_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let rows = sqlx::query!(
            "SELECT client_id, grant_type FROM oauth_client_grant_types WHERE client_id = ANY($1)",
            client_ids
        )
        .fetch_all(&*self.pool)
        .await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            map.entry(row.client_id).or_default().push(row.grant_type);
        }
        Ok(map)
    }

    async fn load_response_types_batch(
        &self,
        client_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let rows = sqlx::query!(
            "SELECT client_id, response_type FROM oauth_client_response_types WHERE client_id = ANY($1)",
            client_ids
        )
        .fetch_all(&*self.pool)
        .await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            map.entry(row.client_id).or_default().push(row.response_type);
        }
        Ok(map)
    }

    async fn load_scopes_batch(
        &self,
        client_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let rows = sqlx::query!(
            "SELECT client_id, scope FROM oauth_client_scopes WHERE client_id = ANY($1)",
            client_ids
        )
        .fetch_all(&*self.pool)
        .await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            map.entry(row.client_id).or_default().push(row.scope);
        }
        Ok(map)
    }

    async fn load_contacts_batch(
        &self,
        client_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let rows = sqlx::query!(
            "SELECT client_id, contact_email FROM oauth_client_contacts WHERE client_id = ANY($1)",
            client_ids
        )
        .fetch_all(&*self.pool)
        .await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            map.entry(row.client_id).or_default().push(row.contact_email);
        }
        Ok(map)
    }

    async fn load_redirect_uris(&self, client_id: &ClientId) -> Result<Vec<String>> {
        let client_id_str = client_id.as_str();
        let rows = sqlx::query_scalar!(
            "SELECT redirect_uri FROM oauth_client_redirect_uris WHERE client_id = $1 ORDER BY \
             is_primary DESC",
            client_id_str
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_grant_types(&self, client_id: &ClientId) -> Result<Vec<String>> {
        let client_id_str = client_id.as_str();
        let rows = sqlx::query_scalar!(
            "SELECT grant_type FROM oauth_client_grant_types WHERE client_id = $1",
            client_id_str
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_response_types(&self, client_id: &ClientId) -> Result<Vec<String>> {
        let client_id_str = client_id.as_str();
        let rows = sqlx::query_scalar!(
            "SELECT response_type FROM oauth_client_response_types WHERE client_id = $1",
            client_id_str
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_scopes(&self, client_id: &ClientId) -> Result<Vec<String>> {
        let client_id_str = client_id.as_str();
        let rows = sqlx::query_scalar!(
            "SELECT scope FROM oauth_client_scopes WHERE client_id = $1",
            client_id_str
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_contacts(&self, client_id: &ClientId) -> Result<Option<Vec<String>>> {
        let client_id_str = client_id.as_str();
        let rows = sqlx::query_scalar!(
            "SELECT contact_email FROM oauth_client_contacts WHERE client_id = $1",
            client_id_str
        )
        .fetch_all(&*self.pool)
        .await?;

        if rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(rows))
        }
    }
}
