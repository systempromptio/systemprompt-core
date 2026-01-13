use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::cli_registry::{
    build_command_tree, CliArgumentInfo, CliCommandInfo, CommandTreeItem, ExecutionMode,
};

#[derive(Debug)]
pub struct CommandsState {
    pub command_tree: CliCommandInfo,
    pub expanded_paths: HashSet<String>,
    pub visible_items: Vec<CommandTreeItem>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub output: Option<String>,
    pub output_scroll: usize,
    pub is_executing: bool,
    pub modal_state: Option<ParameterModalState>,
}

impl CommandsState {
    pub fn new() -> Self {
        let command_tree = build_command_tree();
        let expanded_paths = HashSet::new();

        let mut state = Self {
            command_tree,
            expanded_paths,
            visible_items: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            output: None,
            output_scroll: 0,
            is_executing: false,
            modal_state: None,
        };

        state.rebuild_visible_items();
        state
    }

    pub fn set_output(&mut self, output: String) {
        self.output = Some(output);
        self.output_scroll = 0;
        self.is_executing = false;
    }

    pub fn set_error(&mut self, error: &str) {
        self.output = Some(format!("Error: {error}"));
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

    pub fn rebuild_visible_items(&mut self) {
        self.visible_items.clear();
        let subcommands = self.command_tree.subcommands.clone();
        self.build_items_recursive(&subcommands, 0);
    }

    fn build_items_recursive(&mut self, commands: &[CliCommandInfo], depth: usize) {
        for cmd in commands {
            let path = cmd.full_path();

            if cmd.has_subcommands() {
                let is_expanded = self.expanded_paths.contains(&path);
                let child_count = Self::count_descendants(cmd);

                self.visible_items.push(CommandTreeItem::Domain {
                    name: cmd.name.clone(),
                    path: path.clone(),
                    is_expanded,
                    child_count,
                    depth,
                });

                if is_expanded {
                    self.build_items_recursive(&cmd.subcommands, depth + 1);
                }
            } else {
                self.visible_items.push(CommandTreeItem::Command {
                    info: cmd.clone(),
                    depth,
                });
            }
        }
    }

    fn count_descendants(cmd: &CliCommandInfo) -> usize {
        cmd.subcommands.iter().fold(0, |acc, sub| {
            if sub.has_subcommands() {
                acc + 1 + Self::count_descendants(sub)
            } else {
                acc + 1
            }
        })
    }

    pub fn select_next(&mut self) {
        if !self.visible_items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.visible_items.len();
            self.ensure_visible();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.visible_items.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.visible_items.len() - 1);
            self.ensure_visible();
        }
    }

    fn ensure_visible(&mut self) {
        let viewport_height = 20;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected_index - viewport_height + 1;
        }
    }

    pub fn toggle_expanded(&mut self) {
        let Some(item) = self.visible_items.get(self.selected_index) else {
            return;
        };

        if let CommandTreeItem::Domain { path, .. } = item {
            let path = path.clone();
            if self.expanded_paths.contains(&path) {
                self.expanded_paths.remove(&path);
            } else {
                self.expanded_paths.insert(path);
            }
            self.rebuild_visible_items();
        }
    }

    pub fn expand_current(&mut self) {
        let Some(item) = self.visible_items.get(self.selected_index) else {
            return;
        };

        if let CommandTreeItem::Domain {
            path, is_expanded, ..
        } = item
        {
            if !is_expanded {
                self.expanded_paths.insert(path.clone());
                self.rebuild_visible_items();
            }
        }
    }

    pub fn collapse_current(&mut self) {
        let Some(item) = self.visible_items.get(self.selected_index) else {
            return;
        };

        match item {
            CommandTreeItem::Domain {
                path, is_expanded, ..
            } => {
                if *is_expanded {
                    self.expanded_paths.remove(path);
                    self.rebuild_visible_items();
                }
            },
            CommandTreeItem::Command { info, .. } => {
                if info.path.len() > 1 {
                    let parent_path = info.path[..info.path.len() - 1].join(" ");
                    self.expanded_paths.remove(&parent_path);
                    self.rebuild_visible_items();

                    self.selected_index = self
                        .visible_items
                        .iter()
                        .position(|item| {
                            matches!(item, CommandTreeItem::Domain { path, .. } if *path == parent_path)
                        })
                        .unwrap_or(0);
                }
            },
        }
    }

    pub fn selected_item(&self) -> Option<&CommandTreeItem> {
        self.visible_items.get(self.selected_index)
    }

    pub fn selected_command(&self) -> Option<&CliCommandInfo> {
        match self.selected_item()? {
            CommandTreeItem::Command { info, .. } => Some(info),
            CommandTreeItem::Domain { .. } => None,
        }
    }

    pub fn open_parameter_modal(&mut self) {
        let Some(cmd) = self.selected_command().cloned() else {
            return;
        };

        self.modal_state = Some(ParameterModalState::new(cmd));
    }

    pub fn close_modal(&mut self) {
        self.modal_state = None;
    }

    pub const fn is_modal_open(&self) -> bool {
        self.modal_state.is_some()
    }
}

