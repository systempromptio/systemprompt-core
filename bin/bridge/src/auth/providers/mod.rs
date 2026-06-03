use crate::auth::types::HelperOutput;
use async_trait::async_trait;
use systemprompt_identifiers::SessionId;
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

impl AuthFailedSource {
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        match self {
            Self::Keystore(_) | Self::Loopback(_) => true,
            Self::Gateway(g) => {
                use crate::gateway::GatewayError as G;
                matches!(
                    g,
                    G::PubkeyMissing
                        | G::UnsafePath(_)
                        | G::PubkeyDecode(_)
                        | G::ManifestDecode(_)
                        | G::WhoamiDecode(_)
                        | G::ProfileDecode(_)
                        | G::AuthDecode(_)
                        | G::Serialize(_)
                )
            },
        }
    }
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// The minted JWT binds to `session_id` so it matches the `x-session-id`
    /// the bridge presents on `/v1/messages`.
    async fn authenticate(&self, session_id: &SessionId) -> Result<HelperOutput, AuthError>;
}
