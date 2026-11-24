//! The whole-plane apply resolved from a request and the current snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ApplyMode, ApplyRequest, ChangeSource, ConfigPlaneError, PlaneSnapshot, ResolvedEntities,
    ResolvedEntity,
};

#[must_use]
pub fn resolve_apply(
    request: &ApplyRequest<'_>,
    current: &PlaneSnapshot,
    declared_path: &str,
) -> ResolvedEntities {
    let origin = ChangeSource::Code(declared_path.to_owned());
    let mut resolved = ResolvedEntities::new();
    for (key, value) in &request.declared.entries {
        let kept = request.mode == ApplyMode::InsertOnly && current.contains_key(key);
        if request.scope.admits(key) && current.get(key) != Some(value) && !kept {
            resolved.insert(
                key.clone(),
                ResolvedEntity {
                    value: Some(value.clone()),
                    source: origin.clone(),
                },
            );
        }
    }
    if request.mode != ApplyMode::InsertOnly {
        for key in current.keys() {
            if request.scope.admits(key) && !request.declared.entries.contains_key(key) {
                resolved.insert(
                    key.clone(),
                    ResolvedEntity {
                        value: None,
                        source: origin.clone(),
                    },
                );
            }
        }
    }
    resolved
}

pub(super) fn unsupported(plane: &str, operation: &'static str) -> ConfigPlaneError {
    ConfigPlaneError::Unsupported {
        plane: plane.to_owned(),
        operation,
    }
}
