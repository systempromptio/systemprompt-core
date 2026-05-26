//! Gateway-section post-parse helpers for the profile loader.
//!
//! Catalog resolution itself lives in
//! [`systemprompt_models::profile::GatewayConfigSpec::resolve`]; this
//! module only owns the route-id backfill that mutates the on-disk spec
//! before persistence.

use systemprompt_models::profile::GatewayConfigSpec;

/// Synthesize stable ids for any route that was authored without one.
/// Returns `true` if any route was mutated and the spec should be
/// persisted back to disk.
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
