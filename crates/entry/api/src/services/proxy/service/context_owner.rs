//! The ownership check on a context id a proxied MCP request names.
//!
//! A request may name the conversation context it runs in (`x-context-id`).
//! The proxy forwards that id to the server, which records tool executions
//! against it, so the id is honoured only when the caller the edge verified
//! owns the context (`DynContextDirectory`, provided by the agent member). On
//! a server that requires no OAuth the presented bearer is verified for this
//! check alone. With a directory admitted, a request with no verified caller,
//! one naming another user's context, or one whose lookup fails is refused
//! before anything is forwarded. A request that names no context keeps the
//! context of its session.
//!
//! A composition in which no member provides a context directory has no
//! contexts to own: a named context is discarded and the request runs under
//! the context derived from its session, the same rule the CLI session
//! follows. Only a directory no member provides selects this: one that is
//! withheld on this host, or whose type the host reserves without holding a
//! value, cannot be read, so a request naming a context there is refused
//! (`context_lookup_failed`, its cause the typed `MissingDependency`), as is
//! an error from an admitted directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

use axum::http::HeaderMap;
use systemprompt_identifiers::{ContextId, ServiceName, headers};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::{Dependencies, MissingDependency};
use systemprompt_runtime::AppContext;
use systemprompt_traits::DynContextDirectory;

use crate::services::proxy::auth::AccessValidator;
use crate::services::proxy::backend::ProxyError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContextScope {
    AsRequested,
    SessionDerived,
}

impl ContextScope {
    pub(super) fn apply(self, context: RequestContext) -> RequestContext {
        match self {
            Self::AsRequested => context,
            Self::SessionDerived => {
                let derived = ContextId::derived_from_session(context.session_id());
                context.with_context_id(derived)
            },
        }
    }
}

pub(super) async fn ensure_owned(
    ctx: &AppContext,
    service_name: &ServiceName,
    request_headers: &HeaderMap,
    user: Option<&AuthenticatedUser>,
    req_context: Option<&RequestContext>,
) -> Result<ContextScope, ProxyError> {
    let Some(raw) = request_headers.get(headers::CONTEXT_ID) else {
        return Ok(ContextScope::AsRequested);
    };
    let state = ctx.kernel().state();
    let Some(directory) = state.find::<DynContextDirectory>() else {
        return without_directory(state, service_name);
    };
    let not_owned = || ProxyError::ContextNotOwned {
        service: service_name.to_string(),
    };
    let context = raw
        .to_str()
        .ok()
        .and_then(|value| ContextId::try_new(value.to_owned()).ok())
        .ok_or_else(not_owned)?;
    let verified = match user {
        Some(user) => Some(user.clone()),
        None => AccessValidator::verified_caller(request_headers, service_name, ctx, req_context)?,
    };
    let user = verified.ok_or_else(not_owned)?;
    match directory.for_caller(&user.id).owned_context(&context).await {
        Ok(Some(_)) => Ok(ContextScope::AsRequested),
        Ok(None) => Err(not_owned()),
        Err(source) => Err(ProxyError::ContextLookupFailed {
            service: service_name.to_string(),
            source: Box::new(source),
        }),
    }
}

fn without_directory(
    state: &Dependencies,
    service_name: &ServiceName,
) -> Result<ContextScope, ProxyError> {
    let unreadable = |missing: MissingDependency| ProxyError::ContextLookupFailed {
        service: service_name.to_string(),
        source: Box::new(missing),
    };
    if state.is_withheld::<DynContextDirectory>() {
        return Err(unreadable(
            MissingDependency::withheld::<DynContextDirectory>(),
        ));
    }
    let directory = TypeId::of::<DynContextDirectory>();
    if state.host_types().any(|(id, _)| id == directory) {
        return Err(unreadable(MissingDependency::of::<DynContextDirectory>()));
    }
    Ok(ContextScope::SessionDerived)
}
