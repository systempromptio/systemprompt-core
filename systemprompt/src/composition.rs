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
use std::collections::{HashMap, HashSet};

use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts, DeclaredCommandParts};
use systemprompt_extension::contribute::{ContributionSink, Contributions};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::{
    Admitted, ContributionError, DeclaredCommands, Extension, ExtensionConfigError, SlotError,
    StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_manifest::Profile;
use systemprompt_traits::authz::SubjectDimension;

#[cfg(feature = "api")]
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
#[cfg(feature = "api")]
use systemprompt_extension::router::{Allowance, BasePath, BaseRef, RouteSet};
#[cfg(feature = "cli")]
use systemprompt_extension::admitted::AdmittedCommand;
#[cfg(feature = "cli")]
use systemprompt_extension::cli::KernelGrant;

/// A conferral is exactly one privileged-shape entry; the reference
/// composition grants a core member every entry its [`CoreId`] owns.
type Conferral = PrivilegedShape;

/// The terms a core member holds.
#[derive(Debug, Clone, Copy)]
struct CoreTerms {
    required: bool,
    core: Option<CoreId>,
    substitutable: &'static [fn() -> TypeId],
}

impl CoreTerms {
    const OPTIONAL: Self = Self {
        required: false,
        core: None,
        substitutable: &[],
    };
    const REQUIRED: Self = Self {
        required: true,
        core: None,
        substitutable: &[],
    };

    const fn conferred(core: CoreId) -> Self {
        Self {
            required: false,
            core: Some(core),
            substitutable: &[],
        }
    }

    fn conferrals(self) -> impl Iterator<Item = &'static Conferral> {
        PRIVILEGED_SHAPES
            .iter()
            .filter(move |shape| Some(shape.owner) == self.core)
    }

    #[cfg(feature = "cli")]
    fn cli(self) -> impl Iterator<Item = &'static systemprompt_extension::reserved::ReservedCliPath> {
        systemprompt_extension::reserved::RESERVED_CLI_PATHS
            .iter()
            .filter(move |reserved| Some(reserved.owner) == self.core)
    }

    fn substitutes(self, ty: TypeId) -> bool {
        self.substitutable.iter().any(|of| of() == ty)
    }
}

#[derive(Debug, Clone, Copy)]
enum Standing {
    Core(CoreTerms),
    Install,
}

impl Standing {
    const fn tag(self) -> StandingTag {
        match self {
            Self::Core(_) => StandingTag::Core,
            Self::Install => StandingTag::Install,
        }
    }

    const fn terms(self) -> Option<CoreTerms> {
        match self {
            Self::Core(terms) => Some(terms),
            Self::Install => None,
        }
    }

    const fn required(self) -> bool {
        matches!(self, Self::Core(CoreTerms { required: true, .. }))
    }
}

struct Member {
    ext: Box<dyn Extension>,
    standing: Standing,
    type_id: TypeId,
}

