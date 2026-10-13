//! Load-time validation of a connector's grant: which fields each grant
//! requires and which it forbids.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ConnectorConfig, ConnectorGrant};
use crate::errors::ServicesValidationError;

impl ConnectorConfig {
    pub(super) fn validate_grant(&self, name: &str) -> Result<(), ServicesValidationError> {
        let grant = self.grant.as_str();
        let invalid = |message: String| {
            ServicesValidationError::invalid_field(format!(
                "MCP server '{name}': connector grant {grant} {message}"
            ))
        };
        let present =
            |field: &Option<String>| field.as_deref().is_some_and(|v| !v.trim().is_empty());
        let require = |field: &Option<String>, label: &str| {
            if present(field) {
                Ok(())
            } else {
                Err(invalid(format!("requires {label}")))
            }
        };
        let forbid = |field: &Option<String>, label: &str| {
            if field.is_some() {
                Err(invalid(format!("does not use {label}")))
            } else {
                Ok(())
            }
        };

        match self.grant {
            ConnectorGrant::AuthorizationCode => {
                forbid(&self.token_url, "token_url")?;
                forbid(&self.private_key_secret, "private_key_secret")?;
                forbid(&self.installation_id, "installation_id")?;
                forbid(&self.app_id, "app_id")?;
            },
            ConnectorGrant::ClientCredentials => {
                require(&self.token_url, "token_url")?;
                require(&self.client_id_secret, "client_id_secret")?;
                require(&self.client_secret, "client_secret")?;
                forbid(&self.private_key_secret, "private_key_secret")?;
                forbid(&self.installation_id, "installation_id")?;
                forbid(&self.app_id, "app_id")?;
            },
            ConnectorGrant::AppInstallation => {
                require(&self.token_url, "token_url")?;
                require(&self.private_key_secret, "private_key_secret")?;
                require(&self.app_id, "app_id")?;
                require(&self.installation_id, "installation_id")?;
            },
        }

        if self.grant.is_machine() {
            if !self.authorization_params.is_empty() || !self.authorization_origins.is_empty() {
                return Err(invalid(
                    "skips the consent flow, so authorization_params and authorization_origins \
                     do not apply"
                        .to_owned(),
                ));
            }
            if let Some(url) = self.token_url.as_deref()
                && !url.starts_with("https://")
            {
                return Err(invalid("requires an https:// token_url".to_owned()));
            }
        }
        Ok(())
    }
}
