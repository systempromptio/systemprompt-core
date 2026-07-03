use std::time::Duration;
use systemprompt_mcp::{McpHttpConfig, McpState, SessionTimeouts};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[test]
fn session_timeouts_both_none() {
    let t = SessionTimeouts {
        init: None,
        keep_alive: None,
    };
    assert!(t.init.is_none());
    assert!(t.keep_alive.is_none());
}

#[test]
fn session_timeouts_both_some() {
    let t = SessionTimeouts {
        init: Some(Duration::from_secs(30)),
        keep_alive: Some(Duration::from_secs(120)),
    };
    assert_eq!(t.init, Some(Duration::from_secs(30)));
    assert_eq!(t.keep_alive, Some(Duration::from_secs(120)));
}

#[test]
fn session_timeouts_init_only() {
    let t = SessionTimeouts {
        init: Some(Duration::from_millis(500)),
        keep_alive: None,
    };
    assert_eq!(t.init, Some(Duration::from_millis(500)));
    assert!(t.keep_alive.is_none());
}

#[test]
fn session_timeouts_keep_alive_only() {
    let t = SessionTimeouts {
        init: None,
        keep_alive: Some(Duration::from_millis(200)),
    };
    assert!(t.init.is_none());
    assert_eq!(t.keep_alive, Some(Duration::from_millis(200)));
}

#[test]
fn session_timeouts_zero_duration() {
    let t = SessionTimeouts {
        init: Some(Duration::ZERO),
        keep_alive: Some(Duration::ZERO),
    };
    assert_eq!(t.init.unwrap().as_millis(), 0);
    assert_eq!(t.keep_alive.unwrap().as_millis(), 0);
}

#[test]
fn session_timeouts_max_duration() {
    let t = SessionTimeouts {
        init: Some(Duration::MAX),
        keep_alive: Some(Duration::MAX),
    };
    assert_eq!(t.init, Some(Duration::MAX));
}

#[test]
fn mcp_http_config_default_session_default() {
    let config = McpHttpConfig::default();
    assert!(config.session.init.is_none());
    assert!(config.session.keep_alive.is_none());
}

#[test]
fn mcp_http_config_custom_origins() {
    let config = McpHttpConfig {
        allowed_hosts: Some(vec!["example.com".to_owned()]),
        allowed_origins: vec!["https://app.example.com".to_owned()],
        session: SessionTimeouts::default(),
    };
    assert_eq!(config.allowed_origins.len(), 1);
    assert_eq!(config.allowed_origins[0], "https://app.example.com");
}

#[test]
fn mcp_http_config_no_allowed_hosts() {
    let config = McpHttpConfig {
        allowed_hosts: None,
        allowed_origins: vec![],
        session: SessionTimeouts::default(),
    };
    assert!(config.allowed_hosts.is_none());
}

#[test]
fn mcp_http_config_multiple_origins() {
    let config = McpHttpConfig {
        allowed_hosts: None,
        allowed_origins: vec![
            "https://a.example.com".to_owned(),
            "https://b.example.com".to_owned(),
        ],
        session: SessionTimeouts::default(),
    };
    assert_eq!(config.allowed_origins.len(), 2);
}

#[test]
fn mcp_http_config_default_hosts_count() {
    let config = McpHttpConfig::default();
    let hosts = config.allowed_hosts.expect("some");
    assert_eq!(hosts.len(), 5);
}

#[test]
fn mcp_http_config_clone_preserves_hosts() {
    let config = McpHttpConfig::default();
    let cloned = config.clone();
    assert_eq!(cloned.allowed_hosts.as_ref().map(|v| v.len()), Some(5));
}

#[test]
fn mcp_http_config_debug_nonempty() {
    let config = McpHttpConfig::default();
    let s = format!("{config:?}");
    assert!(!s.is_empty());
    assert!(s.contains("McpHttpConfig"));
}

#[tokio::test]
async fn mcp_state_debug_and_accessors() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(pool) = fixture_db_pool(&url).await else {
        return;
    };
    let state = McpState::new(pool);
    let debug = format!("{state:?}");
    assert!(debug.contains("McpState"));
    let _pool_ref = state.db_pool();
    let cloned = state.clone();
    let _ = cloned.db_pool();
}
