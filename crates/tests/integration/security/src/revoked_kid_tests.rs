use std::time::Duration;

use systemprompt_security::keys::JwksClientError;

use crate::support::{JwksMock, TestKey, client_for_with_min_refresh};

#[tokio::test]
async fn unknown_kid_does_not_amplify_network_load() {
    let seed = TestKey::generate();
    let mock = JwksMock::start(vec![seed.jwk()]).await;
    let client = client_for_with_min_refresh(&mock.issuer(), Duration::from_secs(60));

    client.fetch(&mock.issuer(), &seed.kid).await.expect("seed");
    assert_eq!(mock.responder.fetches(), 1);

    // A kid that is NOT in the cached JWKS — equivalent to "revoked" or
    // never-registered. The first lookup triggers exactly one refetch; the
    // throttle then coalesces every subsequent lookup until the window
    // elapses.
    let absent_kid = "absent-or-revoked-kid";

    let first = client.fetch(&mock.issuer(), absent_kid).await;
    assert!(
        matches!(
            first,
            Err(JwksClientError::KeyNotFound { ref kid, .. }) if kid == absent_kid
        ),
        "expected KeyNotFound on first miss, got {first:?}",
    );
    assert_eq!(
        mock.responder.fetches(),
        2,
        "first kid-miss triggers exactly one refetch",
    );

    for _ in 0..5 {
        let err = client.fetch(&mock.issuer(), absent_kid).await;
        assert!(
            matches!(err, Err(JwksClientError::KeyNotFound { .. })),
            "expected KeyNotFound on repeated miss, got {err:?}",
        );
    }
    assert_eq!(
        mock.responder.fetches(),
        2,
        "repeated unknown-kid lookups must not amplify network load",
    );
}
