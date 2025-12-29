use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::config::TuiConfig;
use crate::state::ActiveTab;

pub struct KeyBinding {
    pub key: &'static str,
    pub description: &'static str,
}

impl KeyBinding {
    pub const fn new(key: &'static str, description: &'static str) -> Self {
        Self { key, description }
    }
}

const CHAT_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("Enter", "Send message"),
    KeyBinding::new("Alt+↑/↓", "Scroll chat"),
    KeyBinding::new("PgUp/Dn", "Page scroll"),
    KeyBinding::new("t", "Toggle timeline"),
    KeyBinding::new("[/]", "Navigate tools"),
];

const AGENTS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Move up"),
    KeyBinding::new("↓/j", "Move down"),
    KeyBinding::new("Enter", "Activate agent"),
    KeyBinding::new("→/l", "Expand details"),
    KeyBinding::new("←/h", "Collapse"),
    KeyBinding::new("r", "Refresh"),
    KeyBinding::new("Tab", "Next tab"),
];

const ARTIFACTS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Move up"),
    KeyBinding::new("↓/j", "Move down"),
    KeyBinding::new("d", "Delete"),
    KeyBinding::new("r", "Refresh"),
    KeyBinding::new("Tab", "Next tab"),
];

const USERS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Move up"),
    KeyBinding::new("↓/j", "Move down"),
    KeyBinding::new("Tab", "Next tab"),
];

const CONVERSATIONS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Move up"),
    KeyBinding::new("↓/j", "Move down"),
    KeyBinding::new("Enter", "Select"),
    KeyBinding::new("e", "Edit name"),
    KeyBinding::new("d", "Delete"),
    KeyBinding::new("n", "New"),
    KeyBinding::new("r", "Refresh"),
    KeyBinding::new("Tab", "Next tab"),
];

const ANALYTICS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("←/h", "Prev view"),
    KeyBinding::new("→/l", "Next view"),
    KeyBinding::new("1-3", "Select view"),
    KeyBinding::new("↑/k", "Scroll up"),
    KeyBinding::new("↓/j", "Scroll down"),
    KeyBinding::new("r", "Refresh"),
    KeyBinding::new("Tab", "Next tab"),
];

const SERVICES_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Move up"),
    KeyBinding::new("↓/j", "Move down"),
    KeyBinding::new("→/l", "Expand group"),
    KeyBinding::new("←/h", "Collapse group"),
    KeyBinding::new("r", "Restart service"),
    KeyBinding::new("s", "Start service"),
    KeyBinding::new("x", "Stop service"),
    KeyBinding::new("Enter", "Refresh"),
    KeyBinding::new("Tab", "Next tab"),
];

const CONFIG_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Scroll up"),
    KeyBinding::new("↓/j", "Scroll down"),
    KeyBinding::new("Tab", "Next tab"),
];

const SHORTCUTS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Move up"),
    KeyBinding::new("↓/j", "Move down"),
    KeyBinding::new("Enter", "Execute command"),
    KeyBinding::new("Tab", "Next tab"),
];

const LOGS_BINDINGS: &[KeyBinding] = &[
    KeyBinding::new("↑/k", "Scroll up"),
    KeyBinding::new("↓/j", "Scroll down"),
    KeyBinding::new("PgUp/Dn", "Page scroll"),
    KeyBinding::new("g/G", "Top/Bottom"),
    KeyBinding::new("r", "Refresh logs"),
    KeyBinding::new("f", "Toggle follow"),
    KeyBinding::new("e/w/i/d", "Filter level"),
    KeyBinding::new("a", "Show all"),
    KeyBinding::new("c", "Clear logs"),
    KeyBinding::new("Tab", "Next tab"),
];

pub const fn get_tab_keybindings(tab: ActiveTab) -> &'static [KeyBinding] {
    match tab {
        ActiveTab::Chat => CHAT_BINDINGS,
        ActiveTab::Agents => AGENTS_BINDINGS,
        ActiveTab::Artifacts => ARTIFACTS_BINDINGS,
        ActiveTab::Users => USERS_BINDINGS,
        ActiveTab::Conversations => CONVERSATIONS_BINDINGS,
        ActiveTab::Analytics => ANALYTICS_BINDINGS,
        ActiveTab::Services => SERVICES_BINDINGS,
        ActiveTab::Config => CONFIG_BINDINGS,
        ActiveTab::Shortcuts => SHORTCUTS_BINDINGS,
        ActiveTab::Logs => LOGS_BINDINGS,
    }
}

pub fn render_control_guide(frame: &mut Frame, area: Rect, tab: ActiveTab, config: &TuiConfig) {
    let keybindings = get_tab_keybindings(tab);
    let mut lines: Vec<Line> = Vec::new();

    for binding in keybindings {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>10}", binding.key),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(binding.description, Style::default().fg(Color::Gray)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "── Global ──",
        Style::default().fg(Color::DarkGray),
    )));

    for binding in &[
        KeyBinding::new("/help", "Help"),
        KeyBinding::new("Ctrl+C", "Quit"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>10}", binding.key),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(binding.description, Style::default().fg(Color::DarkGray)),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_unfocused))
        .title(Span::styled(
            " Keys ",
            Style::default().fg(Color::Cyan).bold(),
        ))
        .title_alignment(Alignment::Center);

    let block_inner = block.inner(area);
    frame.render_widget(block, area);

    let inner = Rect {
        x: block_inner.x,
        y: block_inner.y + 1,
        width: block_inner.width,
        height: block_inner.height.saturating_sub(1),
    };

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
}

pub const fn control_guide_width() -> u16 {
    30
}
