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
//! `agent` member provides. It provides the open
//! [`AgentSurface`](systemprompt_extension::router::AgentSurface) the API
//! mounts the webhook broadcasts, the SSE streams and the well-known agent
//! cards through; the [`StreamFeeds`](crate::http::stream::StreamFeeds) those
//! streams subscribe through **sealed** (only its own route state reads it);
//! the chat-platform messaging dispatcher **sealed**, built here from the
//! host's authz hook cell, signing keys, issuer and agent dispatch cell, which
//! the kernel binds into the messaging cell while it builds its routes; and
//! the `DynAgentServer` the kernel's `admin agent-host run` child host serves
//! one agent through, **sealed** after governance (it reads the AI provider).
//! A composition without `agent-http` has no agent server: an agent child
//! host refuses to serve, naming this member.
//!
//! With `cli` it contributes `admin agents` (grants `services-tree` and
//! `caller-session`), `core contexts` (`caller-session`, `session-context`)
//! and `core artifacts` (`caller-session`). No command starts, stops or
//! signals an agent: `admin agents status` reads the open status view and
//! `admin agents delete` refuses a running agent.
//!
//! The `/api/v1/agents/{service}` proxy, the agent dispatch it sends
//! through, agent supervision and the `admin agent-host run` child host are
//! the kernel's; the A2A server that child host runs is this crate's
//! [`crate::a2a_server`].
//!
//! | profile disables | routes | commands |
//! |---|---|---|
//! | `agent-http` | 404 (webhook, streams, agent cards, registry) | refused at dispatch |
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

    use systemprompt_database::DbPool;
    use systemprompt_events::EventRouter;
    use systemprompt_extension::prelude::*;
    use systemprompt_extension::router::DynAgentSurface;
    use systemprompt_governance::authz::AuthzHookCell;
    use systemprompt_manifest::Config;
    use systemprompt_models::a2a::dispatch::AgentDispatchCell;
    use systemprompt_security::keys::SigningKeys;
    use systemprompt_traits::{DynAgentServer, MessagingDispatcher};
    use systemprompt_users::UserRepository;

    use crate::a2a_server::serve::HostedAgentServer;
    use crate::http::stream::StreamFeeds;
    use crate::http::{
        AgentHttpSurface, ContextEventPublisher, artifacts_routes, contexts_routes,
        registry_routes, tasks_routes,
    };
    use crate::messaging::{A2aMessagingDispatcher, DispatcherInputs, NoMessagingDispatch};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.provide(
            |r: &Resolved<'_>| -> Result<Arc<ContextEventPublisher>, ProvideError> {
                Ok(Arc::new(ContextEventPublisher::new(
                    r.get::<Arc<EventRouter>>()?,
                )))
            },
        )
        .provide(
            |_r: &Resolved<'_>| -> Result<DynAgentSurface, ProvideError> {
                Ok(Arc::new(AgentHttpSurface))
            },
        )
        .provide_sealed(
            |r: &Resolved<'_>| -> Result<Arc<StreamFeeds>, ProvideError> {
                Ok(Arc::new(StreamFeeds::new(
                    r.get::<Arc<EventRouter>>()?.as_ref(),
                )))
            },
        )
        .provide_sealed(messaging_dispatcher)
        .provide_sealed_after_governance(
            |r: &Resolved<'_>| -> Result<DynAgentServer, ProvideError> {
                Ok(Arc::new(HostedAgentServer::from_resolved(r)?))
            },
        )
        .routes(contexts_routes())
        .routes(tasks_routes())
        .routes(artifacts_routes())
        .routes(registry_routes());
    }

    fn messaging_dispatcher(
        r: &Resolved<'_>,
    ) -> Result<Arc<dyn MessagingDispatcher>, ProvideError> {
        let (Some(keys), Some(dispatch)) = (
            r.optional::<SigningKeys>(),
            r.optional::<AgentDispatchCell>(),
        ) else {
            return Ok(Arc::new(NoMessagingDispatch));
        };
        let db = r.get::<DbPool>()?;
        let config = r.get::<Arc<Config>>()?;
        Ok(Arc::new(A2aMessagingDispatcher::new(DispatcherInputs {
            users: Arc::new(UserRepository::new(&db)),
            authz: r.get::<AuthzHookCell>()?,
            issuer: config.jwt_issuer.clone(),
            keys,
            dispatch,
        })))
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
