use chrono::Local;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{JwtToken, SessionToken};

use super::artifact_renderers::render_artifact;
use super::{border_color, split_left_panel_block};
use crate::config::TuiConfig;
use crate::state::{short_id, AppState};

#[derive(Debug)]
pub struct ArtifactContext<'a> {
    pub state: &'a AppState,
    pub config: &'a TuiConfig,
    pub api_url: &'a str,
    pub token: &'a SessionToken,
}

impl<'a> ArtifactContext<'a> {
    pub const fn new(
        state: &'a AppState,
        config: &'a TuiConfig,
        api_url: &'a str,
        token: &'a SessionToken,
    ) -> Self {
        Self {
            state,
            config,
            api_url,
            token,
        }
    }
}

pub fn render_artifacts(frame: &mut Frame, area: Rect, ctx: &ArtifactContext<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_artifact_list(frame, chunks[0], ctx.state, ctx.config);
    render_artifact_preview(frame, chunks[1], ctx);
}

fn render_artifact_list(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let artifacts = state.artifacts.filtered_artifacts();
    let scroll_offset = state.artifacts.scroll_offset;

    let visible_height = usize::from(area.height.saturating_sub(2));

    let items: Vec<ListItem> = artifacts
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, artifact)| {
            let is_selected = i == state.artifacts.selected_index;

            let style = if is_selected {
                Style::default()
                    .bg(config.theme.brand_primary)
                    .fg(Color::Black)
            } else {
                Style::default()
            };

            let type_badge = artifact
                .artifact_type
                .as_deref()
                .unwrap_or("unknown")
                .to_string();

            let name = artifact.name.as_deref().unwrap_or_else(|| {
                let id_str = artifact.artifact_id.as_ref();
                &id_str[..8.min(id_str.len())]
            });

            let local_time = artifact.created_at.with_timezone(&Local);
            let date_str = local_time.format("%b %d %H:%M").to_string();

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", date_str),
                    if is_selected {
                        Style::default().fg(Color::Black)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    format!("[{}] ", type_badge),
                    if is_selected {
                        Style::default().fg(Color::Black)
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ),
                Span::styled(name.to_string(), style),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(split_left_panel_block(config));

    frame.render_widget(list, area);

    if artifacts.is_empty() {
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No artifacts yet",
                Style::default().fg(Color::DarkGray).italic(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Artifacts will appear here when",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "agents generate content.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);

        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 3,
        });
        frame.render_widget(empty_msg, inner);
    }
}

fn render_artifact_preview(frame: &mut Frame, area: Rect, ctx: &ArtifactContext<'_>) {
    let block = Block::default()
        .borders(Borders::RIGHT | Borders::BOTTOM)
        .border_style(Style::default().fg(border_color(ctx.config, false)));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let Some(artifact_ref) = ctx.state.artifacts.selected_artifact() else {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Select an artifact to view details",
                Style::default().fg(Color::DarkGray).italic(),
            )),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(empty, inner_area);
        return;
    };

    let Ok(c) = SystempromptClient::new(ctx.api_url) else {
        let error_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Failed to create API client",
                Style::default().fg(Color::Red),
            )),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(error_msg, inner_area);
        return;
    };
    let client = c.with_token(JwtToken::new(ctx.token.as_str()));

    let artifact_data = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(async { client.list_all_artifacts(Some(1000)).await })
    });

    let Ok(artifacts_json) = artifact_data else {
        let error_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Failed to load artifacts",
                Style::default().fg(Color::Red),
            )),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(error_msg, inner_area);
        return;
    };
    let artifact = artifacts_json
        .into_iter()
        .filter_map(|v| serde_json::from_value::<systemprompt_models::A2aArtifact>(v).ok())
        .find(|a| a.id == artifact_ref.artifact_id);

    let Some(artifact) = artifact else {
        let error_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Artifact not found",
                Style::default().fg(Color::Red),
            )),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(error_msg, inner_area);
        return;
    };

    let header_height = 6;
    let header_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: header_height.min(inner_area.height),
    };

    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                artifact.name.as_deref().unwrap_or("Unnamed"),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                format!("  [{}]", artifact.metadata.artifact_type),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::raw(short_id(artifact.id.as_ref())),
            Span::styled("  Task: ", Style::default().fg(Color::DarkGray)),
            Span::raw(short_id(artifact.metadata.task_id.as_str())),
        ]),
        Line::from(vec![Span::styled(
            "─".repeat(usize::from(inner_area.width.saturating_sub(2))),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let header = Paragraph::new(header_lines);
    frame.render_widget(header, header_area);

    if inner_area.height > header_height {
        let content_area = Rect {
            x: inner_area.x,
            y: inner_area.y + header_height,
            width: inner_area.width,
            height: inner_area.height - header_height,
        };

        render_artifact(&artifact, content_area, frame.buffer_mut(), 0);
    }
}
