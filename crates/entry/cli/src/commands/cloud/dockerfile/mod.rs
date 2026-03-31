mod builder;
mod validation;

use std::path::Path;

pub use builder::DockerfileBuilder;
pub use validation::{
    get_required_mcp_copy_lines, validate_dockerfile_has_mcp_binaries,
    validate_dockerfile_has_no_stale_binaries, validate_profile_dockerfile,
};

pub fn generate_dockerfile_content(project_root: &Path) -> String {
    DockerfileBuilder::new(project_root).build()
}

pub fn print_dockerfile_suggestion(project_root: &Path) {
    systemprompt_logging::CliService::info(&generate_dockerfile_content(project_root));
}
