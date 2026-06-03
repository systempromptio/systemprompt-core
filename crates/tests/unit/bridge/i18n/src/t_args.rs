//! Tests for the fluent-subset string loader (`t` / `t_args`).
//!
//! Concrete message ids are drawn from the embedded en-US catalog
//! (`web/i18n/en-US/bridge.ftl`), which is always present as the compiled-in
//! fallback. Tests avoid any dependency on the `LANG` environment variable so
//! they remain deterministic regardless of the host locale.

use systemprompt_bridge::i18n::{t, t_args};

#[test]
fn t_unknown_id_returns_id_verbatim() {
    let id = "this-id-does-not-exist-in-the-catalog";
    assert_eq!(t(id), id);
}

#[test]
fn t_empty_id_returns_empty() {
    assert_eq!(t(""), "");
}

#[test]
fn t_known_id_returns_catalog_value() {
    assert_eq!(t("ready"), "Ready.");
    assert_eq!(t("setup-heading"), "Welcome to systemprompt bridge");
    assert_eq!(t("sync-button"), "Sync now");
    assert_eq!(t("nav-settings"), "Settings");
}

#[test]
fn t_known_id_with_placeable_returns_raw_template() {
    assert_eq!(t("sync-failure"), "Sync failed: { $error }");
    assert_eq!(t("last-sync"), "Last sync: { $summary }");
}

#[test]
fn t_args_unknown_id_with_no_placeables_is_identity() {
    let id = "no-such-message-id";
    assert_eq!(t_args(id, &[]), id);
    assert_eq!(t_args(id, &[("error", "boom")]), id);
}

#[test]
fn t_args_known_id_without_placeables_returns_plain_value() {
    assert_eq!(t_args("ready", &[]), "Ready.");
    assert_eq!(t_args("sync-button", &[("ignored", "x")]), "Sync now");
}

#[test]
fn t_args_substitutes_single_placeable() {
    assert_eq!(
        t_args("sync-failure", &[("error", "connection reset")]),
        "Sync failed: connection reset"
    );
}

#[test]
fn t_args_substitutes_and_preserves_surrounding_text() {
    assert_eq!(
        t_args("last-sync", &[("summary", "5 minutes ago")]),
        "Last sync: 5 minutes ago"
    );
}

#[test]
fn t_args_unmatched_placeable_is_dropped() {
    // The template references `$error`; supplying a non-matching arg name
    // leaves the placeable unfilled, and an unfilled placeable is removed.
    assert_eq!(
        t_args("sync-failure", &[("wrong-name", "value")]),
        "Sync failed: "
    );
}

#[test]
fn t_args_no_args_drops_placeable() {
    assert_eq!(t_args("sync-failure", &[]), "Sync failed: ");
    assert_eq!(t_args("last-sync", &[]), "Last sync: ");
}

#[test]
fn t_args_multiple_placeables_all_substituted() {
    assert_eq!(
        t_args(
            "sync-gateway-unauthorized",
            &[("status", "401"), ("endpoint", "https://gw.example/api")]
        ),
        "Sync failed: gateway rejected the cached credentials (HTTP 401 from https://gw.example/api). Run `systemprompt-bridge login` with a fresh PAT."
    );
}

#[test]
fn t_args_multiple_placeables_one_unmatched() {
    // Only `status` is supplied; `endpoint` has no matching arg and is dropped.
    assert_eq!(
        t_args("sync-gateway-unauthorized", &[("status", "403")]),
        "Sync failed: gateway rejected the cached credentials (HTTP 403 from ). Run `systemprompt-bridge login` with a fresh PAT."
    );
}

#[test]
fn t_args_arg_order_independent() {
    assert_eq!(
        t_args(
            "sync-gateway-unauthorized",
            &[("endpoint", "https://gw.example/api"), ("status", "401")]
        ),
        "Sync failed: gateway rejected the cached credentials (HTTP 401 from https://gw.example/api). Run `systemprompt-bridge login` with a fresh PAT."
    );
}

#[test]
fn t_args_substituted_value_may_contain_unicode() {
    assert_eq!(
        t_args("agents-status-cloud-signed-in", &[("email", "tëst@exämple.io")]),
        "signed in as tëst@exämple.io"
    );
}

#[test]
fn t_args_preserves_unicode_literal_around_placeable() {
    // `marketplace-detail-copied` carries a non-ASCII checkmark literal with
    // no placeables; substitution must leave it byte-for-byte intact.
    assert_eq!(t_args("marketplace-detail-copied", &[]), "Copied ✓");
}

#[test]
fn t_args_placeable_with_extra_whitespace_matches_trimmed_name() {
    // The proxy-listening template uses `{ $latency }` and `{ $status }` with
    // surrounding spaces inside the braces; the loader trims before matching.
    assert_eq!(
        t_args(
            "agents-status-proxy-listening",
            &[("latency", "12"), ("status", "ok")]
        ),
        "Listening · 12ms · ok"
    );
}

#[test]
fn t_args_empty_args_on_plain_message_is_identity() {
    assert_eq!(t_args("gateway-unreachable", &[]), "offline");
}
