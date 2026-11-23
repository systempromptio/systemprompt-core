//! Values built from every admitted member's factories for one contribution
//! slot, each paired with the member that contributed it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Factory, SlotError, build_factories};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::AppContext;

pub(super) fn admitted_factories<T: 'static>(
    ctx: &AppContext,
    slot: &'static str,
    factories: fn(&ContributionSink) -> &[Box<dyn Factory<T>>],
) -> Result<Vec<(ExtensionId, T)>, SlotError> {
    let resolved = ctx.kernel().resolved();
    let mut built = Vec::new();
    for member in ctx.admitted().built().members() {
        for value in build_factories(
            slot,
            &member.owner,
            member.standing,
            factories(&member.sink),
            &resolved,
        )? {
            built.push((member.owner.clone(), value));
        }
    }
    Ok(built)
}
