use super::ClientRepository;
use crate::models::{ClientRelations, OAuthClient, OAuthClientRow};
use anyhow::Result;

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

    async fn load_redirect_uris(&self, client_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar!(
            "SELECT redirect_uri FROM oauth_client_redirect_uris WHERE client_id = $1 ORDER BY \
             is_primary DESC",
            client_id
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_grant_types(&self, client_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar!(
            "SELECT grant_type FROM oauth_client_grant_types WHERE client_id = $1",
            client_id
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_response_types(&self, client_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar!(
            "SELECT response_type FROM oauth_client_response_types WHERE client_id = $1",
            client_id
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_scopes(&self, client_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar!(
            "SELECT scope FROM oauth_client_scopes WHERE client_id = $1",
            client_id
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    async fn load_contacts(&self, client_id: &str) -> Result<Option<Vec<String>>> {
        let rows = sqlx::query_scalar!(
            "SELECT contact_email FROM oauth_client_contacts WHERE client_id = $1",
            client_id
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
