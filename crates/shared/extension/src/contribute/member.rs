//! The scoped resolution every contributed factory is built with.
//!
//! A [`MemberScope`] names one admitted member and its admitted standing. It
//! is read off an admitted member or a slot entry (or built for an install,
//! the narrowest standing); a core scope cannot be spelled by member code.
//! A [`MemberResolved`] is a resolution narrowed to such a scope, and it is
//! the only thing [`Factory::build`](super::Factory::build) accepts, so a
//! factory of any kind runs only through a scoped path. Narrowing never
//! widens: a resolution already scoped to an install stays scoped to that
//! install whatever scope it is narrowed to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Deref;

use systemprompt_identifiers::ExtensionId;

use super::resolved::Resolved;
use super::standing::StandingTag;

/// One admitted member and the standing it was admitted with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberScope {
    owner: ExtensionId,
    standing: StandingTag,
}

impl MemberScope {
    pub(crate) const fn admitted(owner: ExtensionId, standing: StandingTag) -> Self {
        Self { owner, standing }
    }

    #[must_use]
    pub const fn install(owner: ExtensionId) -> Self {
        Self {
            owner,
            standing: StandingTag::Install,
        }
    }

    #[must_use]
    pub const fn owner(&self) -> &ExtensionId {
        &self.owner
    }

    #[must_use]
    pub const fn standing(&self) -> StandingTag {
        self.standing
    }
}

/// A resolution narrowed to one member's scope: what a factory is built with.
#[derive(Debug)]
pub struct MemberResolved<'a> {
    inner: Resolved<'a>,
}

impl<'a> MemberResolved<'a> {
    #[must_use]
    pub fn within<'r: 'a>(r: &Resolved<'r>, scope: &'a MemberScope) -> Self {
        Self {
            inner: r.scoped_to(&scope.owner, scope.standing),
        }
    }

    pub(crate) const fn from_scoped(inner: Resolved<'a>) -> Self {
        Self { inner }
    }
}

impl<'a> Deref for MemberResolved<'a> {
    type Target = Resolved<'a>;

    fn deref(&self) -> &Resolved<'a> {
        &self.inner
    }
}
