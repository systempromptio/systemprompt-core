//! Gateway-section post-parse helpers for the profile loader.
//!
//! Catalog resolution itself lives in
//! [`systemprompt_models::profile::GatewayConfigSpec::resolve`]; this
//! module only owns the in-memory route-id backfill applied to the parsed
//! spec before resolution.

use systemprompt_models::profile::GatewayConfigSpec;

/// Ids are deterministic, so this in-memory backfill is reapplied identically
/// on every load rather than persisted to disk.
pub fn backfill_route_ids(spec: &mut GatewayConfigSpec) -> bool {
    let mut mutated = false;
    for route in &mut spec.routes {
        if route.id.as_str().trim().is_empty() {
            route.ensure_id();
            mutated = true;
        }
    }
    mutated
}
