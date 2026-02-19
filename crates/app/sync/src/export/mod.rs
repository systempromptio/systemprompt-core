mod agents;
mod content;
mod skills;

pub use agents::{export_agent_to_disk, generate_agent_config, generate_agent_system_prompt};
pub use content::{export_content_to_file, generate_content_markdown};
pub use skills::{export_skill_to_disk, generate_skill_config, generate_skill_markdown};

pub fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
