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

use super::resolved::Resolved;
use crate::reserved::InstallConferral;

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

impl Resolved<'_> {
    pub fn member_cipher(&self) -> Result<Arc<dyn MemberCipher>, MissingDependency> {
        self.conferred::<dyn MemberCipher>(InstallConferral::MemberCipher)
    }

    pub fn share_tokens(&self) -> Result<Arc<dyn ShareTokenSigner>, MissingDependency> {
        self.conferred::<dyn ShareTokenSigner>(InstallConferral::ShareTokens)
    }

    pub fn git_source_sync(&self) -> Result<Arc<dyn GitSourceSync>, MissingDependency> {
        self.conferred::<dyn GitSourceSync>(InstallConferral::GitSourceSync)
    }

    pub fn token_minter(&self) -> Result<Arc<dyn ScopedTokenMinter>, MissingDependency> {
        self.conferred::<dyn ScopedTokenMinter>(InstallConferral::ScopedTokens)
    }

    fn conferred<C: ?Sized + 'static>(
        &self,
        row: InstallConferral,
    ) -> Result<Arc<C>, MissingDependency> {
        let missing = MissingDependency::of::<Arc<C>>;
        let (owner, state) = self.secret_reader().ok_or_else(missing)?;
        let grant = state
            .find::<Arc<ConferralDeclarations>>()
            .and_then(|declarations| declarations.grant(owner, row))
            .ok_or_else(missing)?;
        let from_provider = match state.provider_of::<DynConferralIssuer<C>>() {
            Some(DependencyProvider::Host) => {
                row.provider() == crate::reserved::CoreId::Kernel.id()
            },
            Some(DependencyProvider::Extension(provider)) => provider.as_str() == row.provider(),
            None => false,
        };
        if !from_provider || !state.is_sealed::<DynConferralIssuer<C>>() {
            return Err(missing());
        }
        let issuer = state.find::<DynConferralIssuer<C>>().ok_or_else(missing)?;
        Ok(issuer.issue(&grant))
    }
}
