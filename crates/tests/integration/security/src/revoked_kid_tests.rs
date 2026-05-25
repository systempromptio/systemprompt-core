use std::time::Duration;

use systemprompt_security::keys::JwksClientError;

use crate::support::{JwksMock, TestKey, client_for_with_min_refresh};

#[tokio::test]
async fn revoked_kid_returns_key_not_found_without_looping() {
    let key = TestKey::generate();
    let mock = JwksMock::start(vec![key.jwk()]).await;
    let client = client_for_with_min_refresh(&mock.issuer(), Duration::from_secs(60));

    client.fetch(&mock.issuer(), &key.kid).await.expect("seed");
    assert_eq!(mock.responder.fetches(), 1);

    // Issuer revokes the key.
    mock.responder.set_keys(vec![]);

    let first = client.fetch(&mock.issuer(), &key.kid).await;
    assert!(
        matches!(
            first,
            Err(JwksClientError::KeyNotFound { ref kid, .. }) if kid == &key.kid
        ),
        "expected KeyNotFound on first miss, got {first:?}",
    );
    assert_eq!(
        mock.responder.fetches(),
        2,
        "first kid-miss after revocation triggers exactly one refetch",
    );

    // Subsequent calls must NOT trigger further refetches — the
    // throttle prevents an infinite refetch loop while still returning
    // a clear error.
    for _ in 0..5 {
        let err = client.fetch(&mock.issuer(), &key.kid).await;
        assert!(
            matches!(err, Err(JwksClientError::KeyNotFound { .. })),
            "expected KeyNotFound on repeated miss, got {err:?}",
        );
    }
    assert_eq!(
        mock.responder.fetches(),
        2,
        "repeated revoked-kid lookups must not amplify network load",
    );
}
