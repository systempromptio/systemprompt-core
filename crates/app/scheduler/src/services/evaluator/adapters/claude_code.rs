//! Claude Code runs with explicit configuration inside the evaluator container.
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

#[path = "claude_code_output.rs"]
mod output;

#[derive(Debug, Clone, Copy)]
pub struct ClaudeCodeAdapter;
pub static ADAPTER: ClaudeCodeAdapter = ClaudeCodeAdapter;

const ADAPTER_VERSION: &str = "claude-code-native-v1";
const EXECUTABLE: &str = "/usr/local/bin/claude";
const MCP_FILE: &str = "/home/tester/.claude/evaluator-mcp.json";
const EMPTY_MCP_FILE: &str = "/home/tester/.claude/evaluator-no-mcp.json";
const FIXTURE_TOOL: &str = "mcp__evaluation_fixture__evaluation_fixture";
const FORBIDDEN: &str = "Bash,PowerShell,Agent,Task,WebSearch,WebFetch,Computer,NotebookEdit,REPL";

impl NativeAdapter for ClaudeCodeAdapter {
    fn client(&self) -> ClientKind {
        ClientKind::ClaudeCode
    }
    fn adapter_version(&self) -> &'static str {
        ADAPTER_VERSION
    }
    fn executable(&self) -> &'static str {
        EXECUTABLE
    }
    fn skill_directory(&self) -> &'static str {
        ".claude/skills"
    }

    fn arguments(&self, input: &AdapterInvocation<'_>) -> Result<Vec<OsString>> {
        input.limits.validate()?;
        if input.prompt.len() > 65_536
            || input.prompt.contains('\0')
            || input.model.as_str().len() > 128
            || input.model.as_str().starts_with('-')
            || input.model.as_str().chars().any(char::is_control)
        {
            return Err(invalid(
                "Claude invocation exceeds prompt or model argument bounds",
            ));
        }
        let execution = input.purpose == ClientPurpose::Execution;
        let tools = if execution {
            "Read,Write,Edit,Glob,Grep,Skill"
        } else {
            "Read,Glob,Grep"
        };
        let allowed = if execution {
            format!("{tools},{FIXTURE_TOOL}")
        } else {
            tools.to_owned()
        };
        let denied = if execution {
            FORBIDDEN.to_owned()
        } else {
            format!("{FORBIDDEN},Write,Edit,Skill,mcp__*")
        };
        let turns = match input.purpose {
            ClientPurpose::Execution => input.limits.max_turns,
            ClientPurpose::Judge => input.limits.max_turns.min(2),
            ClientPurpose::Suggestion => input.limits.max_turns.min(3),
        };
        let settings = serde_json::json!({
            "disableAllHooks": true,
            "enableAllProjectMcpServers": false,
            "permissions": {
                "defaultMode": "dontAsk",
                "disableBypassPermissionsMode": "disable",
                "allow": allowed.split(',').collect::<Vec<_>>(),
                "deny": denied.split(',').chain([
                    "Read(/home/tester/.claude/evaluator-mcp.json)",
                    "Read(/home/tester/.claude/.credentials.json)",
                    "Read(/proc/**)",
                    "Edit(/home/tester/.claude/**)",
                    "Write(/home/tester/.claude/**)",
                    "Edit(/home/tester/work/.claude/**)",
                    "Write(/home/tester/work/.claude/**)",
                    "Edit(/home/tester/work/.mcp.json)",
                    "Write(/home/tester/work/.mcp.json)"
                ]).collect::<Vec<_>>()
            },
            "env": {"CLAUDE_CODE_MAX_OUTPUT_TOKENS": input.limits.max_output_tokens.to_string()}
        });
        let mut arguments: Vec<OsString> = [
            EXECUTABLE,
            "-p",
            "--bare",
            "--restricted",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            input.model.as_str(),
            "--max-turns",
            &turns.to_string(),
            "--tools",
            tools,
            "--allowedTools",
            &allowed,
            "--disallowedTools",
            &denied,
            "--settings",
            &serde_json::to_string(&settings)?,
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--mcp-config",
            if execution { MCP_FILE } else { EMPTY_MCP_FILE },
            "--permission-mode",
            "dontAsk",
            "--permission-prompts",
            "none",
            "--no-session-persistence",
            "--no-chrome",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        if execution {
            arguments.extend([OsString::from("--add-dir"), OsString::from("/home/tester/.claude/skills"),
                OsString::from("--append-system-prompt"), OsString::from("Evaluation skills are installed at /home/tester/.claude/skills. Read the applicable SKILL.md instructions and supporting files before executing the case. Use only the declared tools and evaluation fixture service.")]);
        } else {
            arguments.push(OsString::from("--disable-slash-commands"));
        }
        arguments.extend([OsString::from("--"), OsString::from(input.prompt)]);
        Ok(arguments)
    }

    fn configuration(&self, context: &AdapterContext<'_>) -> Result<EvidenceArchive> {
        validate_context(context)?;
        let mcp = serde_json::json!({"mcpServers":{"evaluation_fixture":{
            "type":"http", "url":format!("{}/mcp/evaluation_fixture", context.relay_url),
            "headers":{"Authorization":format!("Bearer {}", context.execution_token),
                "x-session-id":context.session_id.as_str()}
        }}});
        let environment = format!(
            "HOME=/home/tester\nUSERPROFILE=/home/tester\nCLAUDE_CONFIG_DIR=/home/tester/.claude\nXDG_CONFIG_HOME=/home/tester/.config\nXDG_CACHE_HOME=/home/tester/.cache\nXDG_DATA_HOME=/home/tester/.local/share\nXDG_STATE_HOME=/home/tester/.local/state\nANTHROPIC_BASE_URL={}\nANTHROPIC_API_KEY={}\nANTHROPIC_AUTH_TOKEN=\nCLAUDE_CODE_OAUTH_TOKEN=\nANTHROPIC_CUSTOM_HEADERS=x-session-id: {}\nCLAUDE_CODE_USE_BEDROCK=0\nCLAUDE_CODE_USE_VERTEX=0\nCLAUDE_CODE_USE_FOUNDRY=0\nCLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1\nCLAUDE_CODE_DISABLE_AUTO_MEMORY=1\nDISABLE_AUTOUPDATER=1\nDISABLE_TELEMETRY=1\nDISABLE_ERROR_REPORTING=1\nSYSTEMPROMPT_EXECUTION_ID={}\nSYSTEMPROMPT_FIXTURE_CLOCK={}\nTZ={}\n",
            context.relay_url,
            context.execution_token,
            context.session_id,
            context.execution_id,
            context.frozen.fixture_clock,
            context.frozen.fixture_timezone
        );
        let files = BTreeMap::from([
            ("client.env".to_owned(), file(environment.into_bytes())),
            (
                "home/.claude/evaluator-mcp.json".to_owned(),
                file(serde_json::to_vec(&mcp)?),
            ),
            (
                "home/.claude/evaluator-no-mcp.json".to_owned(),
                file(b"{\"mcpServers\":{}}".to_vec()),
            ),
        ]);
        let archive = EvidenceArchive { files };
        archive.validate()?;
        Ok(archive)
    }

    fn version_arguments(&self) -> Vec<OsString> {
        vec![OsString::from("--version")]
    }

    fn parse_version(&self, bytes: &[u8]) -> Result<String> {
        if bytes.len() > 256 {
            return Err(invalid("Claude version output exceeds its bound"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|error| malformed("Claude version output is not UTF-8", error))?
            .trim();
        let version = text
            .strip_suffix(" (Claude Code)")
            .ok_or_else(|| invalid("Unrecognized Claude Code version output"))?;
        if !exact_version(version) {
            return Err(invalid("Claude version must be an exact release"));
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
    if context.target.client != ClientKind::ClaudeCode
        || context.target.adapter_version != ADAPTER_VERSION
        || !exact_version(&context.target.client_version)
        || context.target.platform != "linux"
    {
        return Err(invalid(
            "Claude configuration does not match the pinned Linux adapter",
        ));
    }
    let relay_name = context
        .relay_url
        .strip_prefix("http://eval-relay-")
        .and_then(|value| value.strip_suffix(":8090"))
        .ok_or_else(|| invalid("Claude must connect only to its isolated execution relay"))?;
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
            "Claude relay configuration contains invalid environment values",
        ));
    }
    Ok(())
}
