//! Tests for `AgentArgs::has_any_value`.
//!
//! Every field is checked in isolation: the function is a 19-way
//! short-circuiting `||` chain, so a field omitted from it reports "no args
//! supplied" for a caller who did supply one, and the command silently falls
//! through to its interactive path.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::agents::shared::AgentArgs;

#[test]
fn default_args_have_no_value() {
    assert!(!AgentArgs::default().has_any_value());
}

#[test]
fn each_field_alone_is_detected() {
    let cases: Vec<(&str, AgentArgs)> = vec![
        (
            "port",
            AgentArgs {
                port: Some(9001),
                ..Default::default()
            },
        ),
        (
            "endpoint",
            AgentArgs {
                endpoint: Some("/a2a".to_owned()),
                ..Default::default()
            },
        ),
        (
            "dev_only",
            AgentArgs {
                dev_only: true,
                ..Default::default()
            },
        ),
        (
            "is_primary",
            AgentArgs {
                is_primary: true,
                ..Default::default()
            },
        ),
        (
            "default",
            AgentArgs {
                default: true,
                ..Default::default()
            },
        ),
        (
            "display_name",
            AgentArgs {
                display_name: Some("Helper".to_owned()),
                ..Default::default()
            },
        ),
        (
            "description",
            AgentArgs {
                description: Some("Helps".to_owned()),
                ..Default::default()
            },
        ),
        (
            "version",
            AgentArgs {
                version: Some("1.2.3".to_owned()),
                ..Default::default()
            },
        ),
        (
            "icon_url",
            AgentArgs {
                icon_url: Some("https://example.invalid/i.png".to_owned()),
                ..Default::default()
            },
        ),
        (
            "documentation_url",
            AgentArgs {
                documentation_url: Some("https://example.invalid/d".to_owned()),
                ..Default::default()
            },
        ),
        (
            "streaming",
            AgentArgs {
                streaming: Some(false),
                ..Default::default()
            },
        ),
        (
            "push_notifications",
            AgentArgs {
                push_notifications: Some(false),
                ..Default::default()
            },
        ),
        (
            "state_transition_history",
            AgentArgs {
                state_transition_history: Some(false),
                ..Default::default()
            },
        ),
        (
            "provider",
            AgentArgs {
                provider: Some("anthropic".to_owned()),
                ..Default::default()
            },
        ),
        (
            "model",
            AgentArgs {
                model: Some("claude-sonnet-5".to_owned()),
                ..Default::default()
            },
        ),
        (
            "system_prompt",
            AgentArgs {
                system_prompt: Some("be helpful".to_owned()),
                ..Default::default()
            },
        ),
        (
            "system_prompt_file",
            AgentArgs {
                system_prompt_file: Some("/tmp/prompt.md".to_owned()),
                ..Default::default()
            },
        ),
        (
            "mcp_servers",
            AgentArgs {
                mcp_servers: vec!["svc".to_owned()],
                ..Default::default()
            },
        ),
        (
            "skills",
            AgentArgs {
                skills: vec!["writer".to_owned()],
                ..Default::default()
            },
        ),
    ];

    assert_eq!(
        cases.len(),
        19,
        "one case per field in the has_any_value chain"
    );

    for (field, args) in cases {
        assert!(
            args.has_any_value(),
            "setting only `{field}` must be reported as a supplied argument"
        );
    }
}

#[test]
fn false_valued_options_still_count_as_supplied() {
    // `Some(false)` is an explicit choice, not an absence: the caller asked for
    // streaming off, which must not be mistaken for "no flags given".
    let args = AgentArgs {
        streaming: Some(false),
        push_notifications: Some(false),
        state_transition_history: Some(false),
        ..Default::default()
    };
    assert!(args.has_any_value());
}

#[test]
fn empty_collections_do_not_count_as_supplied() {
    let args = AgentArgs {
        mcp_servers: vec![],
        skills: vec![],
        ..Default::default()
    };
    assert!(
        !args.has_any_value(),
        "empty repeatable flags are an absence, not a value"
    );
}

#[test]
fn false_booleans_do_not_count_as_supplied() {
    let args = AgentArgs {
        dev_only: false,
        is_primary: false,
        default: false,
        ..Default::default()
    };
    assert!(!args.has_any_value());
}
