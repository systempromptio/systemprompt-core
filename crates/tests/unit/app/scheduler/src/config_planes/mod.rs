mod db_plane;
mod e2e;
mod fake_plane;
mod guard;
mod reconcile_db;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use systemprompt_provider_contracts::{ConfigPlane, PlaneAuthority};
use systemprompt_runtime::ConfigPlaneSet;

use fake_plane::FakePlane;

fn plane_set(root: &Path, planes: &[Arc<FakePlane>], db: &[&str]) -> ConfigPlaneSet {
    let authority: BTreeMap<String, PlaneAuthority> = db
        .iter()
        .map(|name| ((*name).to_owned(), PlaneAuthority::Db))
        .collect();
    let planes: Vec<Arc<dyn ConfigPlane>> = planes
        .iter()
        .map(|p| Arc::clone(p) as Arc<dyn ConfigPlane>)
        .collect();
    ConfigPlaneSet::new(planes, authority, root.to_path_buf()).expect("plane set")
}
