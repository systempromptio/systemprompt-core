//! The sealed kernel seed API kernel code reads by type: the connector token
//! cache. It is read from the kernel state, which only kernel code and core
//! provide fns see in full; an absent seed is a typed `MissingDependency`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::AppContext;
use systemprompt_security::credential::TokenCache;

pub(crate) fn connector_tokens(ctx: &AppContext) -> Result<Arc<TokenCache>, MissingDependency> {
    ctx.kernel().state().get::<Arc<TokenCache>>().cloned()
}
