//! Self-issued device enrolment: the fingerprint is a stable per-user
//! SHA-256, and `enroll_into` persists the gateway's credential while
//! keeping the installation id across a re-enrolment of the same device.

use super::transport::mock_server;
use systemprompt_bridge::feedback::credentials::Enrollment;
use systemprompt_bridge::feedback::enrol::{device_fingerprint, enroll_into};
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::identity::InstallId;
use systemprompt_identifiers::{UserId, ValidatedUrl};

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn enrolment_body(device: &str, consumer: &str, credential: &str) -> String {
    serde_json::json!({
        "device_id": device,
        "consumer_id": consumer,
        "credential": credential,
    })
    .to_string()
}

fn client_for(gateway: &str) -> GatewayClient {
    GatewayClient::new(
        ValidatedUrl::try_new(gateway).expect("mock gateway url"),
        reqwest::Client::new(),
    )
}

#[test]
fn fingerprint_is_stable_lowercase_sha256_and_differs_per_user() {
    let install = InstallId::ephemeral();
    let alice = UserId::new("alice");
    let bob = UserId::new("bob");

    let first = device_fingerprint(&install, &alice);
    assert!(is_lower_hex_64(&first), "{first}");
    assert_eq!(first, device_fingerprint(&install, &alice));
    assert_ne!(first, device_fingerprint(&install, &bob));
    assert_ne!(first, device_fingerprint(&InstallId::ephemeral(), &alice));
}

#[tokio::test]
async fn enrolment_saves_device_json_and_sends_the_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let install = InstallId::ephemeral();
    let user = UserId::new("consumer");
    let (gateway, server) = mock_server(vec![(
        200,
        String::new(),
        enrolment_body("device-1", "consumer", "sp_device_first"),
    )]);
    let client = client_for(&gateway);
    let bearer = BearerToken::new("per-user-bridge-secret");

    let enrollment = enroll_into(dir.path(), &client, &bearer, &install, &user, false)
        .await
        .expect("self enrolment");
    assert_eq!(enrollment.consumer_id, user);
    assert_eq!(enrollment.device_id.as_str(), "device-1");
    assert_eq!(enrollment.credential(), "sp_device_first");

    let saved = Enrollment::load(dir.path(), &gateway).expect("device.json persisted");
    assert_eq!(saved.installation_id, enrollment.installation_id);
    assert_eq!(saved.credential(), "sp_device_first");

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(request.starts_with("POST /v1/bridge/device "), "{request}");
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer per-user-bridge-secret")
    );
    assert!(request.contains(&device_fingerprint(&install, &user)));
}

#[tokio::test]
async fn existing_enrolment_for_the_user_is_reused_without_a_request() {
    let dir = tempfile::tempdir().unwrap();
    let install = InstallId::ephemeral();
    let user = UserId::new("consumer");
    let (gateway, server) = mock_server(vec![(
        200,
        String::new(),
        enrolment_body("device-1", "consumer", "sp_device_first"),
    )]);
    let client = client_for(&gateway);
    let bearer = BearerToken::new("per-user-bridge-secret");

    let first = enroll_into(dir.path(), &client, &bearer, &install, &user, false)
        .await
        .expect("first enrolment");
    let again = enroll_into(dir.path(), &client, &bearer, &install, &user, false)
        .await
        .expect("reuse must not need the gateway");
    assert_eq!(again.installation_id, first.installation_id);
    assert_eq!(again.credential(), first.credential());
    assert_eq!(server.join().unwrap().len(), 1);
}

#[tokio::test]
async fn forced_rotation_keeps_the_installation_id_for_the_same_device() {
    let dir = tempfile::tempdir().unwrap();
    let install = InstallId::ephemeral();
    let user = UserId::new("consumer");
    let (gateway, server) = mock_server(vec![
        (
            200,
            String::new(),
            enrolment_body("device-1", "consumer", "sp_device_first"),
        ),
        (
            200,
            String::new(),
            enrolment_body("device-1", "consumer", "sp_device_second"),
        ),
    ]);
    let client = client_for(&gateway);
    let bearer = BearerToken::new("per-user-bridge-secret");

    let first = enroll_into(dir.path(), &client, &bearer, &install, &user, true)
        .await
        .expect("first enrolment");
    let rotated = enroll_into(dir.path(), &client, &bearer, &install, &user, true)
        .await
        .expect("rotation");
    assert_eq!(rotated.installation_id, first.installation_id);
    assert_eq!(rotated.credential(), "sp_device_second");
    assert_eq!(
        Enrollment::load(dir.path(), &gateway).unwrap().credential(),
        "sp_device_second"
    );
    assert_eq!(server.join().unwrap().len(), 2);
}

#[tokio::test]
async fn a_different_user_re_enrols_with_a_fresh_installation_id() {
    let dir = tempfile::tempdir().unwrap();
    let install = InstallId::ephemeral();
    let (gateway, server) = mock_server(vec![
        (
            200,
            String::new(),
            enrolment_body("device-1", "alice", "sp_device_alice"),
        ),
        (
            200,
            String::new(),
            enrolment_body("device-2", "bob", "sp_device_bob"),
        ),
    ]);
    let client = client_for(&gateway);
    let bearer = BearerToken::new("per-user-bridge-secret");

    let alice = enroll_into(
        dir.path(),
        &client,
        &bearer,
        &install,
        &UserId::new("alice"),
        false,
    )
    .await
    .expect("alice");
    let bob = enroll_into(
        dir.path(),
        &client,
        &bearer,
        &install,
        &UserId::new("bob"),
        false,
    )
    .await
    .expect("bob replaces alice's enrolment");
    assert_ne!(bob.installation_id, alice.installation_id);
    assert_eq!(bob.consumer_id.as_str(), "bob");
    assert_eq!(server.join().unwrap().len(), 2);
}

#[tokio::test]
async fn gateway_refusal_leaves_no_enrolment_behind() {
    let dir = tempfile::tempdir().unwrap();
    let (gateway, server) = mock_server(vec![(409, String::new(), "{}".to_owned())]);
    let client = client_for(&gateway);
    let bearer = BearerToken::new("per-user-bridge-secret");

    let error = enroll_into(
        dir.path(),
        &client,
        &bearer,
        &InstallId::ephemeral(),
        &UserId::new("consumer"),
        false,
    )
    .await
    .expect_err("409 must surface");
    assert!(
        error.to_string().contains("409"),
        "status must be reported: {error}"
    );
    assert!(Enrollment::load(dir.path(), &gateway).is_err());
    server.join().unwrap();
}
