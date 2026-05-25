use std::time::Duration;

use systemprompt_security::keys::JwksClient;

use crate::support::{JwksMock, TestKey};

fn short_ttl_client(host: &str) -> JwksClient {
    let host = url::Url::parse(host)
        .unwrap()
        .host_str()
        .unwrap()
        .to_string();
    JwksClient::new(vec![host])
        .with_cache_ttl(
            Duration::from_millis(1),
            Duration::from_secs(3600),
            Duration::from_millis(200),
        )
        .with_min_refresh_interval(Duration::ZERO)
}

#[tokio::test]
async fn known_kid_within_ttl_does_not_refetch() {
    let key = TestKey::generate();
    let mock = JwksMock::start(vec![key.jwk()]).await;
    mock.responder.set_cache_control(Some("max-age=60"));
    let client = short_ttl_client(&mock.issuer());

    client.fetch(&mock.issuer(), &key.kid).await.expect("seed");
    client
        .fetch(&mock.issuer(), &key.kid)
        .await
        .expect("cached");
    client
        .fetch(&mock.issuer(), &key.kid)
        .await
        .expect("cached");

    assert_eq!(
        mock.responder.fetches(),
        1,
        "repeated known-kid lookups inside TTL must reuse the cache",
    );
}

#[tokio::test]
async fn expired_ttl_forces_refetch_for_known_kid() {
    let key = TestKey::generate();
    let mock = JwksMock::start(vec![key.jwk()]).await;
    mock.responder.set_cache_control(Some("max-age=1"));
    // Force the cache TTL to a tight ~150ms via the test-only override.
    let client = url::Url::parse(&mock.issuer())
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .map(|host| {
            JwksClient::new(vec![host]).with_cache_ttl(
                Duration::from_millis(150),
                Duration::from_millis(150),
                Duration::from_millis(150),
            )
        })
        .expect("client");

    client.fetch(&mock.issuer(), &key.kid).await.expect("seed");
    assert_eq!(mock.responder.fetches(), 1);

    tokio::time::sleep(Duration::from_millis(80)).await;
    client
        .fetch(&mock.issuer(), &key.kid)
        .await
        .expect("still within TTL");
    assert_eq!(
        mock.responder.fetches(),
        1,
        "fetch at TTL - 70ms must not refetch",
    );

    tokio::time::sleep(Duration::from_millis(120)).await;
    client
        .fetch(&mock.issuer(), &key.kid)
        .await
        .expect("post-expiry fetch succeeds");
    assert_eq!(
        mock.responder.fetches(),
        2,
        "fetch at TTL + 50ms must refetch",
    );
}
