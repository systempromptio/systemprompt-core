use crate::auth::types::HelperOutput;
use thiserror::Error;

pub mod mtls;
pub mod pat;
pub mod session;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("not configured")]
    NotConfigured,
    #[error("{provider}: {source}")]
    Failed {
        provider: &'static str,
        #[source]
        source: AuthFailedSource,
    },
}

#[derive(Debug, Error)]
pub enum AuthFailedSource {
    #[error(transparent)]
    Keystore(#[from] crate::auth::keystore::KeystoreError),
    #[error(transparent)]
    Loopback(#[from] crate::auth::loopback::LoopbackError),
    #[error(transparent)]
    Gateway(#[from] crate::gateway::GatewayError),
}

pub trait AuthProvider {
    fn name(&self) -> &'static str;
    fn authenticate(&self) -> Result<HelperOutput, AuthError>;
}
