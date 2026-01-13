use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use systemprompt_cloud::constants::build;
use systemprompt_cloud::ProjectContext;

use crate::shared::project::ProjectRoot;

#[derive(Debug)]
pub struct DeployConfig {
    pub binary: PathBuf,
    pub web_dist: PathBuf,
    pub web_images: PathBuf,
    pub dockerfile: PathBuf,
}

impl DeployConfig {
    pub fn from_project(project: &ProjectRoot, profile_name: &str) -> Result<Self> {
        let root = project.as_path();
        let binary = root
            .join(build::CARGO_TARGET)
            .join("release")
            .join(build::BINARY_NAME);
        let web_dist = root.join(build::WEB_DIST);
        let web_images = root.join(build::WEB_IMAGES);

        let ctx = ProjectContext::new(root.to_path_buf());
        let dockerfile = ctx.profile_dockerfile(profile_name);

        let config = Self {
            binary,
            web_dist,
            web_images,
            dockerfile,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if !self.binary.exists() {
            return Err(anyhow!(
                "Release binary not found: {}\n\nRun: cargo build --release --bin systemprompt",
                self.binary.display()
            ));
        }

        if !self.web_dist.exists() {
            return Err(anyhow!(
                "Web dist not found: {}\n\nRun: npm run build",
                self.web_dist.display()
            ));
        }

        let index_html = self.web_dist.join("index.html");
        if !index_html.exists() {
            return Err(anyhow!(
                "Web dist missing index.html: {}\n\nRun: npm run build",
                self.web_dist.display()
            ));
        }

        if !self.web_images.exists() {
            return Err(anyhow!(
                "Web images directory not found: {}\n\nEnsure core/web/src/assets/images/ exists \
                 with blog/, social/, and logos/ subdirectories",
                self.web_images.display()
            ));
        }

        self.validate_images_structure()?;

        if !self.dockerfile.exists() {
            return Err(anyhow!(
                "Dockerfile not found: {}\n\nCreate a Dockerfile at this location",
                self.dockerfile.display()
            ));
        }

        Ok(())
    }

    fn validate_images_structure(&self) -> Result<()> {
        let logos_path = self.web_images.join("logos");
        if !logos_path.exists() {
            bail!(
                "Web images missing logos/ subdirectory: {}\n\nRequired structure:\n  {}/\n    \
                 logos/",
                logos_path.display(),
                self.web_images.display()
            );
        }
        Ok(())
    }
}
