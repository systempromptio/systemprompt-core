use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone)]
pub struct AppLayout {
    pub header: Rect,
    pub main_block: Rect,
    pub full_area: Rect,
}

impl AppLayout {
    pub fn new(area: Rect) -> Self {
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(10)])
            .split(area);

        Self {
            header: vertical_chunks[0],
            main_block: vertical_chunks[1],
            full_area: area,
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
