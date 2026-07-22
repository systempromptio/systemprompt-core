//! Tests for `admin agents registry` card projection: status extraction from
//! the service-status extension, running detection, and verbose truncation.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::json;
use systemprompt_cli::admin::agents::registry::{
    AgentCardResponse, extract_status, is_agent_running, to_agent_info,
};

fn card(status: Option<&str>, description: &str) -> AgentCardResponse {
    let extensions = status.map(|s| {
        json!([{
            "uri": "systemprompt:service-status",
            "params": {"status": s}
        }])
    });
    serde_json::from_value(json!({
        "name": "coder",
        "description": description,
        "supportedInterfaces": [{"url": "https://a.example.com/a2a"}],
        "version": "1.2.3",
        "capabilities": {
            "streaming": true,
            "extensions": extensions
        },
        "skills": [{"name": "review"}, {"name": "plan"}]
    }))
    .unwrap()
}

#[test]
fn extract_status_reads_service_status_extension() {
    assert_eq!(extract_status(&card(Some("running"), "d")), "running");
    assert_eq!(extract_status(&card(Some("stopped"), "d")), "stopped");
    assert_eq!(extract_status(&card(None, "d")), "unknown");
}

#[test]
fn is_agent_running_requires_running_status() {
    assert!(is_agent_running(&card(Some("running"), "d")));
    assert!(!is_agent_running(&card(Some("stopped"), "d")));
    assert!(!is_agent_running(&card(None, "d")));
}

#[test]
fn to_agent_info_verbose_keeps_description_and_skills() {
    let long = "x".repeat(80);
    let info = to_agent_info(card(Some("running"), &long), true);
    assert_eq!(info.name, "coder");
    assert_eq!(info.description, long);
    assert_eq!(info.url, "https://a.example.com/a2a");
    assert_eq!(info.version, "1.2.3");
    assert_eq!(info.status, "running");
    assert!(info.streaming);
    assert_eq!(info.skills_count, 2);
    assert_eq!(info.skills, vec!["review".to_string(), "plan".to_string()]);
}

#[test]
fn to_agent_info_compact_truncates_description_and_hides_skills() {
    let long = "x".repeat(80);
    let info = to_agent_info(card(None, &long), false);
    assert!(info.description.len() < 60);
    assert!(info.description.ends_with("..."));
    assert!(info.skills.is_empty());
    assert_eq!(info.skills_count, 2);
    assert_eq!(info.status, "unknown");
}
