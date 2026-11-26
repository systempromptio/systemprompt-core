//! The closed table of sealed conferrals.
//!
//! A conferral lets the route state, and the commands, of exactly one named
//! core member read a
//! value another core member provided sealed, or a sealed host seed (the
//! provider is then [`CoreId::Kernel`]). Each row names the provider's id and
//! the holding [`CoreId`]; nothing else carries a sealed member value across
//! a member boundary at runtime. A host seed's row is recorded by the host
//! that seeds it (`Dependencies::with_sealed_conferred`). A provider declares
//! its row with `Contributions::provide_conferred`; the kernel refuses the
//! provide unless the provider has core standing and is the row's provider, and
//! every read through a route set's resolution re-checks the row against the
//! reading owner and its standing ([`conferral_admits`]), so a forged row, a
//! forged provider or an install holder reads nothing.
//!
//! The second table, [`INSTALL_CONFERRALS`], names the capabilities core
//! confers on any admitted member, install standing included, that declared
//! the row (`Contributions::conferral`, `Contributions::token_audience`). A
//! row names its issuer's core provider; the issuer is sealed, no member
//! reads it, and the kernel refuses any other member's provide of its type.
//! A member reads only its own handle, through its provide fns, factories and
//! slot builds (`Resolved::member_cipher`, `Resolved::share_tokens`,
//! `Resolved::git_source_sync`, `Resolved::token_minter`,
//! `Resolved::broker_calls`); a command's or a
//! route set's resolution reads none, and an undeclared row, an absent issuer
//! or an issuer from another provider is the `MissingDependency` of the
//! handle type, so the provide fails typed and boot names the grant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{TypeId, type_name};

use systemprompt_models::auth::ScopedTokenMinter;
use systemprompt_traits::{
    BrokerCallVerifier, DynConferralIssuer, GitSourceSync, MemberCipher, ShareTokenSigner,
};

use super::CoreId;

/// One row of [`SEALED_CONFERRALS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConferralRow {
    GatewayEngines,
    GatewayAtRest,
    GatewaySessionMint,
    GatewayProviderCredentials,
    IdentityTokenAuthority,
    DocumentLinkTokens,
    IdentitySigningKeys,
    IdentitySignInSessions,
    IdentityOAuthRepositories,
    GatewayManifestSigner,
    BridgeReleaseSource,
    GatewaySettings,
    BridgeEventStream,
    DriveTokens,
    IdentityOwnerReassignments,
    IdentityPluginTokens,
    AgentSecretPresence,
}

/// One sealed value its core provider confers on one core holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedConferral {
    pub row: ConferralRow,
    pub provider: &'static str,
    pub holder: CoreId,
}

pub const SEALED_CONFERRALS: &[SealedConferral] = &[
    ConferralRow::GatewayEngines.entry(),
    ConferralRow::GatewayAtRest.entry(),
    ConferralRow::GatewaySessionMint.entry(),
    ConferralRow::GatewayProviderCredentials.entry(),
    ConferralRow::IdentityTokenAuthority.entry(),
    ConferralRow::DocumentLinkTokens.entry(),
    ConferralRow::IdentitySigningKeys.entry(),
    ConferralRow::IdentitySignInSessions.entry(),
    ConferralRow::IdentityOAuthRepositories.entry(),
    ConferralRow::GatewayManifestSigner.entry(),
    ConferralRow::BridgeReleaseSource.entry(),
    ConferralRow::GatewaySettings.entry(),
    ConferralRow::BridgeEventStream.entry(),
    ConferralRow::DriveTokens.entry(),
    ConferralRow::IdentityOwnerReassignments.entry(),
    ConferralRow::IdentityPluginTokens.entry(),
    ConferralRow::AgentSecretPresence.entry(),
];

