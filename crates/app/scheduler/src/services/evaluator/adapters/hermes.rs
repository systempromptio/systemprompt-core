//! Hermes runs in a separate PID and filesystem namespace behind the execution
//! relay.
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

#[path = "hermes_output.rs"]
mod output;
/// Pinned Hermes execution with namespace isolation and normalized native
/// evidence.
#[derive(Debug, Clone, Copy)]
pub struct HermesAdapter;
pub static ADAPTER: HermesAdapter = HermesAdapter;
const ADAPTER_VERSION: &str = "hermes-native-v1";
const EXECUTABLE: &str = "/opt/hermes-source/.venv/bin/hermes";
const PINNED_VERSION: &str = "0.21.3";

impl NativeAdapter for HermesAdapter {
    fn client(&self) -> ClientKind {
        ClientKind::Hermes
    }
    fn adapter_version(&self) -> &'static str {
        ADAPTER_VERSION
    }
    fn executable(&self) -> &'static str {
        EXECUTABLE
    }
    fn skill_directory(&self) -> &'static str {
        ".hermes/skills"
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
            return Err(invalid("Hermes invocation exceeds prompt or model bounds"));
        }
        let prompt = if input.purpose == ClientPurpose::Execution {
            format!(
                "Read applicable SKILL.md instructions and supporting files from /home/tester/.hermes/skills using read_file before executing the case. Use only the workspace and declared evaluation fixture.\n\n{}",
                input.prompt
            )
        } else {
            input.prompt.to_owned()
        };
        if prompt.len() > 65_536 {
            return Err(invalid("Hermes execution prompt exceeds its bound"));
        }
        let turns = match input.purpose {
            ClientPurpose::Execution => input.limits.max_turns,
            ClientPurpose::Judge => input.limits.max_turns.min(2),
            ClientPurpose::Suggestion => input.limits.max_turns.min(3),
        };
        Ok([
            "/usr/local/bin/node".to_owned(),
            "/opt/systemprompt/hermes-runner.cjs".to_owned(),
            turns.to_string(),
            input.limits.max_output_tokens.to_string(),
            input.model.to_string(),
            if input.purpose == ClientPurpose::Execution {
                "execution"
            } else {
                "review"
            }
            .to_owned(),
            prompt,
        ]
        .into_iter()
        .map(OsString::from)
        .collect())
    }
    fn configuration(&self, context: &AdapterContext<'_>) -> Result<EvidenceArchive> {
        validate_context(context)?;
        let config = serde_json::json!({
            "_config_version":44,"fallback_providers":[],"hooks":{},"hooks_auto_accept":false,
            "auxiliary":{"title_generation":{"enabled":false}},
            "plugins":{"enabled":[],"disabled":["*"]},"updates":{"check":false},
            "memory":{"memory_enabled":false,"user_profile_enabled":false},
            "skills":{"project_discovery":false,"external_dirs":[],"inline_shell":false,"template_vars":false},
            "agent":{"disabled_toolsets":["terminal","browser","web","delegate","memory","skills","cronjob","connections"]},
            "terminal":{"backend":"local","cwd":"/home/tester/work"},
            "model":{"provider":"custom","base_url":"http://127.0.0.1:8091/v1","api_mode":"chat_completions"}
        });
        let environment = format!(
            "HOME=/home/tester\nUSERPROFILE=/home/tester\nHERMES_HOME=/home/tester/.hermes\nXDG_CONFIG_HOME=/home/tester/.config\nXDG_DATA_HOME=/home/tester/.local/share\nXDG_CACHE_HOME=/home/tester/.cache\nHERMES_EVALUATION_RELAY={}\nHERMES_EVALUATION_TOKEN={}\nHERMES_EVALUATION_SESSION={}\nSYSTEMPROMPT_EXECUTION_ID={}\nSYSTEMPROMPT_FIXTURE_CLOCK={}\nTZ={}\n",
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
                (
                    "home/.hermes/evaluator-config.json".to_owned(),
                    file(serde_json::to_vec(&config)?),
                ),
            ]),
        };
        archive.validate()?;
        Ok(archive)
    }
    fn version_arguments(&self) -> Vec<OsString> {
        vec![OsString::from("--version")]
    }
    fn parse_version(&self, bytes: &[u8]) -> Result<String> {
        if bytes.len() > 4096 {
            return Err(invalid("Hermes version output exceeds its bound"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|error| malformed("Hermes version is not UTF-8", error))?;
        let mut lines = text.lines();
        let version = lines
            .next()
            .and_then(|line| line.strip_prefix("Hermes Agent v"))
            .and_then(|line| line.strip_suffix(" (2026.9.14)"))
            .ok_or_else(|| invalid("Unrecognized pinned Hermes release identity"))?;
        if !exact_version(version)
            || lines.any(|line| {
                !line.is_empty()
                    && ![
                        "Install directory:",
                        "Install method:",
                        "Python:",
                        "OpenAI SDK:",
                        "Up to date",
                        "Update available",
                    ]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            })
        {
            return Err(invalid("Ambiguous Hermes version output"));
        }
        Ok(version.to_owned())
    }
    fn normalize(&self, bytes: &[u8]) -> Result<NormalizedClientOutput> {
        output::normalize(bytes)
    }
}

fn validate_context(context: &AdapterContext<'_>) -> Result<()> {
    context.target.validate()?;
    context.frozen.validate()?;
    if context.target.client != ClientKind::Hermes
        || context.target.adapter_version != ADAPTER_VERSION
        || context.target.client_version != PINNED_VERSION
        || context.target.platform != "linux"
    {
        return Err(invalid(
            "Hermes configuration does not match the pinned Linux adapter",
        ));
    }
    let relay_name = context
        .relay_url
        .strip_prefix("http://eval-relay-")
        .and_then(|value| value.strip_suffix(":8090"))
        .ok_or_else(|| invalid("Hermes must connect only to its isolated execution relay"))?;
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
            "Hermes relay configuration contains invalid environment values",
        ));
    }
    Ok(())
}
