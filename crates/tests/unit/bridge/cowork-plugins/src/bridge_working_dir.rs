// The bridge's working state (staging + sync metadata) lives in a
// USER-WRITABLE location distinct from the published org-plugins tree.
// On Windows the published tree is `C:\Program Files\Claude\org-plugins\`
// (admin-write-only), so bridge scratch MUST live elsewhere — otherwise
// `bridge sync` fails with `io error in create staging: Access is denied`.

use systemprompt_bridge::config::paths;

#[test]
fn working_dir_resolves() {
    let dir = paths::bridge_working_dir().expect("bridge_working_dir must resolve");
    let name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .expect("working dir has a leaf");
    assert_eq!(name, "systemprompt-bridge");
}

#[test]
fn staging_dir_is_under_working_dir() {
    let working = paths::bridge_working_dir().expect("working dir resolves");
    let staging = paths::bridge_staging_dir().expect("staging dir resolves");
    assert!(
        staging.starts_with(&working),
        "{} must be under {}",
        staging.display(),
        working.display()
    );
    assert_eq!(staging.file_name().and_then(|s| s.to_str()), Some("staging"));
}

#[test]
fn metadata_dir_is_under_working_dir() {
    let working = paths::bridge_working_dir().expect("working dir resolves");
    let meta = paths::bridge_metadata_dir().expect("metadata dir resolves");
    assert!(
        meta.starts_with(&working),
        "{} must be under {}",
        meta.display(),
        working.display()
    );
    assert_eq!(meta.file_name().and_then(|s| s.to_str()), Some("metadata"));
}

#[cfg(target_os = "windows")]
#[test]
fn working_dir_is_under_localappdata_on_windows() {
    let working = paths::bridge_working_dir().expect("working dir resolves");
    let localappdata = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA set");
    assert!(
        working.starts_with(&localappdata),
        "{} must live under LOCALAPPDATA ({})",
        working.display(),
        localappdata
    );
}

#[cfg(target_os = "windows")]
#[test]
fn working_dir_is_NOT_under_program_files() {
    // The whole point of this refactor: bridge scratch must not live inside
    // the admin-write-only published tree.
    let working = paths::bridge_working_dir().expect("working dir resolves");
    let program_files = std::env::var("ProgramFiles").unwrap_or_default();
    if !program_files.is_empty() {
        assert!(
            !working.starts_with(&program_files),
            "{} accidentally lives under Program Files ({}) — unelevated sync will fail with \
             Access denied",
            working.display(),
            program_files
        );
    }
}