impl ConferralRow {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GatewayEngines => "gateway-engines",
            Self::GatewayAtRest => "gateway-at-rest",
            Self::GatewaySessionMint => "gateway-session-mint",
            Self::GatewayProviderCredentials => "gateway-provider-credentials",
            Self::IdentityTokenAuthority => "identity-token-authority",
            Self::DocumentLinkTokens => "document-link-tokens",
            Self::IdentitySigningKeys => "identity-signing-keys",
            Self::IdentitySignInSessions => "identity-sign-in-sessions",
            Self::IdentityOAuthRepositories => "identity-oauth-repositories",
            Self::GatewayManifestSigner => "gateway-manifest-signer",
            Self::BridgeReleaseSource => "bridge-release-source",
            Self::GatewaySettings => "gateway-settings",
            Self::BridgeEventStream => "bridge-event-stream",
            Self::DriveTokens => "drive-tokens",
            Self::IdentityOwnerReassignments => "identity-owner-reassignments",
            Self::IdentityPluginTokens => "identity-plugin-tokens",
            Self::AgentSecretPresence => "agent-secret-presence",
        }
    }

    #[must_use]
    pub const fn entry(self) -> SealedConferral {
        let (provider, holder) = match self {
            Self::GatewayEngines
            | Self::GatewayAtRest
            | Self::GatewayManifestSigner
            | Self::BridgeReleaseSource
            | Self::GatewaySettings
            | Self::BridgeEventStream => ("gateway", CoreId::GatewayHttp),
            Self::GatewaySessionMint => ("users", CoreId::GatewayHttp),
            Self::GatewayProviderCredentials => ("ai", CoreId::GatewayHttp),
            Self::IdentityTokenAuthority
            | Self::IdentitySigningKeys
            | Self::IdentityOwnerReassignments => (CoreId::Kernel.id(), CoreId::IdentityHttp),
            Self::IdentityOAuthRepositories | Self::IdentityPluginTokens => {
                ("oauth", CoreId::IdentityHttp)
            },
            Self::IdentitySignInSessions => ("users", CoreId::IdentityHttp),
            Self::DocumentLinkTokens | Self::DriveTokens => ("documents", CoreId::FilesHttp),
            Self::AgentSecretPresence => ("agent", CoreId::AgentHttp),
        };
        SealedConferral {
            row: self,
            provider,
            holder,
        }
    }
}

#[must_use]
pub fn conferral_named(name: &str) -> Option<&'static SealedConferral> {
    SEALED_CONFERRALS
        .iter()
        .find(|entry| entry.row.name() == name)
}

#[must_use]
pub fn conferral_admits(name: &str, provider: &str, holder: &str) -> bool {
    conferral_named(name)
        .is_some_and(|entry| entry.provider == provider && entry.holder.id() == holder)
}

/// One capability of [`INSTALL_CONFERRALS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallConferral {
    MemberCipher,
    ShareTokens,
    GitSourceSync,
    ScopedTokens,
    BrokerCalls,
}

pub const INSTALL_CONFERRALS: &[InstallConferral] = &[
    InstallConferral::MemberCipher,
    InstallConferral::ShareTokens,
    InstallConferral::GitSourceSync,
    InstallConferral::ScopedTokens,
    InstallConferral::BrokerCalls,
];

impl InstallConferral {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MemberCipher => "member-cipher",
            Self::ShareTokens => "share-tokens",
            Self::GitSourceSync => "git-source-sync",
            Self::ScopedTokens => "scoped-tokens",
            Self::BrokerCalls => "broker-calls",
        }
    }

    #[must_use]
    pub const fn provider(self) -> &'static str {
        match self {
            Self::MemberCipher | Self::ShareTokens | Self::ScopedTokens | Self::BrokerCalls => {
                CoreId::Kernel.id()
            },
            Self::GitSourceSync => "managed_resources",
        }
    }

    #[must_use]
    pub const fn issuer_type_id(self) -> TypeId {
        match self {
            Self::MemberCipher => TypeId::of::<DynConferralIssuer<dyn MemberCipher>>(),
            Self::ShareTokens => TypeId::of::<DynConferralIssuer<dyn ShareTokenSigner>>(),
            Self::GitSourceSync => TypeId::of::<DynConferralIssuer<dyn GitSourceSync>>(),
            Self::ScopedTokens => TypeId::of::<DynConferralIssuer<dyn ScopedTokenMinter>>(),
            Self::BrokerCalls => TypeId::of::<DynConferralIssuer<dyn BrokerCallVerifier>>(),
        }
    }

    #[must_use]
    pub fn issuer_type_name(self) -> &'static str {
        match self {
            Self::MemberCipher => type_name::<DynConferralIssuer<dyn MemberCipher>>(),
            Self::ShareTokens => type_name::<DynConferralIssuer<dyn ShareTokenSigner>>(),
            Self::GitSourceSync => type_name::<DynConferralIssuer<dyn GitSourceSync>>(),
            Self::ScopedTokens => type_name::<DynConferralIssuer<dyn ScopedTokenMinter>>(),
            Self::BrokerCalls => type_name::<DynConferralIssuer<dyn BrokerCallVerifier>>(),
        }
    }
}
