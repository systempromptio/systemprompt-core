use std::path::Path;

use systemprompt_oauth_issuance::IdentityPlane;
use systemprompt_provider_contracts::{ChangeSource, ConfigPlane, ConfigPlaneError};

const USERS_YAML: &str = r"
users:
  - email: Alice@Example.com
    name: Alice
    roles: [user, admin, user]
  - email: robot@example.com
    name: Robot
    kind: service
";

const CLIENTS_YAML: &str = r"
oauth_clients:
  - client_id: portal
    name: Portal
    owner: alice@example.com
    redirect_uris:
      - https://portal.example.com/cb
      - https://b.example.com/cb
      - https://a.example.com/cb
    scopes: [write, read]
    secret_key: portal_client_secret
";

fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("identity")).expect("identity dir");
    for (name, body) in files {
        std::fs::write(dir.path().join("identity").join(name), body).expect("write");
    }
    dir
}

fn invalid_message(root: &Path) -> String {
    match IdentityPlane.load_declared(root) {
        Err(ConfigPlaneError::Invalid { message, .. }) => message,
        other => panic!("expected an invalid declaration, got {other:?}"),
    }
}

#[test]
fn absent_identity_dir_is_undeclared() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        IdentityPlane
            .load_declared(dir.path())
            .expect("load")
            .is_none()
    );
}

#[test]
fn entities_are_keyed_and_canonical() {
    let dir = tree(&[("users.yaml", USERS_YAML), ("clients.yaml", CLIENTS_YAML)]);
    let declared = IdentityPlane
        .load_declared(dir.path())
        .expect("load")
        .expect("declared");
    let keys: Vec<&str> = declared.entries.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "oauth_client:portal",
            "user:alice@example.com",
            "user:robot@example.com"
        ]
    );
    let alice: serde_json::Value = serde_json::from_str(
        declared
            .entries
            .get("user:alice@example.com")
            .expect("user:alice@example.com declared"),
    )
    .expect("json");
    assert_eq!(alice["roles"], serde_json::json!(["admin", "user"]));
    assert_eq!(alice["kind"], "person");
    assert_eq!(alice["status"], "active");
    let robot: serde_json::Value = serde_json::from_str(
        declared
            .entries
            .get("user:robot@example.com")
            .expect("user:robot@example.com declared"),
    )
    .expect("json");
    assert_eq!(robot["kind"], "service");
    assert_eq!(robot["roles"], serde_json::json!(["user"]));
    let portal: serde_json::Value = serde_json::from_str(
        declared
            .entries
            .get("oauth_client:portal")
            .expect("oauth_client:portal declared"),
    )
    .expect("json");
    assert_eq!(
        portal["redirect_uris"],
        serde_json::json!([
            "https://portal.example.com/cb",
            "https://a.example.com/cb",
            "https://b.example.com/cb"
        ])
    );
    assert_eq!(portal["secret_key"], "portal_client_secret");
    assert_eq!(
        portal["grant_types"],
        serde_json::json!(["authorization_code", "refresh_token"])
    );
}

#[test]
fn formatting_and_order_do_not_change_the_hash() {
    let a = tree(&[("users.yaml", USERS_YAML)]);
    let b = tree(&[(
        "users.yaml",
        "users:\n- {email: robot@example.com, name: Robot, kind: service, roles: [user]}\n- \
         {email: alice@example.com, name: Alice, roles: [admin, user], status: active}\n",
    )]);
    let ha = IdentityPlane
        .load_declared(a.path())
        .expect("a")
        .expect("a");
    let hb = IdentityPlane
        .load_declared(b.path())
        .expect("b")
        .expect("b");
    assert_eq!(ha.hash, hb.hash);
}

#[test]
fn each_entity_is_attributed_to_its_file() {
    let dir = tree(&[("users.yaml", USERS_YAML), ("clients.yaml", CLIENTS_YAML)]);
    let sources = IdentityPlane
        .sources(dir.path())
        .expect("sources")
        .expect("declared");
    assert_eq!(
        sources["user:alice@example.com"].source,
        ChangeSource::Code("identity/users.yaml".to_owned())
    );
    assert_eq!(
        sources["oauth_client:portal"].source,
        ChangeSource::Code("identity/clients.yaml".to_owned())
    );
}

