//! The sealed kernel seeds API kernel code reads by type: the connector
//! token cache and the AI repositories. Each is read from the kernel state,
//! which only kernel code and core provide fns see in full; an absent seed is
//! a typed `MissingDependency`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::repository::AiRepositories;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::AppContext;
use systemprompt_security::credential::TokenCache;

pub(crate) fn connector_tokens(ctx: &AppContext) -> Result<Arc<TokenCache>, MissingDependency> {
    ctx.kernel().state().get::<Arc<TokenCache>>().cloned()
}

pub(crate) fn ai_repositories(ctx: &AppContext) -> Result<Arc<AiRepositories>, MissingDependency> {
    ctx.kernel().state().get::<Arc<AiRepositories>>().cloned()
}
