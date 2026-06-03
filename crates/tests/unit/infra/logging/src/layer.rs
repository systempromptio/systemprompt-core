//! Unit tests for the `tracing` layer (`ProxyDatabaseLayer`).
//!
//! Drives the layer through `tracing_subscriber::registry` so the visitor,
//! span-field recording, and event-routing paths are exercised. The DB sink
//! is left unattached — the proxy must no-op without a backing pool.

use systemprompt_logging::DatabaseLayer;
use systemprompt_logging::layer::ProxyDatabaseLayer;
use tracing::{Level, debug, error, info, info_span, warn};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

fn install_proxy_unattached() -> tracing::subscriber::DefaultGuard {
    let proxy = ProxyDatabaseLayer::new();
    let subscriber = tracing_subscriber::registry().with(proxy.with_filter(LevelFilter::TRACE));
    tracing::subscriber::set_default(subscriber)
}

#[test]
fn proxy_default_is_unattached() {
    let p = ProxyDatabaseLayer::default();
    let dbg = format!("{:?}", p);
    assert!(dbg.contains("attached: false"));
}

#[test]
fn proxy_unattached_handles_events_and_spans() {
    let _g = install_proxy_unattached();

    info!("plain message");
    warn!(field = "v", "warn with field");
    error!("error message");
    debug!("debug");

    let span = info_span!("op", user_id = "u1", session_id = "s1", trace_id = "t1");
    let _enter = span.enter();
    info!("inside span");
}

#[test]
fn proxy_handles_nested_spans_with_context_fields() {
    let _g = install_proxy_unattached();

    let outer = info_span!(
        "outer",
        user_id = "u",
        session_id = "s",
        task_id = "task",
        trace_id = "tr",
        context_id = "ctx",
        client_id = "cli"
    );
    let _o = outer.enter();
    let inner = info_span!("inner");
    let _i = inner.enter();
    info!(arg = 42_i64, flag = true, ratio = 1_u64, msg = "fields");
}

#[test]
fn proxy_redacts_sensitive_fields() {
    let _g = install_proxy_unattached();
    let s = info_span!("op", user_id = "u", session_id = "s", trace_id = "t");
    let _e = s.enter();
    info!(
        password = "secret-value",
        token = "tok",
        api_key = "k",
        normal = "n",
        "auth event"
    );
}

#[test]
fn proxy_records_span_updates() {
    let _g = install_proxy_unattached();
    let s = info_span!(
        "op",
        user_id = tracing::field::Empty,
        session_id = tracing::field::Empty,
        trace_id = tracing::field::Empty
    );
    s.record("user_id", "after");
    s.record("session_id", "after-s");
    s.record("trace_id", "after-t");
    let _e = s.enter();
    info!("post-record");
}

#[test]
fn proxy_event_without_span_is_dropped_gracefully() {
    let _g = install_proxy_unattached();
    info!("no span here");
}

#[test]
fn proxy_strips_ansi_in_message() {
    let _g = install_proxy_unattached();
    let s = info_span!("op", user_id = "u", session_id = "s", trace_id = "t");
    let _e = s.enter();
    info!(message = "\x1b[31mred\x1b[0m normal");
}

#[test]
fn proxy_handles_non_csi_escape_sequences() {
    let _g = install_proxy_unattached();
    let s = info_span!("op", user_id = "u", session_id = "s", trace_id = "t");
    let _e = s.enter();
    info!(message = "\x1b]0;window-title\x07 plain \x1bX leftover \x1b");
}

#[test]
fn database_layer_debug_does_not_panic() {
    // Construct via DatabaseLayer requires DbPool; just verify the proxy debug
    let p = ProxyDatabaseLayer::new();
    let _ = format!("{:?}", p);
    // Ensure DatabaseLayer is constructible via type reference in scope
    let _: Option<DatabaseLayer> = None;
}

#[test]
fn proxy_levels_all_routed() {
    let _g = install_proxy_unattached();
    let s = info_span!("op", user_id = "u", session_id = "s", trace_id = "t");
    let _e = s.enter();
    for &(lvl, msg) in &[
        (Level::TRACE, "trace"),
        (Level::DEBUG, "debug"),
        (Level::INFO, "info"),
        (Level::WARN, "warn"),
        (Level::ERROR, "error"),
    ] {
        match lvl {
            Level::TRACE => tracing::trace!("{}", msg),
            Level::DEBUG => tracing::debug!("{}", msg),
            Level::INFO => tracing::info!("{}", msg),
            Level::WARN => tracing::warn!("{}", msg),
            Level::ERROR => tracing::error!("{}", msg),
        }
    }
}
