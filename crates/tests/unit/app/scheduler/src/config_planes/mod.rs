mod fake_plane;
mod guard;
mod reconcile_db;

use std::path::Path;
use std::sync::Arc;

use systemprompt_manifest::profile::{ConfigPlanesConfig, PlaneAuthority};
use systemprompt_provider_contracts::ConfigPlane;
use systemprompt_runtime::ConfigPlaneSet;

use fake_plane::FakePlane;

fn plane_set(root: &Path, planes: &[Arc<FakePlane>], db: &[&str]) -> ConfigPlaneSet {
    let mut config = ConfigPlanesConfig::default();
    for name in db {
        config
            .authority
            .insert((*name).to_owned(), PlaneAuthority::Db);
    }
    let planes: Vec<Arc<dyn ConfigPlane>> = planes
        .iter()
        .map(|p| Arc::clone(p) as Arc<dyn ConfigPlane>)
        .collect();
    ConfigPlaneSet::new(planes, config, root.to_path_buf()).expect("plane set")
}
