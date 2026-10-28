//! Per-plane authority for declared configuration planes.
//!
//! Every plane an extension registers is `db`-authoritative unless the
//! profile names it under `config_planes.authority` as `file`. A `db` plane is
//! changed by console edits and by `config apply`, which merges the
//! declaration three ways against the last applied base and records the
//! entities both sides changed as conflicts. A `file` plane whose declaration
//! exists is managed in Git: console and API writers refuse to edit it, and
//! `config apply`, the scheduler node's boot and the `config_reconcile` job
//! overwrite the database with the file, audited, resolving any open
//! conflicts in the file's favour. A plane whose declaration is absent is
//! undeclared and never file-managed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
pub use systemprompt_provider_contracts::PlaneAuthority;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPlanesConfig {
    #[serde(default)]
    pub authority: BTreeMap<String, PlaneAuthority>,
}

impl ConfigPlanesConfig {
    #[must_use]
    pub fn authority_for(&self, plane: &str) -> PlaneAuthority {
        self.authority.get(plane).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn is_valid_plane_name(name: &str) -> bool {
        let mut chars = name.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && name.len() <= 64
    }
}
