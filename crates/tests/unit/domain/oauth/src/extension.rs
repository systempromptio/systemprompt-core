use systemprompt_extension::admitted::AdmittedMember;
use systemprompt_extension::{Extension, StandingTag};
use systemprompt_oauth::OauthExtension;
use systemprompt_test_fixtures::admitted_member;

fn admitted() -> AdmittedMember {
    admitted_member(&OauthExtension, StandingTag::Core).expect("the oauth member admits")
}

#[test]
fn metadata_identifies_the_oauth_extension() {
    let meta = OauthExtension.metadata();
    assert_eq!(meta.id, "oauth");
    assert_eq!(meta.name, "OAuth");
    assert!(!meta.version.is_empty());
}

#[test]
fn schemas_cover_client_token_webauthn_and_bridge_tables() {
    let member = admitted();
    let schemas = member.sink.schemas();
    let names: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();

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

    for schema in schemas {
        assert!(
            !schema.sql.trim().is_empty(),
            "schema {} embeds empty SQL",
            schema.table.as_deref().unwrap_or("<table-less>")
        );
    }
}

#[test]
fn migrations_are_discovered_unique_and_ordered() {
    let member = admitted();
    let migrations = member.sink.migrations();
    let versions: Vec<u32> = migrations.iter().map(|m| m.version).collect();
    let mut sorted = versions.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        versions, sorted,
        "migration versions must be unique and ascending"
    );
    for m in migrations {
        assert!(
            !m.sql.trim().is_empty(),
            "migration {} embeds empty SQL",
            m.name
        );
    }
}
