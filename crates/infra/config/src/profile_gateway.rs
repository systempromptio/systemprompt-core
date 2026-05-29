//! Gateway-section post-parse helpers for the profile loader.
//!
//! Catalog resolution itself lives in
//! [`systemprompt_models::profile::GatewayConfigSpec::resolve`]; this
//! module only owns the in-memory route-id backfill applied to the parsed
//! spec before resolution.

use systemprompt_models::profile::GatewayConfigSpec;

/// Synthesize stable ids for any route that was authored without one.
///
/// Returns `true` if any route was mutated. Ids are deterministic, so the
/// backfill is reapplied identically on every load rather than persisted.
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
