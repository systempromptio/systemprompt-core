use std::time::Duration;

use systemprompt_security::keys::JwksClientError;

use crate::support::{JwksMock, TestKey, client_for_with_min_refresh};

#[tokio::test]
async fn rapid_unknown_kids_do_not_amplify_jwks_fetches() {
    let key = TestKey::generate();
    let mock = JwksMock::start(vec![key.jwk()]).await;
    let client = client_for_with_min_refresh(&mock.issuer(), Duration::from_secs(60));

    client.fetch(&mock.issuer(), &key.kid).await.expect("seed");
    assert_eq!(mock.responder.fetches(), 1, "warm-up fetch");

    for i in 0..50_u32 {
        let attacker_kid = format!("attacker-kid-{i}");
        let err = client.fetch(&mock.issuer(), &attacker_kid).await;
        assert!(
            matches!(err, Err(JwksClientError::KeyNotFound { .. })),
            "expected KeyNotFound for unknown kid, got {err:?}",
        );
    }

    // One refetch is allowed when the unknown-kid throttle is virgin;
    // every subsequent unknown-kid request inside the window must be
    // served from the cache without hitting the network.
    let total = mock.responder.fetches();
    assert!(
        total <= 2,
        "expected <= 2 JWKS fetches under unknown-kid spam, got {total}",
    );
}

#[tokio::test]
async fn dos_guard_allows_refetch_after_window_elapses() {
    let key = TestKey::generate();
    let mock = JwksMock::start(vec![key.jwk()]).await;
    let client =
        client_for_with_min_refresh(&mock.issuer(), Duration::from_millis(150));

    client.fetch(&mock.issuer(), &key.kid).await.expect("seed");

    let _ = client.fetch(&mock.issuer(), "attacker-1").await;
    let fetches_after_first_miss = mock.responder.fetches();
    let _ = client.fetch(&mock.issuer(), "attacker-2").await;
    assert_eq!(
        mock.responder.fetches(),
        fetches_after_first_miss,
        "second miss inside throttle window must not hit network",
    );

    tokio::time::sleep(Duration::from_millis(200)).await;
    let _ = client.fetch(&mock.issuer(), "attacker-3").await;
    assert_eq!(
        mock.responder.fetches(),
        fetches_after_first_miss + 1,
        "miss outside the throttle window is allowed to refetch once",
    );
}
