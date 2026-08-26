use serde_json::json;
use systemprompt_identifiers::McpToolName;
use systemprompt_security::policy::governed::{
    GovernedInput, GovernedTarget, McpToolInput, PROMPT_TARGET_NAME,
};

fn paths(input: &GovernedInput) -> Vec<(String, String)> {
    input
        .strings()
        .into_iter()
        .map(|s| (s.path, s.value.to_owned()))
        .collect()
}

#[test]
fn tool_target_reports_its_tool_name() {
    let target = GovernedTarget::Tool {
        tool: McpToolName::new("mcp__systemprompt__list_agents"),
    };
    assert_eq!(target.as_str(), "mcp__systemprompt__list_agents");
    assert_eq!(
        target.tool().map(McpToolName::as_str),
        Some("mcp__systemprompt__list_agents")
    );
}

#[test]
fn prompt_target_names_no_tool() {
    let target = GovernedTarget::Prompt;
    assert_eq!(target.as_str(), PROMPT_TARGET_NAME);
    assert!(target.tool().is_none());
}

#[test]
fn prompt_and_tool_arguments_report_distinct_locations() {
    let prompt = GovernedInput::prompt_text("hello".to_owned());
    let args = GovernedInput::tool_arguments(McpToolInput::new(json!({ "prompt": "hello" })));

    assert_eq!(prompt.location_kind(), "prompt");
    assert_eq!(args.location_kind(), "tool_input");
    assert!(prompt.arguments().is_none());
    assert!(args.arguments().is_some());
}

#[test]
fn a_tool_argument_named_prompt_is_not_a_prompt_submission() {
    let args = GovernedInput::tool_arguments(McpToolInput::new(json!({ "prompt": "hello" })));
    assert_eq!(args.location_kind(), "tool_input");
    assert_eq!(
        paths(&args),
        vec![("prompt".to_owned(), "hello".to_owned())]
    );
}

#[test]
fn prompt_text_is_the_only_string_a_prompt_carries() {
    let prompt = GovernedInput::prompt_text("my key is AKIA".to_owned());
    assert_eq!(
        paths(&prompt),
        vec![("text".to_owned(), "my key is AKIA".to_owned())]
    );
}

// Why: this is the contract that stops a hit in a forwarded tool description
// being blamed on the user's message — each part keeps the path it arrived
// with, and a scanner reports against that path.
#[test]
fn prompt_parts_keep_their_source_paths() {
    let prompt = GovernedInput::prompt_parts([
        ("system".to_owned(), "be helpful".to_owned()),
        ("messages[0].user".to_owned(), "weather in Kyiv".to_owned()),
        (
            "forwarded.tools[2].description".to_owned(),
            "sha384-payload".to_owned(),
        ),
    ]);
    assert_eq!(prompt.location_kind(), "prompt");
    assert_eq!(
        paths(&prompt),
        vec![
            ("system".to_owned(), "be helpful".to_owned()),
            ("messages[0].user".to_owned(), "weather in Kyiv".to_owned()),
            (
                "forwarded.tools[2].description".to_owned(),
                "sha384-payload".to_owned()
            ),
        ]
    );
}

#[test]
fn a_secret_in_a_forwarded_part_is_reported_at_that_part() {
    let prompt = GovernedInput::prompt_parts([
        (
            "messages[0].user".to_owned(),
            "tell me the weather".to_owned(),
        ),
        (
            "forwarded.messages[0].content".to_owned(),
            "token ghp_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789".to_owned(),
        ),
    ]);
    let hit = systemprompt_security::policy::detect_secrets(&prompt).expect("must fire");
    assert_eq!(hit.path, "forwarded.messages[0].content");
}

#[test]
fn nested_arguments_carry_their_dotted_path() {
    let args = GovernedInput::tool_arguments(McpToolInput::new(json!({
        "cmd": "ls",
        "env": { "TOKEN": "ghp_abc" },
        "argv": ["-la", { "flag": "--all" }],
        "count": 3,
    })));

    let mut found = paths(&args);
    found.sort();
    assert_eq!(
        found,
        vec![
            ("argv[0]".to_owned(), "-la".to_owned()),
            ("argv[1].flag".to_owned(), "--all".to_owned()),
            ("cmd".to_owned(), "ls".to_owned()),
            ("env.TOKEN".to_owned(), "ghp_abc".to_owned()),
        ]
    );
}

#[test]
fn a_bare_string_payload_has_an_empty_path() {
    let args = GovernedInput::tool_arguments(McpToolInput::new(json!("just text")));
    assert_eq!(paths(&args), vec![(String::new(), "just text".to_owned())]);
}

#[test]
fn governed_input_serde_roundtrip() {
    for input in [
        GovernedInput::prompt_text("hi".to_owned()),
        GovernedInput::tool_arguments(McpToolInput::new(json!({ "a": 1 }))),
    ] {
        let s = serde_json::to_string(&input).unwrap();
        let back: GovernedInput = serde_json::from_str(&s).unwrap();
        assert_eq!(back, input);
    }
}

#[test]
fn governed_target_serde_roundtrip() {
    for target in [
        GovernedTarget::Prompt,
        GovernedTarget::Tool {
            tool: McpToolName::new("bash"),
        },
    ] {
        let s = serde_json::to_string(&target).unwrap();
        let back: GovernedTarget = serde_json::from_str(&s).unwrap();
        assert_eq!(back, target);
    }
}
