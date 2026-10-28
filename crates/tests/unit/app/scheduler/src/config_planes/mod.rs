mod apply_race_db;
mod core_planes_db;
mod db_plane;
mod e2e;
mod fake_plane;
mod guard;
mod merge;
mod reconcile_db;
mod reconcile_ops_db;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use systemprompt_provider_contracts::{ConfigPlane, PlaneAuthority};
use systemprompt_runtime::ConfigPlaneSet;

use fake_plane::FakePlane;

fn plane_set(root: &Path, planes: &[Arc<FakePlane>], db: &[&str]) -> ConfigPlaneSet {
    let mut authority: BTreeMap<String, PlaneAuthority> = planes
        .iter()
        .map(|p| (p.name.to_owned(), PlaneAuthority::File))
        .collect();
    for name in db {
        authority.insert((*name).to_owned(), PlaneAuthority::Db);
    }
    let planes: Vec<Arc<dyn ConfigPlane>> = planes
        .iter()
        .map(|p| Arc::clone(p) as Arc<dyn ConfigPlane>)
        .collect();
    ConfigPlaneSet::new(planes, authority, root.to_path_buf()).expect("plane set")
}
