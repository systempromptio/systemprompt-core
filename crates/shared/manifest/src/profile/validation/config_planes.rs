//! `config_planes` profile checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{ConfigPlanesConfig, Profile};

impl Profile {
    pub(crate) fn validate_config_planes(&self, errors: &mut Vec<String>) {
        for name in self.config_planes.authority.keys() {
            if !ConfigPlanesConfig::is_valid_plane_name(name) {
                errors.push(format!(
                    "config_planes.authority: '{name}' is not a plane name (lowercase letters, \
                     digits and '_', starting with a letter, at most 64 characters)"
                ));
            }
        }
    }
}
