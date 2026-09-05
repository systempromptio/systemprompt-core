//! Reading and writing the saved settings-window geometry.

use systemprompt_bridge::window_state::{WindowGeometry, load, save};

fn sandbox<R>(f: impl FnOnce(&std::path::Path) -> R) -> R {
    let state = tempfile::TempDir::new().expect("state tempdir");
    let home = tempfile::TempDir::new().expect("home tempdir");
    let path = state.path().to_path_buf();
    temp_env::with_vars(
        [
            ("HOME", Some(home.path().to_string_lossy().into_owned())),
            (
                "XDG_STATE_HOME",
                Some(path.to_string_lossy().into_owned()),
            ),
        ],
        || f(&path),
    )
}

fn geometry() -> WindowGeometry {
    WindowGeometry {
        x: 120,
        y: 64,
        width: 1280,
        height: 800,
        maximized: false,
    }
}

#[test]
fn with_nothing_saved_yet_there_is_no_geometry_to_restore() {
    sandbox(|_| {
        assert_eq!(load(), None);
    });
}

#[test]
fn a_saved_geometry_is_read_back_field_for_field() {
    sandbox(|_| {
        save(geometry());
        assert_eq!(load(), Some(geometry()));
    });
}

#[test]
fn saving_again_replaces_the_previous_geometry_rather_than_appending() {
    sandbox(|_| {
        save(geometry());
        let moved = WindowGeometry {
            x: -40,
            maximized: true,
            ..geometry()
        };
        save(moved);
        assert_eq!(load(), Some(moved));
    });
}

#[test]
fn saving_creates_the_metadata_directory_the_file_lives_in() {
    sandbox(|state| {
        save(geometry());
        let written = walk_for(state, "window-state.json").expect("geometry file was written");
        let bytes = std::fs::read(&written).expect("read back");
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("valid json");
        assert_eq!(parsed["width"], 1280);
        assert_eq!(parsed["maximized"], false);
    });
}

#[test]
fn a_geometry_file_that_is_not_json_is_ignored_rather_than_crashing_the_window() {
    sandbox(|state| {
        save(geometry());
        let written = walk_for(state, "window-state.json").expect("geometry file was written");
        std::fs::write(&written, b"not json at all").expect("corrupt the file");
        assert_eq!(
            load(),
            None,
            "a corrupt geometry falls back to OS placement"
        );
    });
}

#[test]
fn a_geometry_written_without_the_maximized_flag_still_loads() {
    sandbox(|state| {
        save(geometry());
        let written = walk_for(state, "window-state.json").expect("geometry file was written");
        std::fs::write(
            &written,
            br#"{"x":10,"y":20,"width":900,"height":700}"#,
        )
        .expect("rewrite without the flag");

        let loaded = load().expect("an older geometry still loads");
        assert_eq!(loaded.x, 10);
        assert_eq!(loaded.width, 900);
        assert!(!loaded.maximized);
    });
}

fn walk_for(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(root).ok()? {
        let path = entry.ok()?.path();
        if path.is_dir() {
            if let Some(found) = walk_for(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|f| f == name) {
            return Some(path);
        }
    }
    None
}
