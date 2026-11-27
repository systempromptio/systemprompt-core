//! The brain extension wired over a fake instance as the composition root
//! wires it: provided, applied, and its probe running.

use std::sync::Arc;

use serde_json::json;
use systemprompt_engine::run::EngineExtension;
use systemprompt_harness::extension::{CancellationToken, ManifestApplier, SpContext, SpExtension};
use systemprompt_harness::members::brain::{BrainExtension, BrainService, BrainStatus};
use systemprompt_traits::OwnedTask;
use tempfile::TempDir;

use super::{FakeInstance, FakeRequest, FakeResponse, applied, context, eventually};

pub(crate) struct Rig {
    pub(crate) fake: FakeInstance,
    pub(crate) ctx: SpContext,
    pub(crate) service: Arc<BrainService>,
    pub(crate) engine: Arc<dyn EngineExtension>,
    pub(crate) applier: Arc<dyn ManifestApplier>,
    cancel: CancellationToken,
    _probe: OwnedTask<()>,
    _root: TempDir,
}

pub(crate) fn listing(request: &FakeRequest) -> Option<FakeResponse> {
    (request.method == "GET" && request.path() == "/api/v1/brain/facts")
        .then(|| FakeResponse::json(200, &json!({ "facts": [], "next": null })))
}

impl Rig {
    pub(crate) async fn start<H>(handler: H) -> Self
    where
        H: Fn(&FakeRequest) -> FakeResponse + Send + Sync + 'static,
    {
        let fake = FakeInstance::start(handler).await;
        let root = tempfile::tempdir().unwrap();
        let ctx = context(root.path());
        BrainExtension.provide(&ctx);
        let service = ctx.handles.get::<BrainService>().unwrap();
        let engine = BrainExtension.engine(&ctx).unwrap();
        let applier = BrainExtension.appliers(&ctx).pop().unwrap();
        let probe = BrainExtension.services(&ctx).pop().unwrap();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let task = OwnedTask::spawn("brain-probe", async move { probe.run(token).await });
        applier.apply(&applied(&fake.url)).unwrap();
        Self {
            fake,
            ctx,
            service,
            engine,
            applier,
            cancel,
            _probe: task,
            _root: root,
        }
    }

    pub(crate) async fn settled(&self) -> BrainStatus {
        eventually("the probe settles", || {
            self.service.status() != BrainStatus::Checking
        })
        .await;
        self.service.status()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
