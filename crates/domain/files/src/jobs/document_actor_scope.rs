//! Every scheduled or manual job run acts on documents as
//! [`DocumentActor::System`] named after the job.
//!
//! The document access service reads the actor from a task-local scope and
//! treats a call outside any scope as an unnamed, capped agent. Registering
//! this [`JobRunScope`] puts each job run inside `act_as(System { name })`
//! for as long as this crate is linked, so a job's document writes carry the
//! job's system identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::future::BoxFuture;
use systemprompt_traits::{JobResult, JobRunScope, ProviderResult, register_job_run_scope};

use crate::documents::agents::{DocumentActor, act_as};

struct DocumentActorScope;

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

register_job_run_scope!("files", &DocumentActorScope);
