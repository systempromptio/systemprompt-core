//! Captures the on-disk files of a configured inventory entry.
//!
//! Only configured skills are captured (by `publish_latest`), and a skill is
//! always a directory under the services root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::InventoryEntry;
use super::catalog::invalid;
use crate::managed::{Result, RevisionFiles, capture_inventory_files};

pub(super) fn configured_files(root: &Path, entry: &InventoryEntry) -> Result<RevisionFiles> {
    capture_inventory_files(
        root,
        entry
            .configured_key
            .as_deref()
            .ok_or_else(|| invalid("Missing authoring path"))?,
    )
}
