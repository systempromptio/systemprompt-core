//! The capabilities the kernel confers on a member in place of a sealed seed.
//!
//! They are an encrypt/decrypt handle over the deployment's master key
//! ([`MemberCipher`]), a share-token signer ([`ShareTokenSigner`]) and a
//! Git source sync handle ([`GitSourceSync`]); the scoped token minter is
//! beside the token authority in `systemprompt_models::auth`.
//!
//! Core holds one [`ConferralIssuer`] per capability, sealed. A member never
//! reads an issuer: it reads its own handle through `Resolved`, which the
//! issuer builds for that member's [`MemberGrant`] alone. Every handle is
//! bound to the member it was issued for: the cipher and the share-token
//! signer derive their key from the member id under a label of their own, so
//! one member's handle cannot open another member's ciphertext or verify its
//! tokens, and no handle exports, or names, the key it was derived from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_identifiers::{
    ExtensionId, ManagedResourceId, ManagedSourceId, ResourceRevisionId, SourceSnapshotId,
    WithdrawalProposalId,
};

use crate::BoxedSource;

/// One audience a member declared it mints tokens for, and the scopes it
/// may put in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAudienceGrant {
    pub audience: String,
    pub scopes: Vec<String>,
}

/// What an issuer builds a member's handle from: the member and what it
/// declared at contribution, after admission accepted it.
#[derive(Debug, Clone)]
pub struct MemberGrant {
    pub member: ExtensionId,
    pub token_audiences: Arc<[TokenAudienceGrant]>,
}

/// Builds one member's handle of the capability `C`; held sealed by core.
pub trait ConferralIssuer<C: ?Sized>: Send + Sync + std::fmt::Debug {
    fn issue(&self, grant: &MemberGrant) -> Arc<C>;
}

pub type DynConferralIssuer<C> = Arc<dyn ConferralIssuer<C>>;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum MemberCipherError {
    #[error("the sealed value is malformed")]
    Malformed,
    #[error("the sealed value did not authenticate under this member's key")]
    NotAuthentic,
}

/// Authenticated encryption under a key derived for one member.
pub trait MemberCipher: Send + Sync + std::fmt::Debug {
    fn seal(&self, plaintext: &str) -> Result<String, MemberCipherError>;

    fn open(&self, sealed: &str) -> Result<String, MemberCipherError>;
}

pub const SHARE_TOKEN_MAX_TTL: Duration = Duration::from_hours(365 * 24);

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ShareTokenError {
    #[error("a share-token purpose is 1 to 64 characters of [a-z0-9._-]")]
    Purpose,
    #[error("a share token lives between one second and 365 days")]
    Lifetime,
    #[error("the share token is malformed")]
    Malformed,
    #[error("the share token did not verify under this member's key for this purpose")]
    NotAuthentic,
    #[error("the share token has expired")]
    Expired,
}

/// Signs and verifies one member's own share tokens, each bound to a
/// purpose and an expiry.
pub trait ShareTokenSigner: Send + Sync + std::fmt::Debug {
    fn sign(&self, purpose: &str, payload: &[u8], ttl: Duration)
    -> Result<String, ShareTokenError>;

    fn verify(&self, purpose: &str, token: &str) -> Result<Vec<u8>, ShareTokenError>;
}

/// One Git source sync a member asks for.
#[derive(Debug, Clone)]
pub struct GitSourceSyncRequest {
    pub source_id: ManagedSourceId,
    pub resource_id: ManagedResourceId,
    pub upstream_root: String,
    pub upstream_base_revision_id: Option<ResourceRevisionId>,
}

/// What a sync recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitSourceSynced {
    Incoming {
        snapshot_id: SourceSnapshotId,
        revision_id: ResourceRevisionId,
        commit: String,
    },
    WithdrawalProposed {
        snapshot_id: SourceSnapshotId,
        proposal_id: WithdrawalProposalId,
        commit: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum GitSourceSyncError {
    #[error("the source is not a registered Git source of this owner")]
    NotGitSource,
    #[error("the source's stored credential is not available")]
    CredentialUnavailable,
    #[error("the sync could not be audited, so it did not run")]
    Audit(#[source] BoxedSource),
    #[error("the Git source sync failed")]
    Failed(#[source] BoxedSource),
}

/// Syncs a Git source the system admin registered.
///
/// The owner is the one installs attribute managed inventory to; the
/// credential core holds is never returned, and every sync is audited first.
/// `#[async_trait]` keeps the trait `dyn`-compatible for its issuer.
#[async_trait]
pub trait GitSourceSync: Send + Sync + std::fmt::Debug {
    async fn sync(
        &self,
        request: &GitSourceSyncRequest,
    ) -> Result<GitSourceSynced, GitSourceSyncError>;
}
