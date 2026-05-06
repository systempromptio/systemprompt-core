//! Tests for the bridge's plugin hook-token cache.
//!
//! Covers in-memory `PluginTokenCache` freshness and eviction. The credentials
//! storage path is split between an on-disk non-secret JSON file and the OS
//! keyring; it is exercised by integration tests against a real keyring rather
//! than here.

use systemprompt_bridge::auth::plugin_oauth::{CachedHookToken, PluginTokenCache};

#[test]
fn cache_returns_fresh_token() {
    let cache = PluginTokenCache::default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = CachedHookToken {
        access_token: "jwt.value".into(),
        expires_at_unix: now + 3600,
    };
    cache.put("plugin-a", token.clone());

    let got = cache.get("plugin-a", 300).expect("token should be fresh");
    assert_eq!(got.access_token, "jwt.value");
}

#[test]
fn cache_drops_token_within_threshold_of_expiry() {
    let cache = PluginTokenCache::default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Expires in 30s, but we ask for tokens with at least 300s lifetime.
    let token = CachedHookToken {
        access_token: "jwt.value".into(),
        expires_at_unix: now + 30,
    };
    cache.put("plugin-a", token);

    assert!(cache.get("plugin-a", 300).is_none());
}

#[test]
fn cache_invalidate_drops_specific_plugin() {
    let cache = PluginTokenCache::default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let make = |name: &str| CachedHookToken {
        access_token: format!("jwt.{name}"),
        expires_at_unix: now + 3600,
    };

    cache.put("plugin-a", make("a"));
    cache.put("plugin-b", make("b"));

    cache.invalidate("plugin-a");

    assert!(cache.get("plugin-a", 60).is_none());
    assert_eq!(
        cache.get("plugin-b", 60).expect("b still cached").access_token,
        "jwt.b"
    );
}

#[test]
fn cache_miss_for_unknown_plugin_id() {
    let cache = PluginTokenCache::default();
    assert!(cache.get("never-cached", 60).is_none());
}
