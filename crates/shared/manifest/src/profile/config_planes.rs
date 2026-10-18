//! Per-plane authority for declared configuration planes.
//!
//! Every plane an extension registers is `file`-authoritative unless the
//! profile names it under `config_planes.authority` as `db`. A file-authority
//! plane whose declaration exists is reconciled to that file at boot and by
//! the `config_reconcile` job, and console writers refuse to edit it. A
//! `db`-authority plane is seeded from its file only while its projection is
//! empty; afterwards drift is reported and never written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum PlaneAuthority {
    #[default]
    File,
    Db,
}

impl PlaneAuthority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Db => "db",
        }
    }
}

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
