//! A composition member's standing: core (with its terms: required, the core
//! id it fills and the types it may substitute) or install.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES};
use systemprompt_extension::{Extension, StandingTag};

use super::Conferral;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CoreTerms {
    pub(crate) required: bool,
    pub(crate) core: Option<CoreId>,
    pub(crate) substitutable: &'static [fn() -> TypeId],
}

impl CoreTerms {
    pub(crate) const OPTIONAL: Self = Self {
        required: false,
        core: None,
        substitutable: &[],
    };
    pub(crate) const REQUIRED: Self = Self {
        required: true,
        core: None,
        substitutable: &[],
    };

    #[cfg(any(
        feature = "files",
        feature = "content",
        feature = "analytics",
        feature = "notifications",
        feature = "marketplace",
        feature = "mcp-servers",
        feature = "agents",
        feature = "identity",
        feature = "brain",
        feature = "cloud",
        feature = "slack",
        feature = "teams",
        all(feature = "api", feature = "files"),
        all(feature = "api", feature = "ai"),
        all(feature = "api", feature = "agent-runs"),
    ))]
    pub(crate) const fn conferred(core: CoreId) -> Self {
        Self {
            required: false,
            core: Some(core),
            substitutable: &[],
        }
    }

    pub(crate) fn conferrals(self) -> impl Iterator<Item = &'static Conferral> {
        PRIVILEGED_SHAPES
            .iter()
            .filter(move |shape| Some(shape.owner) == self.core)
    }

    #[cfg(feature = "cli")]
    pub(crate) fn cli(
        self,
    ) -> impl Iterator<Item = &'static systemprompt_extension::reserved::ReservedCliPath> {
        systemprompt_extension::reserved::RESERVED_CLI_PATHS
            .iter()
            .filter(move |reserved| Some(reserved.owner) == self.core)
    }

    pub(crate) fn substitutes(self, ty: TypeId) -> bool {
        self.substitutable.iter().any(|of| of() == ty)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Standing {
    Core(CoreTerms),
    Install,
}

impl Standing {
    pub(crate) const fn tag(self) -> StandingTag {
        match self {
            Self::Core(_) => StandingTag::Core,
            Self::Install => StandingTag::Install,
        }
    }

    pub(crate) const fn terms(self) -> Option<CoreTerms> {
        match self {
            Self::Core(terms) => Some(terms),
            Self::Install => None,
        }
    }

    pub(crate) const fn required(self) -> bool {
        matches!(self, Self::Core(CoreTerms { required: true, .. }))
    }
}

pub(crate) struct Member {
    pub(crate) ext: Box<dyn Extension>,
    pub(crate) standing: Standing,
    pub(crate) type_id: TypeId,
}

impl Member {
    pub(crate) fn core<E: Extension + Default>(terms: CoreTerms) -> Self {
        Self {
            ext: Box::new(E::default()),
            standing: Standing::Core(terms),
            type_id: TypeId::of::<E>(),
        }
    }
}
