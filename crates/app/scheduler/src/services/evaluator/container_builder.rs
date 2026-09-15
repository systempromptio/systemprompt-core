//! Builder for a verified, labelled evaluator container launch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ClientVerifier, ContainerLaunch, PathBuf, PinnedClientVerifier, SchedulerError,
    SchedulerResult, safe_label, safe_name,
};

#[derive(Debug)]
pub struct ContainerLaunchBuilder {
    docker: PathBuf,
    directory: PathBuf,
    image: Option<String>,
    network: Option<String>,
    name: Option<String>,
    output_stem: String,
    owner_label: String,
    execution_label: String,
    lease: Option<systemprompt_evaluation::repository::experiments::ExecutionLease>,
    verifier: std::sync::Arc<dyn ClientVerifier>,
}

impl ContainerLaunchBuilder {
    pub(super) fn new(docker: PathBuf, directory: PathBuf) -> Self {
        Self {
            docker,
            directory,
            image: None,
            network: None,
            name: None,
            output_stem: "client".to_owned(),
            owner_label: String::new(),
            execution_label: String::new(),
            lease: None,
            verifier: std::sync::Arc::new(PinnedClientVerifier),
        }
    }

    pub fn verifier(mut self, verifier: std::sync::Arc<dyn ClientVerifier>) -> Self {
        self.verifier = verifier;
        self
    }

    pub fn image(mut self, image: String) -> Self {
        self.image = Some(image);
        self
    }
    pub fn network(mut self, network: String) -> Self {
        self.network = Some(network);
        self
    }
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    pub fn output_stem(mut self, output_stem: impl Into<String>) -> Self {
        self.output_stem = output_stem.into();
        self
    }
    pub fn lease(
        mut self,
        lease: &systemprompt_evaluation::repository::experiments::ExecutionLease,
    ) -> Self {
        self.lease = Some(lease.clone());
        self
    }

    pub fn ownership(mut self, owner: impl Into<String>, execution: impl Into<String>) -> Self {
        self.owner_label = owner.into();
        self.execution_label = execution.into();
        self
    }
    pub fn build(self) -> SchedulerResult<ContainerLaunch> {
        let invalid = || {
            SchedulerError::ConfigError { message: "Evaluator requires an absolute Docker path, workspace, pinned image, private network and execution name".to_owned() }
        };
        let image = self.image.ok_or_else(invalid)?;
        let digest = systemprompt_evaluation::capabilities::proofs::ImmutableImage::parse(&image)
            .map_err(|error| {
                SchedulerError::config_error(format!(
                    "Evaluator requires a pinned immutable image: {error}"
                ))
            })?
            .digest();
        let network = self.network.ok_or_else(invalid)?;
        let name = self.name.ok_or_else(invalid)?;
        #[cfg(unix)]
        let runtime_user = {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(&self.directory)?;
            if metadata.uid() == 0 {
                return Err(SchedulerError::config_error(
                    "Evaluator supervisor must not run as root",
                ));
            }
            format!("{}:{}", metadata.uid(), metadata.gid())
        };
        #[cfg(not(unix))]
        let runtime_user = "1001:1001".to_owned();
        if digest.len() != 64
            || !digest.bytes().all(|c| c.is_ascii_hexdigit())
            || !self.docker.is_absolute()
            || !self.directory.is_absolute()
            || self.directory.to_string_lossy().contains(',')
            || !name.starts_with("eval-")
            || !safe_name(&name)
            || !safe_name(&network)
            || matches!(network.as_str(), "host" | "bridge" | "default" | "none")
            || !safe_name(&self.output_stem)
            || !safe_label(&self.owner_label)
            || !safe_label(&self.execution_label)
            || self.lease.as_ref().is_some_and(|lease| {
                lease.execution_id.as_str() != self.execution_label
                    || lease.fencing_token <= 0
                    || !safe_label(lease.worker_id.as_str())
            })
        {
            return Err(invalid());
        }
        Ok(ContainerLaunch {
            docker: self.docker,
            directory: self.directory,
            image,
            network,
            name,
            output_stem: self.output_stem,
            owner_label: self.owner_label,
            execution_label: self.execution_label,
            lease: self.lease,
            runtime_user,
            verifier: self.verifier,
        })
    }
}
