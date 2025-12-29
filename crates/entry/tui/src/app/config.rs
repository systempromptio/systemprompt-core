use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Debug, Clone, Copy, Default)]
pub struct TuiConfig {
    pub layout: LayoutConfig,
    pub theme: ThemeConfig,
    pub keybindings: KeybindingsConfig,
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    pub chat_width_percent: u16,
    pub sidebar_width_percent: u16,
    pub logs_height_percent: u16,
    pub min_chat_width: u16,
    pub min_sidebar_width: u16,
    pub sidebar_visible: bool,
    pub logs_visible: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            chat_width_percent: 70,
            sidebar_width_percent: 30,
            logs_height_percent: 20,
            min_chat_width: 40,
            min_sidebar_width: 25,
            sidebar_visible: true,
            logs_visible: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeConfig {
    pub status_running: ratatui::style::Color,
    pub status_stopped: ratatui::style::Color,
    pub status_error: ratatui::style::Color,
    pub log_error: ratatui::style::Color,
    pub log_warn: ratatui::style::Color,
    pub log_info: ratatui::style::Color,
    pub log_debug: ratatui::style::Color,
    pub user_message: ratatui::style::Color,
    pub assistant_message: ratatui::style::Color,
    pub tool_call: ratatui::style::Color,
    pub border_focused: ratatui::style::Color,
    pub border_unfocused: ratatui::style::Color,
    pub brand_primary: ratatui::style::Color,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        use ratatui::style::Color;
        Self {
            status_running: Color::Green,
            status_stopped: Color::Red,
            status_error: Color::Red,
            log_error: Color::Red,
            log_warn: Color::Yellow,
            log_info: Color::Blue,
            log_debug: Color::Gray,
            user_message: Color::Cyan,
            assistant_message: Color::Magenta,
            tool_call: Color::Magenta,
            border_focused: Color::Cyan,
            border_unfocused: Color::DarkGray,
            brand_primary: Color::Rgb(251, 156, 52),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeybindingsConfig {
    pub quit: Keybinding,
    pub toggle_logs: Keybinding,
    pub toggle_sidebar: Keybinding,
    pub command_palette: Keybinding,
    pub show_help: Keybinding,
    pub next_tab: Keybinding,
    pub prev_tab: Keybinding,
    pub send_message: Keybinding,
    pub cancel: Keybinding,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            quit: Keybinding::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
            toggle_logs: Keybinding::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            toggle_sidebar: Keybinding::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            command_palette: Keybinding::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            show_help: Keybinding::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            next_tab: Keybinding::new(KeyCode::Tab, KeyModifiers::NONE),
            prev_tab: Keybinding::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            send_message: Keybinding::new(KeyCode::Enter, KeyModifiers::NONE),
            cancel: Keybinding::new(KeyCode::Esc, KeyModifiers::NONE),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Keybinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Keybinding {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.code == code && self.modifiers == modifiers
    }
}
