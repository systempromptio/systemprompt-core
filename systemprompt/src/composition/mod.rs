//! The composition: which members a binary is built from, the standing each
//! holds, and the admission of their contributions against a profile.
//!
//! [`reference`] is the reference composition: every core member in
//! dependency order, each with the [`CoreTerms`] the conferral table in the
//! kernel's [`reserved`](systemprompt_extension::reserved) constants grants
//! it. An install adds members with [`Composition::with`], always as
//! installs, and removes an optional core member with
//! [`Composition::without`]; there is no API that replaces a member,
//! configures a core member's terms or confers anything on an install.
//!
//! Composition runs in two phases. [`Composition::declare`] calls every
//! member's `contribute` once, before any profile is loaded: it validates
//! ids, admits CLI commands against the reserved CLI paths, and yields the
//! [`Declared`] set whose [`Declared::commands`] the CLI builds its command
//! tree from. [`Declared::admit`] then reads the profile: it first drops
//! every contribution of a disabled member — routes, commands, jobs,
//! retention, projections, config planes, guards, subject providers,
//! policies, slot entries, provide fns, factories and asset closures — and
//! only then admits routes against the conferrals and checks providers. No
//! provide fn, factory, slot build or asset closure runs here; the runtime
//! runs them over the [`Admitted`] handoff.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::collections::HashSet;

use systemprompt_extension::contribute::{ContributionSink, Contributions};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::{Admitted, Extension, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Profile;

#[cfg(feature = "cli")]
use admit::admit_commands;
#[cfg(feature = "config")]
use admit::check_secrets;
use admit::{apply_removals, check_member_dependencies};
pub use error::{CompositionError, RunError};

mod admit;
mod declared;
mod error;
mod providers;
mod reference;
mod routes;

pub use declared::Declared;
use declared::DeclaredMember;

pub use reference::reference;

pub(super) type Conferral = PrivilegedShape;

#[derive(Debug, Clone, Copy)]
pub(super) struct CoreTerms {
    pub(super) required: bool,
    pub(super) core: Option<CoreId>,
    pub(super) substitutable: &'static [fn() -> TypeId],
}

impl CoreTerms {
    pub(super) const OPTIONAL: Self = Self {
        required: false,
        core: None,
        substitutable: &[],
    };
    pub(super) const REQUIRED: Self = Self {
        required: true,
        core: None,
        substitutable: &[],
    };

    pub(super) const fn conferred(core: CoreId) -> Self {
        Self {
            required: false,
            core: Some(core),
            substitutable: &[],
        }
    }

    pub(super) fn conferrals(self) -> impl Iterator<Item = &'static Conferral> {
        PRIVILEGED_SHAPES
            .iter()
            .filter(move |shape| Some(shape.owner) == self.core)
    }

    #[cfg(feature = "cli")]
    pub(super) fn cli(
        self,
    ) -> impl Iterator<Item = &'static systemprompt_extension::reserved::ReservedCliPath> {
        systemprompt_extension::reserved::RESERVED_CLI_PATHS
            .iter()
            .filter(move |reserved| Some(reserved.owner) == self.core)
    }

    pub(super) fn substitutes(self, ty: TypeId) -> bool {
        self.substitutable.iter().any(|of| of() == ty)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Standing {
    Core(CoreTerms),
    Install,
}

impl Standing {
    pub(super) const fn tag(self) -> StandingTag {
        match self {
            Self::Core(_) => StandingTag::Core,
            Self::Install => StandingTag::Install,
        }
    }

    pub(super) const fn terms(self) -> Option<CoreTerms> {
        match self {
            Self::Core(terms) => Some(terms),
            Self::Install => None,
        }
    }

    pub(super) const fn required(self) -> bool {
        matches!(self, Self::Core(CoreTerms { required: true, .. }))
    }
}

pub(super) struct Member {
    pub(super) ext: Box<dyn Extension>,
    pub(super) standing: Standing,
    pub(super) type_id: TypeId,
}

impl Member {
    pub(super) fn core<E: Extension + Default>(terms: CoreTerms) -> Self {
        Self {
            ext: Box::new(E::default()),
            standing: Standing::Core(terms),
            type_id: TypeId::of::<E>(),
        }
    }
}

/// The secrets source kinds a composition can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsKind {
    File,
    Env,
    Vault,
    Install(&'static str),
}

pub(super) const RESERVED_SECRETS_KINDS: &[&str] = &["file", "env", "vault", "external"];

#[cfg(feature = "config")]
struct SecretsSourceDecl {
    kind: SecretsKind,
    factory: systemprompt_config::SecretsSourceFactory,
}

/// The members a binary is built from, with their standing.
pub struct Composition {
    members: Vec<Member>,
    removed: Vec<TypeId>,
    #[cfg(feature = "config")]
    secrets: Vec<SecretsSourceDecl>,
}

impl std::fmt::Debug for Composition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composition")
            .field("members", &self.members.len())
            .field("removed", &self.removed.len())
            .finish_non_exhaustive()
    }
}

