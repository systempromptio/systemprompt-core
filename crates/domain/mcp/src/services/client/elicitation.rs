//! Elicitation routing for the outbound MCP client.
//!
//! A server may pause a call and request direct user input (SEP-2322 rounds
//! carrying elicitation requests, including the URL-mode in-person approval
//! flow). The client only advertises the elicitation capability when a
//! [`ElicitationDelegate`] is installed; without one, any request a server
//! sends regardless is declined rather than errored, which terminates the
//! round cleanly on both sides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use rmcp::model::{ElicitRequestParams, ElicitResult, ElicitationAction};

/// Routes a server's elicitation request to a human. `#[async_trait]` for
/// `dyn`-compatibility: delegates are injected as trait objects at the
/// composition root.
#[async_trait::async_trait]
pub trait ElicitationDelegate: Send + Sync + std::fmt::Debug {
    async fn elicit(&self, params: ElicitRequestParams) -> ElicitResult;
}

pub type SharedElicitationDelegate = Arc<dyn ElicitationDelegate>;

pub async fn handle_elicitation(
    delegate: Option<&SharedElicitationDelegate>,
    params: ElicitRequestParams,
) -> ElicitResult {
    if let Some(delegate) = delegate {
        delegate.elicit(params).await
    } else {
        tracing::warn!(
            mode = params_mode(&params),
            "Elicitation request received with no delegate installed; declining"
        );
        ElicitResult::new(ElicitationAction::Decline)
    }
}

const fn params_mode(params: &ElicitRequestParams) -> &'static str {
    match params {
        ElicitRequestParams::FormElicitationParams { .. } => "form",
        ElicitRequestParams::UrlElicitationParams { .. } => "url",
        _ => "unknown",
    }
}
