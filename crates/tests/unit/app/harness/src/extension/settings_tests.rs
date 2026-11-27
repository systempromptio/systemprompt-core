//! Typed per-extension preferences, shared handles and extension paths.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use systemprompt_harness::extension::{ExtensionSettings, Handles, SettingsAccess, SettingsError};

use crate::extension::support::{MemorySettings, context, id};

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct DrivePrefs {
    folder: Option<String>,
    paused: bool,
}

#[test]
fn unset_preferences_read_as_the_default() {
    let settings = ExtensionSettings::new(id("drive"), MemorySettings::new());
    assert_eq!(settings.get::<DrivePrefs>().unwrap(), DrivePrefs::default());
}

#[test]
fn preferences_round_trip_and_announce_the_change() {
    let store = MemorySettings::new();
    let access: Arc<dyn SettingsAccess> = Arc::<MemorySettings>::clone(&store);
    let settings = ExtensionSettings::new(id("drive"), access);
    let changes = settings.changes();
    let prefs = DrivePrefs {
        folder: Some("C:/Drive".to_owned()),
        paused: true,
    };
    settings.set(&prefs).unwrap();
    assert_eq!(settings.get::<DrivePrefs>().unwrap(), prefs);
    assert!(changes.has_changed().unwrap());
    assert!(store.read(&id("brain")).is_none());
}

#[test]
fn preferences_that_do_not_encode_as_an_object_are_refused() {
    let settings = ExtensionSettings::new(id("drive"), MemorySettings::new());
    let error = settings.set(&7u32).unwrap_err();
    assert!(matches!(error, SettingsError::NotAnObject { extension } if extension == id("drive")));
}

#[test]
fn preferences_of_another_shape_fail_to_decode() {
    let settings = ExtensionSettings::new(id("drive"), MemorySettings::new());
    settings
        .set(&serde_json::json!({ "paused": "yes" }))
        .unwrap();
    assert!(matches!(
        settings.get::<DrivePrefs>(),
        Err(SettingsError::Decode { .. })
    ));
}

#[test]
fn handles_are_found_by_type_from_every_clone() {
    let handles = Handles::default();
    let clone = handles.clone();
    handles.insert(Arc::new(String::from("drive sync")));
    assert_eq!(
        clone.get::<String>().as_deref().map(String::as_str),
        Some("drive sync")
    );
    assert!(clone.contains::<String>());
    assert!(clone.get::<u32>().is_none());
}

#[tokio::test]
async fn each_extension_has_its_own_state_directory() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    assert_eq!(
        ctx.paths.extension_state(&id("brain")),
        root.path().join("state").join("extensions").join("brain")
    );
    assert_ne!(
        ctx.paths.extension_cache(&id("brain")),
        ctx.paths.extension_state(&id("brain"))
    );
    let settings = ctx.settings_for(&id("brain"));
    settings.set(&DrivePrefs::default()).unwrap();
    assert!(ctx.settings.read(&id("brain")).is_some());
}