impl Member {
    fn core<E: Extension + Default>(terms: CoreTerms) -> Self {
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

const RESERVED_SECRETS_KINDS: &[&str] = &["file", "env", "vault", "external"];

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

/// The reference composition: every core member of this build, in
/// dependency order.
#[must_use]
pub fn reference() -> Composition {
    let mut members: Vec<Member> = Vec::new();
    #[cfg(feature = "logging")]
    members.push(Member::core::<systemprompt_logging::LoggingExtension>(CoreTerms::REQUIRED));
    #[cfg(feature = "database")]
    members.push(Member::core::<systemprompt_database::DatabaseExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "events")]
    members.push(Member::core::<systemprompt_events::EventsExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "security")]
    members.push(Member::core::<systemprompt_governance::authz::AuthzExtension>(
        CoreTerms::REQUIRED,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_users::UsersExtension>(CoreTerms::REQUIRED));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_oauth::OauthExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "api")]
    members.push(Member::core::<systemprompt_oauth_issuance::IdentityExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_files::FilesExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_ai::AiExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_mcp::McpExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_agent::AgentExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "content")]
    members.push(Member::core::<systemprompt_content::ContentExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "analytics")]
    members.push(Member::core::<systemprompt_analytics::AnalyticsExtension>(
        CoreTerms::REQUIRED,
    ));
    #[cfg(feature = "brain")]
    members.push(Member::core::<systemprompt_brain::BrainExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "notifications")]
    members.push(Member::core::<systemprompt_notifications::NotificationsExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_marketplace::ManagedResourcesExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "api")]
    members.push(Member::core::<systemprompt_gateway::extension::GatewayExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "agent-runs")]
    members.push(Member::core::<systemprompt_agent_runs::AgentRunsExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_generator::GeneratorExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_scheduler::SchedulerExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "content")]
    members.push(Member::core::<systemprompt_content_entry::extension::ContentHttpExtension>(
        CoreTerms::conferred(CoreId::ContentHttp),
    ));
    #[cfg(feature = "analytics")]
    members.push(Member::core::<systemprompt_analytics_entry::extension::AnalyticsHttpExtension>(
        CoreTerms::conferred(CoreId::AnalyticsHttp),
    ));
    #[cfg(feature = "notifications")]
    members.push(Member::core::<
        systemprompt_notifications_entry::extension::NotificationsHttpExtension,
    >(CoreTerms::conferred(CoreId::NotificationsHttp)));
    #[cfg(all(feature = "agent-runs", feature = "api"))]
    members.push(Member::core::<systemprompt_agent_runs_entry::http::AgentRunsApiExtension>(
        CoreTerms::conferred(CoreId::AgentRunsApi),
    ));
    #[cfg(all(feature = "cloud", feature = "cli"))]
    members.push(Member::core::<systemprompt_cloud_entry::CloudCliExtension>(
        CoreTerms::conferred(CoreId::Cloud),
    ));
    #[cfg(feature = "slack")]
    members.push(Member::core::<systemprompt_slack_entry::SlackExtension>(CoreTerms::conferred(
        CoreId::Slack,
    )));
    #[cfg(feature = "teams")]
    members.push(Member::core::<systemprompt_teams_entry::TeamsExtension>(CoreTerms::conferred(
        CoreId::Teams,
    )));
    Composition {
        members,
        removed: Vec::new(),
        #[cfg(feature = "config")]
        secrets: Vec::new(),
    }
}

impl Composition {
    /// Adds an install member. An install never holds a conferral.
    #[must_use]
    pub fn with<E: Extension>(mut self, e: E) -> Self {
        self.members.push(Member {
            ext: Box::new(e),
            standing: Standing::Install,
            type_id: TypeId::of::<E>(),
        });
        self
    }

    /// Removes a member; refused at declaration if it is required. Removing a
    /// member frees none of its reservations.
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
            let id = ExtensionId::try_new(meta.id)
                .map_err(|source| CompositionError::InvalidId { id: meta.id, source })?;
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
        systemprompt_cli::run_composed(
            systemprompt_models::BuildIdentity::core(),
            commands,
            move |profile| {
                declared
                    .admit(profile)
                    .map_err(systemprompt_traits::BoxedSource::from)
            },
        )
        .map_err(RunError::Cli)
    }
}

struct DeclaredMember {
    id: ExtensionId,
    standing: Standing,
    type_id: TypeId,
    sink: ContributionSink,
}

/// Every member's declarations, before the profile is applied.
pub struct Declared {
    members: Vec<DeclaredMember>,
    #[cfg(feature = "cli")]
    commands: Vec<AdmittedCommand>,
    #[cfg(feature = "config")]
    secrets: Vec<(SecretsKind, systemprompt_config::SecretsSourceFactory)>,
}

