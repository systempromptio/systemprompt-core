//! `OpenCode` uses an isolated provider, explicit permissions and bounded JSON
//! events.
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

#[path = "opencode_output.rs"]
mod output;

#[derive(Debug, Clone, Copy)]
pub struct OpenCodeAdapter;
pub static ADAPTER: OpenCodeAdapter = OpenCodeAdapter;
const ADAPTER_VERSION: &str = "opencode-native-v2";
const EXECUTABLE: &str = "/usr/local/bin/opencode";
const PINNED_VERSION: &str = "1.18.29";

impl NativeAdapter for OpenCodeAdapter {
    fn client(&self) -> ClientKind {
        ClientKind::Opencode
    }
    fn adapter_version(&self) -> &'static str {
        ADAPTER_VERSION
    }
    fn executable(&self) -> &'static str {
        EXECUTABLE
    }
    fn skill_directory(&self) -> &'static str {
        ".config/opencode/skills"
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
            return Err(invalid(
                "OpenCode invocation exceeds prompt or model argument bounds",
            ));
        }
        let execution = input.purpose == ClientPurpose::Execution;
        let turns = match input.purpose {
            ClientPurpose::Execution => input.limits.max_turns,
            ClientPurpose::Judge => input.limits.max_turns.min(2),
            ClientPurpose::Suggestion => input.limits.max_turns.min(3),
        };
        let edits = if execution {
            serde_json::json!({"*":"allow",
                "../*":"deny", "home/tester/.config/*":"deny", ".opencode/*":"deny", "opencode.json":"deny", "opencode.jsonc":"deny", "home/tester/work/.opencode/*":"deny", "home/tester/work/opencode.json":"deny", "home/tester/work/opencode.jsonc":"deny"})
        } else {
            serde_json::json!("deny")
        };
        let permission = serde_json::json!({
            "*": "deny",
            "read": {"*":"allow", "../*":"deny", "/proc/*":"deny", "../.config/opencode/skills/*": if execution {"allow"} else {"deny"}},
            "glob":"allow", "grep":"allow",
            "edit": edits,
            "skill": if execution {"allow"} else {"deny"},
            "external_directory": {"*":"deny", "/home/tester/.config/opencode/skills/*": if execution {"allow"} else {"deny"}},
            "evaluation_fixture_evaluation_fixture": if execution {"allow"} else {"deny"}
        });
        let config = serde_json::json!({
            "agent":{"title":{"disable":true},"summary":{"disable":true},"compaction":{"disable":true},"evaluation":{"mode":"primary","steps":turns,"permission":permission,
                "prompt": if execution {"Read applicable SKILL.md files from /home/tester/.config/opencode/skills before executing the case. Use only declared tools and the evaluation fixture."} else {"Read retained evidence and respond directly. Do not execute skills or modify files."}}},
            "permission":permission,
            "provider":{"systemprompt":{"models":{(input.model.as_str()):{
                "id":input.model.as_str(),"name":input.model.as_str(),"tool_call":true,
                "limit":{"context":200_000,"output":input.limits.max_output_tokens}}}}}
        });
        Ok([
            "/usr/local/bin/node".to_owned(),
            "/opt/systemprompt/opencode-runner.cjs".to_owned(),
            turns.to_string(),
            input.limits.max_output_tokens.to_string(),
            input.model.as_str().to_owned(),
            if execution { "execution" } else { "review" }.to_owned(),
            serde_json::to_string(&config)?,
            "--pure".to_owned(),
            "run".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
            "--model".to_owned(),
            format!("systemprompt/{}", input.model.as_str()),
            "--agent".to_owned(),
            "evaluation".to_owned(),
            "--".to_owned(),
            input.prompt.to_owned(),
        ]
        .into_iter()
        .map(OsString::from)
        .collect())
    }

    fn configuration(&self, context: &AdapterContext<'_>) -> Result<EvidenceArchive> {
        validate_context(context)?;
        let config = serde_json::json!({
            "$schema":"https://opencode.ai/config.json", "share":"disabled", "autoupdate":false,
            "enabled_providers":["systemprompt"], "plugin":[], "instructions":[], "formatter":false,"lsp":false,
            "compaction":{"auto":false,"prune":false}, "permission":{"*":"deny"},
            "provider":{"systemprompt":{"npm":"@ai-sdk/openai-compatible","name":"Evaluation gateway",
                "options":{"baseURL":format!("{}/v1",context.relay_url), "apiKey":context.execution_token,
                    "headers":{"x-session-id":context.session_id.as_str(),"x-inference-protocol":"openai-chat"}}}},
            "mcp":{"evaluation_fixture":{"type":"remote","url":format!("{}/mcp/evaluation_fixture",context.relay_url),
                "oauth":false,"timeout":5000,
                "headers":{"Authorization":format!("Bearer {}",context.execution_token),"x-session-id":context.session_id.as_str()}}}
        });
        let environment = format!(
            "HOME=/home/tester\nUSERPROFILE=/home/tester\nXDG_CONFIG_HOME=/home/tester/.config\nXDG_CACHE_HOME=/home/tester/.cache\nXDG_DATA_HOME=/home/tester/.local/share\nXDG_STATE_HOME=/home/tester/.local/state\nOPENCODE_CONFIG=/home/tester/.config/opencode/opencode.json\nOPENCODE_DISABLE_PROJECT_CONFIG=1\nOPENCODE_DISABLE_AUTOUPDATE=1\nOPENCODE_DISABLE_MODELS_FETCH=1\nOPENCODE_DISABLE_AUTOCOMPACT=1\nOPENCODE_DISABLE_PRUNE=1\nOPENCODE_EXPERIMENTAL_DISABLE_FILEWATCHER=1\nOPENCODE_DISABLE_FFF=1\nDO_NOT_TRACK=1\nSYSTEMPROMPT_EXECUTION_ID={}\nSYSTEMPROMPT_FIXTURE_CLOCK={}\nTZ={}\n",
            context.execution_id, context.frozen.fixture_clock, context.frozen.fixture_timezone
        );
        let archive = EvidenceArchive {
            files: BTreeMap::from([
                ("client.env".to_owned(), file(environment.into_bytes())),
                (
                    "home/.config/opencode/opencode.json".to_owned(),
                    file(serde_json::to_vec(&config)?),
                ),
                (
                    "home/.local/share/opencode/auth.json".to_owned(),
                    file(b"{}".to_vec()),
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
        if bytes.len() > 256 {
            return Err(invalid("OpenCode version output exceeds its bound"));
        }
        let version = std::str::from_utf8(bytes)
            .map_err(|error| malformed("OpenCode version is not UTF-8", error))?
            .trim();
        if !exact_version(version) {
            return Err(invalid("OpenCode version must be an exact release"));
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
    if context.target.client != ClientKind::Opencode
        || context.target.adapter_version != ADAPTER_VERSION
        || context.target.client_version != PINNED_VERSION
        || context.target.platform != "linux"
    {
        return Err(invalid(
            "OpenCode configuration does not match the pinned Linux adapter",
        ));
    }
    let relay_name = context
        .relay_url
        .strip_prefix("http://eval-relay-")
        .and_then(|value| value.strip_suffix(":8090"))
        .ok_or_else(|| invalid("OpenCode must connect only to its isolated execution relay"))?;
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
            "OpenCode relay configuration contains invalid environment values",
        ));
    }
    Ok(())
}
