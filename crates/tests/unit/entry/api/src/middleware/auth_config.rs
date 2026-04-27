//! Unit tests for ApiAuthMiddlewareConfig
//!
//! Tests cover:
//! - Default public paths list
//! - is_public_path for various API and non-API paths
//! - Wellknown paths always public
//! - Custom config with additional paths

use systemprompt_api::services::middleware::ApiAuthMiddlewareConfig;
use systemprompt_models::modules::ApiPaths;

#[test]
fn default_config_has_public_paths() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(!config.public_paths.is_empty());
}

#[test]
fn new_produces_same_as_default() {
    let new_config = ApiAuthMiddlewareConfig::new();
    let default_config = ApiAuthMiddlewareConfig::default();
    assert_eq!(
        new_config.public_paths.len(),
        default_config.public_paths.len()
    );
}

#[test]
fn oauth_session_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::OAUTH_SESSION));
}

#[test]
fn oauth_register_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::OAUTH_REGISTER));
}

#[test]
fn oauth_authorize_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::OAUTH_AUTHORIZE));
}

#[test]
fn oauth_token_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::OAUTH_TOKEN));
}

#[test]
fn oauth_callback_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::OAUTH_CALLBACK));
}

#[test]
fn oauth_consent_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::OAUTH_CONSENT));
}

#[test]
fn wellknown_base_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::WELLKNOWN_BASE));
}

#[test]
fn wellknown_agent_card_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::WELLKNOWN_AGENT_CARD));
}

#[test]
fn wellknown_oauth_server_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::WELLKNOWN_OAUTH_SERVER));
}

#[test]
fn stream_base_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::STREAM_BASE));
}

#[test]
fn contexts_webhook_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::CONTEXTS_WEBHOOK));
}

#[test]
fn discovery_endpoint_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::DISCOVERY));
}

#[test]
fn non_api_path_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path("/"));
}

#[test]
fn static_content_path_is_public() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path("/some-page"));
}

#[test]
fn api_v1_paths_are_public_via_discovery() {
    let config = ApiAuthMiddlewareConfig::default();
    assert!(config.is_public_path(ApiPaths::CORE_CONTEXTS));
    assert!(config.is_public_path(ApiPaths::AGENTS_BASE));
    assert!(config.is_public_path(ApiPaths::ADMIN_BASE));
    assert!(config.is_public_path(ApiPaths::AUTH_ME));
}

#[test]
fn custom_config_without_discovery_protects_api_paths() {
    let config = ApiAuthMiddlewareConfig {
        public_paths: vec![ApiPaths::OAUTH_TOKEN],
    };
    assert!(!config.is_public_path(ApiPaths::AGENTS_BASE));
    assert!(!config.is_public_path(ApiPaths::ADMIN_BASE));
    assert!(!config.is_public_path(ApiPaths::AUTH_ME));
    assert!(config.is_public_path(ApiPaths::OAUTH_TOKEN));
}

#[test]
fn custom_config_with_empty_public_paths() {
    let config = ApiAuthMiddlewareConfig {
        public_paths: vec![],
    };
    assert!(!config.is_public_path(ApiPaths::OAUTH_SESSION));
}

#[test]
fn wellknown_subpath_always_public_regardless_of_config() {
    let config = ApiAuthMiddlewareConfig {
        public_paths: vec![],
    };
    assert!(config.is_public_path("/.well-known/openid-configuration"));
}
