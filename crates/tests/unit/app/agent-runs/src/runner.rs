//! The A2A runner's pure mappings: config from the instance URLs, the run
//! directory under the data root, the claimed request to its credentials,
//! and the origin kind it claims, which must be the one the A2A server
//! queues.

use std::path::Path;
use std::time::Duration;

use systemprompt_agent_runs::runner::{
    A2A_RUNNER_MAX_CONCURRENT, A2aRunnerConfig, AGENT_RUN_ORIGIN_KIND, CREDENTIAL_GRACE,
    credential_request,
};
use systemprompt_identifiers::{AgentRunId, PluginId, UserId};

use crate::repository::request;

#[test]
fn config_derives_gateway_and_hooks_from_the_internal_url() {
    let config = A2aRunnerConfig::new("http://127.0.0.1:8080/", "iss", Path::new("/data"));
    assert_eq!(config.gateway_base_url, "http://127.0.0.1:8080");
    assert_eq!(config.hooks.govern_url, "http://127.0.0.1:8080/api/public/hooks/govern");
    assert_eq!(config.hooks.track_url, "http://127.0.0.1:8080/api/public/hooks/track");
    assert_eq!(config.issuer, "iss");
    let id = AgentRunId::new("run-1");
    assert_eq!(config.layout(&id).root, Path::new("/data/agent-runs/run-1"));
}

#[test]
fn credentials_follow_the_row_user_and_request_plugin() {
    let mut req = request();
    req.plugin_id = Some(PluginId::new("gov-plugin"));
    req.max_budget_usd = Some(1.5);
    req.wall_clock = Duration::from_secs(60);
    let user = UserId::new("u-1");
    let id = AgentRunId::new("run-9");
    let issue = credential_request(&user, &id, &req);
    assert_eq!(issue.user_id, user);
    assert_eq!(issue.name, "a2a-run-run-9");
    assert!(issue.scopes.is_empty());
    assert_eq!(issue.plugin_id, Some(PluginId::new("gov-plugin")));
    assert_eq!(issue.ttl, Duration::from_secs(60) + CREDENTIAL_GRACE);
    assert_eq!(issue.limits.max_budget_usd, Some(1.5));
    assert_eq!(issue.limits.max_requests, None);
}

#[test]
fn claims_the_a2a_origin_with_bounded_concurrency() {
    assert_eq!(AGENT_RUN_ORIGIN_KIND, "a2a");
    assert_eq!(A2A_RUNNER_MAX_CONCURRENT, 2);
}
