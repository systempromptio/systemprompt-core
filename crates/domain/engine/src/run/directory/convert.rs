//! Turning settings into what the other engine crates are configured with:
//! built-in agents, MCP server configs, formatter and language server settings,
//! and the built-in tool set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use crate::config::{
    AgentDef, AgentMode as ConfigMode, Loaded, McpServer, Paths, Ruleset, Settings, ToolFilter,
};
use crate::lsp::{LspConfig, ServerId, ServerSpec as LspServerSpec};
use crate::mcp::{RemoteHeaders, ServerConfig, ServerSpec};
use crate::prompt::agents::{AgentMode as PromptMode, BUILTIN_AGENTS};
use crate::tools::{
    BackgroundJobs, EditTool, FetchTool, GlobTool, GrepTool, PatchTool, ReadTool, ShellKind,
    ShellSpec, ShellTool, TodoSink, TodoTool, Tool, ToolsConfig, WriteTool,
};
use crate::workspace::{FormatterConfig, FormatterSpec};

use crate::run::governor::McpSpec;
use crate::run::inner::Inner;

pub(crate) fn builtin_agents(paths: &Paths) -> Vec<AgentDef> {
    BUILTIN_AGENTS
        .iter()
        .map(|builtin| {
            let mode = match builtin.mode {
                PromptMode::Primary => ConfigMode::Primary,
                PromptMode::Subagent => ConfigMode::Subagent,
                PromptMode::All => ConfigMode::All,
            };
            let mut agent = AgentDef::new(builtin.name, mode);
            agent.description = Some(builtin.description.to_owned());
            agent.prompt = Some(builtin.prompt.to_owned());
            agent.hidden = builtin.hidden;
            agent.defaults = if builtin.read_only {
                Ruleset::plan_defaults(paths)
            } else {
                Ruleset::build_defaults(paths)
            };
            agent.tools = ToolFilter {
                allow: Vec::new(),
                deny: builtin
                    .tools_denied
                    .iter()
                    .map(|tool| (*tool).to_owned())
                    .collect(),
            };
            agent
        })
        .collect()
}

pub(crate) fn mcp_servers(loaded: &Loaded, managed: &[McpSpec]) -> BTreeMap<String, ServerConfig> {
    let mut servers = BTreeMap::new();
    for (name, server) in &loaded.settings.mcp {
        servers.insert(name.clone(), user_server(loaded, server));
    }
    for spec in managed {
        let remote = ServerSpec::Remote {
            url: spec.url.clone(),
            headers: RemoteHeaders::Source(Arc::clone(&spec.headers)),
        };
        servers.insert(spec.name.clone(), ServerConfig::new(remote));
    }
    servers
}

fn user_server(loaded: &Loaded, server: &McpServer) -> ServerConfig {
    match server {
        McpServer::Remote {
            url,
            headers,
            enabled,
            timeout_ms,
        } => ServerConfig {
            spec: ServerSpec::Remote {
                url: url.clone(),
                headers: RemoteHeaders::Static(
                    headers
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                ),
            },
            enabled: *enabled,
            connect_timeout: timeout_ms.map(Duration::from_millis),
        },
        McpServer::Local {
            command,
            args,
            env,
            enabled,
            timeout_ms,
        } => ServerConfig {
            spec: ServerSpec::Local {
                command: command.clone(),
                args: args.clone(),
                env: env
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                cwd: Some(loaded.worktree.root.clone()),
            },
            enabled: *enabled,
            connect_timeout: timeout_ms.map(Duration::from_millis),
        },
    }
}

pub(crate) fn formatter_config(
    settings: &Settings,
    search_path: Option<OsString>,
) -> FormatterConfig {
    FormatterConfig {
        custom: settings
            .formatters
            .iter()
            .filter(|(_, formatter)| !formatter.disabled)
            .map(|(name, formatter)| FormatterSpec {
                name: name.clone(),
                command: formatter.command.clone(),
                args: formatter.args.clone(),
                extensions: formatter.extensions.clone(),
                root_markers: Vec::new(),
                node_modules: false,
            })
            .collect(),
        disabled: settings
            .formatters
            .iter()
            .filter(|(_, formatter)| formatter.disabled)
            .map(|(name, _)| name.clone())
            .collect(),
        search_path,
        ..FormatterConfig::default()
    }
}

pub(crate) fn lsp_config(settings: &Settings, inner: &Inner) -> LspConfig {
    let mut config = LspConfig {
        servers: settings
            .lsp
            .iter()
            .filter(|(_, server)| !server.disabled)
            .map(|(name, server)| LspServerSpec {
                id: ServerId::new(name.clone()),
                command: server.command.clone(),
                args: server.args.clone(),
                env: server
                    .env
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                extensions: server.extensions.clone(),
                root_markers: server.root_markers.clone(),
                initialization_options: server.initialization.clone(),
            })
            .collect(),
        disabled: settings
            .lsp
            .iter()
            .filter(|(_, server)| server.disabled)
            .map(|(name, _)| ServerId::new(name.clone()))
            .collect(),
        search_path: inner.search_path.clone(),
        path_ext: inner.path_ext.clone(),
        ..LspConfig::default()
    };
    if let Some(name) = &inner.lsp_client_name {
        config.client_name.clone_from(name);
    }
    config
}

pub(crate) fn builtin_tools(
    shell: &ShellSpec,
    jobs: &BackgroundJobs,
    todos: &Arc<dyn TodoSink>,
) -> Vec<Arc<dyn Tool>> {
    let config = ToolsConfig::default();
    vec![
        Arc::new(ShellTool::with_spec(
            shell.clone(),
            config.shell.clone(),
            config.output,
            jobs.clone(),
        )),
        Arc::new(ReadTool::new(config.read)),
        Arc::new(WriteTool),
        Arc::new(EditTool),
        Arc::new(PatchTool),
        Arc::new(GlobTool::new(config.search)),
        Arc::new(GrepTool::new(config.search)),
        Arc::new(FetchTool::new(config.fetch.clone(), config.output)),
        Arc::new(TodoTool::new(Arc::clone(todos))),
    ]
}

pub(crate) const fn prompt_shell(kind: ShellKind) -> crate::prompt::ShellKind {
    match kind {
        ShellKind::Pwsh => crate::prompt::ShellKind::Pwsh,
        ShellKind::WindowsPowerShell => crate::prompt::ShellKind::WindowsPowerShell,
        ShellKind::Cmd => crate::prompt::ShellKind::Cmd,
        ShellKind::GitBash | ShellKind::Posix => crate::prompt::ShellKind::Bash,
    }
}
