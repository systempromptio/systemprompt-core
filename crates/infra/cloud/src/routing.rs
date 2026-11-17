//! Operator credentials exchanged for a tenant session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::SessionToken;

use crate::deploy::provider::tenant_base_url;
use crate::error::CloudError;
use crate::operator::{OperatorCredentials, exchange_for_tenant};
use crate::paths::{CloudPath, get_cloud_paths};

#[derive(Debug, thiserror::Error)]
pub enum OperatorTokenError {
    #[error(transparent)]
    Credentials(CloudError),
    #[error("Operator token exchange with {hostname} failed")]
    Exchange {
        hostname: String,
        #[source]
        source: CloudError,
    },
}

pub async fn operator_tenant_token(hostname: &str) -> Result<SessionToken, OperatorTokenError> {
    let path = get_cloud_paths().resolve(CloudPath::OperatorCredentials);
    let operator =
        OperatorCredentials::load_valid(&path).map_err(OperatorTokenError::Credentials)?;
    let session = exchange_for_tenant(&tenant_base_url(hostname), &operator)
        .await
        .map_err(|source| OperatorTokenError::Exchange {
            hostname: hostname.to_owned(),
            source,
        })?;
    Ok(SessionToken::new(session.access_token))
}
