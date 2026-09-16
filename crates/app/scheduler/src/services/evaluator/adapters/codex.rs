//! Codex executes behind a bounded authenticated loopback relay in the pinned
//! image.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::client::ClientPurpose;
use super::{
    AdapterContext, AdapterInvocation, NativeAdapter, NormalizedClientOutput, exact_version, file,
    invalid, malformed,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use systemprompt_evaluation::Result;
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::EvidenceArchive;

#[path = "codex_output.rs"]
mod output;

#[derive(Debug, Clone, Copy)]
pub struct CodexAdapter;
pub static ADAPTER: CodexAdapter = CodexAdapter;
const ADAPTER_VERSION: &str = "codex-native-v1";
const EXECUTABLE: &str = "/opt/systemprompt/codex/bin/codex";
const PINNED_VERSION: &str = "0.154.0";
const BASE_SETTINGS: &[&str] = &[
    "approval_policy=\"never\"",
    "allow_login_shell=false",
    "default_permissions=\"evaluation\"",
    "permissions.evaluation.network.enabled=false",
    "shell_environment_policy.inherit=\"none\"",
    "shell_environment_policy.set.PATH=\"/usr/local/bin:/usr/bin:/bin\"",
    "shell_environment_policy.set.HOME=\"/home/tester\"",
    "shell_environment_policy.experimental_use_profile=false",
    "model_provider=\"systemprompt\"",
    "model_providers.systemprompt.name=\"Evaluation gateway\"",
    "model_providers.systemprompt.base_url=\"http://127.0.0.1:8091/v1\"",
    "model_providers.systemprompt.env_key=\"CODEX_LOCAL_PROXY_TOKEN\"",
    "model_providers.systemprompt.wire_api=\"responses\"",
    "model_providers.systemprompt.requires_openai_auth=false",
    "model_providers.systemprompt.supports_websockets=false",
    "model_providers.systemprompt.supports_standalone_web_search=false",
    "model_providers.systemprompt.request_max_retries=0",
    "model_providers.systemprompt.stream_max_retries=0",
    "model_providers.systemprompt.stream_idle_timeout_ms=30000",
    "mcp_servers.evaluation_fixture.url=\"http://127.0.0.1:8091/mcp/evaluation_fixture\"",
    "mcp_servers.evaluation_fixture.bearer_token_env_var=\"CODEX_LOCAL_PROXY_TOKEN\"",
    "mcp_servers.evaluation_fixture.enabled_tools=[\"evaluation_fixture\"]",
    "mcp_servers.evaluation_fixture.tools.evaluation_fixture.approval_mode=\"approve\"",
    "mcp_servers.evaluation_fixture.startup_timeout_sec=5",
    "mcp_servers.evaluation_fixture.tool_timeout_sec=10",
    "features.apps=false",
    "features.plugins=false",
    "features.remote_plugin=false",
    "features.recommended_plugins=false",
    "features.hooks=false",
    "features.memories=false",
    "features.multi_agent=false",
    "features.multi_agent_v2=false",
    "features.computer_use=false",
    "features.in_app_browser=false",
    "features.shell_snapshot=false",
    "features.skill_mcp_dependency_install=false",
    "features.skill_search=false",
    "web_search=\"disabled\"",
    "features.view_image=false",
    "features.image_generation=false",
    "project_doc_max_bytes=0",
    "projects={\"/home/tester/work\"={trust_level=\"untrusted\"}}",
    "history.persistence=\"none\"",
    "check_for_update_on_startup=false",
    "analytics.enabled=false",
];

impl NativeAdapter for CodexAdapter {
    fn client(&self) -> ClientKind {
        ClientKind::Codex
    }
    fn adapter_version(&self) -> &'static str {
        ADAPTER_VERSION
    }
    fn executable(&self) -> &'static str {
        EXECUTABLE
    }
    fn skill_directory(&self) -> &'static str {
        ".agents/skills"
    }

    fn arguments(&self, input: &AdapterInvocation<'_>) -> Result<Vec<OsString>> {
        input.limits.validate()?;
        if input.prompt.len() > 65_536
            || input.prompt.contains('\0')
            || input.model.as_str().is_empty()
            || input.model.as_str().len() > 128
            || !input
                .model
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/' | b':'))
        {
            return Err(invalid("Codex invocation exceeds prompt or model bounds"));
        }
        let execution = input.purpose == ClientPurpose::Execution;
        let turns = match input.purpose {
            ClientPurpose::Execution => input.limits.max_turns,
            ClientPurpose::Judge => input.limits.max_turns.min(2),
            ClientPurpose::Suggestion => input.limits.max_turns.min(3),
        };
        let mut arguments: Vec<OsString> = [
            "/usr/local/bin/node".to_owned(),
            "/opt/systemprompt/codex-runner.cjs".to_owned(),
            turns.to_string(),
            input.limits.max_output_tokens.to_string(),
            input.model.to_string(),
            if execution { "execution" } else { "review" }.to_owned(),
            "--".to_owned(),
            "exec".to_owned(),
            "--json".to_owned(),
            "--ephemeral".to_owned(),
            "--ignore-user-config".to_owned(),
            "--ignore-rules".to_owned(),
            "--strict-config".to_owned(),
            "--skip-git-repo-check".to_owned(),
            "--color".to_owned(),
            "never".to_owned(),
            "--model".to_owned(),
            input.model.to_string(),
            "--cd".to_owned(),
            "/home/tester/work".to_owned(),
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        for setting in BASE_SETTINGS {
            config(&mut arguments, setting);
        }
        purpose_settings(&mut arguments, execution, input.limits.max_output_tokens)?;
        arguments.extend([OsString::from("--"), OsString::from(input.prompt)]);
        Ok(arguments)
    }
    fn configuration(&self, context: &AdapterContext<'_>) -> Result<EvidenceArchive> {
        validate_context(context)?;
        let environment = format!(
            "HOME=/home/tester\nUSERPROFILE=/home/tester\nCODEX_HOME=/home/tester/.codex\nXDG_CONFIG_HOME=/home/tester/.config\nXDG_DATA_HOME=/home/tester/.local/share\nXDG_CACHE_HOME=/home/tester/.cache\nCODEX_EVALUATION_RELAY={}\nCODEX_EVALUATION_TOKEN={}\nCODEX_EVALUATION_SESSION={}\nSYSTEMPROMPT_EXECUTION_ID={}\nSYSTEMPROMPT_FIXTURE_CLOCK={}\nTZ={}\n",
            context.relay_url,
            context.execution_token,
            context.session_id,
            context.execution_id,
            context.frozen.fixture_clock,
            context.frozen.fixture_timezone
        );
        let archive = EvidenceArchive {
            files: BTreeMap::from([
                ("client.env".to_owned(), file(environment.into_bytes())),
                ("home/.codex/config.toml".to_owned(), file(Vec::new())),
            ]),
        };
        archive.validate()?;
        Ok(archive)
    }
    fn version_arguments(&self) -> Vec<OsString> {
        vec![OsString::from("--version")]
    }
    fn parse_version(&self, bytes: &[u8]) -> Result<String> {
        if bytes.len() > 256 {
            return Err(invalid("Codex version output exceeds its bound"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|error| malformed("Codex version is not UTF-8", error))?
            .trim();
        let version = text
            .strip_prefix("codex-cli ")
            .ok_or_else(|| invalid("Unrecognized Codex executable identity"))?;
        if !exact_version(version) {
            return Err(invalid("Codex version must be an exact release"));
        }
        Ok(version.to_owned())
    }
    fn normalize(&self, bytes: &[u8]) -> Result<NormalizedClientOutput> {
        output::normalize(bytes)
    }
}
fn config(arguments: &mut Vec<OsString>, setting: &str) {
    arguments.extend([OsString::from("-c"), OsString::from(setting)]);
}

fn purpose_settings(
    arguments: &mut Vec<OsString>,
    execution: bool,
    max_output_tokens: u32,
) -> Result<()> {
    // Why: CLI overrides split dotted keys without TOML quoting; path keys belong
    // in a table value.
    let workspace_access = if execution { "write" } else { "read" };
    let skill_access = if execution { "read" } else { "deny" };
    config(
        arguments,
        &format!(
            r#"permissions.evaluation.filesystem={{":minimal"="read","/proc"="deny","/opt/systemprompt/codex"="read","/home/tester/.codex"="deny","/home/tester/work/.codex"="deny","/home/tester/work"="{workspace_access}","/home/tester/.agents/skills"="{skill_access}"}}"#
        ),
    );
    config(
        arguments,
        if execution {
            "mcp_servers.evaluation_fixture.enabled=true"
        } else {
            "mcp_servers.evaluation_fixture.enabled=false"
        },
    );
    config(
        arguments,
        &format!("tool_output_token_limit={max_output_tokens}"),
    );
    let instruction = if execution {
        "Read applicable SKILL.md instructions from /home/tester/.agents/skills before executing the case. Use only the sandboxed workspace and evaluation fixture."
    } else {
        "Evaluate retained evidence and respond directly. Do not execute skills or modify files."
    };
    config(
        arguments,
        &format!(
            "developer_instructions={}",
            serde_json::to_string(instruction)?
        ),
    );
    Ok(())
}

fn validate_context(context: &AdapterContext<'_>) -> Result<()> {
    context.target.validate()?;
    context.frozen.validate()?;
    if context.target.client != ClientKind::Codex
        || context.target.adapter_version != ADAPTER_VERSION
        || context.target.client_version != PINNED_VERSION
        || context.target.platform != "linux"
    {
        return Err(invalid(
            "Codex configuration does not match the pinned Linux adapter",
        ));
    }
    let relay_name = context
        .relay_url
        .strip_prefix("http://eval-relay-")
        .and_then(|value| value.strip_suffix(":8090"))
        .ok_or_else(|| invalid("Codex must connect only to its isolated execution relay"))?;
    if relay_name.is_empty()
        || relay_name.len() > 128
        || !relay_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || context.execution_token.is_empty()
        || context.execution_token.len() > 128
        || !context
            .execution_token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || [
            &context.frozen.fixture_clock,
            &context.frozen.fixture_timezone,
        ]
        .iter()
        .any(|value| value.chars().any(char::is_control))
    {
        return Err(invalid(
            "Codex relay configuration contains invalid environment values",
        ));
    }
    Ok(())
}
