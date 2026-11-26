//! The toolset the host member itself contributes: `brain`.
//!
//! The `brain` toolset is built from the `DynBrainToolsetSource` the brain
//! member provides; it offers no tools when that member is not admitted or
//! the profile has the brain off, so a server that lists it still boots.
//! Every other toolset, core's `issues` and `documents` among them, is
//! contributed by the member that owns it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::{BRAIN_TOOLSET, DynBrainToolsetSource};
use systemprompt_identifiers::McpServerId;

use super::factory::ToolsetBuildResult;
use crate::HostContext;
use crate::contract::ToolsetSlotBuilder;

#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is the ToolsetBuildFn pointer"
)]
fn build_brain(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    Ok(host
        .state()
        .find::<DynBrainToolsetSource>()
        .and_then(|source| source.toolset(server)))
}

pub(crate) fn contribute(slot: &mut ToolsetSlotBuilder) {
    slot.toolset(BRAIN_TOOLSET, build_brain);
}
