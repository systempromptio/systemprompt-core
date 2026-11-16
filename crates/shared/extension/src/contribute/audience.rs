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
//! RoleAudience>`) is appended last, so a contributed resolver is consulted
//! first. The value is inserted as `Arc<AudienceResolvers>`; the
//! notifications and brain slots both read that one set, so one registration
//! serves both. A subject no resolver in the set handles is withheld by its
//! reader. A factory that fails refuses the build: a resolver is never
//! silently left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_traits::CoreSeam;
use systemprompt_traits::core_seam::RoleAudience;

use super::{Contributions, Factory, Resolved, SlotError, build_factories};
use crate::admitted::BuiltState;
use crate::notifications::NotificationAudienceResolver;

const POINT: &str = "audiences";

/// Every admitted audience resolver, contributed ones first in composition
/// order and the core user-role resolver last.
#[derive(Clone, Default)]
pub struct AudienceResolvers {
    resolvers: Vec<Arc<dyn NotificationAudienceResolver>>,
}

impl AudienceResolvers {
    pub fn from_admitted(built: &BuiltState, r: &Resolved<'_>) -> Result<Self, SlotError> {
        let mut resolvers = Vec::new();
        for member in built.members() {
            resolvers.extend(build_factories(
                POINT,
                &member.owner,
                member.sink.audience_resolvers(),
                r,
            )?);
        }
        if let Some(roles) = r.optional::<CoreSeam<dyn NotificationAudienceResolver, RoleAudience>>()
        {
            resolvers.push(roles.seam());
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

impl Contributions<'_> {
    pub fn audience_resolver<F>(&mut self, f: F) -> &mut Self
    where
        F: Factory<Arc<dyn NotificationAudienceResolver>>,
    {
        self.sink.audience_resolvers.push(Box::new(f));
        self
    }
}

impl fmt::Debug for AudienceResolvers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.resolvers.iter().map(|resolver| resolver.name()))
            .finish()
    }
}
