//! Delegating seam over `login`'s private helpers so the separate test
//! workspace can drive them directly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

pub fn extract_code(pasted: &str) -> Result<String, String> {
    super::extract_code(pasted)
}

#[must_use]
pub fn strip_terminal_noise(pasted: &str) -> String {
    super::strip_terminal_noise(pasted)
}

#[must_use]
pub fn code_after_flag(pasted: &str) -> Option<String> {
    super::code_after_flag(pasted)
}

pub fn resolve_gateway(gateway: Option<&str>) -> Result<ValidatedUrl, String> {
    super::resolve_gateway(gateway)
}

#[must_use]
pub fn default_device_name() -> Option<String> {
    super::default_device_name()
}
