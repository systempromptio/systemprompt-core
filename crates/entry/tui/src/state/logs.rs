use std::collections::VecDeque;

use crate::messages::{LogEntry, LogLevel};

#[derive(Debug)]
pub struct LogsState {
    pub entries: VecDeque<LogEntry>,
    pub max_entries: usize,
    pub filter_level: Option<LogLevel>,
    pub filter_module: Option<String>,
    pub scroll_offset: usize,
    pub follow_tail: bool,
    pub search_query: Option<String>,
}

impl LogsState {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            filter_level: None,
            filter_module: None,
            scroll_offset: 0,
            follow_tail: true,
            search_query: None,
        }
    }

    pub fn add_entry(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);

        if self.follow_tail {
            self.scroll_to_bottom();
        }
    }

    pub fn add_entries(&mut self, entries: Vec<LogEntry>) {
        let entry_count = entries.len();

        for entry in entries {
            if self.entries.len() >= self.max_entries {
                self.entries.pop_front();
            }
            self.entries.push_back(entry);
        }

        if self.follow_tail && entry_count > 0 {
            self.scroll_to_bottom();
        }
    }

    pub fn filtered_entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().filter(|entry| {
            if let Some(level) = self.filter_level {
                if entry.level != level {
                    return false;
                }
            }
            if let Some(module) = &self.filter_module {
                if !entry.module.contains(module) {
                    return false;
                }
            }
            if let Some(query) = &self.search_query {
                if !entry.message.to_lowercase().contains(&query.to_lowercase()) {
                    return false;
                }
            }
            true
        })
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered_entries().count()
    }

    pub fn set_level_filter(&mut self, level: Option<LogLevel>) {
        self.filter_level = level;
        self.scroll_offset = 0;
    }

    pub fn set_module_filter(&mut self, module: Option<String>) {
        self.filter_module = module;
        self.scroll_offset = 0;
    }

    pub fn set_search(&mut self, query: Option<String>) {
        self.search_query = query;
        self.scroll_offset = 0;
    }

    pub fn clear_filters(&mut self) {
        self.filter_level = None;
        self.filter_module = None;
        self.search_query = None;
        self.scroll_offset = 0;
    }

    pub fn toggle_follow(&mut self) {
        self.follow_tail = !self.follow_tail;
        if self.follow_tail {
            self.scroll_to_bottom();
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        let count = self.filtered_count();
        self.scroll_offset = count.saturating_sub(1);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.follow_tail = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max = self.filtered_count().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + amount).min(max);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0;
    }
}

impl Default for LogsState {
    fn default() -> Self {
        Self::new(1000)
    }
}