#[test]
fn an_entity_declared_twice_is_refused() {
    let dir = tree(&[("a.yaml", USERS_YAML), ("b.yaml", USERS_YAML)]);
    assert!(invalid_message(dir.path()).contains("declared more than once"));
}

#[test]
fn a_deleted_status_cannot_be_declared() {
    let dir = tree(&[(
        "users.yaml",
        "users:\n- {email: a@example.com, name: A, status: deleted}\n",
    )]);
    assert!(invalid_message(dir.path()).contains("status"));
}

#[test]
fn an_inline_secret_is_refused() {
    let dir = tree(&[(
        "clients.yaml",
        "oauth_clients:\n- {client_id: c, name: C, owner: a@example.com, client_secret: hunter2}\n",
    )]);
    assert!(IdentityPlane.load_declared(dir.path()).is_err());
}

#[test]
fn an_undeclarable_grant_type_is_refused() {
    let dir = tree(&[(
        "clients.yaml",
        "oauth_clients:\n- {client_id: c, name: C, owner: a@example.com, grant_types: [password]}\n",
    )]);
    assert!(invalid_message(dir.path()).contains("grant type"));
}

#[test]
fn rendered_declarations_load_back_to_the_same_entities() {
    let dir = tree(&[("users.yaml", USERS_YAML), ("clients.yaml", CLIENTS_YAML)]);
    let declared = IdentityPlane
        .load_declared(dir.path())
        .expect("load")
        .expect("declared");
    let rendered = IdentityPlane
        .render_declarations(&declared.entries)
        .expect("render");
    let exported = tree(&[("exported.yaml", rendered.as_str())]);
    let reloaded = IdentityPlane
        .load_declared(exported.path())
        .expect("reload")
        .expect("declared");
    assert_eq!(reloaded.entries, declared.entries);
    assert_eq!(reloaded.hash, declared.hash);
}

fn person_ui_client(
    id: &str,
) -> systemprompt_oauth_issuance::identity::declaration::DeclaredClient {
    systemprompt_oauth_issuance::identity::declaration::DeclaredClient {
        client_id: systemprompt_identifiers::ClientId::new(id),
        name: "Desk".to_owned(),
        owner: "ops@example.com".to_owned(),
        redirect_uris: vec![
            "http://127.0.0.1:53682/callback".to_owned(),
            "com.example.desk:/oauth".to_owned(),
        ],
        grant_types: vec!["authorization_code".to_owned(), "refresh_token".to_owned()],
        scopes: vec!["user".to_owned()],
        secret_key: None,
        active: true,
        person_ui: true,
    }
}

#[test]
fn a_public_native_person_ui_client_is_declarable() {
    person_ui_client("desk_ui").validate().expect("valid");
}

#[test]
fn person_ui_is_refused_on_a_platform_client() {
    for id in ["sp_web", "sp_bridge", "sp_cli", "sys_agent"] {
        assert!(person_ui_client(id).validate().is_err(), "{id}");
    }
}

#[test]
fn a_person_ui_client_is_public_and_code_only() {
    let mut secret = person_ui_client("desk_ui");
    secret.secret_key = Some("desk_secret".to_owned());
    assert!(secret.validate().is_err());
    let mut machine = person_ui_client("desk_ui");
    machine.grant_types.push("client_credentials".to_owned());
    assert!(machine.validate().is_err());
    let mut refresh_only = person_ui_client("desk_ui");
    refresh_only.grant_types = vec!["refresh_token".to_owned()];
    assert!(refresh_only.validate().is_err());
}

#[test]
fn a_person_ui_client_redirects_only_to_loopback_or_a_private_scheme() {
    for uri in [
        "https://desk.example.com/cb",
        "http://localhost:8080/cb",
        "http://192.168.1.4:8080/cb",
        "http://127.0.0.1:8080/cb#fragment",
        "desk:/cb",
    ] {
        let mut client = person_ui_client("desk_ui");
        client.redirect_uris = vec![uri.to_owned()];
        assert!(client.validate().is_err(), "{uri}");
    }
    let mut none = person_ui_client("desk_ui");
    none.redirect_uris.clear();
    assert!(none.validate().is_err());
}

#[test]
fn a_client_that_is_not_person_ui_keeps_its_canonical_value() {
    let mut client = person_ui_client("portal");
    client.person_ui = false;
    let value = serde_json::to_string(&client.value()).expect("encode");
    assert!(!value.contains("person_ui"));
}