impl Composition {
    #[must_use]
    pub fn with<E: Extension>(mut self, e: E) -> Self {
        self.members.push(Member {
            ext: Box::new(e),
            standing: Standing::Install,
            type_id: TypeId::of::<E>(),
        });
        self
    }

    #[must_use]
    pub fn member_ids(&self) -> Vec<&'static str> {
        self.members
            .iter()
            .map(|member| member.ext.metadata().id)
            .collect()
    }

    #[must_use]
    pub fn without<E: Extension>(mut self) -> Self {
        self.removed.push(TypeId::of::<E>());
        self
    }

    #[cfg(feature = "config")]
    #[must_use]
    pub fn secrets_source(
        mut self,
        kind: SecretsKind,
        f: systemprompt_config::SecretsSourceFactory,
    ) -> Self {
        self.secrets.push(SecretsSourceDecl { kind, factory: f });
        self
    }

    pub fn declare(self) -> Result<Declared, CompositionError> {
        #[cfg(feature = "config")]
        let secrets = check_secrets(self.secrets)?;
        let members = apply_removals(self.members, &self.removed)?;
        let mut declared = Vec::with_capacity(members.len());
        let mut ids = HashSet::new();
        for member in members {
            let meta = member.ext.metadata();
            let id =
                ExtensionId::try_new(meta.id).map_err(|source| CompositionError::InvalidId {
                    id: meta.id,
                    source,
                })?;
            if !ids.insert(id.clone()) {
                return Err(CompositionError::DuplicateId(id));
            }
            let mut sink = ContributionSink::default();
            member
                .ext
                .contribute(&mut Contributions::__new(id.clone(), &mut sink))
                .map_err(|source| CompositionError::Contribution {
                    owner: id.clone(),
                    source,
                })?;
            if let Some(source) = sink.take_errors().into_iter().next() {
                return Err(CompositionError::Contribution { owner: id, source });
            }
            declared.push(DeclaredMember {
                metadata: meta,
                id,
                standing: member.standing,
                type_id: member.type_id,
                sink,
            });
        }
        check_member_dependencies(&declared)?;
        #[cfg(feature = "cli")]
        let commands = admit_commands(&mut declared)?;
        Ok(Declared {
            members: declared,
            #[cfg(feature = "cli")]
            commands,
            #[cfg(feature = "config")]
            secrets,
        })
    }

    pub fn build(self, profile: &Profile) -> Result<Admitted, CompositionError> {
        self.declare()?.admit(profile)
    }

    #[cfg(feature = "cli")]
    pub fn run(self) -> Result<std::process::ExitCode, RunError> {
        let declared = self.declare()?;
        let commands = declared.commands();
        let secrets = declared.secrets_sources();
        systemprompt_cli::run_composed(
            systemprompt_models::BuildIdentity::core(),
            commands,
            secrets,
            move |profile: Option<&Profile>| {
                match profile {
                    Some(profile) => declared.admit(profile),
                    None => declared.admit_unprofiled(),
                }
                .map_err(systemprompt_traits::BoxedSource::from)
            },
        )
        .map_err(RunError::Cli)
    }
}
