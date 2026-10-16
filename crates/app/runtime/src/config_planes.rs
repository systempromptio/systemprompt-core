//! The registered [`ConfigPlane`]s and the authority each one runs under.
//!
//! [`ConfigPlaneSet`] is the write guard console and API writers consult:
//! [`ConfigPlaneSet::ensure_db_writable`] refuses a write to a plane whose
//! declaration file exists and whose profile authority is `file`, because
//! the reconciler would overwrite the edit on its next pass. A plane whose
//! file is absent is never file-managed, whatever the profile says.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use systemprompt_manifest::profile::{ConfigPlanesConfig, PlaneAuthority};
use systemprompt_provider_contracts::{ConfigPlane, ConfigPlaneError, ConfigPlaneResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveAuthority {
    File,
    Db,
    Undeclared,
}

impl EffectiveAuthority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Db => "db",
            Self::Undeclared => "undeclared",
        }
    }

    #[must_use]
    pub const fn is_file_managed(self) -> bool {
        matches!(self, Self::File)
    }
}

#[derive(Clone)]
pub struct ConfigPlaneSet {
    planes: Vec<Arc<dyn ConfigPlane>>,
    config: ConfigPlanesConfig,
    services_root: PathBuf,
}

impl std::fmt::Debug for ConfigPlaneSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigPlaneSet")
            .field("planes", &self.names())
            .field("config", &self.config)
            .field("services_root", &self.services_root)
            .finish()
    }
}

impl ConfigPlaneSet {
    pub fn new(
        planes: Vec<Arc<dyn ConfigPlane>>,
        config: ConfigPlanesConfig,
        services_root: PathBuf,
    ) -> ConfigPlaneResult<Self> {
        let mut seen = HashSet::new();
        for plane in &planes {
            if !seen.insert(plane.name()) {
                return Err(ConfigPlaneError::DuplicatePlane(plane.name().to_owned()));
            }
        }
        Ok(Self {
            planes,
            config,
            services_root,
        })
    }

    #[must_use]
    pub fn planes(&self) -> &[Arc<dyn ConfigPlane>] {
        &self.planes
    }

    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.planes.iter().map(|p| p.name()).collect()
    }

    #[must_use]
    pub fn services_root(&self) -> &Path {
        &self.services_root
    }

    #[must_use]
    pub const fn config(&self) -> &ConfigPlanesConfig {
        &self.config
    }

    pub fn find(&self, name: &str) -> ConfigPlaneResult<&Arc<dyn ConfigPlane>> {
        self.planes
            .iter()
            .find(|p| p.name() == name)
            .ok_or_else(|| ConfigPlaneError::UnknownPlane(name.to_owned()))
    }

    #[must_use]
    pub fn declared_exists(&self, plane: &dyn ConfigPlane) -> bool {
        self.services_root.join(plane.declared_path()).exists()
    }

    #[must_use]
    pub fn authority_of(&self, plane: &dyn ConfigPlane) -> EffectiveAuthority {
        match self.config.authority_for(plane.name()) {
            PlaneAuthority::Db => EffectiveAuthority::Db,
            PlaneAuthority::File if self.declared_exists(plane) => EffectiveAuthority::File,
            PlaneAuthority::File => EffectiveAuthority::Undeclared,
        }
    }

    pub fn authority(&self, name: &str) -> ConfigPlaneResult<EffectiveAuthority> {
        Ok(self.authority_of(self.find(name)?.as_ref()))
    }

    pub fn is_file_managed(&self, name: &str) -> ConfigPlaneResult<bool> {
        Ok(self.authority(name)?.is_file_managed())
    }

    pub fn ensure_db_writable(&self, name: &str) -> ConfigPlaneResult<()> {
        let plane = self.find(name)?;
        if self.authority_of(plane.as_ref()).is_file_managed() {
            return Err(ConfigPlaneError::FileManaged {
                plane: name.to_owned(),
                path: plane.declared_path().to_owned(),
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn unregistered_authority_keys(&self) -> Vec<&str> {
        self.config
            .authority
            .keys()
            .map(String::as_str)
            .filter(|key| !self.planes.iter().any(|p| p.name() == *key))
            .collect()
    }
}
