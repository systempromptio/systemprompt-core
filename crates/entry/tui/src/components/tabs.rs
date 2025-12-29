use ratatui::prelude::*;
use ratatui::text::Span;

use crate::state::ActiveTab;

pub fn build_tabs_line(active_tab: ActiveTab) -> Line<'static> {
    let titles = [
        "Chat",
        "Convos",
        "Agents",
        "Artifacts",
        "Users",
        "Analytics",
        "Services",
        "Config",
        "Commands",
        "Logs",
    ];

    let selected = match active_tab {
        ActiveTab::Chat => 0,
        ActiveTab::Conversations => 1,
        ActiveTab::Agents => 2,
        ActiveTab::Artifacts => 3,
        ActiveTab::Users => 4,
        ActiveTab::Analytics => 5,
        ActiveTab::Services => 6,
        ActiveTab::Config => 7,
        ActiveTab::Shortcuts => 8,
        ActiveTab::Logs => 9,
    };

    let mut spans = Vec::new();
    for (i, title) in titles.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        if i == selected {
            spans.push(Span::styled(
                *title,
                Style::default().fg(Color::Cyan).bold(),
            ));
        } else {
            spans.push(Span::styled(*title, Style::default().fg(Color::DarkGray)));
        }
    }

    Line::from(spans)
}

pub fn render_tabs(frame: &mut Frame, area: Rect, active_tab: ActiveTab) {
    let line = build_tabs_line(active_tab);
    let paragraph = ratatui::widgets::Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
