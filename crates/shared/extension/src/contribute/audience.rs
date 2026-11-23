//! The audience-resolver contribution point the notification pipeline and
//! brain recall both read.
//!
//! A member contributes a [`NotificationAudienceResolver`] with
//! `Contributions::audience_resolver`; the method stores a factory and runs
//! nothing. Admission drops a disabled member's factories before any runs.
//! The kernel then builds [`AudienceResolvers`] once, after every provide
//! and before any slot: each admitted member's factories are called against
//! the resolved state, in composition order, and the user-role resolver the
//! `users` member provides (`CoreSeam<dyn NotificationAudienceResolver,
//! RoleAudience>`) is appended last. The value is
//! inserted as `Arc<AudienceResolvers>`; the notifications and brain slots
//! both read that one set, so one registration serves both. A subject no
//! resolver in the set handles is withheld by its reader. A factory that
//! fails refuses the build: a resolver is never silently left out.
//!
//! The subjects `User`, `Role` and `Everyone`, and a `Dimension` whose kind
//! is a reserved subject dimension ([`RESERVED_SUBJECT_DIMENSIONS`]), are
//! answered only by core: an install resolver that handles any of them is
//! refused when the set is built ([`SlotError::ReservedAudience`]), and
//! every install resolver is held behind a guard that never handles,
//! expands, answers or reports holding a reserved subject, so no reader can
//! ask it one. The guard also answers a custom `Dimension` only when its kind
//! is among the resolver's declared `dimensions`, so an install handles no
//! kind it has not declared. Two resolvers sharing a name are refused, so an
//! install cannot answer under a core resolver's name. A custom dimension a
//! resolver declares that another resolver also declares or handles is
//! refused ([`SlotError::SharedAudienceDimension`], naming both), so one
//! member never silently shadows another's dimension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_traits::CoreSeam;
use systemprompt_traits::authz::RESERVED_SUBJECT_DIMENSIONS;
use systemprompt_traits::core_seam::{CoreSeamKind, RoleAudience};

use super::{Contributions, Factory, Resolved, SlotError, StandingTag, build_factories};
use crate::admitted::BuiltState;
use crate::notifications::{Membership, NotificationAudienceError, NotificationAudienceResolver};

const POINT: &str = "audiences";

/// One resolver offered to the set, with its member and that member's
/// admitted standing.
#[derive(Clone)]
pub struct AudienceEntry {
    pub owner: ExtensionId,
    pub standing: StandingTag,
    pub resolver: Arc<dyn NotificationAudienceResolver>,
}

/// Every admitted audience resolver in composition order, the core
/// user-role resolver last; install resolvers are held behind the
/// reserved-subject guard.
#[derive(Clone, Default)]
pub struct AudienceResolvers {
    resolvers: Vec<Arc<dyn NotificationAudienceResolver>>,
}

impl AudienceResolvers {
    pub fn from_admitted(built: &BuiltState, r: &Resolved<'_>) -> Result<Self, SlotError> {
        let mut contributed = Vec::new();
        for member in built.members() {
            let resolvers = build_factories(
                POINT,
                &member.owner,
                member.standing,
                member.sink.audience_resolvers(),
                r,
            )?;
            contributed.extend(resolvers.into_iter().map(|resolver| AudienceEntry {
                owner: member.owner.clone(),
                standing: member.standing,
                resolver,
            }));
        }
        if let Some(roles) =
            r.optional::<CoreSeam<dyn NotificationAudienceResolver, RoleAudience>>()
        {
            contributed.push(AudienceEntry {
                owner: ExtensionId::new(RoleAudience::OWNER),
                standing: StandingTag::Core,
                resolver: roles.seam(),
            });
        }
        Self::new(contributed)
    }

    pub fn new(entries: Vec<AudienceEntry>) -> Result<Self, SlotError> {
        refuse_shared_dimensions(&entries)?;
        let mut resolvers: Vec<Arc<dyn NotificationAudienceResolver>> =
            Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.standing == StandingTag::Core {
                resolvers.push(entry.resolver);
                continue;
            }
            if let Some(subject) = reserved_probes().find(|subject| entry.resolver.handles(subject))
            {
                return Err(SlotError::ReservedAudience {
                    owner: entry.owner,
                    resolver: entry.resolver.name(),
                    subject: subject.to_string(),
                });
            }
            resolvers.push(Arc::new(InstallScoped(entry.resolver)));
        }
        let mut names: Vec<&'static str> = Vec::with_capacity(resolvers.len());
        for resolver in &resolvers {
            if names.contains(&resolver.name()) {
                return Err(SlotError::DuplicateId {
                    slot: POINT,
                    id: resolver.name().to_owned(),
                });
            }
            names.push(resolver.name());
        }
        Ok(Self { resolvers })
    }

    #[must_use]
    pub fn resolvers(&self) -> &[Arc<dyn NotificationAudienceResolver>] {
        &self.resolvers
    }

    #[must_use]
    pub fn to_vec(&self) -> Vec<Arc<dyn NotificationAudienceResolver>> {
        self.resolvers.clone()
    }
}

