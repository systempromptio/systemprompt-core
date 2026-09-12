//! Disjoint current-request and historical safety surfaces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CanonicalRequest;
use super::request::flatten_part;

impl CanonicalRequest {
    pub fn safety_parts(&self, history: bool) -> Vec<(String, String)> {
        let leaves = self.forwarded_surface.leaves();
        if !leaves.is_empty() {
            let newest = leaves
                .iter()
                .filter_map(|leaf| message_index(&leaf.path))
                .max();
            return leaves
                .iter()
                .filter(|leaf| {
                    let historical =
                        message_index(&leaf.path).is_some_and(|index| Some(index) != newest);
                    historical == history
                })
                .map(|leaf| (leaf.path.clone(), leaf.value.clone()))
                .collect();
        }
        let mut parts = Vec::new();
        if !history && let Some(system) = &self.system {
            parts.push(("system".to_owned(), system.clone()));
        }
        for (index, message) in self.messages.iter().enumerate() {
            if (index + 1 < self.messages.len()) != history {
                continue;
            }
            let mut text = String::new();
            for part in &message.content {
                flatten_part(&mut text, part);
            }
            if !text.is_empty() {
                parts.push((format!("messages[{index}]"), text));
            }
        }
        parts
    }
}

fn message_index(path: &str) -> Option<usize> {
    ["$.messages[", "$.contents[", "$.input["]
        .iter()
        .find_map(|prefix| path.strip_prefix(prefix))?
        .split_once(']')?
        .0
        .parse()
        .ok()
}
