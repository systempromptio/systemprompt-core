//! [`ConfigPlaneSet`]: the registered [`ConfigPlane`]s in dependency order,
//! the authority each one runs under, and the only way to obtain a
//! [`PlaneWriter`].
//!
//! [`ConfigPlaneSet::new`] refuses duplicate names, a `depends_on` naming an
//! unregistered plane and a dependency cycle, and orders the planes so every
//! plane follows the planes it depends on (registration order breaks ties).
//! The reconciler walks [`ConfigPlaneSet::planes`] in that order.
//!
//! A plane the profile does not name runs under `db` authority. A console or
//! API writer edits a plane's rows only while holding a [`PlaneWriter`] for
//! it, and [`ConfigPlaneSet::writer`] mints one only when the plane is not
//! file-managed: a plane the profile names `file` whose declaration exists
//! answers [`ConfigPlaneError::FileManaged`], because the next `config apply`
//! takes the file over the edit. A plane whose file is absent is undeclared
//! and never file-managed, whatever the profile says.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{ConfigPlane, ConfigPlaneError, ConfigPlaneResult};

/// The authority a profile names for one plane under
/// `config_planes.authority`; a plane the profile does not name is `db`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum PlaneAuthority {
    File,
    #[default]
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

/// The authority a plane actually runs under once the declaration file's
/// presence is taken into account.
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

/// Proof that the holder may write a plane's rows directly.
///
/// It has no public constructor: [`ConfigPlaneSet::writer`] is the only
/// source, so a write path that takes a `&PlaneWriter` cannot run against a
/// file-managed plane.
pub struct PlaneWriter<'a> {
    plane: &'a dyn ConfigPlane,
    authority: EffectiveAuthority,
}

impl<'a> PlaneWriter<'a> {
    #[must_use]
    pub fn plane(&self) -> &'a dyn ConfigPlane {
        self.plane
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        self.plane.name()
    }

    #[must_use]
    pub const fn authority(&self) -> EffectiveAuthority {
        self.authority
    }
}

impl fmt::Debug for PlaneWriter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaneWriter")
            .field("plane", &self.plane.name())
            .field("authority", &self.authority)
            .finish()
    }
}

#[derive(Clone)]
pub struct ConfigPlaneSet {
    planes: Vec<Arc<dyn ConfigPlane>>,
    authority: BTreeMap<String, PlaneAuthority>,
    services_root: PathBuf,
}

impl fmt::Debug for ConfigPlaneSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigPlaneSet")
            .field("planes", &self.names())
            .field("authority", &self.authority)
            .field("services_root", &self.services_root)
            .finish()
    }
}

impl ConfigPlaneSet {
    pub fn new(
        planes: Vec<Arc<dyn ConfigPlane>>,
        authority: BTreeMap<String, PlaneAuthority>,
        services_root: PathBuf,
    ) -> ConfigPlaneResult<Self> {
        Ok(Self {
            planes: dependency_order(planes)?,
            authority,
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
        if !self.declared_exists(plane) {
            return EffectiveAuthority::Undeclared;
        }
        match self
            .authority
            .get(plane.name())
            .copied()
            .unwrap_or_default()
        {
            PlaneAuthority::File => EffectiveAuthority::File,
            PlaneAuthority::Db => EffectiveAuthority::Db,
        }
    }

    pub fn authority(&self, name: &str) -> ConfigPlaneResult<EffectiveAuthority> {
        Ok(self.authority_of(self.find(name)?.as_ref()))
    }

    pub fn writer(&self, name: &str) -> ConfigPlaneResult<PlaneWriter<'_>> {
        let plane = self.find(name)?;
        let authority = self.authority_of(plane.as_ref());
        if authority.is_file_managed() {
            return Err(ConfigPlaneError::FileManaged {
                plane: name.to_owned(),
                path: plane.declared_path().to_owned(),
            });
        }
        Ok(PlaneWriter {
            plane: plane.as_ref(),
            authority,
        })
    }

    #[must_use]
    pub fn unregistered_authority_keys(&self) -> Vec<&str> {
        self.authority
            .keys()
            .map(String::as_str)
            .filter(|key| !self.planes.iter().any(|p| p.name() == *key))
            .collect()
    }
}

fn dependency_order(
    planes: Vec<Arc<dyn ConfigPlane>>,
) -> ConfigPlaneResult<Vec<Arc<dyn ConfigPlane>>> {
    let mut seen = HashSet::new();
    for plane in &planes {
        if !seen.insert(plane.name()) {
            return Err(ConfigPlaneError::DuplicatePlane(plane.name().to_owned()));
        }
    }
    for plane in &planes {
        if let Some(missing) = plane.depends_on().iter().find(|dep| !seen.contains(*dep)) {
            return Err(ConfigPlaneError::UnknownDependency {
                plane: plane.name().to_owned(),
                dependency: (*missing).to_owned(),
            });
        }
    }
    let mut pending: Vec<Option<Arc<dyn ConfigPlane>>> = planes.into_iter().map(Some).collect();
    let mut placed: HashSet<&'static str> = HashSet::new();
    let mut ordered = Vec::with_capacity(pending.len());
    while ordered.len() < pending.len() {
        let ready = pending.iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|p| p.depends_on().iter().all(|dep| placed.contains(dep)))
        });
        let Some(index) = ready else {
            let stuck = pending.iter().flatten().next().map_or("", |p| p.name());
            return Err(ConfigPlaneError::DependencyCycle(stuck.to_owned()));
        };
        if let Some(plane) = pending.get_mut(index).and_then(Option::take) {
            placed.insert(plane.name());
            ordered.push(plane);
        }
    }
    Ok(ordered)
}
