//! `config migrate --profiles <dir>` rewrites a directory of pre-0.73 profiles:
//! a legacy `<name>.secrets.profile.yaml` moves to `<name>/profile.yaml`, an
//! overlay and the shared base are rewritten in place, every result loads
//! through the 0.73 loader, and a directory with one profile that cannot be
//! migrated is left exactly as it was.

use std::path::Path;

use systemprompt_cli::commands::admin::config::migrate::profiles::{apply, describe, plan};
use systemprompt_manifest::Profile;

const BASE_072: &str = r"site:
  name: Site
database:
  type: postgres
server:
  host: 0.0.0.0
  port: 8080
  cors_allowed_origins:
    - http://localhost:5173
paths:
  system: .
  services: services
  bin: bin
security:
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences: [api]
rate_limits: {}
system_admin:
  username: admin
runtime:
  log_level: verbose
";

fn overlay_072(name: &str, url: &str) -> String {
    format!(
        "name: {name}\ndisplay_name: {name}\nserver:\n  # the public edge\n  api_server_url: {url}\n  api_external_url: {url}\n  use_https: true\nsecurity:\n  jwt_issuer: {url}\n"
    )
}

fn standalone_072(name: &str, url: &str) -> String {
    let server = format!(
        "server:\n  # the public edge\n  api_server_url: {url}\n  api_external_url: {url}\n  use_https: true\n"
    );
    let body = BASE_072.replacen("server:\n", &server, 1);
    format!("name: {name}\ndisplay_name: {name}\n{body}").replacen(
        "security:\n",
        &format!("security:\n  jwt_issuer: {url}\n"),
        1,
    )
}

fn tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let profiles = dir.path().join("profiles");
    std::fs::create_dir_all(profiles.join("staging")).expect("mkdir");
    std::fs::write(profiles.join("base.yaml"), BASE_072).expect("base");
    std::fs::write(
        profiles.join("prod.secrets.profile.yaml"),
        standalone_072("prod", "https://app.example.com"),
    )
    .expect("legacy profile");
    std::fs::write(
        profiles.join("staging").join("profile.yaml"),
        overlay_072("staging", "https://staging.example.com"),
    )
    .expect("overlay");
    dir
}

fn profiles(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("profiles")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read")
}

#[test]
fn a_directory_of_072_profiles_migrates_and_loads() {
    let tree = tree();
    let root = profiles(&tree);

    let pending = plan(&root).expect("plan");
    assert_eq!(pending.len(), 3, "base, legacy and overlay are pending");
    let report = describe(&pending);
    assert!(
        report.contains("now loads over base.yaml"),
        "the legacy profile joining the base is reported:\n{report}"
    );
    apply(&root).expect("apply");

    assert!(
        !root.join("prod.secrets.profile.yaml").exists(),
        "the legacy file moved"
    );
    let prod = Profile::load_layered(&root, "prod").expect("prod loads");
    assert_eq!(prod.server.external_url, "https://app.example.com");
    assert_eq!(prod.security.issuer, "https://app.example.com");
    assert_eq!(
        Path::new(&prod.paths.services),
        root.join("services").as_path(),
        "the moved profile still resolves services where 0.72 did"
    );
    let staging = Profile::load_layered(&root, "staging").expect("staging loads");
    assert_eq!(staging.server.external_url, "https://staging.example.com");
    assert!(
        staging
            .server
            .cors_allowed_origins
            .contains(&"http://localhost:5173".to_owned()),
        "the base's extra origin is kept"
    );
    assert!(read(&root.join("prod").join("profile.yaml")).contains("# the public edge"));
    assert!(
        plan(&root).expect("second plan").is_empty(),
        "a second run changes nothing"
    );
}

#[test]
fn one_profile_that_cannot_migrate_leaves_the_directory_untouched() {
    let tree = tree();
    let root = profiles(&tree);
    let bad = root.join("drift.secrets.profile.yaml");
    std::fs::write(
        &bad,
        "name: drift\nserver:\n  api_server_url: https://a.example.com\n  api_external_url: https://b.example.com\n",
    )
    .expect("bad overlay");
    let before_base = read(&root.join("base.yaml"));

    let err = apply(&root).expect_err("disagreeing URLs stop the migration");

    assert!(format!("{err:#}").contains("api_server_url"), "{err:#}");
    assert_eq!(
        read(&root.join("base.yaml")),
        before_base,
        "the base was not rewritten"
    );
    assert!(
        root.join("prod.secrets.profile.yaml").exists(),
        "nothing moved"
    );
    assert!(bad.exists());
}

#[test]
fn a_legacy_file_beside_its_new_location_is_refused() {
    let tree = tree();
    let root = profiles(&tree);
    std::fs::create_dir_all(root.join("prod")).expect("mkdir");
    std::fs::write(root.join("prod").join("profile.yaml"), "name: prod\n").expect("clash");

    let err = plan(&root).expect_err("two copies of one profile");

    assert!(format!("{err:#}").contains("both exist"), "{err:#}");
}
