use crate::support::{JwksMock, TestKey, client_for};

#[tokio::test]
async fn old_kid_resolves_when_jwks_contains_both_during_rotation() {
    let old = TestKey::generate();
    let new = TestKey::generate();
    assert_ne!(old.kid, new.kid, "rsa kids must differ");

    let mock = JwksMock::start(vec![old.jwk(), new.jwk()]).await;
    let client = client_for(&mock.issuer());

    let resolved_old = client
        .fetch(&mock.issuer(), &old.kid)
        .await
        .expect("old kid resolves");
    assert_eq!(resolved_old.kid, old.kid);

    let resolved_new = client
        .fetch(&mock.issuer(), &new.kid)
        .await
        .expect("new kid resolves from same cached document");
    assert_eq!(resolved_new.kid, new.kid);

    assert_eq!(
        mock.responder.fetches(),
        1,
        "second lookup must hit the cache, not the network"
    );
}

#[tokio::test]
async fn unknown_kid_triggers_refetch_and_resolves_after_rotation() {
    let old = TestKey::generate();
    let mock = JwksMock::start(vec![old.jwk()]).await;
    let client = client_for(&mock.issuer());

    client
        .fetch(&mock.issuer(), &old.kid)
        .await
        .expect("seed the cache with old jwks");
    assert_eq!(mock.responder.fetches(), 1);

    let new = TestKey::generate();
    mock.responder.set_keys(vec![old.jwk(), new.jwk()]);

    let resolved = client
        .fetch(&mock.issuer(), &new.kid)
        .await
        .expect("unknown kid forces refetch");
    assert_eq!(resolved.kid, new.kid);
    assert_eq!(
        mock.responder.fetches(),
        2,
        "unknown kid must trigger exactly one refetch"
    );
}
