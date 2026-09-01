//! `.reg` serialisation for the Claude Desktop managed-policy profile.
//!
//! [`render_reg`] and [`crate::install::reg_values::parse_reg_entries`] are
//! inverses, kept platform-independent so the round-trip is testable on every
//! target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::shared::{ProfileGenInputs, default_models};
use crate::install::reg_values::render_reg_values;


#[must_use]
pub fn profile_entries(inputs: &ProfileGenInputs) -> Vec<(&'static str, String)> {
    let models = if inputs.models.is_empty() {
        default_models()
    } else {
        inputs.models.clone()
    };
    let models_json = serde_json::to_string(&models).unwrap_or_else(|_| "[]".into());
    let mut entries = vec![
        ("inferenceProvider", "gateway".to_owned()),
        ("inferenceGatewayBaseUrl", inputs.gateway_base_url.clone()),
        ("inferenceGatewayApiKey", inputs.api_key.clone()),
        ("inferenceGatewayAuthScheme", "bearer".to_owned()),
        ("inferenceModels", models_json),
    ];
    if !inputs.headers.is_empty() {
        let headers_json = serde_json::to_string(&inputs.headers).unwrap_or_else(|_| "{}".into());
        entries.push(("inferenceCustomHeaders", headers_json));
    }
    entries
}

#[must_use]
pub fn render_reg(elevated: bool, inputs: &ProfileGenInputs) -> String {
    render_reg_values(elevated, &profile_entries(inputs))
}
