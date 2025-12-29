use crate::messages::{
    AgentSubcommand, Command, DbSubcommand, McpSubcommand, SlashCommand, WebSubcommand,
};
use crate::state::{ActiveTab, FocusedPanel};

use super::super::TuiApp;

impl TuiApp {
    pub(crate) fn handle_focus_panel(&mut self, panel: FocusedPanel) -> Vec<Command> {
        self.state.focus = panel;
        vec![Command::None]
    }

    pub(crate) fn handle_switch_tab(&mut self, tab: ActiveTab) -> Vec<Command> {
        self.state.active_tab = tab;
        vec![Command::None]
    }

    pub(crate) fn handle_toggle_logs(&mut self) -> Vec<Command> {
        self.state.active_tab = ActiveTab::Logs;
        vec![Command::None]
    }

    pub(crate) fn handle_toggle_sidebar(&mut self) -> Vec<Command> {
        self.state.sidebar_visible = !self.state.sidebar_visible;
        vec![Command::None]
    }

    pub(crate) fn handle_slash_command(&mut self, command: SlashCommand) -> Vec<Command> {
        tracing::info!("[UPDATE] handle_slash_command: {:?}", command);
        self.prepare_command_output(&command);
        self.dispatch_slash_command(command)
    }

    fn prepare_command_output(&mut self, command: &SlashCommand) {
        let has_output = matches!(
            command,
            SlashCommand::Agents(_)
                | SlashCommand::Db(_)
                | SlashCommand::Mcp(_)
                | SlashCommand::Config
                | SlashCommand::Cleanup
                | SlashCommand::Skills
                | SlashCommand::Status
        );
        if has_output {
            self.state.commands.is_executing = true;
            self.state.commands.output = Some("Executing...".to_string());
        }
    }

    fn dispatch_slash_command(&mut self, command: SlashCommand) -> Vec<Command> {
        match command {
            SlashCommand::Services => {
                self.state.active_tab = ActiveTab::Services;
                vec![Command::RefreshServices]
            },
            SlashCommand::Logs => {
                self.state.active_tab = ActiveTab::Logs;
                vec![Command::None]
            },
            SlashCommand::Help => {
                self.state.active_tab = ActiveTab::Shortcuts;
                vec![Command::None]
            },
            SlashCommand::Clear => self.handle_clear_command(),
            SlashCommand::Status => vec![Command::RefreshServices],
            SlashCommand::Users => {
                self.state.active_tab = ActiveTab::Users;
                vec![Command::RefreshUsers]
            },
            SlashCommand::UserRole { user_id, role } => {
                vec![Command::UpdateUserRole { user_id, role }]
            },
            SlashCommand::Agents(sub) => Self::handle_agent_subcommand(sub),
            SlashCommand::Db(sub) => Self::handle_db_subcommand(sub),
            SlashCommand::Mcp(sub) => Self::handle_mcp_subcommand(sub),
            SlashCommand::Config => {
                self.state.active_tab = ActiveTab::Config;
                vec![Command::ShowConfig]
            },
            SlashCommand::Cleanup => vec![Command::RunCleanup],
            SlashCommand::Skills => vec![Command::ShowSkills],
            SlashCommand::Web(sub) => Self::handle_web_subcommand(sub),
            SlashCommand::Sync(sub) => vec![Command::Sync(sub)],
        }
    }

    fn handle_clear_command(&mut self) -> Vec<Command> {
        let old_context_id = self.state.chat.context_id.clone();
        self.state.chat.clear();
        self.state.commands.set_output("Chat cleared".to_string());
        let mut commands = Vec::new();
        if let Some(context_id) = old_context_id {
            commands.push(Command::DeleteConversation(context_id.to_string()));
        }
        commands.push(Command::CreateNewContext);
        commands
    }

    fn handle_agent_subcommand(sub: AgentSubcommand) -> Vec<Command> {
        match sub {
            AgentSubcommand::List => vec![Command::AgentList],
            AgentSubcommand::Enable(name) => vec![Command::AgentEnable(name)],
            AgentSubcommand::Disable(name) => vec![Command::AgentDisable(name)],
            AgentSubcommand::Restart(name) => vec![Command::AgentRestart(name)],
            AgentSubcommand::Status => vec![Command::AgentStatus],
            AgentSubcommand::Health(name) => vec![Command::AgentHealth(name)],
            AgentSubcommand::Cleanup => vec![Command::AgentCleanup],
        }
    }

    fn handle_db_subcommand(sub: DbSubcommand) -> Vec<Command> {
        match sub {
            DbSubcommand::Tables => vec![Command::DbTables],
            DbSubcommand::Info => vec![Command::DbInfo],
            DbSubcommand::Query(sql) => vec![Command::ExecuteDbQuery(sql)],
            DbSubcommand::Describe(table) => vec![Command::DbDescribe(table)],
        }
    }

    fn handle_mcp_subcommand(sub: McpSubcommand) -> Vec<Command> {
        match sub {
            McpSubcommand::List => vec![Command::McpList],
            McpSubcommand::Start(name) => vec![Command::McpStart(name)],
            McpSubcommand::Stop(name) => vec![Command::McpStop(name)],
            McpSubcommand::Status => vec![Command::McpStatus],
            McpSubcommand::Restart(name) => vec![Command::McpRestart(name)],
        }
    }

    fn handle_web_subcommand(sub: WebSubcommand) -> Vec<Command> {
        match sub {
            WebSubcommand::Build => vec![Command::WebBuild],
            WebSubcommand::Serve => vec![Command::WebServe],
        }
    }
}