impl Default for CommandsState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ParameterModalState {
    pub command: CliCommandInfo,
    pub field_values: HashMap<String, String>,
    pub focused_field_index: usize,
    pub validation_errors: HashMap<String, String>,
    pub cursor_position: usize,
}

impl ParameterModalState {
    pub fn new(command: CliCommandInfo) -> Self {
        let mut field_values = HashMap::new();

        for arg in &command.arguments {
            if let Some(default) = &arg.default_value {
                field_values.insert(arg.name.to_string(), default.to_string());
            }
        }

        Self {
            command,
            field_values,
            focused_field_index: 0,
            validation_errors: HashMap::new(),
            cursor_position: 0,
        }
    }

    pub fn focused_field(&self) -> Option<&CliArgumentInfo> {
        self.command.arguments.get(self.focused_field_index)
    }

    pub fn focused_field_name(&self) -> Option<&Cow<'static, str>> {
        self.focused_field().map(|f| &f.name)
    }

    pub fn focused_value(&self) -> String {
        self.focused_field_name()
            .and_then(|name| self.field_values.get(name.as_ref()))
            .cloned()
            .unwrap_or_default()
    }

    pub fn next_field(&mut self) {
        if !self.command.arguments.is_empty() {
            self.focused_field_index =
                (self.focused_field_index + 1) % self.command.arguments.len();
            self.cursor_position = self.focused_value().len();
        }
    }

    pub fn prev_field(&mut self) {
        if !self.command.arguments.is_empty() {
            self.focused_field_index = self
                .focused_field_index
                .checked_sub(1)
                .unwrap_or(self.command.arguments.len() - 1);
            self.cursor_position = self.focused_value().len();
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let Some(field_name) = self.focused_field_name().map(ToString::to_string) else {
            return;
        };

        let value = self.field_values.entry(field_name).or_default();
        if self.cursor_position <= value.len() {
            value.insert(self.cursor_position, c);
            self.cursor_position += 1;
        }
    }

    pub fn delete_char(&mut self) {
        let Some(field_name) = self.focused_field_name().map(ToString::to_string) else {
            return;
        };

        let Some(value) = self.field_values.get_mut(&field_name) else {
            return;
        };

        if self.cursor_position > 0 && self.cursor_position <= value.len() {
            value.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        let max = self.focused_value().len();
        self.cursor_position = (self.cursor_position + 1).min(max);
    }

    pub fn validate(&mut self) -> bool {
        self.validation_errors.clear();

        for arg in &self.command.arguments {
            if arg.required {
                let value = self.field_values.get(arg.name.as_ref());
                if value.is_none() || value.is_some_and(|v| v.trim().is_empty()) {
                    self.validation_errors
                        .insert(arg.name.to_string(), "Required field".to_string());
                }
            }
        }

        self.validation_errors.is_empty()
    }

    pub fn build_command_string(&self) -> String {
        let mut parts: Vec<String> = vec!["systemprompt".to_string()];

        for path_part in &self.command.path {
            parts.push(path_part.to_string());
        }

        for arg in &self.command.arguments {
            let Some(value) = self.field_values.get(arg.name.as_ref()) else {
                continue;
            };

            if value.trim().is_empty() {
                continue;
            }

            match arg.arg_type {
                crate::cli_registry::CliArgType::Bool => {
                    if value == "true" || value == "1" || value.to_lowercase() == "yes" {
                        if let Some(long) = &arg.long {
                            parts.push(format!("--{long}"));
                        } else if let Some(short) = arg.short {
                            parts.push(format!("-{short}"));
                        }
                    }
                },
                _ => {
                    if let Some(long) = &arg.long {
                        parts.push(format!("--{long}"));
                        parts.push(value.clone());
                    } else if let Some(short) = arg.short {
                        parts.push(format!("-{short}"));
                        parts.push(value.clone());
                    } else if arg.required {
                        parts.push(value.clone());
                    }
                },
            }
        }

        parts.join(" ")
    }

    pub const fn execution_mode(&self) -> ExecutionMode {
        self.command.execution_mode
    }
}

#[derive(Debug, Clone)]
pub struct CommandItem {
    pub slash_command: String,
    pub description: String,
}
