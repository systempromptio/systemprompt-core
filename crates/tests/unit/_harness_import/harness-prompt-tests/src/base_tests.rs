use harness_prompt::base::{CORE_PROMPT, EXPLORE_PROMPT, GENERAL_PROMPT, PLAN_PROMPT};

#[test]
fn derived_prompts_start_with_the_core_prompt() {
    assert!(PLAN_PROMPT.starts_with(CORE_PROMPT));
    assert!(GENERAL_PROMPT.starts_with(CORE_PROMPT));
    assert!(PLAN_PROMPT.len() > CORE_PROMPT.len());
    assert!(!EXPLORE_PROMPT.starts_with(CORE_PROMPT));
}

#[test]
fn core_prompt_names_the_dedicated_tools() {
    for tool in [
        "read", "glob", "grep", "edit", "patch", "write", "todo", "task", "question",
    ] {
        assert!(
            CORE_PROMPT.contains(tool),
            "core prompt does not mention {tool}"
        );
    }
    assert!(CORE_PROMPT.contains("path:line"));
}