fn refuse_shared_dimensions(entries: &[AudienceEntry]) -> Result<(), SlotError> {
    for (index, claimant) in entries.iter().enumerate() {
        for dimension in claimant.resolver.dimensions() {
            let probe = AudienceSubject::Dimension {
                kind: (*dimension).to_owned(),
                value: PROBE.to_owned(),
            };
            let rival = entries.iter().enumerate().find(|(other, entry)| {
                *other != index
                    && (entry.resolver.dimensions().contains(dimension)
                        || entry.resolver.handles(&probe))
            });
            if let Some((other, entry)) = rival {
                let (first, second) = if other < index {
                    (entry, claimant)
                } else {
                    (claimant, entry)
                };
                return Err(SlotError::SharedAudienceDimension {
                    dimension: (*dimension).to_owned(),
                    first_owner: first.owner.clone(),
                    first: first.resolver.name(),
                    second_owner: second.owner.clone(),
                    second: second.resolver.name(),
                });
            }
        }
    }
    Ok(())
}

#[must_use]
pub fn is_reserved_audience(subject: &AudienceSubject) -> bool {
    match subject {
        AudienceSubject::User(_) | AudienceSubject::Role(_) | AudienceSubject::Everyone => true,
        AudienceSubject::Dimension { kind, .. } => RESERVED_SUBJECT_DIMENSIONS
            .iter()
            .any(|dimension| dimension.rule_type.as_str() == kind),
    }
}

const PROBE: &str = "reserved-subject-probe";

fn reserved_probes() -> impl Iterator<Item = AudienceSubject> {
    [
        AudienceSubject::User(UserId::new(PROBE)),
        AudienceSubject::Role(PROBE.to_owned()),
        AudienceSubject::Everyone,
    ]
    .into_iter()
    .chain(
        RESERVED_SUBJECT_DIMENSIONS
            .iter()
            .map(|dimension| AudienceSubject::Dimension {
                kind: dimension.rule_type.as_str().to_owned(),
                value: PROBE.to_owned(),
            }),
    )
}

struct InstallScoped(Arc<dyn NotificationAudienceResolver>);

impl InstallScoped {
    fn answers(&self, subject: &AudienceSubject) -> bool {
        if is_reserved_audience(subject) {
            return false;
        }
        match subject {
            AudienceSubject::Dimension { kind, .. } => self.0.dimensions().contains(&kind.as_str()),
            AudienceSubject::User(_) | AudienceSubject::Role(_) | AudienceSubject::Everyone => true,
        }
    }
}

#[async_trait::async_trait]
impl NotificationAudienceResolver for InstallScoped {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        self.answers(subject) && self.0.handles(subject)
    }

    fn dimensions(&self) -> &'static [&'static str] {
        self.0.dimensions()
    }

    async fn members(
        &self,
        subject: &AudienceSubject,
        limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        if !self.answers(subject) {
            return Ok(Vec::new());
        }
        self.0.members(subject, limit).await
    }

    async fn is_member(
        &self,
        subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        if !self.answers(subject) {
            return Ok(Membership::Unknown);
        }
        self.0.is_member(subject, user).await
    }

    async fn held(&self, user: &UserId) -> Option<Vec<AudienceSubject>> {
        self.0.held(user).await.map(|subjects| {
            subjects
                .into_iter()
                .filter(|subject| self.answers(subject))
                .collect()
        })
    }
}

impl Contributions<'_> {
    pub fn audience_resolver<F>(&mut self, f: F) -> &mut Self
    where
        F: Factory<Arc<dyn NotificationAudienceResolver>>,
    {
        self.sink.audience_resolvers.push(Box::new(f));
        self
    }
}

impl fmt::Debug for AudienceEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudienceEntry")
            .field("owner", &self.owner)
            .field("standing", &self.standing)
            .field("resolver", &self.resolver.name())
            .finish()
    }
}

impl fmt::Debug for AudienceResolvers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.resolvers.iter().map(|resolver| resolver.name()))
            .finish()
    }
}
