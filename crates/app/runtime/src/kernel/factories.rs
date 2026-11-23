//! Building one factory kind of every admitted member at runtime: each
//! member's factories run with that member's scoped resolution, so a readiness
//! check, a diagnostic probe or a runner service built by the API reads only
//! what its member's standing admits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{BuiltState, Factory, Resolved, SlotError, build_factories};
use systemprompt_identifiers::ExtensionId;

use super::Kernel;

impl Kernel {
    pub fn admitted_factories<T: 'static>(
        &self,
        built: &BuiltState,
        slot: &'static str,
        factories: fn(&ContributionSink) -> &[Box<dyn Factory<T>>],
    ) -> Result<Vec<(ExtensionId, T)>, SlotError> {
        let resolved = Resolved::__new(self, &self.state);
        let mut out = Vec::new();
        for member in built.members() {
            for value in build_factories(slot, &member.scope(), factories(&member.sink), &resolved)?
            {
                out.push((member.owner.clone(), value));
            }
        }
        Ok(out)
    }
}
