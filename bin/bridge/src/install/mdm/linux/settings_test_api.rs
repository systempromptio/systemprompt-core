//! Delegating seam over the Linux managed-settings writer so the separate
//! test workspace can drive its seeding and error arms.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::install::mdm::MdmError;

pub fn seed_default_model(model: &str) -> Result<bool, MdmError> {
    super::seed_default_model(model)
}

#[must_use]
pub fn managed_settings_path() -> Option<PathBuf> {
    super::managed_settings_path()
}
