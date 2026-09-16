//! Pinned native client invocations; suite data never supplies shell commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_identifiers::ModelId;

#[derive(Debug, Clone)]
pub struct NativeClient {
    kind: ClientKind,
    model: ModelId,
    limits: ExecutionLimits,
    pins: Option<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPurpose {
    Execution,
    Judge,
    Suggestion,
}

impl NativeClient {
    pub fn builder(kind: ClientKind, model: ModelId) -> NativeClientBuilder {
        NativeClientBuilder {
            client: Self {
                kind,
                model,
                limits: ExecutionLimits::default(),
                pins: None,
            },
        }
    }

    pub fn arguments(&self, prompt: &str) -> systemprompt_evaluation::Result<Vec<OsString>> {
        self.arguments_for(ClientPurpose::Execution, prompt)
    }

    pub fn arguments_for(
        &self,
        purpose: ClientPurpose,
        prompt: &str,
    ) -> systemprompt_evaluation::Result<Vec<OsString>> {
        self.adapter()?
            .arguments(&super::adapters::AdapterInvocation {
                model: &self.model,
                limits: &self.limits,
                purpose,
                prompt,
            })
    }

    pub fn adapter(
        &self,
    ) -> systemprompt_evaluation::Result<&'static dyn super::adapters::NativeAdapter> {
        super::adapters::adapter(self.kind)
    }

    pub fn admitted_target(
        &self,
        image: &str,
    ) -> systemprompt_evaluation::Result<
        &'static systemprompt_evaluation::capabilities::VerifiedNativeTarget,
    > {
        let Some((version, digest)) = &self.pins else {
            return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
                "Unpinned native execution is unsupported".to_owned(),
            ));
        };
        let image_digest =
            systemprompt_evaluation::capabilities::proofs::ImmutableImage::parse(image)?.digest();
        systemprompt_evaluation::capabilities::verified_native_targets()
            .iter()
            .find(|target| {
                target.validate().is_ok()
                    && target.client == self.kind
                    && &target.client_version == version
                    && &target.image_digest == digest
                    && image_digest == digest.as_str()
                    && systemprompt_evaluation::capabilities::proofs::image_config_for_target(
                        target, image,
                    )
                    .is_ok()
                    && target.supports_platform(std::env::consts::OS, std::env::consts::ARCH)
            })
            .ok_or_else(|| {
                systemprompt_evaluation::EvaluationError::InvalidSpec(
                    "Native target has no matching retained isolation and metering proofs"
                        .to_owned(),
                )
            })
    }

    pub const fn limits(&self) -> &ExecutionLimits {
        &self.limits
    }
}

#[derive(Debug)]
pub struct NativeClientBuilder {
    client: NativeClient,
}

impl NativeClientBuilder {
    pub fn pinned(mut self, version: String, image_digest: String) -> Self {
        self.client.pins = Some((version, image_digest));
        self
    }

    pub const fn limits(mut self, limits: ExecutionLimits) -> Self {
        self.client.limits = limits;
        self
    }
    pub fn build(self) -> systemprompt_evaluation::Result<NativeClient> {
        self.client.limits.validate()?;
        Ok(self.client)
    }
}
