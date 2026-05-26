//! Extra `SyncApiClient` builders not covered by `api_client.rs`.

use systemprompt_sync::SyncApiClient;

#[test]
fn with_direct_sync_origin_sets_origin() {
    let c = SyncApiClient::new("https://api.example.com", "tok")
        .expect("client")
        .with_direct_sync_origin(Some("https://app.example.com".to_owned()));
    let dbg = format!("{c:?}");
    assert!(dbg.contains("app.example.com"));
}

#[test]
fn with_direct_sync_origin_none_resets() {
    let c = SyncApiClient::new("https://api.example.com", "tok")
        .expect("client")
        .with_direct_sync(Some("first".to_owned()))
        .with_direct_sync_origin(None);
    let dbg = format!("{c:?}");
    assert!(dbg.contains("None") || !dbg.contains("first"));
}
