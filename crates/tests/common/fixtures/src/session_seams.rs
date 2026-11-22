//! A detached command kernel holding only the conversation-context seams the
//! agent member provides, over the test database: what a CLI session needs to
//! be created and revalidated.

use std::sync::Arc;

use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_agent::services::ContextProviderService;
use systemprompt_runtime::Kernel;
use systemprompt_traits::{BackgroundTasks, DynContextDirectory, DynSessionContextIssuer};

pub async fn session_seam_kernel() -> Arc<Kernel> {
    let pool = crate::test_db_pool().await;
    let service = Arc::new(ContextProviderService::new(ContextRepository::new(&pool)));
    let directory: DynContextDirectory = service.clone();
    let issuer: DynSessionContextIssuer = service;
    let seed = systemprompt_extension::prelude::Dependencies::new()
        .with(directory)
        .with(issuer);
    Arc::new(Kernel::detached(BackgroundTasks::new(), seed))
}
