//! The capability slot identity issuance owns.
//!
//! [`SsoSlot`] collects the browser-redirect SSO providers, the sign-in audit
//! sinks and the role resolvers members contribute:
//!
//! ```ignore
//! let sso = c.slot::<SsoSlot>();
//! sso.provider(|r: &Resolved<'_>| my_provider(&SsoHookContext::from_resolved(r)?))
//!     .event_sink(|r: &Resolved<'_>| Ok(Arc::new(MySink::new(r.get()?)) as Arc<dyn AuthEventSink>))
//!     .role_resolver(|r: &Resolved<'_>| Ok(Arc::new(MyRoles::new(r.get()?)) as Arc<dyn RoleResolver>));
//! ```
//!
//! The slot is [`SlotOrder::CoreFirst`]. Two providers with one id are
//! refused with [`SlotError::DuplicateId`]. Every sink receives every
//! sign-in event. The role resolver a sign-in consults is the first entry in
//! core-first order, so a core member's resolver is always the one used; with
//! none contributed the directory's roles stand. The SSO driver reads the
//! built value with `r.get::<Arc<SsoProviders>>()`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{
    Factory, MemberResolved, Resolved, Slot, SlotEntry, SlotError, SlotOrder, build_factories,
    build_present_factories,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::sso::{AuthEventSink, SsoProvider};

use crate::sso::{DirectoryOnlyRoleResolver, RoleResolver};

const SSO_PROVIDER_SLOT: &str = "sso_provider";
const SSO_SINK_SLOT: &str = "auth_event_sink";
const SSO_ROLES_SLOT: &str = "role_resolver";

type ProviderFactory = Box<dyn Factory<Option<Arc<dyn SsoProvider>>>>;
type SinkFactory = Box<dyn Factory<Arc<dyn AuthEventSink>>>;
type ResolverFactory = Box<dyn Factory<Arc<dyn RoleResolver>>>;

/// SSO providers, sign-in audit sinks and role resolvers.
#[derive(Debug, Clone, Copy)]
pub struct SsoSlot;

#[derive(Default)]
pub struct SsoBuilder {
    providers: Vec<ProviderFactory>,
    sinks: Vec<SinkFactory>,
    resolvers: Vec<ResolverFactory>,
}

impl std::fmt::Debug for SsoBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoBuilder")
            .field("providers", &self.providers.len())
            .field("sinks", &self.sinks.len())
            .field("resolvers", &self.resolvers.len())
            .finish()
    }
}

impl SsoBuilder {
    pub fn provider<F: Factory<Option<Arc<dyn SsoProvider>>>>(&mut self, f: F) -> &mut Self {
        self.providers.push(Box::new(f));
        self
    }

    pub fn event_sink<F: Factory<Arc<dyn AuthEventSink>>>(&mut self, f: F) -> &mut Self {
        self.sinks.push(Box::new(f));
        self
    }

    pub fn role_resolver<F: Factory<Arc<dyn RoleResolver>>>(&mut self, f: F) -> &mut Self {
        self.resolvers.push(Box::new(f));
        self
    }
}

/// The built SSO contributions the driver runs.
#[derive(Clone)]
pub struct SsoProviders {
    providers: Vec<Arc<dyn SsoProvider>>,
    sinks: Vec<Arc<dyn AuthEventSink>>,
    roles: Arc<dyn RoleResolver>,
}

impl Default for SsoProviders {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            sinks: Vec::new(),
            roles: Arc::new(DirectoryOnlyRoleResolver),
        }
    }
}

impl std::fmt::Debug for SsoProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoProviders")
            .field(
                "providers",
                &self.providers.iter().map(|p| p.id()).collect::<Vec<_>>(),
            )
            .field("sinks", &self.sinks.len())
            .field("roles", &self.roles)
            .finish()
    }
}

impl SsoProviders {
    #[must_use]
    pub fn providers(&self) -> &[Arc<dyn SsoProvider>] {
        &self.providers
    }

    #[must_use]
    pub fn sinks(&self) -> &[Arc<dyn AuthEventSink>] {
        &self.sinks
    }

    #[must_use]
    pub fn roles(&self) -> Arc<dyn RoleResolver> {
        Arc::clone(&self.roles)
    }
}

impl Slot for SsoSlot {
    type Builder = SsoBuilder;
    type Built = SsoProviders;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut built = SsoProviders::default();
        let mut resolver: Option<(ExtensionId, Arc<dyn RoleResolver>)> = None;
        for entry in entries {
            let scope = entry.scope();
            let owner = entry.owner;
            let scoped = MemberResolved::within(r, &scope);
            for provider in
                build_present_factories(SSO_PROVIDER_SLOT, &scope, &entry.builder.providers, r)?
            {
                if built.providers.iter().any(|p| p.id() == provider.id()) {
                    return Err(SlotError::DuplicateId {
                        slot: SSO_PROVIDER_SLOT,
                        id: provider.id().to_owned(),
                    });
                }
                built.providers.push(provider);
            }
            built.sinks.extend(build_factories(
                SSO_SINK_SLOT,
                &scope,
                &entry.builder.sinks,
                r,
            )?);
            for factory in entry.builder.resolvers {
                if let Some((used, _)) = &resolver {
                    tracing::warn!(
                        used = %used,
                        ignored = %owner,
                        "SSO role resolver not consulted: an earlier member's resolver is used"
                    );
                    continue;
                }
                let built_resolver = factory
                    .build(&scoped)
                    .map_err(|e| e.into_slot_error(SSO_ROLES_SLOT, &owner))?;
                resolver = Some((owner.clone(), built_resolver));
            }
        }
        if let Some((_, roles)) = resolver {
            built.roles = roles;
        }
        Ok(built)
    }
}
