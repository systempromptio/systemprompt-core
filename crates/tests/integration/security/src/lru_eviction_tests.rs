use systemprompt_security::keys::JwksClient;

use crate::support::{JwksMock, TestKey};

#[tokio::test]
async fn least_recently_used_issuer_jwks_is_evicted_and_refetched() {
    let key_a = TestKey::generate();
    let key_b = TestKey::generate();
    let key_c = TestKey::generate();

    let mock_a = JwksMock::start(vec![key_a.jwk()]).await;
    let mock_b = JwksMock::start(vec![key_b.jwk()]).await;
    let mock_c = JwksMock::start(vec![key_c.jwk()]).await;

    let host_a = url::Url::parse(&mock_a.issuer()).unwrap().host_str().unwrap().to_string();
    let host_b = url::Url::parse(&mock_b.issuer()).unwrap().host_str().unwrap().to_string();
    let host_c = url::Url::parse(&mock_c.issuer()).unwrap().host_str().unwrap().to_string();

    let client = JwksClient::with_capacity(vec![host_a, host_b, host_c], 2);

    client.fetch(&mock_a.issuer(), &key_a.kid).await.expect("a");
    client.fetch(&mock_b.issuer(), &key_b.kid).await.expect("b");
    client.fetch(&mock_c.issuer(), &key_c.kid).await.expect("c evicts a");

    assert_eq!(mock_a.responder.fetches(), 1);
    assert_eq!(mock_b.responder.fetches(), 1);
    assert_eq!(mock_c.responder.fetches(), 1);

    client
        .fetch(&mock_a.issuer(), &key_a.kid)
        .await
        .expect("re-resolves after eviction");
    assert_eq!(
        mock_a.responder.fetches(),
        2,
        "LRU-evicted issuer must be re-fetched on next use",
    );
}

#[tokio::test]
async fn most_recently_used_issuer_survives_eviction_pressure() {
    let key_a = TestKey::generate();
    let key_b = TestKey::generate();
    let key_c = TestKey::generate();

    let mock_a = JwksMock::start(vec![key_a.jwk()]).await;
    let mock_b = JwksMock::start(vec![key_b.jwk()]).await;
    let mock_c = JwksMock::start(vec![key_c.jwk()]).await;

    let host_a = url::Url::parse(&mock_a.issuer()).unwrap().host_str().unwrap().to_string();
    let host_b = url::Url::parse(&mock_b.issuer()).unwrap().host_str().unwrap().to_string();
    let host_c = url::Url::parse(&mock_c.issuer()).unwrap().host_str().unwrap().to_string();

    let client = JwksClient::with_capacity(vec![host_a, host_b, host_c], 2);

    client.fetch(&mock_a.issuer(), &key_a.kid).await.expect("a");
    client.fetch(&mock_b.issuer(), &key_b.kid).await.expect("b");
    // Touch A to make B the LRU entry.
    client.fetch(&mock_a.issuer(), &key_a.kid).await.expect("a hit");
    client.fetch(&mock_c.issuer(), &key_c.kid).await.expect("c evicts b");

    client
        .fetch(&mock_a.issuer(), &key_a.kid)
        .await
        .expect("a still cached");
    assert_eq!(mock_a.responder.fetches(), 1, "A must not have been evicted");
    client
        .fetch(&mock_b.issuer(), &key_b.kid)
        .await
        .expect("b refetched");
    assert_eq!(mock_b.responder.fetches(), 2, "B was the LRU victim");
}
