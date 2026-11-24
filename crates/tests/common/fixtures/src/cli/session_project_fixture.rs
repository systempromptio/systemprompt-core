//! An owned project root holding one profile copied from the test bootstrap
//! (optionally bound to a cloud tenant), entered as the working directory for
//! the life of the value.

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_runtime::Kernel;

use crate::ensure_test_bootstrap;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_cloud::paths::ResolvedPaths;
use systemprompt_extension::cli::{CliConfig, OutputFormat};

pub struct Project {
    previous: PathBuf,
    pub root: tempfile::TempDir,
    pub profile: PathBuf,
    seams: Option<Arc<Kernel>>,
}

impl Project {
    pub fn new(username: &str, tenant: bool) -> Self {
        let boot = ensure_test_bootstrap();
        let root = crate::canonical_tempdir();
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
            seams: None,
        }
    }

    pub async fn with_session_seams(mut self) -> Self {
        self.seams = Some(crate::session_seams::session_seam_kernel().await);
        self
    }

    pub fn paths(&self) -> ResolvedPaths {
        ResolvedPaths::for_root(self.root.path())
    }

    pub fn context(&self, interactive: bool) -> CommandContext {
        let ctx = CommandContext::new(
            CliConfig::new()
                .with_interactive(interactive)
                .with_output_format(OutputFormat::Json)
                .with_profile_override(Some(self.profile.display().to_string())),
            EnvOverrides::default(),
        )
        .with_admitted(Some(crate::fixture_admitted()));
        match &self.seams {
            Some(kernel) => ctx.with_kernel(Arc::clone(kernel)),
            None => ctx.with_detached_kernel(),
        }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).unwrap();
    }
}
