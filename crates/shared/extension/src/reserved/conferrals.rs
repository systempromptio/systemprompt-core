//! The closed table of sealed conferrals.
//!
//! A conferral lets the route state of exactly one named core member read a
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
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CoreId;

/// One row of [`SEALED_CONFERRALS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConferralRow {
    GatewayEngines,
    GatewayAtRest,
    GatewaySessionMint,
    GatewayProviderCredentials,
    IdentityTokenAuthority,
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
        }
    }

    #[must_use]
    pub const fn entry(self) -> SealedConferral {
        let (provider, holder) = match self {
            Self::GatewayEngines | Self::GatewayAtRest => ("gateway", CoreId::GatewayHttp),
            Self::GatewaySessionMint => ("users", CoreId::GatewayHttp),
            Self::GatewayProviderCredentials => ("ai", CoreId::GatewayHttp),
            Self::IdentityTokenAuthority => (CoreId::Kernel.id(), CoreId::IdentityHttp),
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
