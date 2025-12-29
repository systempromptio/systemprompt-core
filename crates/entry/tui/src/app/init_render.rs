use std::io::Stdout;

use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::TuiConfig;
use crate::state::AppState;

use super::TuiApp;

impl TuiApp {
    pub(super) fn draw_init_frame(
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        state: &AppState,
        config: &TuiConfig,
    ) -> Result<()> {
        use crate::layout::AppLayout;

        terminal.draw(|frame| {
            let area = frame.area();
            let layout = AppLayout::new(area);

            Self::render_init_header(frame, &layout, state, config);
            Self::render_init_main_block(frame, &layout, state, config);

            if state.init_status.is_initializing {
                Self::render_init_overlay(frame, area, state, config);
            }
        })?;

        Ok(())
    }

    fn render_init_header(
        frame: &mut ratatui::Frame,
        layout: &crate::layout::AppLayout,
        state: &AppState,
        config: &TuiConfig,
    ) {
        use ratatui::prelude::*;
        use ratatui::widgets::Paragraph;

        let orange = config.theme.brand_primary;
        let white = Color::White;
        let dim = Color::DarkGray;

        let header_lines = vec![
            Line::from(vec![
                Span::styled(" Welcome to ", Style::default().fg(dim)),
                Span::styled("</", Style::default().fg(orange).bold()),
                Span::styled("SYSTEMPROMPT", Style::default().fg(white).bold()),
                Span::styled(".", Style::default().fg(orange).bold()),
                Span::styled("io", Style::default().fg(white)),
                Span::styled(">", Style::default().fg(orange).bold()),
                Span::styled(
                    format!(" [{}]", state.mode_info.display_name()),
                    Style::default().fg(dim),
                ),
            ]),
            Line::from(""),
        ];
        let header = Paragraph::new(header_lines);
        frame.render_widget(header, layout.header);
    }

    fn render_init_main_block(
        frame: &mut ratatui::Frame,
        layout: &crate::layout::AppLayout,
        state: &AppState,
        config: &TuiConfig,
    ) {
        use crate::components::{border_color, build_tabs_line};
        use ratatui::prelude::*;
        use ratatui::widgets::{Block, Borders};

        let tabs_title = build_tabs_line(state.active_tab);
        let main_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color(config, false)))
            .title(tabs_title)
            .title_alignment(Alignment::Left);
        frame.render_widget(main_block, layout.main_block);
    }

    fn render_init_overlay(
        frame: &mut ratatui::Frame,
        area: ratatui::prelude::Rect,
        state: &AppState,
        config: &TuiConfig,
    ) {
        use ratatui::prelude::*;
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let orange = config.theme.brand_primary;
        let white = Color::White;
        let dim = Color::DarkGray;

        let overlay_area = Self::calculate_overlay_area(area);
        frame.render_widget(Clear, overlay_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(orange))
            .title(" Initializing ")
            .title_style(Style::default().fg(orange).bold());

        let inner = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        let lines = Self::build_progress_lines(state, inner.width, orange, white, dim);
        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    fn calculate_overlay_area(area: ratatui::prelude::Rect) -> ratatui::prelude::Rect {
        let overlay_width = 50.min(area.width.saturating_sub(4));
        let overlay_height = 8.min(area.height.saturating_sub(4));
        let overlay_x = (area.width.saturating_sub(overlay_width)) / 2;
        let overlay_y = (area.height.saturating_sub(overlay_height)) / 2;

        ratatui::prelude::Rect {
            x: area.x + overlay_x,
            y: area.y + overlay_y,
            width: overlay_width,
            height: overlay_height,
        }
    }

    fn build_progress_lines<'a>(
        state: &AppState,
        inner_width: u16,
        orange: ratatui::prelude::Color,
        white: ratatui::prelude::Color,
        dim: ratatui::prelude::Color,
    ) -> Vec<ratatui::prelude::Line<'a>> {
        use ratatui::prelude::*;

        let progress = if state.init_status.total_steps > 0 {
            let completed = u32::try_from(state.init_status.steps_completed).unwrap_or(u32::MAX);
            let total = u32::try_from(state.init_status.total_steps).unwrap_or(u32::MAX);
            f64::from(completed) / f64::from(total)
        } else {
            0.0
        };
        let bar_width_u16 = inner_width.saturating_sub(4);
        let bar_width = usize::from(bar_width_u16);
        let max_filled = f64::from(bar_width_u16);
        let filled = (progress * max_filled).clamp(0.0, max_filled) as usize;
        let empty = bar_width.saturating_sub(filled);
        let progress_bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(empty));

        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                state.init_status.current_step.clone(),
                Style::default().fg(white),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                progress_bar,
                Style::default().fg(orange),
            )]),
            Line::from(vec![Span::styled(
                format!(
                    "{}/{}",
                    state.init_status.steps_completed, state.init_status.total_steps
                ),
                Style::default().fg(dim),
            )]),
        ]
    }
}
