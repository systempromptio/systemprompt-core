use std::collections::HashMap;

use serde_yaml::Value;
use systemprompt_manifest::env::InterpolationError;
use systemprompt_manifest::services::interpolation::{
    ServicesInterpolation, interpolate_services_yaml,
};

fn run(
    yaml: &str,
    vars: &[(&str, &str)],
    mode: ServicesInterpolation,
) -> Result<(Value, Vec<InterpolationError>), InterpolationError> {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let mut value: Value = serde_yaml::from_str(yaml).expect("fixture parses");
    let unresolved = interpolate_services_yaml(&mut value, &|name| map.get(name).cloned(), mode)?;
    Ok((value, unresolved))
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter()
        .fold(value, |v, key| v.get(*key).expect("key present"))
        .as_str()
        .expect("string scalar")
}

const PROVIDER: &str =
    "providers:\n  anthropic:\n    endpoints:\n      api:\n        url: \"${PROXY_URL}\"\n";

#[test]
fn a_provider_endpoint_placeholder_resolves() {
    let (value, _) = run(
        PROVIDER,
        &[("PROXY_URL", "https://proxy.internal/v1")],
        ServicesInterpolation::Resolve,
    )
    .expect("resolves");
    assert_eq!(
        at(
            &value,
            &["providers", "anthropic", "endpoints", "api", "url"]
        ),
        "https://proxy.internal/v1"
    );
}

#[test]
fn a_numeric_looking_value_stays_a_string() {
    let (value, _) = run(
        "mcp_servers:\n  docs:\n    env:\n      TOKEN: ${TOKEN}\n",
        &[("TOKEN", "007")],
        ServicesInterpolation::Resolve,
    )
    .expect("resolves");
    assert_eq!(at(&value, &["mcp_servers", "docs", "env", "TOKEN"]), "007");
}

#[test]
fn a_missing_variable_is_an_error_naming_it_and_the_key_path() {
    let err = run(PROVIDER, &[], ServicesInterpolation::Resolve).expect_err("must fail");
    assert_eq!(err.variable, "PROXY_URL");
    assert_eq!(err.key_path, "providers.anthropic.endpoints.api.url");
}

#[test]
fn inspection_keeps_an_unset_placeholder_as_text_and_reports_it() {
    let (value, unresolved) =
        run(PROVIDER, &[], ServicesInterpolation::Inspect).expect("inspection tolerates");
    assert_eq!(
        at(
            &value,
            &["providers", "anthropic", "endpoints", "api", "url"]
        ),
        "${PROXY_URL}"
    );
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].variable, "PROXY_URL");
}

#[test]
fn verbatim_rewrites_nothing() {
    let (value, unresolved) = run(
        PROVIDER,
        &[("PROXY_URL", "https://proxy.internal/v1")],
        ServicesInterpolation::Verbatim,
    )
    .expect("verbatim");
    assert!(unresolved.is_empty());
    assert_eq!(
        at(
            &value,
            &["providers", "anthropic", "endpoints", "api", "url"]
        ),
        "${PROXY_URL}"
    );
}

#[test]
fn prompt_instruction_description_and_hook_text_stays_literal() {
    let yaml = "agents:\n  writer:\n    endpoint: ${AGENT_URL}\n    metadata:\n      system_prompt: \"Echo ${USER_INPUT} back\"\n    card:\n      description: \"Uses ${NAME}\"\nskills:\n  skills:\n    s:\n      instructions: \"Read ${FILE}\"\nplugins:\n  p:\n    hooks:\n      PreToolUse:\n        - command: \"${CLAUDE_PLUGIN_ROOT}/check.sh\"\n";
    let (value, _) = run(
        yaml,
        &[("AGENT_URL", "http://agent:9000")],
        ServicesInterpolation::Resolve,
    )
    .expect("only the endpoint is interpolated");
    assert_eq!(
        at(&value, &["agents", "writer", "endpoint"]),
        "http://agent:9000"
    );
    assert_eq!(
        at(&value, &["agents", "writer", "metadata", "system_prompt"]),
        "Echo ${USER_INPUT} back"
    );
    assert_eq!(
        at(&value, &["agents", "writer", "card", "description"]),
        "Uses ${NAME}"
    );
    assert_eq!(
        at(&value, &["skills", "skills", "s", "instructions"]),
        "Read ${FILE}"
    );
    let command = value["plugins"]["p"]["hooks"]["PreToolUse"][0]["command"]
        .as_str()
        .expect("command");
    assert_eq!(command, "${CLAUDE_PLUGIN_ROOT}/check.sh");
}

#[test]
fn a_disk_hook_command_keeps_the_placeholder_its_client_expands() {
    let yaml = "id: session_log\nevent: SubagentStop\ncommand: node \"${CLAUDE_PLUGIN_ROOT}/scripts/log.mjs\"\n";
    let (value, unresolved) = run(yaml, &[], ServicesInterpolation::Resolve)
        .expect("an unset client variable in a hook command does not fail the load");
    assert!(unresolved.is_empty());
    assert_eq!(
        at(&value, &["command"]),
        "node \"${CLAUDE_PLUGIN_ROOT}/scripts/log.mjs\""
    );
}

#[test]
fn skill_frontmatter_is_carried_through_unresolved_even_when_the_variable_is_set() {
    let yaml = "id: apply\nfrontmatter:\n  allowed-tools: Bash(node \"${CLAUDE_PLUGIN_ROOT}/a.mjs\" *) Read(${CLAUDE_SKILL_DIR}/**)\n";
    let (value, unresolved) = run(
        yaml,
        &[("CLAUDE_PLUGIN_ROOT", "/server/side/path")],
        ServicesInterpolation::Resolve,
    )
    .expect("frontmatter placeholders belong to the client");
    assert!(unresolved.is_empty());
    assert_eq!(
        at(&value, &["frontmatter", "allowed-tools"]),
        "Bash(node \"${CLAUDE_PLUGIN_ROOT}/a.mjs\" *) Read(${CLAUDE_SKILL_DIR}/**)"
    );
}

#[test]
fn a_sibling_of_a_verbatim_key_still_fails_on_an_unset_variable() {
    let yaml = "id: apply\nfrontmatter:\n  allowed-tools: ${CLAUDE_PLUGIN_ROOT}\nfile: \"${SKILL_FILE}\"\n";
    let err = run(yaml, &[], ServicesInterpolation::Resolve)
        .expect_err("a non-verbatim field with an unset variable fails the load");
    assert!(format!("{err}").contains("SKILL_FILE"), "{err}");
}
