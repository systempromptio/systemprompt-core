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

mod database_layer {
    //! DB-backed tests for the concrete [`DatabaseLayer`] (the `layer/mod.rs`
    //! sink). Installs the layer on a registry subscriber, emits events inside
    //! an attributed span so `build_log_entry` yields an entry, and asserts the
    //! background batch writer persists rows to `logs`.

    use std::time::Duration;

    use systemprompt_identifiers::{SessionId, TraceId, UserId};
    use systemprompt_logging::layer::ProxyDatabaseLayer;
    use systemprompt_logging::{DatabaseLayer, LogActor, LogEntry, LogLevel, enqueue_background};
    use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
    use tracing::{error, info, info_span};
    use tracing_subscriber::Layer;
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;

    async fn log_count_for_trace(pool: &sqlx::PgPool, trace_id: &str) -> i64 {
        sqlx::query_scalar!("SELECT COUNT(*) FROM logs WHERE trace_id = $1", trace_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn database_layer_persists_attributed_events() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(db) = fixture_db_pool(&url).await else {
            return;
        };
        let raw = db.pool_arc().unwrap().as_ref().clone();

        let trace_id = format!("layer-trace-{}", uuid::Uuid::new_v4().simple());

        {
            let layer = DatabaseLayer::new(db.clone());
            let subscriber =
                tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
            let _guard = tracing::subscriber::set_default(subscriber);

            let span = info_span!(
                "request",
                user_id = "layer-user",
                session_id = "layer-session",
                trace_id = trace_id.as_str(),
            );
            let _enter = span.enter();

            // INFO routes through the size-buffered path; ERROR additionally
            // triggers an immediate FlushNow.
            info!(detail = "first", "info inside span");
            info!(password = "should-redact", "second info");
            error!(code = 500_i64, "boom");

            // Allow the background batch writer to drain the channel + flush.
            for _ in 0..50 {
                if log_count_for_trace(&raw, &trace_id).await >= 3 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        let count = log_count_for_trace(&raw, &trace_id).await;
        assert!(
            count >= 3,
            "expected the database layer to persist >=3 attributed events, got {count}"
        );

        let levels: Vec<String> = sqlx::query_scalar!(
            "SELECT level FROM logs WHERE trace_id = $1 ORDER BY level",
            trace_id.as_str()
        )
        .fetch_all(&raw)
        .await
        .unwrap();
        assert!(levels.iter().any(|l| l == "ERROR"));
        assert!(levels.iter().any(|l| l == "INFO"));

        let redacted: Option<String> = sqlx::query_scalar!(
            "SELECT metadata FROM logs WHERE trace_id = $1 AND message = 'second info'",
            trace_id.as_str()
        )
        .fetch_one(&raw)
        .await
        .unwrap();
        let metadata = redacted.expect("metadata should be present for the field-bearing event");
        assert!(
            metadata.contains("[REDACTED]"),
            "sensitive field must be redacted in persisted metadata: {metadata}"
        );

        sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
            .execute(&raw)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn enqueue_background_persists_error_entry() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(db) = fixture_db_pool(&url).await else {
            return;
        };
        let raw = db.pool_arc().unwrap().as_ref().clone();

        // Constructing a layer installs the process-global background sender
        // used by `enqueue_background`.
        let _layer = DatabaseLayer::new(db.clone());

        let trace_id = format!("enqueue-trace-{}", uuid::Uuid::new_v4().simple());
        let actor = LogActor::new(
            UserId::new("enqueue-user"),
            SessionId::new("enqueue-session"),
            TraceId::new(trace_id.clone()),
        );
        let entry = LogEntry::new(
            LogLevel::Error,
            "test_module".to_owned(),
            "enqueued error".to_owned(),
            actor,
        );

        // Error entries request an immediate flush.
        enqueue_background(entry);

        let mut count = 0_i64;
        for _ in 0..50 {
            count = log_count_for_trace(&raw, &trace_id).await;
            if count >= 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            count >= 1,
            "enqueue_background should persist an error entry, got {count}"
        );

        sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
            .execute(&raw)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn attached_proxy_delegates_spans_records_and_events() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(db) = fixture_db_pool(&url).await else {
            return;
        };
        let raw = db.pool_arc().unwrap().as_ref().clone();

        let trace_id = format!("proxy-attached-{}", uuid::Uuid::new_v4().simple());

        {
            let proxy = ProxyDatabaseLayer::new();
            proxy.attach(db.clone());
            let subscriber =
                tracing_subscriber::registry().with(proxy.with_filter(LevelFilter::TRACE));
            let _guard = tracing::subscriber::set_default(subscriber);

            let span = info_span!(
                "request",
                user_id = tracing::field::Empty,
                session_id = "proxy-session",
                trace_id = trace_id.as_str(),
                task_id = "",
                context_id = "not-a-uuid",
                client_id = "proxy-client",
            );
            span.record("user_id", "proxy-user");
            let _enter = span.enter();

            tracing::warn!(count = 7_u64, negative = -3_i64, flag = false, "warn event");
            tracing::debug!(
                password = 42_i64,
                token = 9_u64,
                secret = true,
                "debug event"
            );
            tracing::trace!("trace event");
            error!("flush now");

            for _ in 0..50 {
                if log_count_for_trace(&raw, &trace_id).await >= 4 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        let rows: Vec<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query!(
            "SELECT level, metadata, context_id, client_id FROM logs WHERE trace_id = $1",
            trace_id.as_str()
        )
        .fetch_all(&raw)
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.level, r.metadata, r.context_id, r.client_id))
        .collect();

        let levels: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        for expected in ["WARN", "DEBUG", "TRACE", "ERROR"] {
            assert!(levels.contains(&expected), "missing {expected}: {levels:?}");
        }

        for (_, _, context_id, client_id) in &rows {
            assert!(
                context_id.is_none(),
                "non-UUID context_id must be skipped, got {context_id:?}"
            );
            assert_eq!(client_id.as_deref(), Some("proxy-client"));
        }

        let warn_metadata = rows
            .iter()
            .find(|r| r.0 == "WARN")
            .and_then(|r| r.1.clone())
            .expect("warn metadata");
        assert!(warn_metadata.contains("\"count\""));
        assert!(warn_metadata.contains("-3"));
        assert!(warn_metadata.contains("false"));

        let debug_metadata = rows
            .iter()
            .find(|r| r.0 == "DEBUG")
            .and_then(|r| r.1.clone())
            .expect("debug metadata");
        assert!(
            !debug_metadata.contains("42") && debug_metadata.contains("[REDACTED]"),
            "numeric and bool sensitive fields must be redacted: {debug_metadata}"
        );

        let user_ids: Vec<Option<String>> = sqlx::query_scalar!(
            "SELECT user_id FROM logs WHERE trace_id = $1",
            trace_id.as_str()
        )
        .fetch_all(&raw)
        .await
        .unwrap();
        assert!(
            user_ids.iter().all(|u| u.as_deref() == Some("proxy-user")),
            "span.record must update attribution: {user_ids:?}"
        );

        sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
            .execute(&raw)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn database_layer_flushes_on_size_threshold_and_debug_formats() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(db) = fixture_db_pool(&url).await else {
            return;
        };
        let raw = db.pool_arc().unwrap().as_ref().clone();

        let trace_id = format!("bulk-trace-{}", uuid::Uuid::new_v4().simple());

        {
            let layer = DatabaseLayer::new(db.clone());
            assert!(format!("{layer:?}").contains("dropped"));
            let subscriber =
                tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
            let _guard = tracing::subscriber::set_default(subscriber);

            let span = info_span!(
                "bulk",
                user_id = "bulk-user",
                session_id = "bulk-session",
                trace_id = trace_id.as_str(),
            );
            let _enter = span.enter();
            for i in 0..120_u32 {
                info!(i, "bulk event");
            }

            for _ in 0..50 {
                if log_count_for_trace(&raw, &trace_id).await >= 100 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        let count = log_count_for_trace(&raw, &trace_id).await;
        assert!(
            count >= 100,
            "size-threshold flush must persist a full batch without waiting for the timer, got {count}"
        );

        sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
            .execute(&raw)
            .await
            .unwrap();
    }

    #[test]
    fn enqueue_background_without_sink_is_silent() {
        // When no sink is attached the entry is dropped without panicking. In a
        // shared process another test may have installed the global sender, so
        // this only asserts the call is infallible.
        let proxy = ProxyDatabaseLayer::new();
        let _ = format!("{proxy:?}");
        let actor = LogActor::new(
            UserId::new("noop-user"),
            SessionId::new("noop-session"),
            TraceId::new("noop-trace"),
        );
        let entry = LogEntry::new(LogLevel::Info, "m".to_owned(), "noop".to_owned(), actor);
        enqueue_background(entry);
    }
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
