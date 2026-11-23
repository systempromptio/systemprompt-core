//! The ownership check on a context id a proxied MCP request names.
//!
//! A request may name the conversation context it runs in (`x-context-id`).
//! The proxy forwards that id to the server, which records tool executions
//! against it, so the id is honoured only when the caller the edge verified
//! owns the context (`DynContextDirectory`, provided by the agent member). On
//! a server that requires no OAuth the presented bearer is verified for this
//! check alone. A request with no verified caller, one naming another user's context, or one
//! arriving when no context directory is admitted is refused before anything
//! is forwarded. A request that names no context keeps the context of its
//! session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::{ContextId, ServiceName, headers};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{BoxedSource, DynContextDirectory};

use crate::services::proxy::auth::AccessValidator;
use crate::services::proxy::backend::ProxyError;

pub(super) async fn ensure_owned(
    ctx: &AppContext,
    service_name: &ServiceName,
    request_headers: &HeaderMap,
    user: Option<&AuthenticatedUser>,
    req_context: Option<&RequestContext>,
) -> Result<(), ProxyError> {
    let Some(raw) = request_headers.get(headers::CONTEXT_ID) else {
        return Ok(());
    };
    let not_owned = || ProxyError::ContextNotOwned {
        service: service_name.to_string(),
    };
    let context = raw
        .to_str()
        .ok()
        .and_then(|value| ContextId::try_new(value.to_owned()).ok())
        .ok_or_else(not_owned)?;
    let verified = user.map_or_else(
        || AccessValidator::verified_caller(request_headers, service_name, ctx, req_context),
        |user| Some(user.clone()),
    );
    let user = verified.ok_or_else(not_owned)?;
    let lookup_failed = |source: BoxedSource| ProxyError::ContextLookupFailed {
        service: service_name.to_string(),
        source,
    };
    let directory = ctx
        .kernel()
        .state()
        .get::<DynContextDirectory>()
        .map_err(|absent| lookup_failed(Box::new(absent)))?;
    match directory.owned_context(&user.id, &context).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(not_owned()),
        Err(source) => Err(lookup_failed(Box::new(source))),
    }
}
