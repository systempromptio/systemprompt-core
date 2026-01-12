mod overlays;
mod panels;

use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use systemprompt_identifiers::SessionToken;

use crate::components::chat::render_chat_messages;
use crate::components::{
    border_color, build_tabs_line, render_agent_card, render_agents, render_analytics,
    render_artifacts, render_config, render_conversations, render_logs_tab, render_shortcuts,
    render_sidebar, render_users, ArtifactContext,
};
use crate::config::TuiConfig;
use crate::layout::AppLayout;
use crate::state::{ActiveTab, AppState, SseStatus};

use super::TuiApp;
use overlays::render_overlays;
use panels::{render_chat_input, render_with_control_guide};

struct RenderContext<'a> {
    state: &'a AppState,
    config: &'a TuiConfig,
    api_url: &'a str,
    token: &'a SessionToken,
}

impl TuiApp {
    pub(crate) fn draw(&mut self) -> Result<()> {
        let ctx = RenderContext {
            state: &self.state,
            config: &self.config,
            api_url: &self.api_external_url,
            token: &self.session_token,
        };

        self.terminal.draw(|frame| {
            let area = frame.area();
            let layout = AppLayout::new(area);

            render_header(frame, &layout, &ctx);

            if let Some(cursor_pos) = render_main_block_with_tabs(frame, &layout, &ctx) {
                frame.set_cursor_position(cursor_pos);
            }

            render_overlays(frame, &layout, ctx.state, ctx.config);
        })?;

        Ok(())
    }
}

fn render_header(frame: &mut Frame, layout: &AppLayout, ctx: &RenderContext) {
    use ratatui::text::Text;

    let orange = ctx.config.theme.brand_primary;
    let white = Color::White;
    let dim = Color::DarkGray;

    let profile_name = ctx.state.mode_info.display_name();
    let env = ctx.state.mode_info.environment();
    let env_color = if env == "Sandbox" {
        Color::Yellow
    } else {
        Color::Green
    };

    let context_display = ctx
        .state
        .chat
        .context_id
        .as_ref()
        .map(|id| format!(" ctx:{}", &id.as_str()[..8]))
        .unwrap_or_default();

    let (sse_text, sse_color) = match ctx.state.sse_status {
        SseStatus::Connected => ("●", Color::Green),
        SseStatus::Connecting => ("○", Color::Yellow),
        SseStatus::Reconnecting => ("↻", Color::Yellow),
        SseStatus::Disconnected => ("○", Color::DarkGray),
        SseStatus::Failed => ("✗", Color::Red),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" Welcome to ", Style::default().fg(dim)),
            Span::styled("</", Style::default().fg(orange).bold()),
            Span::styled("SYSTEMPROMPT", Style::default().fg(white).bold()),
            Span::styled(".", Style::default().fg(orange).bold()),
            Span::styled("io", Style::default().fg(white)),
            Span::styled(">", Style::default().fg(orange).bold()),
            Span::styled(
                format!(" [{}]", profile_name),
                Style::default().fg(white).bold(),
            ),
            Span::styled(format!(" ({})", env), Style::default().fg(env_color)),
            Span::styled(context_display, Style::default().fg(dim)),
            Span::styled(format!(" {}", sse_text), Style::default().fg(sse_color)),
        ]),
        Line::from(""),
    ];

    let header = Paragraph::new(Text::from(lines));
    frame.render_widget(header, layout.header);
}

fn render_main_block_with_tabs(
    frame: &mut Frame,
    layout: &AppLayout,
    ctx: &RenderContext,
) -> Option<Position> {
    let tabs_title = build_tabs_line(ctx.state.active_tab);
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(ctx.config, false)))
        .title(tabs_title)
        .title_alignment(Alignment::Left);

    let inner_area = main_block.inner(layout.main_block);
    frame.render_widget(main_block, layout.main_block);

    let content_area = Rect {
        x: inner_area.x,
        y: inner_area.y + 1,
        width: inner_area.width,
        height: inner_area.height.saturating_sub(1),
    };

    render_main_content(frame, content_area, ctx)
}

fn render_main_content(
    frame: &mut Frame,
    content_area: Rect,
    ctx: &RenderContext,
) -> Option<Position> {
    let state = ctx.state;
    let config = ctx.config;

    match state.active_tab {
        ActiveTab::Chat => Some(render_chat_tab(frame, content_area, state, config)),
        ActiveTab::Conversations => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_conversations(f, a, state, config);
            });
            None
        },
        ActiveTab::Agents => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_agents(f, a, state, config);
            });
            None
        },
        ActiveTab::Artifacts => {
            let artifact_ctx = ArtifactContext::new(state, config, ctx.api_url, ctx.token);
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_artifacts(f, a, &artifact_ctx);
            });
            None
        },
        ActiveTab::Users => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_users(f, a, state, config);
            });
            None
        },
        ActiveTab::Analytics => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_analytics(f, a, state, config);
            });
            None
        },
        ActiveTab::Services => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_sidebar(f, a, state, config);
            });
            None
        },
        ActiveTab::Config => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_config(f, a, state, config);
            });
            None
        },
        ActiveTab::Shortcuts => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_shortcuts(f, a, state, config);
            });
            None
        },
        ActiveTab::Logs => {
            render_with_control_guide(frame, content_area, state, config, |f, a| {
                render_logs_tab(f, a, state, config);
            });
            None
        },
    }
}

fn render_chat_tab(
    frame: &mut Frame,
    content_area: Rect,
    state: &AppState,
    config: &TuiConfig,
) -> Position {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(content_area);

    if state.sidebar_visible {
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(config.layout.chat_width_percent),
                Constraint::Percentage(config.layout.sidebar_width_percent),
            ])
            .split(vertical_chunks[0]);
        render_chat_messages(frame, horizontal_chunks[0], state, config);
        render_agent_card(frame, horizontal_chunks[1], state, config);
    } else {
        render_chat_messages(frame, vertical_chunks[0], state, config);
    }

    render_chat_input(frame, vertical_chunks[1], state, config)
}
