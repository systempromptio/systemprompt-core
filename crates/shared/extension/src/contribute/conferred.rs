//! The capabilities a member reads in place of a sealed seed: what it
//! declared (`Contributions::conferral`, `Contributions::token_audience`)
//! and admission accepted, each a handle the row's core issuer builds for
//! that member alone ([`crate::reserved::INSTALL_CONFERRALS`]).
//!
//! The kernel inserts one [`ConferralDeclarations`] sealed, after refusing a
//! declared audience or scope a member may not mint for. A provide fn,
//! factory or slot build scoped to a member reads its handle through
//! [`Resolved::member_cipher`], [`Resolved::share_tokens`],
//! [`Resolved::git_source_sync`] or [`Resolved::token_minter`]; the read is
//! refused when the member did not declare the row, when the issuer is absent
//! on this host, or when the issuer was provided by anyone but the row's core
//! provider. A command's or a route set's resolution reads none.
//!
//! A host that confers nothing (a child host serving MCP or an agent) seeds
//! [`ConferralsWithheld`] sealed. There a declared read is
//! [`ConferralError::Withheld`], which a provide fn's `?` turns into
//! `ProvideError::Withheld`: the member's value is withheld on that host and
//! the host boots, and a member that can run without the handle matches the
//! variant. An undeclared read is still the typed missing dependency, so no
//! member reads another member's conferral on any host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::ScopedTokenMinter;
use systemprompt_provider_contracts::{DependencyProvider, MissingDependency};
use systemprompt_traits::{
    DynConferralIssuer, GitSourceSync, MemberCipher, MemberGrant, ShareTokenSigner,
    TokenAudienceGrant,
};

use super::Contributions;
use super::error::{FactoryError, ProvideError};
use super::resolved::Resolved;
use crate::reserved::InstallConferral;

/// The host marker that withholds every conferral on this host; seeded
/// sealed by a child host only, and reserved on the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConferralsWithheld;

/// Why a member's conferral read returned no handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConferralError {
    #[error(transparent)]
    Missing(#[from] MissingDependency),
    #[error("the {} conferral is withheld on this host", .0.name())]
    Withheld(InstallConferral),
}

impl From<ConferralError> for ProvideError {
    fn from(error: ConferralError) -> Self {
        match error {
            ConferralError::Missing(missing) => Self::from(missing),
            ConferralError::Withheld(_) => Self::Withheld,
        }
    }
}

impl From<ConferralError> for FactoryError {
    fn from(error: ConferralError) -> Self {
        match error {
            ConferralError::Missing(missing) => Self::Missing(missing),
            withheld @ ConferralError::Withheld(_) => Self::Failed(Box::new(withheld)),
        }
    }
}

/// One admitted member's declared capabilities and token audiences.
#[derive(Debug, Clone)]
pub struct MemberConferrals {
    pub rows: Arc<[InstallConferral]>,
    pub token_audiences: Arc<[TokenAudienceGrant]>,
}

/// Every admitted member's accepted conferral declarations; inserted sealed
/// by the kernel only.
#[derive(Debug, Default)]
pub struct ConferralDeclarations {
    declared: HashMap<ExtensionId, MemberConferrals>,
}

impl ConferralDeclarations {
    #[must_use]
    pub const fn new(declared: HashMap<ExtensionId, MemberConferrals>) -> Self {
        Self { declared }
    }

    fn grant(&self, owner: &ExtensionId, row: InstallConferral) -> Option<MemberGrant> {
        let member = self.declared.get(owner)?;
        member.rows.contains(&row).then(|| MemberGrant {
            member: owner.clone(),
            token_audiences: Arc::clone(&member.token_audiences),
        })
    }
}

impl Contributions<'_> {
    pub fn conferral(&mut self, row: InstallConferral) -> &mut Self {
        if !self.sink.conferrals.contains(&row) {
            self.sink.conferrals.push(row);
        }
        self
    }

    pub fn token_audience(&mut self, audience: &str, scopes: &[&str]) -> &mut Self {
        self.conferral(InstallConferral::ScopedTokens);
        self.sink.token_audiences.push(TokenAudienceGrant {
            audience: audience.to_owned(),
            scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        });
        self
    }
}

impl Resolved<'_> {
    pub fn member_cipher(&self) -> Result<Arc<dyn MemberCipher>, ConferralError> {
        self.conferred::<dyn MemberCipher>(InstallConferral::MemberCipher)
    }

    pub fn share_tokens(&self) -> Result<Arc<dyn ShareTokenSigner>, ConferralError> {
        self.conferred::<dyn ShareTokenSigner>(InstallConferral::ShareTokens)
    }

    pub fn git_source_sync(&self) -> Result<Arc<dyn GitSourceSync>, ConferralError> {
        self.conferred::<dyn GitSourceSync>(InstallConferral::GitSourceSync)
    }

    pub fn token_minter(&self) -> Result<Arc<dyn ScopedTokenMinter>, ConferralError> {
        self.conferred::<dyn ScopedTokenMinter>(InstallConferral::ScopedTokens)
    }

    fn conferred<C: ?Sized + 'static>(
        &self,
        row: InstallConferral,
    ) -> Result<Arc<C>, ConferralError> {
        let missing = MissingDependency::of::<Arc<C>>;
        let (owner, state) = self.secret_reader().ok_or_else(missing)?;
        let grant = state
            .find::<Arc<ConferralDeclarations>>()
            .and_then(|declarations| declarations.grant(owner, row))
            .ok_or_else(missing)?;
        if state.find::<ConferralsWithheld>().is_some()
            && state.is_sealed::<ConferralsWithheld>()
            && matches!(
                state.provider_of::<ConferralsWithheld>(),
                Some(DependencyProvider::Host)
            )
        {
            return Err(ConferralError::Withheld(row));
        }
        let from_provider = match state.provider_of::<DynConferralIssuer<C>>() {
            Some(DependencyProvider::Host) => {
                row.provider() == crate::reserved::CoreId::Kernel.id()
            },
            Some(DependencyProvider::Extension(provider)) => provider.as_str() == row.provider(),
            None => false,
        };
        if !from_provider || !state.is_sealed::<DynConferralIssuer<C>>() {
            return Err(missing().into());
        }
        let issuer = state.find::<DynConferralIssuer<C>>().ok_or_else(missing)?;
        Ok(issuer.issue(&grant))
    }
}
