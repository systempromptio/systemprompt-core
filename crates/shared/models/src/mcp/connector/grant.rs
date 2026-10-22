//! Load-time validation of a connector's grant: which fields each grant
//! requires and which it forbids.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ConnectorConfig, ConnectorGrant};
use crate::errors::ServicesValidationError;
use systemprompt_identifiers::{ConnectorAppId, ConnectorInstallationId};

impl ConnectorConfig {
    pub(super) fn validate_grant(&self, name: &str) -> Result<(), ServicesValidationError> {
        let grant = self.grant.as_str();
        let invalid = |message: String| {
            ServicesValidationError::invalid_field(format!(
                "MCP server '{name}': connector grant {grant} {message}"
            ))
        };
        let present = |field: Option<&str>| field.is_some_and(|v| !v.trim().is_empty());
        let require = |field: Option<&str>, label: &str| {
            if present(field) {
                Ok(())
            } else {
                Err(invalid(format!("requires {label}")))
            }
        };
        let forbid = |set: bool, label: &str| {
            if set {
                Err(invalid(format!("does not use {label}")))
            } else {
                Ok(())
            }
        };

        match self.grant {
            ConnectorGrant::AuthorizationCode => {
                forbid(self.token_url.is_some(), "token_url")?;
                forbid(self.private_key_secret.is_some(), "private_key_secret")?;
                forbid(self.installation_id.is_some(), "installation_id")?;
                forbid(self.app_id.is_some(), "app_id")?;
            },
            ConnectorGrant::ClientCredentials => {
                require(self.token_url.as_deref(), "token_url")?;
                require(self.client_id_secret.as_deref(), "client_id_secret")?;
                require(self.client_secret.as_deref(), "client_secret")?;
                forbid(self.private_key_secret.is_some(), "private_key_secret")?;
                forbid(self.installation_id.is_some(), "installation_id")?;
                forbid(self.app_id.is_some(), "app_id")?;
            },
            ConnectorGrant::AppInstallation => {
                require(self.token_url.as_deref(), "token_url")?;
                require(self.private_key_secret.as_deref(), "private_key_secret")?;
                require(self.app_id.as_ref().map(ConnectorAppId::as_str), "app_id")?;
                require(
                    self.installation_id
                        .as_ref()
                        .map(ConnectorInstallationId::as_str),
                    "installation_id",
                )?;
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