impl std::fmt::Debug for Declared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Declared")
            .field(
                "members",
                &self.members.iter().map(|m| &m.id).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Declared {
    /// The install secrets sources, for the secrets bootstrap.
    #[cfg(feature = "config")]
    #[must_use]
    pub fn secrets_sources(&self) -> &[(SecretsKind, systemprompt_config::SecretsSourceFactory)] {
        &self.secrets
    }

    #[must_use]
    pub fn commands(&self) -> DeclaredCommands {
        DeclaredCommands::__from_facade(DeclaredCommandParts {
            #[cfg(feature = "cli")]
            commands: self.commands.clone(),
        })
    }

    pub fn admit(self, profile: &Profile) -> Result<Admitted, CompositionError> {
        let disabled = disabled_members(&self.members, profile)?;
        let (enabled, dropped): (Vec<DeclaredMember>, Vec<DeclaredMember>) = self
            .members
            .into_iter()
            .partition(|member| !disabled.contains(&member.id));
        check_providers(&enabled, &dropped)?;
        drop(dropped);
        let mut members = Vec::with_capacity(enabled.len());
        #[cfg(feature = "api")]
        let mut routes = Vec::new();
        #[cfg(feature = "api")]
        let mut gateway_routes = Vec::new();
        for mut member in enabled {
            #[cfg(feature = "api")]
            {
                let (sets, gateways) = member.sink.take_routes();
                for set in sets {
                    routes.push(admit_route_set(&member, set)?);
                }
                for decl in gateways {
                    gateway_routes.push(admit_gateway_route(&member, decl)?);
                }
            }
            #[cfg(feature = "cli")]
            drop(member.sink.take_commands());
            members.push(AdmittedMember {
                owner: member.id,
                standing: member.standing.tag(),
                sink: member.sink,
            });
        }
        #[cfg(feature = "api")]
        {
            check_route_conflicts(&routes)?;
            check_gateway_overlaps(&gateway_routes)?;
        }
        Ok(Admitted::__from_facade(AdmittedParts {
            members,
            #[cfg(feature = "api")]
            routes,
            #[cfg(feature = "api")]
            gateway_routes,
        }))
    }
}

fn apply_removals(members: Vec<Member>, removed: &[TypeId]) -> Result<Vec<Member>, CompositionError> {
    let mut kept = Vec::with_capacity(members.len());
    for member in members {
        if removed.contains(&member.type_id) {
            if member.standing.required() {
                let meta = member.ext.metadata();
                let id = ExtensionId::try_new(meta.id)
                    .map_err(|source| CompositionError::InvalidId { id: meta.id, source })?;
                return Err(CompositionError::RequiredDisabled(id));
            }
            continue;
        }
        kept.push(member);
    }
    Ok(kept)
}

#[cfg(feature = "config")]
fn check_secrets(
    secrets: Vec<SecretsSourceDecl>,
) -> Result<Vec<(SecretsKind, systemprompt_config::SecretsSourceFactory)>, CompositionError> {
    let mut seen: Vec<&'static str> = Vec::new();
    let mut out = Vec::with_capacity(secrets.len());
    for decl in secrets {
        let name = match decl.kind {
            SecretsKind::File => "file",
            SecretsKind::Env => "env",
            SecretsKind::Vault => "vault",
            SecretsKind::Install(name) => name,
        };
        if !matches!(decl.kind, SecretsKind::Install(_)) || RESERVED_SECRETS_KINDS.contains(&name)
        {
            return Err(CompositionError::ReservedSecretsKind(name));
        }
        if seen.contains(&name) {
            return Err(CompositionError::DuplicateSecretsKind(name));
        }
        seen.push(name);
        out.push((decl.kind, decl.factory));
    }
    Ok(out)
}

fn check_member_dependencies(members: &[DeclaredMember]) -> Result<(), CompositionError> {
    for member in members {
        for dependency in member.sink.depends_on() {
            let Some(target) = members
                .iter()
                .find(|other| other.type_id == dependency.type_id())
            else {
                return Err(CompositionError::Missing {
                    owner: member.id.clone(),
                    ty: dependency.type_name(),
                });
            };
            if target.standing.tag() != member.standing.tag() {
                return Err(CompositionError::CrossCapabilityDependency {
                    owner: member.id.clone(),
                    target: target.id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn disabled_members(
    members: &[DeclaredMember],
    profile: &Profile,
) -> Result<Vec<ExtensionId>, CompositionError> {
    let disabled = &profile.extensions.disabled;
    for id in disabled {
        let Some(member) = members.iter().find(|member| &member.id == id) else {
            return Err(CompositionError::UnknownExtension(id.as_str().to_owned()));
        };
        if member.standing.required() {
            return Err(CompositionError::RequiredDisabled(member.id.clone()));
        }
    }
    Ok(disabled.clone())
}

fn check_providers(
    enabled: &[DeclaredMember],
    dropped: &[DeclaredMember],
) -> Result<(), CompositionError> {
    let mut providers: HashMap<TypeId, (&ExtensionId, Standing)> = HashMap::new();
    for member in enabled {
        for entry in member.sink.provides() {
            if let Some((first, _)) = providers.insert(entry.type_id(), (&member.id, member.standing))
            {
                return Err(CompositionError::DuplicateProvider {
                    ty: entry.type_name(),
                    first: first.clone(),
                    second: member.id.clone(),
                });
            }
        }
    }
    for member in enabled {
        for requirement in member.sink.requires() {
            let ty = requirement.type_id();
            match providers.get(&ty) {
                Some((provider, Standing::Install)) => {
                    if let Some(terms) = member.standing.terms()
                        && !terms.substitutes(ty)
                    {
                        return Err(CompositionError::InstallSubstitution {
                            owner: member.id.clone(),
                            ty: requirement.type_name(),
                            provider: (*provider).clone(),
                        });
                    }
                },
                Some((_, Standing::Core(_))) => {},
                None => {
                    if let Some(provider) = dropped
                        .iter()
                        .find(|other| other.sink.provides().iter().any(|p| p.type_id() == ty))
                    {
                        return Err(CompositionError::DisabledDependency {
                            owner: member.id.clone(),
                            ty: requirement.type_name(),
                            provider: provider.id.clone(),
                        });
                    }
                },
            }
        }
    }
    Ok(())
}

#[cfg(feature = "cli")]
const COMMAND_PARENTS: &[&[&str]] = &[
    &[],
    &["core"],
    &["infra"],
    &["admin"],
    &["web"],
    &["plugins"],
    &["core", "content"],
];

#[cfg(feature = "cli")]
fn admit_commands(members: &mut [DeclaredMember]) -> Result<Vec<AdmittedCommand>, CompositionError> {
    let mut admitted: Vec<AdmittedCommand> = Vec::new();
    for member in members.iter_mut() {
        for decl in member.sink.take_commands() {
            let path = decl.path();
            let reserved = systemprompt_extension::reserved::RESERVED_CLI_PATHS
                .iter()
                .find(|reserved| reserved.path == path.as_slice());
            let conferred = member
                .standing
                .terms()
                .is_some_and(|terms| terms.cli().any(|cli| cli.path == path.as_slice()));
            if let Some(reserved) = reserved
                && !conferred
            {
                return Err(CompositionError::ReservedCliPath {
                    owner: member.id.clone(),
                    path: reserved.path,
                });
            }
            for grant in decl.grants() {
                if !reserved.is_some_and(|reserved| conferred && reserved.grants.contains(grant)) {
                    return Err(CompositionError::GrantNotConferred {
                        owner: member.id.clone(),
                        grant: *grant,
                    });
                }
            }
            if admitted.iter().any(|other| other.decl.path() == path) {
                return Err(CompositionError::CommandConflict { path });
            }
            admitted.push(AdmittedCommand {
                owner: member.id.clone(),
                decl,
            });
        }
    }
    for command in &admitted {
        let parent = command.decl.parent();
        let provided = COMMAND_PARENTS.contains(&parent)
            || admitted
                .iter()
                .any(|other| other.decl.path().as_slice() == parent);
        if !provided {
            return Err(CompositionError::ParentMissing {
                leaf: command.decl.name(),
                parent,
            });
        }
    }
    Ok(admitted)
}

#[cfg(feature = "api")]
fn admit_route_set(member: &DeclaredMember, set: RouteSet) -> Result<AdmittedRouteSet, CompositionError> {
    let base = set.base();
    let conferrals: Vec<&'static Conferral> = member
        .standing
        .terms()
        .map(|terms| {
            terms
                .conferrals()
                .filter(|c| c.base == BaseRef::Path(base))
                .collect()
        })
        .unwrap_or_default();
    if conferrals.is_empty() {
        admit_unconferred(member, &set)?;
        return Ok(AdmittedRouteSet {
            owner: member.id.clone(),
            standing: member.standing.tag(),
            set,
            handles: Vec::new(),
        });
    }
    let counting: Vec<&Conferral> = conferrals
        .iter()
        .copied()
        .filter(|c| !matches!(c.allowance, Allowance::KernelHandle(_)))
        .collect();
    for def in set.defs() {
        let pinned = counting.iter().any(|c| {
            c.routes
                .is_some_and(|routes| routes.iter().any(|(m, p)| *m == def.method && *p == def.path.as_str()))
        });
        let matches = counting
            .iter()
            .filter(|c| {
                c.group == set.group()
                    && c.shape == def.spec.shape()
                    && match c.routes {
                        Some(routes) => routes
                            .iter()
                            .any(|(m, p)| *m == def.method && *p == def.path.as_str()),
                        None => !pinned,
                    }
            })
            .count();
        if matches != 1 {
            return Err(CompositionError::ConferralShapeMismatch {
                owner: member.id.clone(),
                base,
            });
        }
    }
    let mut handles = Vec::new();
    for conferral in &conferrals {
        if let Allowance::KernelHandle(handle) = conferral.allowance {
            let backed = counting
                .iter()
                .any(|c| c.group == conferral.group && c.shape == conferral.shape);
            if !backed {
                return Err(CompositionError::ConferralShapeMismatch {
                    owner: member.id.clone(),
                    base,
                });
            }
            handles.push(handle);
        }
    }
    Ok(AdmittedRouteSet {
        owner: member.id.clone(),
        standing: member.standing.tag(),
        set,
        handles,
    })
}

#[cfg(feature = "api")]
fn admit_unconferred(member: &DeclaredMember, set: &RouteSet) -> Result<(), CompositionError> {
    use systemprompt_extension::router::{ApiBase, BodyLimit, WebBase};
    use systemprompt_extension::{ContextKind, HandlerVerifier};

    let base = set.base();
    let refuse = |allowance: Allowance| CompositionError::NotConferred {
        owner: member.id.clone(),
        base,
        allowance,
    };
    if let Some(prefix) = base.reserved_prefix() {
        return Err(CompositionError::ReservedPrefix {
            owner: member.id.clone(),
            prefix,
        });
    }
    match base {
        BasePath::Web(WebBase::TrackEngagement) => return Err(refuse(Allowance::WebPath)),
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(WebBase::Root) => {},
        BasePath::Api(_) => return Err(refuse(Allowance::Plain)),
    }
    for def in set.defs() {
        if let Some(verifier) = def.spec.policy.handler_verifier() {
            return Err(refuse(Allowance::HandlerVerified(verifier)));
        }
        if def.spec.context == ContextKind::HandlerResolved {
            return Err(refuse(Allowance::HandlerVerified(HandlerVerifier::GatewayCredential)));
        }
        if def.spec.limits.body == BodyLimit::Agent {
            return Err(refuse(Allowance::BodyLimit));
        }
    }
    Ok(())
}

#[cfg(feature = "api")]
fn admit_gateway_route(
    member: &DeclaredMember,
    decl: systemprompt_extension::router::GatewayRouteDecl,
) -> Result<AdmittedGatewayRoute, CompositionError> {
    let Some(conferral) = member.standing.terms().and_then(|terms| {
        terms
            .conferrals()
            .find(|c| c.base == BaseRef::Gateway(decl.path) && c.allowance == Allowance::GatewayRoute)
    }) else {
        return Err(CompositionError::UnlistedGatewayRoute {
            owner: member.id.clone(),
        });
    };
    let relative = systemprompt_models::modules::ApiPaths::gateway_relative(decl.path.path());
    if let Some(builtin) = systemprompt_extension::reserved::BUILTIN_GATEWAY_PREFIXES
        .iter()
        .copied()
        .find(|prefix| shadows(relative, prefix))
    {
        return Err(CompositionError::GatewayRouteShadowsBuiltin {
            owner: member.id.clone(),
            builtin,
        });
    }
    let shape_ok = decl.limits == conferral.shape.limits
        && decl.router.group() == conferral.group
        && decl
            .router
            .defs()
            .iter()
            .all(|def| def.spec.shape() == conferral.shape);
    if !shape_ok {
        return Err(CompositionError::ConferralShapeMismatch {
            owner: member.id.clone(),
            base: decl.router.base(),
        });
    }
    Ok(AdmittedGatewayRoute {
        owner: member.id.clone(),
        decl,
    })
}

#[cfg(feature = "api")]
fn shadows(relative: &str, prefix: &str) -> bool {
    if prefix == "/" {
        return relative == "/";
    }
    relative == prefix
        || relative
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(feature = "api")]
fn full_path(base: BasePath, path: &str) -> String {
    let base = base.path();
    match (base, path) {
        ("/", path) => path.to_owned(),
        (base, "/") => base.to_owned(),
        (base, path) => format!("{base}{path}"),
    }
}

#[cfg(feature = "api")]
fn check_route_conflicts(routes: &[AdmittedRouteSet]) -> Result<(), CompositionError> {
    let mut seen: HashSet<(http::Method, String)> = HashSet::new();
    for admitted in routes {
        for def in admitted.set.defs() {
            let key = (def.method.clone(), full_path(admitted.set.base(), def.path.as_str()));
            if !seen.insert(key.clone()) {
                return Err(CompositionError::RouteConflict {
                    method: key.0,
                    path: key.1,
                });
            }
        }
    }
    Ok(())
}

#[cfg(feature = "api")]
fn check_gateway_overlaps(routes: &[AdmittedGatewayRoute]) -> Result<(), CompositionError> {
    for (index, first) in routes.iter().enumerate() {
        if let Some(second) = routes[index + 1..]
            .iter()
            .find(|other| other.decl.path == first.decl.path)
        {
            return Err(CompositionError::GatewayRouteOverlap {
                first: first.owner.clone(),
                second: second.owner.clone(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    #[error("unknown extension in profile extensions.disabled: {0}")]
    UnknownExtension(String),
    #[error("extension id is invalid: {id}")]
    InvalidId {
        id: &'static str,
        #[source]
        source: IdValidationError,
    },
    #[error("duplicate extension id {0}")]
    DuplicateId(ExtensionId),
    #[error("required extension {0} cannot be disabled or removed")]
    RequiredDisabled(ExtensionId),
    #[error("{owner} requires {ty}, which no enabled member provides")]
    Missing { owner: ExtensionId, ty: &'static str },
    #[error("{owner} requires {ty}; its core provider {provider} is disabled")]
    DisabledDependency {
        owner: ExtensionId,
        ty: &'static str,
        provider: ExtensionId,
    },
    #[error("{owner} (core) requires {ty}, provided only by install member {provider}")]
    InstallSubstitution {
        owner: ExtensionId,
        ty: &'static str,
        provider: ExtensionId,
    },
    #[error("{ty} provided twice: {first} and {second}")]
    DuplicateProvider {
        ty: &'static str,
        first: ExtensionId,
        second: ExtensionId,
    },
    #[error("{owner} depends on {target} across capabilities")]
    CrossCapabilityDependency { owner: ExtensionId, target: ExtensionId },
    #[cfg(feature = "api")]
    #[error("{owner}'s routes at {base:?} do not match the conferred shape")]
    ConferralShapeMismatch { owner: ExtensionId, base: BasePath },
    #[cfg(feature = "api")]
    #[error("{owner} uses {allowance:?} at {base:?} without a conferral")]
    NotConferred {
        owner: ExtensionId,
        base: BasePath,
        allowance: Allowance,
    },
    #[error("{owner} mounts under reserved prefix {prefix}")]
    ReservedPrefix {
        owner: ExtensionId,
        prefix: &'static str,
    },
    #[error("{owner} gateway route without conferral")]
    UnlistedGatewayRoute { owner: ExtensionId },
    #[error("{owner} gateway route shadows built-in {builtin}")]
    GatewayRouteShadowsBuiltin {
        owner: ExtensionId,
        builtin: &'static str,
    },
    #[error("gateway routes overlap: {first} and {second}")]
    GatewayRouteOverlap {
        first: ExtensionId,
        second: ExtensionId,
    },
    #[cfg(feature = "api")]
    #[error("{owner} requests handle {handle:?} without conferral at {base:?}")]
    HandleNotConferred {
        owner: ExtensionId,
        handle: systemprompt_extension::HandleId,
        base: BasePath,
    },
    #[cfg(feature = "cli")]
    #[error("{owner} requests {grant:?} without conferral")]
    GrantNotConferred { owner: ExtensionId, grant: KernelGrant },
    #[error("reserved CLI path {path:?} taken by {owner}")]
    ReservedCliPath {
        owner: ExtensionId,
        path: &'static [&'static str],
    },
    #[error("{leaf} attaches under {parent:?}, which nothing provides")]
    ParentMissing {
        leaf: &'static str,
        parent: &'static [&'static str],
    },
    #[error("command path {path:?} declared twice")]
    CommandConflict { path: Vec<&'static str> },
    #[error("route {method} {path} declared twice")]
    RouteConflict { method: http::Method, path: String },
    #[error("subject dimension {dimension:?} declared by {second}, already owned by {first}")]
    DimensionConflict {
        dimension: SubjectDimension,
        first: ExtensionId,
        second: ExtensionId,
    },
    #[error("{owner} declares reserved subject dimension {dimension:?}")]
    ReservedDimension {
        owner: ExtensionId,
        dimension: SubjectDimension,
    },
    #[error("secrets source kind {0} registered twice")]
    DuplicateSecretsKind(&'static str),
    #[error("secrets source kind {0} is reserved for core")]
    ReservedSecretsKind(&'static str),
    #[error("extension {owner} failed to contribute")]
    Contribution {
        owner: ExtensionId,
        #[source]
        source: ContributionError,
    },
    #[error("slot build failed")]
    Slot(#[from] SlotError),
    #[error("config for {owner} invalid")]
    Config {
        owner: ExtensionId,
        #[source]
        source: ExtensionConfigError,
    },
}

#[cfg(feature = "cli")]
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("composition refused")]
    Composition(#[from] CompositionError),
    #[error("cli run failed")]
    Cli(#[source] systemprompt_cli::CliRunError),
}
