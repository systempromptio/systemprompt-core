use crate::messages::SlashCommand;

#[derive(Debug)]
pub struct CommandsState {
    pub commands: Vec<CommandItem>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub output: Option<String>,
    pub output_scroll: usize,
    pub is_executing: bool,
}

impl CommandsState {
    pub fn new() -> Self {
        let commands = Self::build_command_list();
        Self {
            commands,
            selected_index: 0,
            scroll_offset: 0,
            output: None,
            output_scroll: 0,
            is_executing: false,
        }
    }

    pub fn set_output(&mut self, output: String) {
        self.output = Some(output);
        self.output_scroll = 0;
        self.is_executing = false;
    }

    pub fn set_error(&mut self, error: &str) {
        self.output = Some(format!("Error: {}", error));
        self.output_scroll = 0;
        self.is_executing = false;
    }

    pub fn clear_output(&mut self) {
        self.output = None;
        self.output_scroll = 0;
    }

    pub fn scroll_output_up(&mut self, amount: usize) {
        self.output_scroll = self.output_scroll.saturating_sub(amount);
    }

    pub fn scroll_output_down(&mut self, amount: usize) {
        self.output_scroll = self.output_scroll.saturating_add(amount);
    }

    fn build_command_list() -> Vec<CommandItem> {
        SlashCommand::commands_tab_list()
            .into_iter()
            .map(|(cmd, desc)| CommandItem {
                slash_command: cmd.to_string(),
                description: desc.to_string(),
            })
            .collect()
    }

    pub fn select_next(&mut self) {
        if !self.commands.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.commands.len();
            self.ensure_visible();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.commands.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.commands.len() - 1);
            self.ensure_visible();
        }
    }

    pub fn selected_command(&self) -> Option<&CommandItem> {
        self.commands.get(self.selected_index)
    }

    fn ensure_visible(&mut self) {
        let viewport_height = 20;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected_index - viewport_height + 1;
        }
    }

    pub fn execute_selected(&self) -> Option<SlashCommand> {
        self.selected_command()
            .and_then(|cmd| SlashCommand::from_str(&cmd.slash_command))
    }
}

impl Default for CommandsState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct CommandItem {
    pub slash_command: String,
    pub description: String,
}
