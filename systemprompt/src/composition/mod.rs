//! The composition: which members a binary is built from, the standing each
//! holds, and the admission of their contributions against a profile.
//!
//! [`reference()`] is the reference composition: every core member in
//! dependency order, each with the `CoreTerms` the conferral table in the
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
//! only then admits routes against the conferrals and checks providers. A
//! member that `depends_on` a member removed with [`Composition::without`],
//! disabled in the profile, or itself withheld is withheld with it (its
//! contributions dropped exactly as a disabled member's) and listed in
//! [`Admitted::withheld`]; withholding a required member refuses the
//! composition (`RequiredDisabled`). No provide fn, factory, slot build or
//! asset closure runs here; the runtime runs them over [`Admitted`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::collections::HashSet;

use systemprompt_extension::contribute::{ContributionSink, Contributions};
use systemprompt_extension::reserved::PrivilegedShape;
use systemprompt_extension::{AbsentDependency, Admitted, Extension};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Profile;

#[cfg(feature = "cli")]
use admit::admit_commands;
#[cfg(feature = "config")]
use admit::check_secrets;
use admit::{apply_removals, check_job_names, check_member_dependencies, withhold_dependants};
pub use error::{CompositionError, RunError};

mod admit;
mod declared;
mod error;
mod http_admission;
mod providers;
mod reference;
mod routes;
mod standing;

pub use declared::Declared;
use declared::DeclaredMember;
pub(super) use standing::{CoreTerms, Member, Standing};

pub use reference::{CORE_MEMBER_IDS, reference};

pub(super) type Conferral = PrivilegedShape;

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
        let (members, removed) = apply_removals(self.members, &self.removed)?;
        let mut declared = Vec::with_capacity(members.len());
        let mut ids = HashSet::new();
        for member in members {
            let meta = member.ext.metadata();
            let id =
                ExtensionId::try_new(meta.id).map_err(|source| CompositionError::InvalidId {
                    id: meta.id,
                    source,
                })?;
            match (member.standing, reference::is_core_member_id(meta.id)) {
                (Standing::Install, true) => return Err(CompositionError::ReservedId(id)),
                (Standing::Core(_), false) => return Err(CompositionError::UnlistedCoreId(id)),
                _ => {},
            }
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
        let (withheld, _removed_with_dependency) =
            withhold_dependants(&mut declared, removed, AbsentDependency::Removed)?;
        check_member_dependencies(&declared)?;
        check_job_names(&declared)?;
        #[cfg(feature = "cli")]
        let commands = admit_commands(&mut declared)?;
        Ok(Declared {
            members: declared,
            withheld,
            #[cfg(feature = "cli")]
            commands,
            #[cfg(feature = "config")]
            secrets,
        })
    }

    pub fn build(self, profile: &Profile) -> Result<Admitted, CompositionError> {
        self.declare()?.admit(&profile.extensions)
    }

    #[cfg(feature = "cli")]
    pub fn run(self) -> Result<std::process::ExitCode, RunError> {
        let declared = self.declare()?;
        let commands = declared.commands();
        let secrets = declared.secrets_sources();
        #[cfg(feature = "cloud")]
        let cloud: Option<systemprompt_extension::cli::DynCloudAccess> = Some(std::sync::Arc::new(
            systemprompt_cloud_entry::cli::access::CloudCliAccess,
        ));
        #[cfg(not(feature = "cloud"))]
        let cloud: Option<systemprompt_extension::cli::DynCloudAccess> = None;
        systemprompt_cli::run_composed(
            systemprompt_models::BuildIdentity::core(),
            &commands,
            secrets,
            cloud,
            move |extensions: Option<&systemprompt_manifest::ExtensionsConfig>| {
                match extensions {
                    Some(extensions) => declared.admit(extensions),
                    None => declared.admit_unprofiled(),
                }
                .map_err(systemprompt_traits::BoxedSource::from)
            },
        )
        .map_err(RunError::Cli)
    }
}
