//! The 0.72 → 0.73 profile rewrite: the stated URL fields collapse into
//! `server.external_url`, the result loads through the 0.73 loader with the
//! same derived values 0.72 stated, and a change the loader would make
//! silently (a new JWT issuer, a different internal URL) is refused.

use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::migrate::{ProfileMigrationError, migrate_profile_layer};

const PROFILE_072: &str = r"name: prod
display_name: Production
site:
  name: Site
database:
  type: postgres
server:
  host: 0.0.0.0
  port: 8080
  # Public URL, as the edge serves it
  api_server_url: https://app.example.com
  api_internal_url: http://localhost:8080
  api_external_url: https://app.example.com/
  use_https: true
  cors_allowed_origins:
    - https://app.example.com
    - https://admin.example.com
  max_concurrent_streams: 256
paths:
  system: .
  services: services
  bin: bin
security:
  jwt_issuer: https://app.example.com
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences: [api]
rate_limits: {}
system_admin:
  username: admin
governance:
  authz:
    hook:
      mode: webhook
      url: http://localhost:8080/api/public/govern/authz
runtime:
  log_level: verbose
";

fn migrated(
    text: &str,
    moved_down: bool,
) -> systemprompt_manifest::profile::migrate::ProfileMigration {
    migrate_profile_layer(text, None, moved_down)
        .expect("the profile migrates")
        .expect("a 0.72 profile has something to migrate")
}

fn load(text: &str) -> Profile {
    let dir = tempfile::tempdir().unwrap();
    let overlay = dir
        .path()
        .join("profiles")
        .join("prod")
        .join("profile.yaml");
    std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
    Profile::from_yaml(text, &overlay).expect("the migrated profile loads through the 0.73 loader")
}

#[test]
fn the_migrated_profile_derives_what_the_old_one_stated() {
    let migration = migrated(PROFILE_072, false);
    let profile = load(&migration.text);

    assert_eq!(profile.server.external_url, "https://app.example.com");
    assert_eq!(profile.server.api_server_url, "https://app.example.com");
    assert_eq!(profile.server.api_external_url, "https://app.example.com");
    assert_eq!(profile.server.api_internal_url, "http://localhost:8080");
    assert!(profile.server.use_https);
    assert_eq!(profile.security.issuer, "https://app.example.com");
    assert_eq!(
        profile.server.cors_allowed_origins,
        vec![
            "https://app.example.com".to_owned(),
            "https://admin.example.com".to_owned()
        ]
    );
    let hook = &profile
        .governance
        .as_ref()
        .unwrap()
        .authz
        .as_ref()
        .unwrap()
        .hook;
    assert_eq!(
        hook.url.as_deref(),
        Some("http://localhost:8080/api/public/govern/authz")
    );
    assert_eq!(
        migration.extra_cors_origins,
        vec!["https://admin.example.com"]
    );
}

#[test]
fn the_rewrite_keeps_comments_and_untouched_keys() {
    let text = migrated(PROFILE_072, false).text;

    assert!(
        text.contains("# Public URL, as the edge serves it"),
        "{text}"
    );
    assert!(text.contains("  max_concurrent_streams: 256"), "{text}");
    assert!(
        text.contains("  jwt_access_token_expiration: 3600"),
        "{text}"
    );
    for gone in [
        "api_server_url",
        "api_internal_url",
        "api_external_url",
        "use_https",
        "jwt_issuer",
        "cors_allowed_origins",
    ] {
        assert!(!text.contains(gone), "{gone} survived:\n{text}");
    }
}

#[test]
fn a_second_run_changes_nothing() {
    let once = migrated(PROFILE_072, false).text;

    assert!(migrate_profile_layer(&once, None, false).unwrap().is_none());
}

#[test]
fn an_issuer_that_differs_from_the_public_url_is_refused() {
    let text = PROFILE_072.replace(
        "jwt_issuer: https://app.example.com",
        "jwt_issuer: https://issuer.example.com",
    );

    let err = migrate_profile_layer(&text, None, false).unwrap_err();

    assert!(
        matches!(err, ProfileMigrationError::IssuerChange { .. }),
        "{err}"
    );
}

#[test]
fn disagreeing_public_urls_are_refused() {
    let text = PROFILE_072.replace(
        "api_server_url: https://app.example.com",
        "api_server_url: https://other.example.com",
    );

    let err = migrate_profile_layer(&text, None, false).unwrap_err();

    assert!(
        matches!(err, ProfileMigrationError::Disagreement { .. }),
        "{err}"
    );
}

#[test]
fn an_internal_url_the_bind_address_does_not_give_is_refused() {
    let text = PROFILE_072.replace(
        "api_internal_url: http://localhost:8080",
        "api_internal_url: http://api.internal:8080",
    );

    let err = migrate_profile_layer(&text, None, false).unwrap_err();

    assert!(
        matches!(err, ProfileMigrationError::InternalUrlChange { .. }),
        "{err}"
    );
}

#[test]
fn an_environment_reference_is_carried_verbatim() {
    let text = PROFILE_072
        .replace(
            "api_server_url: https://app.example.com",
            "api_server_url: ${PUBLIC_URL}",
        )
        .replace(
            "api_external_url: https://app.example.com/",
            "api_external_url: ${PUBLIC_URL}",
        )
        .replace(
            "jwt_issuer: https://app.example.com",
            "jwt_issuer: ${PUBLIC_URL}",
        );

    let migration = migrated(&text, false);

    assert_eq!(migration.external_url.as_deref(), Some("${PUBLIC_URL}"));
    assert!(
        migration.text.contains("external_url: ${PUBLIC_URL}"),
        "{}",
        migration.text
    );
}

#[test]
fn a_profile_moved_one_directory_down_keeps_its_relative_paths() {
    let migration = migrated(PROFILE_072, true);

    assert!(migration.rooted_at_parent);
    let dir = tempfile::tempdir().unwrap();
    let profiles = dir.path().join("profiles");
    let overlay = profiles.join("prod").join("profile.yaml");
    std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
    let profile = Profile::from_yaml(&migration.text, &overlay).unwrap();
    let expected = profiles.join("services");
    assert_eq!(
        std::path::Path::new(&profile.paths.services),
        expected.as_path(),
        "0.72 resolved `services` against the profiles directory"
    );
}

#[test]
fn a_retired_evaluation_section_becomes_judge() {
    let text = format!("{PROFILE_072}evaluation:\n  enabled: false\n");

    let migration = migrated(&text, false);

    assert!(migration.renamed_judge);
    assert!(migration.text.contains("\njudge:\n"), "{}", migration.text);
}

#[test]
fn a_catalog_left_in_the_profile_is_refused_with_the_services_migration() {
    let text = format!("{PROFILE_072}providers: []\n");

    let err = migrate_profile_layer(&text, None, false).unwrap_err();

    assert!(
        matches!(
            err,
            ProfileMigrationError::MovedSection {
                key: "providers",
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_profile_already_in_the_073_shape_has_nothing_to_migrate() {
    let current = "name: prod\nserver:\n  host: 0.0.0.0\n  port: 8080\n  external_url: https://app.example.com\n";

    assert!(
        migrate_profile_layer(current, None, false)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_section_left_empty_is_removed_so_it_cannot_mask_the_base() {
    let overlay = "name: staging\nserver:\n  api_server_url: https://s.example.com\nsecurity:\n  jwt_issuer: https://s.example.com\n";

    let text = migrated(overlay, false).text;

    assert!(
        !text.contains("security"),
        "an empty `security:` would merge as null over the base:\n{text}"
    );
    assert!(
        text.contains("  external_url: https://s.example.com"),
        "{text}"
    );
}
