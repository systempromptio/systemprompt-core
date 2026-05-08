//! Disk serialisation helpers used by the various `*_to_disk` operations.
//! Each submodule emits the YAML config + Markdown body layout expected by
//! the matching `*Local Sync`.

mod content;

pub use content::{export_content_to_file, generate_content_markdown};

pub fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
