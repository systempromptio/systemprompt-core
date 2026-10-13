//! `.reg` serialisation for the Claude Desktop managed-policy profile.
//!
//! The profile *is* the policy: [`desktop_policy`] is the one rendering the
//! repair, the machine sync and the probe all use, so none of them can hold a
//! key the others judge differently. [`render_reg`] and
//! [`crate::install::reg_values::parse_reg_entries`] are inverses, kept
//! platform-independent so the round-trip is testable on every target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::shared::ProfileGenInputs;
use crate::drift::{DesiredProfile, EntryRole};
use crate::install::mdm::MdmError;
use crate::install::mdm::policy::{desktop_policy, reg_values};
use crate::install::reg_values::render_reg_values;

pub use crate::gateway::model_view::with_context_variants;

pub fn profile_entries(inputs: &ProfileGenInputs) -> Result<Vec<(&'static str, String)>, MdmError> {
    Ok(reg_values(&desktop_policy(inputs, None)?)
        .into_iter()
        .map(|(name, _, value)| (name, value))
        .collect())
}

pub fn desired(inputs: &ProfileGenInputs) -> Result<DesiredProfile, MdmError> {
    let mut desired = DesiredProfile::new();
    for (name, value) in profile_entries(inputs)? {
        let (value, role) = match name {
            crate::cowork_compat::POLICY_API_KEY => (
                crate::proxy::secret::fingerprint(value.trim()),
                EntryRole::Secret,
            ),
            "inferenceGatewayBaseUrl" => (value, EntryRole::Endpoint),
            "inferenceModels" => (value, EntryRole::Models),
            "managedMcpServers" => (value, EntryRole::ManagedServers),
            _ => (value, EntryRole::Setting),
        };
        desired.push(name, value, role);
    }
    Ok(desired)
}

pub fn render_reg(elevated: bool, inputs: &ProfileGenInputs) -> Result<String, MdmError> {
    Ok(render_reg_values(elevated, &profile_entries(inputs)?))
}
