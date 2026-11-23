//! [`AgentHttpExtension`] (`agent-http`): the agent conversation route sets
//! and the agent, context and artifact commands.
//!
//! With `http` it contributes four route sets in the agent role group (see
//! [`crate::http`]): `/api/v1/core/{contexts,tasks,artifacts}` behind the
//! user gate with a user request context under the `contexts`, `tasks` and
//! `artifacts` rate-limit scopes, and `GET /api/v1/agents/registry` behind
//! the public gate with a public request context under `agent_registry`. Each
//! set's shape equals the reference composition's conferral for `agent-http`
//! at its base. It provides the [`ContextEventPublisher`] its handlers
//! publish through, built here from the kernel's event router so route state
//! never holds the router, and requires the open `Arc<AgentCardSite>` the
//! `agent` member provides.
//!
//! With `cli` it contributes `admin agents` (grants `services-tree` and
//! `caller-session`), `core contexts` (`caller-session`, `session-context`)
//! and `core artifacts` (`caller-session`). No command starts, stops or
//! signals an agent: `admin agents status` reads the open status view and
//! `admin agents delete` refuses a running agent.
//!
//! The `/api/v1/agents/{service}` proxy, the well-known agent cards, the
//! webhook broadcasts, the messaging dispatcher, agent supervision and the
//! `admin agent-host run` child host are the kernel's.
//!
//! | profile disables | routes | commands |
//! |---|---|---|
//! | `agent-http` | 404 | refused at dispatch |
//! | `agent` | boot refused (unmet `Arc<AgentCardSite>`) | boot refused |
//!
//! [`ContextEventPublisher`]: crate::http::ContextEventPublisher
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;
use systemprompt_extension::reserved::CoreId;

pub const AGENT_HTTP_EXTENSION: &str = CoreId::AgentHttp.id();

/// The member that contributes the agent route sets and commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentHttpExtension;

impl Extension for AgentHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: AGENT_HTTP_EXTENSION,
            name: "Agent HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(any(feature = "http", feature = "cli")),
        expect(
            unused_variables,
            reason = "the member contributes only with the http or cli feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(any(feature = "http", feature = "cli"))]
        c.requires::<std::sync::Arc<systemprompt_agent::services::registry::AgentCardSite>>();
        #[cfg(feature = "http")]
        routes::contribute(c);
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod routes {
    use std::sync::Arc;

    use systemprompt_events::EventRouter;
    use systemprompt_extension::prelude::*;

    use crate::http::{
        ContextEventPublisher, artifacts_routes, contexts_routes, registry_routes, tasks_routes,
    };

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.provide(
            |r: &Resolved<'_>| -> Result<Arc<ContextEventPublisher>, ProvideError> {
                Ok(Arc::new(ContextEventPublisher::new(
                    r.get::<Arc<EventRouter>>()?,
                )))
            },
        )
        .routes(contexts_routes())
        .routes(tasks_routes())
        .routes(artifacts_routes())
        .routes(registry_routes());
    }
}

#[cfg(feature = "cli")]
mod commands {
    use std::sync::Arc;

    use systemprompt_agent::services::agent_orchestration::status_view::AgentStatusView;
    use systemprompt_extension::cli::{CommandClaim, CommandDecl, KernelGrant, parents};
    use systemprompt_extension::prelude::*;
    use systemprompt_traits::{SecretPresence, SecretStore};

    use crate::cli::agents::AgentsCommands;
    use crate::cli::artifacts::ArtifactsCommands;
    use crate::cli::contexts::ContextsCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<AgentStatusView>>()
            .provide(
                |r: &Resolved<'_>| -> Result<Arc<dyn SecretPresence>, ProvideError> {
                    Ok(Arc::new(StoredSecretPresence(
                        r.get::<Arc<dyn SecretStore>>()?,
                    )))
                },
            )
            .command(
                CommandDecl::group_under::<parents::Admin, AgentsCommands>()
                    .requesting(&[KernelGrant::ServicesTree, KernelGrant::CallerSession])
                    .claiming(&[CommandClaim::PreservesData]),
            )
            .command(
                CommandDecl::group_under::<parents::Core, ContextsCommands>()
                    .requesting(&[KernelGrant::CallerSession, KernelGrant::SessionContext])
                    .claiming(&[CommandClaim::PreservesData]),
            )
            .command(
                CommandDecl::group_under::<parents::Core, ArtifactsCommands>()
                    .requesting(&[KernelGrant::CallerSession])
                    .claiming(&[CommandClaim::PreservesData]),
            );
    }

    struct StoredSecretPresence(Arc<dyn SecretStore>);

    impl SecretPresence for StoredSecretPresence {
        fn is_present(&self, name: &str) -> bool {
            self.0
                .secret(name)
                .is_some_and(|secret| !secret.expose().is_empty())
        }
    }
}
