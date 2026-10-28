//! The `services/identity/*.yaml` document and the canonical entity values
//! the identity plane compares.
//!
//! A user is keyed `user:<normalised email>` and a static OAuth client
//! `oauth_client:<client_id>`. Each value is the canonical JSON of its
//! fields: roles, grant types and scopes sorted and deduplicated, redirect URIs
//! with the first (primary) one kept first and the rest sorted, so a file and
//! the rows it produced compare equal exactly when they mean the same thing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ClientId;
use systemprompt_models::auth::{AccountKind, UserStatus};
use systemprompt_users::normalise_email;

pub const USER_PREFIX: &str = "user:";
pub const CLIENT_PREFIX: &str = "oauth_client:";

const DECLARABLE_STATUSES: &[UserStatus] = &[
    UserStatus::Active,
    UserStatus::Inactive,
    UserStatus::Suspended,
];
const DECLARABLE_GRANT_TYPES: &[&str] =
    &["authorization_code", "refresh_token", "client_credentials"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DeclarationError(String);

impl DeclarationError {
    const fn new(message: String) -> Self {
        Self(message)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityDocument {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<DeclaredUser>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub oauth_clients: Vec<DeclaredClient>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredUser {
    pub email: String,
    pub name: String,
    #[serde(default)]
    pub kind: AccountKind,
    #[serde(default = "default_roles")]
    pub roles: Vec<String>,
    #[serde(default = "default_status")]
    pub status: UserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredClient {
    pub client_id: ClientId,
    pub name: String,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redirect_uris: Vec<String>,
    #[serde(default = "default_grant_types")]
    pub grant_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_roles() -> Vec<String> {
    vec!["user".to_owned()]
}

const fn default_status() -> UserStatus {
    UserStatus::Active
}

fn default_grant_types() -> Vec<String> {
    vec!["authorization_code".to_owned(), "refresh_token".to_owned()]
}

const fn default_active() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserValue {
    pub name: String,
    pub kind: AccountKind,
    pub roles: Vec<String>,
    pub status: UserStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientValue {
    pub name: String,
    pub owner: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub secret_key: Option<String>,
    pub active: bool,
}

#[must_use]
pub fn sorted_unique(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[must_use]
pub fn canonical_redirect_uris(uris: &[String]) -> Vec<String> {
    let Some((primary, rest)) = uris.split_first() else {
        return Vec::new();
    };
    let primary = primary.trim().to_owned();
    let rest = sorted_unique(rest).into_iter().filter(|u| *u != primary);
    std::iter::once(primary.clone()).chain(rest).collect()
}

#[must_use]
pub fn user_key(email: &str) -> String {
    format!("{USER_PREFIX}{}", normalise_email(email))
}

#[must_use]
pub fn client_key(client_id: &ClientId) -> String {
    format!("{CLIENT_PREFIX}{}", client_id.as_str().trim())
}

impl DeclaredUser {
    pub fn validate(&self) -> Result<(), DeclarationError> {
        let email = normalise_email(&self.email);
        if !email.contains('@') {
            return Err(DeclarationError::new(format!(
                "user email '{}' is not an email address",
                self.email
            )));
        }
        if self.name.trim().is_empty() {
            return Err(DeclarationError::new(format!(
                "user {email} has an empty name"
            )));
        }
        if sorted_unique(&self.roles).is_empty() {
            return Err(DeclarationError::new(format!(
                "user {email} declares no roles"
            )));
        }
        if !DECLARABLE_STATUSES.contains(&self.status) {
            return Err(DeclarationError::new(format!(
                "user {email} declares status '{}'; only active, inactive and suspended can be \
                 declared",
                self.status
            )));
        }
        Ok(())
    }

    #[must_use]
    pub fn value(&self) -> UserValue {
        UserValue {
            name: self.name.trim().to_owned(),
            kind: self.kind,
            roles: sorted_unique(&self.roles),
            status: self.status,
        }
    }
}

impl DeclaredClient {
    pub fn validate(&self) -> Result<(), DeclarationError> {
        let id = self.client_id.as_str().trim();
        if id.is_empty() {
            return Err(DeclarationError::new(
                "an oauth client has an empty client_id".to_owned(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(DeclarationError::new(format!(
                "oauth client {id} has an empty name"
            )));
        }
        if !normalise_email(&self.owner).contains('@') {
            return Err(DeclarationError::new(format!(
                "oauth client {id}: owner must be a user's email"
            )));
        }
        if let Some(bad) = self
            .grant_types
            .iter()
            .find(|g| !DECLARABLE_GRANT_TYPES.contains(&g.trim()))
        {
            return Err(DeclarationError::new(format!(
                "oauth client {id}: grant type '{bad}' cannot be declared (allowed: {})",
                DECLARABLE_GRANT_TYPES.join(", ")
            )));
        }
        if self
            .secret_key
            .as_deref()
            .is_some_and(|k| k.trim().is_empty())
        {
            return Err(DeclarationError::new(format!(
                "oauth client {id}: secret_key is empty"
            )));
        }
        Ok(())
    }

    #[must_use]
    pub fn value(&self) -> ClientValue {
        ClientValue {
            name: self.name.trim().to_owned(),
            owner: normalise_email(&self.owner),
            redirect_uris: canonical_redirect_uris(&self.redirect_uris),
            grant_types: sorted_unique(&self.grant_types),
            scopes: sorted_unique(&self.scopes),
            secret_key: self.secret_key.as_deref().map(|k| k.trim().to_owned()),
            active: self.active,
        }
    }
}

impl UserValue {
    #[must_use]
    pub fn into_declared(self, email: &str) -> DeclaredUser {
        DeclaredUser {
            email: email.to_owned(),
            name: self.name,
            kind: self.kind,
            roles: self.roles,
            status: self.status,
        }
    }
}

impl ClientValue {
    #[must_use]
    pub fn into_declared(self, client_id: ClientId) -> DeclaredClient {
        DeclaredClient {
            client_id,
            name: self.name,
            owner: self.owner,
            redirect_uris: self.redirect_uris,
            grant_types: self.grant_types,
            scopes: self.scopes,
            secret_key: self.secret_key,
            active: self.active,
        }
    }
}
