mod commands;
pub mod config;
mod draw;
mod init;
mod init_render;
pub mod layout;
pub mod messages;
mod update;

pub use config::TuiConfig;
pub use messages::Message;

use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use crate::events::{EventHandler, TuiEventBus};
use crate::services::{
    cloud_api, AnalyticsSubscriber, ContextStreamSubscriber, LogStreamer, MessageSender,
    UserSubscriber,
};
use crate::state::AppState;
use crate::tools::ToolRegistry;
use systemprompt_identifiers::{CloudAuthToken, ContextId, SessionId, SessionToken, UserId};
use systemprompt_models::Profile;

#[derive(Debug, Clone)]
pub struct LocalSession {
    pub token: SessionToken,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub user_email: String,
}

#[derive(Debug, Clone)]
pub struct CloudConnection {
    pub api_url: String,
    pub token: CloudAuthToken,
    pub tenant_id: Option<String>,
}

#[derive(Debug)]
pub struct TuiParams {
    pub profile: Profile,
    pub session: LocalSession,
    pub cloud: Option<CloudConnection>,
}

pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    pub(crate) state: AppState,
    pub(crate) config: TuiConfig,
    pub(crate) message_tx: mpsc::UnboundedSender<Message>,
    message_rx: mpsc::UnboundedReceiver<Message>,
    pub(crate) tool_registry: Arc<ToolRegistry>,
    pub(crate) message_sender: MessageSender,
    pub(crate) session_token: SessionToken,
    pub(crate) current_agent_name: Option<String>,
    pub(crate) current_context_id: Arc<RwLock<ContextId>>,
    pub(crate) api_external_url: String,
    session_id: SessionId,
}

impl std::fmt::Debug for TuiApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiApp")
            .field("state", &self.state)
            .field("config", &self.config)
            .field("message_sender", &self.message_sender)
            .field("current_agent_name", &self.current_agent_name)
            .finish_non_exhaustive()
    }
}

impl TuiApp {
    pub async fn new_cloud(params: TuiParams) -> Result<Self> {
        let TuiParams {
            profile,
            session,
            cloud,
        } = params;

        let cloud_api_url = cloud.as_ref().map_or_else(
            || profile.server.api_external_url.clone(),
            |c| c.api_url.clone(),
        );
        let user_email = Some(session.user_email.clone());
        let tenant_id = cloud.as_ref().and_then(|c| c.tenant_id.clone());
        let session_token = session.token;
        let session_id = session.session_id;

        Self::log_startup_info(&cloud_api_url, &profile, user_email.as_deref());

        let mut terminal = Self::init_terminal()?;
        let api_external_url = profile.server.api_external_url.clone();
        let config = TuiConfig::default();
        let mut state = Self::init_state(&config, cloud_api_url, user_email, tenant_id, profile);
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        Self::draw_init_frame(&mut terminal, &state, &config)?;

        Self::update_init_progress(&mut state, "Validating credentials...", 1);
        Self::draw_init_frame(&mut terminal, &state, &config)?;

        let message_sender = MessageSender::new_with_url(
            session_token.clone(),
            message_tx.clone(),
            &api_external_url,
        );

        Self::update_init_progress(&mut state, "Loading context...", 2);
        Self::draw_init_frame(&mut terminal, &state, &config)?;

        let context_id =
            cloud_api::fetch_or_create_context(&api_external_url, &session_token).await?;
        let current_context_id = Arc::new(RwLock::new(context_id.clone()));

        state.chat.set_context(context_id.clone());
        Self::load_chat_history(&mut state, &api_external_url, &session_token, &context_id).await;
        Self::load_conversations(&mut state, &api_external_url, &session_token).await;

        Self::update_init_progress(&mut state, "Loading agents...", 3);
        Self::draw_init_frame(&mut terminal, &state, &config)?;

        let current_agent_name =
            Self::load_agents(&mut state, &api_external_url, &session_token).await;
        Self::populate_agent_display_metadata(&mut state);

        Self::update_init_progress(&mut state, "Loading artifacts...", 4);
        Self::draw_init_frame(&mut terminal, &state, &config)?;

        Self::load_artifacts(&mut state, &api_external_url, &session_token).await;

        Self::update_init_progress(&mut state, "Initializing...", 5);
        Self::draw_init_frame(&mut terminal, &state, &config)?;

        let tool_registry = Arc::new(ToolRegistry::new());

        state.init_status.current_step = "Ready!".to_string();
        state.init_status.steps_completed = 6;
        state.init_status.is_initializing = false;
        Self::draw_init_frame(&mut terminal, &state, &config)?;

        info!("TUI initialized successfully in cloud mode");

        Ok(Self {
            terminal,
            state,
            config,
            message_tx,
            message_rx,
            tool_registry,
            message_sender,
            session_token,
            current_agent_name,
            current_context_id,
            api_external_url,
            session_id,
        })
    }

    fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        info!("Terminal initialized");
        Ok(terminal)
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("TUI run loop starting");
        let handles = self.spawn_subscribers();
        self.run_event_loop().await?;
        self.shutdown(handles)
    }

    async fn run_event_loop(&mut self) -> Result<()> {
        let mut tick_interval = tokio::time::interval(Duration::from_millis(100));
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            self.draw()?;

            tokio::select! {
                message = self.message_rx.recv() => {
                    if self.process_message(message).await {
                        break;
                    }
                }
                _ = tick_interval.tick() => {}
            }
        }
        Ok(())
    }

    async fn process_message(&mut self, message: Option<Message>) -> bool {
        let Some(message) = message else {
            return false;
        };

        self.dispatch_message_commands(message).await;
        self.check_quit_requested()
    }

    async fn dispatch_message_commands(&mut self, message: Message) {
        for command in self.update(message) {
            if let Err(e) = self.execute_command(command).await {
                error!(error = %e, "Command execution error");
            }
        }
    }

    fn check_quit_requested(&self) -> bool {
        if self.state.should_quit {
            info!("Quit requested, shutting down");
            true
        } else {
            false
        }
    }

    fn spawn_subscribers(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let event_bus = Arc::new(TuiEventBus::default());
        let event_handler = EventHandler::new(self.message_tx.clone(), Duration::from_millis(50));
        let event_handle = tokio::spawn(async move {
            let _ = event_handler.run().await;
        });

        let context_stream_subscriber = ContextStreamSubscriber::new_with_url(
            self.message_tx.clone(),
            self.session_token.clone(),
            Arc::clone(&self.current_context_id),
            &self.api_external_url,
        );
        let context_stream_handle = context_stream_subscriber.spawn();

        let user_subscriber = UserSubscriber::new(
            self.api_external_url.clone(),
            self.session_token.clone(),
            self.message_tx.clone(),
            Arc::clone(&event_bus),
        );
        let user_handle = user_subscriber.spawn();

        let log_streamer = LogStreamer::new(
            self.api_external_url.clone(),
            self.session_token.clone(),
            self.message_tx.clone(),
            Duration::from_secs(1),
        );
        let log_handle = log_streamer.spawn();

        let analytics_subscriber = AnalyticsSubscriber::new(
            self.api_external_url.clone(),
            self.session_token.clone(),
            self.message_tx.clone(),
            Arc::clone(&event_bus),
        );
        let analytics_handle = analytics_subscriber.spawn();

        vec![
            event_handle,
            context_stream_handle,
            user_handle,
            log_handle,
            analytics_handle,
        ]
    }

    fn shutdown(&mut self, handles: Vec<tokio::task::JoinHandle<()>>) -> Result<()> {
        Self::abort_handles(handles);
        self.end_session()?;
        info!("TUI shutdown complete");
        Ok(())
    }

    fn abort_handles(handles: Vec<tokio::task::JoinHandle<()>>) {
        for handle in handles {
            handle.abort();
        }
    }

    fn end_session(&mut self) -> Result<()> {
        info!(session_id = %self.session_id.as_str(), "Ending TUI session");
        cloud_api::end_tui_session(&self.session_id);
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
