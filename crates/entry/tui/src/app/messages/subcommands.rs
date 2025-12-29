use systemprompt_identifiers::UserId;

use super::types::SyncSubcommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSubcommand {
    List,
    Enable(Option<String>),
    Disable(Option<String>),
    Restart(String),
    Status,
    Health(Option<String>),
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbSubcommand {
    Tables,
    Info,
    Query(String),
    Describe(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpSubcommand {
    List,
    Start(Option<String>),
    Stop(Option<String>),
    Status,
    Restart(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSubcommand {
    Build,
    Serve,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    Services,
    Logs,
    Help,
    Clear,
    Status,
    Users,
    UserRole { user_id: UserId, role: String },

    Agents(AgentSubcommand),
    Db(DbSubcommand),
    Mcp(McpSubcommand),
    Config,
    Cleanup,
    Skills,
    Web(WebSubcommand),
    Sync(SyncSubcommand),
}

impl SlashCommand {
    pub fn from_str(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        if parts.is_empty() || !parts[0].starts_with('/') {
            return None;
        }

        let cmd = parts[0].to_lowercase();
        let args = &parts[1..];

        match cmd.as_str() {
            "/services" => Some(Self::Services),
            "/logs" => Some(Self::Logs),
            "/help" => Some(Self::Help),
            "/clear" => Some(Self::Clear),
            "/status" => Some(Self::Status),
            "/users" => Some(Self::Users),
            "/user-role" => {
                if args.len() >= 2 {
                    Some(Self::UserRole {
                        user_id: UserId::new(args[0]),
                        role: args[1].to_string(),
                    })
                } else {
                    None
                }
            },

            "/agents" | "/a2a" => Self::parse_agents_subcommand(args),
            "/db" => Self::parse_db_subcommand(args),
            "/mcp" => Some(Self::parse_mcp_subcommand(args)),
            "/config" => Some(Self::Config),
            "/cleanup" => Some(Self::Cleanup),
            "/skills" => Some(Self::Skills),
            "/web" => Some(Self::parse_web_subcommand(args)),
            "/sync" => Some(Self::parse_sync_subcommand(args)),

            _ => None,
        }
    }

    fn parse_agents_subcommand(args: &[&str]) -> Option<Self> {
        match args.first().map(|s| s.to_lowercase()).as_deref() {
            Some("enable") => Some(Self::Agents(AgentSubcommand::Enable(
                args.get(1).map(|s| (*s).to_string()),
            ))),
            Some("disable") => Some(Self::Agents(AgentSubcommand::Disable(
                args.get(1).map(|s| (*s).to_string()),
            ))),
            Some("restart") => args
                .get(1)
                .map(|name| Self::Agents(AgentSubcommand::Restart((*name).to_string()))),
            Some("status") => Some(Self::Agents(AgentSubcommand::Status)),
            Some("health") => Some(Self::Agents(AgentSubcommand::Health(
                args.get(1).map(|s| (*s).to_string()),
            ))),
            Some("cleanup") => Some(Self::Agents(AgentSubcommand::Cleanup)),
            _ => Some(Self::Agents(AgentSubcommand::List)),
        }
    }

    fn parse_db_subcommand(args: &[&str]) -> Option<Self> {
        match args.first().map(|s| s.to_lowercase()).as_deref() {
            Some("info") => Some(Self::Db(DbSubcommand::Info)),
            Some("query") => {
                let query = args[1..].join(" ");
                if query.is_empty() {
                    None
                } else {
                    Some(Self::Db(DbSubcommand::Query(query)))
                }
            },
            Some("describe") => args
                .get(1)
                .map(|table| Self::Db(DbSubcommand::Describe((*table).to_string()))),
            _ => Some(Self::Db(DbSubcommand::Tables)),
        }
    }

    fn parse_mcp_subcommand(args: &[&str]) -> Self {
        match args.first().map(|s| s.to_lowercase()).as_deref() {
            Some("start") => Self::Mcp(McpSubcommand::Start(args.get(1).map(|s| (*s).to_string()))),
            Some("stop") => Self::Mcp(McpSubcommand::Stop(args.get(1).map(|s| (*s).to_string()))),
            Some("status") => Self::Mcp(McpSubcommand::Status),
            Some("restart") => Self::Mcp(McpSubcommand::Restart(
                args.get(1).map(|s| (*s).to_string()),
            )),
            _ => Self::Mcp(McpSubcommand::List),
        }
    }

    fn parse_web_subcommand(args: &[&str]) -> Self {
        match args.first().map(|s| s.to_lowercase()).as_deref() {
            Some("serve") => Self::Web(WebSubcommand::Serve),
            _ => Self::Web(WebSubcommand::Build),
        }
    }

    fn parse_sync_subcommand(args: &[&str]) -> Self {
        match args.first().map(|s| s.to_lowercase()).as_deref() {
            Some("code") => Self::Sync(SyncSubcommand::Code),
            Some("migrate") => Self::Sync(SyncSubcommand::Migrate),
            Some("restart") => Self::Sync(SyncSubcommand::Restart),
            _ => Self::Sync(SyncSubcommand::All),
        }
    }

    pub fn all() -> Vec<(&'static str, &'static str)> {
        vec![
            ("/services", "List all services"),
            ("/logs", "Toggle logs panel"),
            ("/help", "Show help"),
            ("/clear", "Clear chat history"),
            ("/status", "Show service status"),
            ("/users", "Show users tab"),
            ("/user-role <id> <role>", "Set user role"),
            ("/agents", "List agents"),
            ("/agents list", "List all agents"),
            ("/agents enable [name]", "Enable agent(s)"),
            ("/agents disable [name]", "Disable agent(s)"),
            ("/agents restart <name>", "Restart an agent"),
            ("/agents status", "Show agent status"),
            ("/agents health [name]", "Check agent health"),
            ("/agents cleanup", "Cleanup orphaned processes"),
            ("/db tables", "List database tables"),
            ("/db info", "Show database info"),
            ("/db describe <table>", "Describe table schema"),
            ("/db query <sql>", "Execute SQL query"),
            ("/mcp", "List MCP servers"),
            ("/mcp list", "List MCP servers"),
            ("/mcp start [name]", "Start MCP server(s)"),
            ("/mcp stop [name]", "Stop MCP server(s)"),
            ("/mcp status", "Show MCP status"),
            ("/mcp restart [name]", "Restart MCP server(s)"),
            ("/config", "Show configuration"),
            ("/cleanup", "Run system cleanup"),
            ("/skills", "List available skills"),
            ("/web build", "Build web assets"),
            ("/web serve", "Serve web assets"),
            ("/sync", "Sync to production (code + migrate + restart)"),
            ("/sync code", "Pull latest code on production"),
            ("/sync migrate", "Run database migrations on production"),
            ("/sync restart", "Restart services on production"),
        ]
    }

    pub fn commands_tab_list() -> Vec<(&'static str, &'static str)> {
        Self::all()
            .into_iter()
            .filter(|(cmd, _)| {
                !cmd.starts_with("/services")
                    && !cmd.starts_with("/logs")
                    && !cmd.starts_with("/users")
                    && !cmd.starts_with("/user-role")
                    && !cmd.starts_with("/web")
            })
            .collect()
    }
}
