//! An owned project root holding one profile copied from the test bootstrap
//! (optionally bound to a cloud tenant), entered as the working directory for
//! the life of the value.

use std::path::PathBuf;

use systemprompt_cloud::paths::ResolvedPaths;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_test_fixtures::ensure_test_bootstrap;

pub struct Project {
    previous: PathBuf,
    pub root: tempfile::TempDir,
    pub profile: PathBuf,
}

impl Project {
    pub fn new(username: &str, tenant: bool) -> Self {
        let boot = ensure_test_bootstrap();
        let root = systemprompt_test_fixtures::canonical_tempdir();
        let profile = root
            .path()
            .join(".systemprompt/profiles/coverage/profile.yaml");
        std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.path().join("services")).unwrap();
        let mut yaml = std::fs::read_to_string(&boot.profile_path)
            .unwrap()
            .replace("username: testadmin", &format!("username: {username}"));
        if tenant {
            yaml.push_str("\ncloud:\n  tenant_id: coverage-tenant\n  validation: strict\n");
        }
        std::fs::write(&profile, yaml).unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        Self {
            previous,
            root,
            profile,
        }
    }

    pub fn paths(&self) -> ResolvedPaths {
        ResolvedPaths::for_root(self.root.path())
    }

    pub fn context(&self, interactive: bool) -> CommandContext {
        CommandContext::new(
            CliConfig::new()
                .with_interactive(interactive)
                .with_output_format(OutputFormat::Json)
                .with_profile_override(Some(self.profile.display().to_string())),
            EnvOverrides::default(),
        )
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).unwrap();
    }
}
