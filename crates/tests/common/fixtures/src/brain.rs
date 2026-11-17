use std::sync::Arc;

use systemprompt_brain::consumer::BrainConsumerSlot;
use systemprompt_brain::{BrainHandle, BrainHandleSlot, BrainSlot};
use systemprompt_database::DbPool;
use systemprompt_extension::brain::{SourceAccess, SourceContent};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_extension::{DynNotificationSink, NotificationAudienceResolver, Slot};
use systemprompt_files::documents::{DocumentSourceAccess, DocumentSourceContent};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_runtime::documents::DocumentsRuntime;
use systemprompt_traits::core_seam::{self, RoleAudience};
use systemprompt_traits::CoreSeam;
use systemprompt_users::{UserRepository, UserRoleAudienceResolver};

pub fn fixture_brain(
    pool: &DbPool,
    documents: Option<&Arc<DocumentsRuntime>>,
    notifications: DynNotificationSink,
    profile: BrainProfile,
) -> Option<Arc<BrainHandle>> {
    let handle = BrainHandleSlot::default();
    let roles: Arc<dyn NotificationAudienceResolver> = Arc::new(UserRoleAudienceResolver::new(
        Arc::new(UserRepository::new(pool)),
    ));
    let mut seed = Dependencies::new()
        .with(Arc::clone(pool))
        .with(crate::unique_instance())
        .with(Arc::new(profile))
        .with(notifications)
        .with(BrainConsumerSlot::default())
        .with(handle.clone())
        .with(CoreSeam::<dyn NotificationAudienceResolver, RoleAudience>::new(roles));
    if let Some(documents) = documents {
        let content: Arc<dyn SourceContent> = Arc::new(DocumentSourceContent::new(
            Some(Arc::clone(documents.service())),
            Arc::clone(documents.repositories()),
        ));
        let access: Arc<dyn SourceAccess> = Arc::new(DocumentSourceAccess::new(
            Arc::clone(documents.access()),
            Arc::clone(documents.repositories()),
        ));
        seed.insert(CoreSeam::<
            dyn SourceContent,
            core_seam::DocumentSourceContent,
        >::new(content));
        seed.insert(CoreSeam::<dyn SourceAccess, core_seam::DocumentSourceAccess>::new(access));
    }
    let audiences = systemprompt_extension::AudienceResolvers::from_admitted(
        &systemprompt_extension::BuiltState::default(),
        &crate::app_context::fixture_kernel(pool, seed.clone()).resolved(),
    )
    .expect("the fixture audience set builds");
    seed.insert(Arc::new(audiences));
    let kernel = crate::app_context::fixture_kernel(pool, seed);
    BrainSlot::build(Vec::new(), &kernel.resolved())
        .expect("the brain slot builds over the fixture seeds");
    handle.get().cloned()
}
