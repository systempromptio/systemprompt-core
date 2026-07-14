// OauthExtension registration surface: metadata, schema set, dependencies,
// and build-script-discovered migrations.

use systemprompt_extension::Extension;
use systemprompt_oauth::OauthExtension;

#[test]
fn metadata_identifies_the_oauth_extension() {
    let meta = OauthExtension.metadata();
    assert_eq!(meta.id, "oauth");
    assert_eq!(meta.name, "OAuth");
    assert!(!meta.version.is_empty());
}

#[test]
fn schemas_cover_client_token_webauthn_and_bridge_tables() {
    let schemas = OauthExtension.schemas();
    let names: Vec<&str> = schemas.iter().map(|s| s.table.as_str()).collect();

    for expected in [
        "oauth_clients",
        "oauth_client_redirect_uris",
        "oauth_client_grant_types",
        "oauth_client_response_types",
        "oauth_client_scopes",
        "webauthn_credentials",
        "webauthn_setup_tokens",
    ] {
        assert!(names.contains(&expected), "missing schema {expected}");
    }

    for schema in &schemas {
        assert!(
            !schema.sql.trim().is_empty(),
            "schema {} embeds empty SQL",
            schema.table
        );
    }
}

#[test]
fn depends_on_users_extension() {
    assert_eq!(OauthExtension.dependencies(), vec!["users"]);
}

#[test]
fn migrations_are_discovered_unique_and_ordered() {
    let migrations = OauthExtension.migrations();
    let versions: Vec<u32> = migrations.iter().map(|m| m.version).collect();
    let mut sorted = versions.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        versions, sorted,
        "migration versions must be unique and ascending"
    );
    for m in &migrations {
        assert!(
            !m.sql.trim().is_empty(),
            "migration {} embeds empty SQL",
            m.name
        );
    }
}
