//! CIMD metadata semantic validator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::fetcher::CimdFetcher;
use crate::error::{ClientMetadataError, OauthResult as Result};
use crate::models::cimd::ClientValidation;
use crate::repository::oauth::OAuthRepository;
use systemprompt_identifiers::ClientId;

#[derive(Debug)]
pub struct ClientValidator {
    dcr_repo: OAuthRepository,
    cimd_fetcher: CimdFetcher,
}

impl ClientValidator {
    pub fn new(dcr_repo: OAuthRepository) -> Result<Self> {
        Ok(Self {
            dcr_repo,
            cimd_fetcher: CimdFetcher::new()?,
        })
    }

    pub async fn validate_client(
        &self,
        client_id: &ClientId,
        redirect_uri: Option<&str>,
    ) -> Result<ClientValidation> {
        match client_id.client_type() {
            systemprompt_identifiers::ClientType::Cimd => {
                self.validate_cimd(client_id, redirect_uri).await
            },
            systemprompt_identifiers::ClientType::FirstParty => Ok(ClientValidation::FirstParty {
                client_id: client_id.clone(),
            }),
            systemprompt_identifiers::ClientType::ThirdParty => self.validate_dcr(client_id).await,
            systemprompt_identifiers::ClientType::System => Ok(ClientValidation::System {
                client_id: client_id.clone(),
            }),
            systemprompt_identifiers::ClientType::Unknown => {
                Err(crate::error::OauthError::InvalidClientMetadata(
                    ClientMetadataError::UnrecognisedClientId {
                        client_id: client_id.clone(),
                    },
                ))
            },
        }
    }

    async fn validate_cimd(
        &self,
        client_id: &ClientId,
        redirect_uri: Option<&str>,
    ) -> Result<ClientValidation> {
        let metadata = self.cimd_fetcher.fetch_metadata(client_id).await?;

        if let Some(uri) = redirect_uri
            && !metadata.has_redirect_uri(uri)
        {
            return Err(crate::error::OauthError::InvalidClientMetadata(
                ClientMetadataError::RedirectUriNotRegistered {
                    uri: uri.to_owned(),
                    client_id: client_id.clone(),
                },
            ));
        }

        Ok(ClientValidation::Cimd {
            client_id: client_id.clone(),
            metadata: Box::new(metadata),
        })
    }

    async fn validate_dcr(&self, client_id: &ClientId) -> Result<ClientValidation> {
        let client = self.dcr_repo.find_client_by_id(client_id).await?;

        if client.is_none() {
            return Err(crate::error::OauthError::DcrClientNotFound {
                client_id: client_id.clone(),
            });
        }

        Ok(ClientValidation::Dcr {
            client_id: client_id.clone(),
        })
    }
}
