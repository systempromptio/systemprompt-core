use std::path::Path;
use std::sync::Arc;

use systemprompt_notifications::channels::push::{PushError, PushParts, push_channels};
use systemprompt_notifications::plane::push::{
    ApnsEnvironment, PushDeclaration, PushDeclarationError, PushSecretReferences,
};
use systemprompt_traits::SecretReferences;

use crate::push_support::{StubDirectory, StubHttp, StubPresence, StubSecrets};

const DECLARATION: &str = "\
apns:
  key_id: ABC123DEFG
  team_id: TEAM123456
  topic: io.example.app
  environment: sandbox
  signing_key_secret: apns_signing_key
fcm:
  service_account_secret: fcm_service_account
";

const SERVICE_ACCOUNT: &str = r#"{
  "type": "service_account",
  "client_email": "push@example-project.iam.gserviceaccount.com",
  "private_key": "held by the secret store",
  "project_id": "example-project"
}"#;

fn services_with(push: Option<&str>) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temp dir");
    if let Some(text) = push {
        let dir = root.path().join("notifications");
        std::fs::create_dir_all(&dir).expect("notifications dir");
        std::fs::write(dir.join("push.yaml"), text).expect("push.yaml");
    }
    root
}

fn load(root: &Path) -> PushDeclaration {
    PushDeclaration::load(root)
        .expect("the declaration parses")
        .expect("the declaration is present")
}

fn parts(secrets: &[(&str, &str)]) -> PushParts {
    PushParts {
        secrets: StubSecrets::holding(secrets),
        directory: StubDirectory::holding(Vec::new()),
        presence: Arc::new(StubPresence(false)),
        http: StubHttp::answering(200, ""),
    }
}

#[test]
fn an_instance_with_no_push_file_declares_no_provider() {
    let root = services_with(None);
    assert_eq!(PushDeclaration::load(root.path()).expect("absent"), None);
}

#[test]
fn the_declaration_names_identifiers_and_secret_names() {
    let root = services_with(Some(DECLARATION));
    let declaration = load(root.path());

    let apns = declaration.apns.as_ref().expect("apns");
    assert_eq!(apns.key_id.as_str(), "ABC123DEFG");
    assert_eq!(apns.team_id.as_str(), "TEAM123456");
    assert_eq!(apns.topic.as_str(), "io.example.app");
    assert_eq!(apns.environment, ApnsEnvironment::Sandbox);
    assert_eq!(
        apns.environment.origin(),
        "https://api.sandbox.push.apple.com"
    );
    let names: Vec<&str> = declaration.secret_names().map(|n| n.as_str()).collect();
    assert_eq!(names, ["apns_signing_key", "fcm_service_account"]);
}

#[test]
fn a_malformed_declaration_is_refused() {
    for text in [
        "apns:\n  key_id: short\n  team_id: TEAM123456\n  topic: io.example.app\n  environment: \
         sandbox\n  signing_key_secret: k\n",
        "fcm:\n  service_account_secret: s\n  private_key: inline\n",
        "apns:\n  key_id: ABC123DEFG\n  team_id: TEAM123456\n  topic: io.example.app\n  \
         signing_key_secret: k\n",
    ] {
        let root = services_with(Some(text));
        assert!(matches!(
            PushDeclaration::load(root.path()),
            Err(PushDeclarationError::Parse { .. })
        ));
    }
}

#[test]
fn no_declared_provider_contributes_no_channel() {
    let channels =
        push_channels(&PushDeclaration::default(), &parts(&[])).expect("nothing to build");
    assert!(channels.is_empty());
}

#[test]
fn a_declared_provider_without_its_secret_contributes_no_channel() {
    let root = services_with(Some(DECLARATION));
    let channels = push_channels(&load(root.path()), &parts(&[])).expect("absence is not an error");
    assert!(channels.is_empty());
}

#[test]
fn a_provider_with_its_credential_contributes_its_channel() {
    let root = services_with(Some(DECLARATION));
    let channels = push_channels(
        &load(root.path()),
        &parts(&[("fcm_service_account", SERVICE_ACCOUNT)]),
    )
    .expect("the Google channel builds");

    let surfaces: Vec<&str> = channels.iter().map(|c| c.surface()).collect();
    assert_eq!(surfaces, ["fcm"]);
}

#[test]
fn an_unusable_credential_refuses_the_build() {
    let root = services_with(Some(DECLARATION));
    let declaration = load(root.path());

    let apns = push_channels(
        &declaration,
        &parts(&[("apns_signing_key", "not a signing key")]),
    );
    assert!(matches!(apns, Err(PushError::CredentialInvalid { .. })));

    let fcm = push_channels(&declaration, &parts(&[("fcm_service_account", "{}")]));
    assert!(matches!(fcm, Err(PushError::CredentialInvalid { .. })));
}

#[test]
fn a_credential_error_never_shows_the_secret_value() {
    let root = services_with(Some(DECLARATION));
    let Err(error) = push_channels(
        &load(root.path()),
        &parts(&[("apns_signing_key", "PRIVATE-KEY-MATERIAL")]),
    ) else {
        panic!("the key is unusable");
    };
    assert!(!format!("{error} {error:?}").contains("PRIVATE-KEY-MATERIAL"));
}

#[test]
fn the_declared_secret_names_are_referenced() {
    let root = services_with(Some(DECLARATION));
    let references = PushSecretReferences::over(root.path().to_path_buf());

    assert!(references.references("apns_signing_key"));
    assert!(references.references("FCM_SERVICE_ACCOUNT"));
    assert!(!references.references("unrelated_secret"));
    assert!(!PushSecretReferences::none().references("apns_signing_key"));
}

#[test]
fn an_unreadable_declaration_references_every_name() {
    let root = services_with(Some("apns: [not, a, mapping]"));
    let references = PushSecretReferences::over(root.path().to_path_buf());
    assert!(references.references("anything"));
}
