mod content;
mod playbooks;
mod skills;

pub use content::{export_content_to_file, generate_content_markdown};
pub use playbooks::{export_playbook_to_disk, generate_playbook_markdown};
pub use skills::{export_skill_to_disk, generate_skill_config, generate_skill_markdown};

pub fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
