//! [`ApnsSigner`]: the provider token Apple's push service authorizes with.
//!
//! The token is an ES256 JWT over the team id, signed with the `.p8` key the
//! declared secret holds and headed with its key id. One token is reused
//! until it is 45 minutes old and then signed again; the key is read from the
//! secret store at each signing, so a rotated key is picked up at the next
//! refresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use systemprompt_identifiers::SecretName;
use systemprompt_traits::SecretStore;
use tokio::sync::Mutex;

use crate::channels::push::{ProviderBearer, ProviderCredential, PushError};
use crate::plane::push::{ApnsDeclaration, ApnsKeyId, ApnsTeamId};

// Why: Apple refuses a provider token older than 60 minutes and throttles a
// provider that signs a new one more often than every 20.
const REFRESH_AFTER: Duration = Duration::from_mins(45);

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    iat: i64,
}

struct Minted {
    bearer: ProviderBearer,
    at: Instant,
}

/// Signs and caches the provider token for one Apple team.
pub struct ApnsSigner {
    secrets: Arc<dyn SecretStore>,
    secret: SecretName,
    key_id: ApnsKeyId,
    team_id: ApnsTeamId,
    minted: Mutex<Option<Minted>>,
}

impl ApnsSigner {
    #[must_use]
    pub fn new(secrets: Arc<dyn SecretStore>, declaration: &ApnsDeclaration) -> Self {
        Self {
            secrets,
            secret: declaration.signing_key_secret.clone(),
            key_id: declaration.key_id.clone(),
            team_id: declaration.team_id.clone(),
            minted: Mutex::new(None),
        }
    }

    pub fn sign(&self) -> Result<ProviderBearer, PushError> {
        let pem = self.secrets.secret(self.secret.as_str()).ok_or_else(|| {
            PushError::CredentialMissing {
                secret: self.secret.clone(),
            }
        })?;
        let key = EncodingKey::from_ec_pem(pem.expose().as_bytes()).map_err(|source| {
            PushError::CredentialInvalid {
                secret: self.secret.clone(),
                source: Box::new(source),
            }
        })?;
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key_id.as_str().to_owned());
        let claims = Claims {
            iss: self.team_id.as_str(),
            iat: chrono::Utc::now().timestamp(),
        };
        jsonwebtoken::encode(&header, &claims, &key)
            .map(ProviderBearer::new)
            .map_err(|source| PushError::CredentialInvalid {
                secret: self.secret.clone(),
                source: Box::new(source),
            })
    }
}

impl fmt::Debug for ApnsSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApnsSigner")
            .field("key_id", &self.key_id)
            .field("team_id", &self.team_id)
            .field("secret", &self.secret)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ProviderCredential for ApnsSigner {
    async fn bearer(&self) -> Result<ProviderBearer, PushError> {
        let mut minted = self.minted.lock().await;
        if let Some(current) = minted.as_ref()
            && current.at.elapsed() < REFRESH_AFTER
        {
            return Ok(current.bearer.clone());
        }
        let bearer = self.sign()?;
        *minted = Some(Minted {
            bearer: bearer.clone(),
            at: Instant::now(),
        });
        drop(minted);
        Ok(bearer)
    }
}
