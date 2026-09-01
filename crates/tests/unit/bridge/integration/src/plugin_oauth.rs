//! Tests for the bridge's plugin hook-token cache.
//!
//! Covers in-memory `PluginTokenCache` freshness, eviction, and gateway
//! isolation. The credentials storage path is split between an on-disk
//! non-secret JSON file and the OS keyring; it is exercised by integration
//! tests against a real keyring rather than here.

use systemprompt_bridge::auth::plugin_oauth::{CachedHookToken, PluginTokenCache};
use systemprompt_identifiers::PluginId;

const LOCAL: &str = "http://localhost:8081";
const PROD: &str = "https://internal.systemprompt.io";

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn token(value: &str, lifetime_secs: u64) -> CachedHookToken {
    CachedHookToken {
        access_token: value.into(),
        expires_at_unix: now() + lifetime_secs,
    }
}

#[test]
fn cache_returns_fresh_token() {
    let cache = PluginTokenCache::default();
    cache.put(LOCAL, &PluginId::new("plugin-a"), token("jwt.value", 3600));

    let got = cache
        .get(LOCAL, &PluginId::new("plugin-a"), 300)
        .expect("token should be fresh");
    assert_eq!(got.access_token, "jwt.value");
}

#[test]
fn cache_drops_token_within_threshold_of_expiry() {
    let cache = PluginTokenCache::default();
    // Expires in 30s, but we ask for tokens with at least 300s lifetime.
    cache.put(LOCAL, &PluginId::new("plugin-a"), token("jwt.value", 30));

    assert!(cache.get(LOCAL, &PluginId::new("plugin-a"), 300).is_none());
}

#[test]
fn cache_invalidate_drops_specific_plugin() {
    let cache = PluginTokenCache::default();

    cache.put(LOCAL, &PluginId::new("plugin-a"), token("jwt.a", 3600));
    cache.put(LOCAL, &PluginId::new("plugin-b"), token("jwt.b", 3600));

    cache.invalidate(LOCAL, &PluginId::new("plugin-a"));

    assert!(cache.get(LOCAL, &PluginId::new("plugin-a"), 60).is_none());
    assert_eq!(
        cache
            .get(LOCAL, &PluginId::new("plugin-b"), 60)
            .expect("b still cached")
            .access_token,
        "jwt.b"
    );
}

#[test]
fn cache_miss_for_unknown_plugin_id() {
    let cache = PluginTokenCache::default();
    assert!(
        cache
            .get(LOCAL, &PluginId::new("never-cached"), 60)
            .is_none()
    );
}

// Why: a hook token is signed by one gateway's RSA authority and is meaningless
// to any other. While the cache was keyed on the plugin alone, repointing the
// bridge from production to a local server handed the still-fresh production
// token to the local governance webhook, which rejected it as an unknown
// signing key and blocked every tool call.
#[test]
fn a_token_minted_for_one_gateway_is_never_served_to_another() {
    let cache = PluginTokenCache::default();
    let plugin = PluginId::new("systemprompt-admin");

    cache.put(PROD, &plugin, token("jwt.prod", 3600));

    assert!(
        cache.get(LOCAL, &plugin, 60).is_none(),
        "the local gateway must mint its own token, not inherit production's"
    );
    assert_eq!(
        cache
            .get(PROD, &plugin, 60)
            .expect("production's own token is still valid for production")
            .access_token,
        "jwt.prod"
    );
}

#[test]
fn each_gateway_keeps_its_own_token_for_the_same_plugin() {
    let cache = PluginTokenCache::default();
    let plugin = PluginId::new("systemprompt-admin");

    cache.put(PROD, &plugin, token("jwt.prod", 3600));
    cache.put(LOCAL, &plugin, token("jwt.local", 3600));

    assert_eq!(
        cache.get(PROD, &plugin, 60).expect("prod").access_token,
        "jwt.prod"
    );
    assert_eq!(
        cache.get(LOCAL, &plugin, 60).expect("local").access_token,
        "jwt.local"
    );
}

// Why: evicting a token the upstream refused must not take out the other
// gateway's working token for the same plugin.
#[test]
fn invalidating_one_gateways_token_leaves_the_others_intact() {
    let cache = PluginTokenCache::default();
    let plugin = PluginId::new("systemprompt-admin");

    cache.put(PROD, &plugin, token("jwt.prod", 3600));
    cache.put(LOCAL, &plugin, token("jwt.local", 3600));

    cache.invalidate(LOCAL, &plugin);

    assert!(cache.get(LOCAL, &plugin, 60).is_none());
    assert!(cache.get(PROD, &plugin, 60).is_some());
}

// Why: when the plugin is uninstalled the token is worthless everywhere, so
// removal spans gateways — unlike a stale token, which is one gateway's
// problem.
#[test]
fn removing_a_plugin_drops_its_token_on_every_gateway() {
    let cache = PluginTokenCache::default();
    let gone = PluginId::new("removed-plugin");
    let kept = PluginId::new("kept-plugin");

    cache.put(PROD, &gone, token("jwt.prod", 3600));
    cache.put(LOCAL, &gone, token("jwt.local", 3600));
    cache.put(LOCAL, &kept, token("jwt.kept", 3600));

    cache.invalidate_plugin(&gone);

    assert!(cache.get(PROD, &gone, 60).is_none());
    assert!(cache.get(LOCAL, &gone, 60).is_none());
    assert!(
        cache.get(LOCAL, &kept, 60).is_some(),
        "an unrelated plugin must survive"
    );
}

// Why: the key is built by concatenation, so a plugin id that happens to end
// with another's name, or a gateway that is a prefix of another, must not be
// able to collide or to be caught by the suffix match in `invalidate_plugin`.
#[test]
fn plugin_and_gateway_names_cannot_collide_across_keys() {
    let cache = PluginTokenCache::default();
    let short = PluginId::new("admin");
    let long = PluginId::new("systemprompt-admin");

    cache.put(LOCAL, &short, token("jwt.short", 3600));
    cache.put(LOCAL, &long, token("jwt.long", 3600));

    cache.invalidate_plugin(&short);

    assert!(cache.get(LOCAL, &short, 60).is_none());
    assert_eq!(
        cache
            .get(LOCAL, &long, 60)
            .expect("a plugin whose id merely ends with the removed one must survive")
            .access_token,
        "jwt.long"
    );
}
