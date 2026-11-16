//! Every scheduled or manual job run acts on documents as
//! [`DocumentActor::System`] named after the job.
//!
//! The document access service reads the actor from a task-local scope and
//! treats a call outside any scope as an unnamed, capped agent. The files
//! member contributes this [`JobRunScope`], which puts each job run inside
//! `act_as(System { name })`, so a job's document writes carry the job's
//! system identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::future::BoxFuture;
use systemprompt_traits::{JobResult, JobRunScope, ProviderResult};

use crate::documents::agents::{DocumentActor, act_as};

#[derive(Debug, Clone, Copy)]
pub(crate) struct DocumentActorScope;

impl JobRunScope for DocumentActorScope {
    fn name(&self) -> &'static str {
        "document_actor"
    }

    fn scope<'a>(
        &self,
        job: &'static str,
        run: BoxFuture<'a, ProviderResult<JobResult>>,
    ) -> BoxFuture<'a, ProviderResult<JobResult>> {
        Box::pin(act_as(DocumentActor::system(job), run))
    }
}
