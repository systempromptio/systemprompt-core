use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_provider_contracts::{ConfigPlane, ConfigPlaneError};
use systemprompt_runtime::{ConfigPlaneSet, EffectiveAuthority};

use super::fake_plane::{FakePlane, unique_plane_name};
use super::plane_set;

#[test]
fn a_declared_plane_defaults_to_file_managed_and_refuses_a_writer() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("guard_file"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let set = plane_set(root.path(), &[Arc::clone(&plane)], &[]);

    assert_eq!(
        set.authority(plane.name).expect("authority"),
        EffectiveAuthority::File
    );
    let err = set.writer(plane.name).expect_err("refused");
    assert!(matches!(err, ConfigPlaneError::FileManaged { ref path, .. } if path == plane.path));
    let api: ApiError = err.into();
    assert!(matches!(api.code, ErrorCode::ConflictError));
    assert_eq!(api.error_key.as_deref(), Some("config_plane_file_managed"));
}

#[test]
fn a_db_authority_plane_mints_a_writer_even_with_a_file() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("guard_db"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let set = plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name]);

    let writer = set.writer(plane.name).expect("writable");
    assert_eq!(writer.authority(), EffectiveAuthority::Db);
    assert_eq!(writer.plane().declared_path(), plane.path);
}

#[test]
fn a_file_authority_plane_without_its_file_is_undeclared_and_writable() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("guard_absent"), &[]));
    let set = plane_set(root.path(), &[Arc::clone(&plane)], &[]);

    assert_eq!(
        set.writer(plane.name).expect("writable").authority(),
        EffectiveAuthority::Undeclared
    );
}

#[test]
fn an_unknown_plane_is_a_typed_error() {
    let root = tempfile::tempdir().expect("tempdir");
    let set = plane_set(root.path(), &[], &[]);
    assert!(matches!(
        set.writer("nope"),
        Err(ConfigPlaneError::UnknownPlane(name)) if name == "nope"
    ));
}

#[test]
fn duplicate_plane_names_are_refused() {
    let name = unique_plane_name("guard_dup");
    let planes: Vec<Arc<dyn ConfigPlane>> = vec![
        Arc::new(FakePlane::new(name, &[])),
        Arc::new(FakePlane::new(name, &[])),
    ];
    let result = ConfigPlaneSet::new(planes, BTreeMap::new(), "/tmp".into());
    assert!(matches!(result, Err(ConfigPlaneError::DuplicatePlane(n)) if n == name));
}

#[test]
fn profile_keys_without_a_registered_plane_are_listed() {
    let root = tempfile::tempdir().expect("tempdir");
    let set = plane_set(root.path(), &[], &["ghost_plane"]);
    assert_eq!(set.unregistered_authority_keys(), vec!["ghost_plane"]);
}

#[test]
fn plane_errors_map_to_stable_http_classes() {
    let unknown: ApiError = ConfigPlaneError::UnknownPlane("x".into()).into();
    assert!(matches!(unknown.code, ErrorCode::NotFound));
    let unsupported: ApiError = ConfigPlaneError::Unsupported {
        plane: "x".into(),
        operation: "export",
    }
    .into();
    assert!(matches!(unsupported.code, ErrorCode::BadRequest));
}
