use std::sync::Arc;

use serde_json::json;
use systemprompt_brain::{BrainExtension, BrainHandle, BrainHandleSlot, FactRepository, NewFact};
use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::brain::{SourceAccess, SourceContent};
use systemprompt_extension::{DynNotificationSink, Extension};
use systemprompt_files::documents::{
    DocumentSourceAccess, DocumentSourceContent, DocumentsHandles, DocumentsHandlesSlot,
};
use systemprompt_identifiers::{BrainFactId, FactKindId, TopicId, UserId};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{FactCandidate, FactScope, FactSource, FactStatus, SubjectKey};
use systemprompt_runtime::Kernel;
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::{BackgroundTasks, CoreSeam, FileStorage, core_seam};
use systemprompt_users::UsersExtension;

// The brain over the kernel seeds a host composes: the users member (its role
// audience and role directory) and the brain member resolve over
// `fixture_kernel_seed`, with the profile, a fresh instance id and the
// producer notification sink the test chooses. With a document store the
// files member's document source seams are present, as the files member
// provides them in a host.
pub fn fixture_brain_kernel(
    pool: &DbPool,
    documents: Option<&DocumentsHandles>,
    notifications: DynNotificationSink,
    profile: BrainProfile,
) -> Kernel {
    fixture_brain_kernel_with(pool, documents, notifications, profile, Vec::new())
}

// As `fixture_brain_kernel`, with further members (a probe contributing brain
// seams, say) admitted after the brain member.
pub fn fixture_brain_kernel_with(
    pool: &DbPool,
    documents: Option<&DocumentsHandles>,
    notifications: DynNotificationSink,
    profile: BrainProfile,
    members: Vec<Box<dyn Extension>>,
) -> Kernel {
    let file_storage: Arc<dyn FileStorage> = Arc::new(LocalFileStorage::new(std::env::temp_dir()));
    let (documents_slot, _binder) = DocumentsHandlesSlot::unbound();
    let mut seed = crate::app_context::fixture_kernel_seed(
        pool,
        crate::app_context::FixtureSeedParts {
            file_storage,
            background_tasks: BackgroundTasks::new(),
            event_router: EventRouter::local_only(Arc::new(EventBroadcasters::new())),
            documents: documents_slot,
        },
    );
    seed.insert(Arc::new(profile));
    seed.insert(crate::unique_instance());
    seed.insert(notifications);
    if let Some(documents) = documents {
        let content: Arc<dyn SourceContent> = Arc::new(DocumentSourceContent::new(
            documents.service.clone(),
            Arc::clone(&documents.repositories),
        ));
        let access: Arc<dyn SourceAccess> = Arc::new(DocumentSourceAccess::new(
            Arc::clone(&documents.access),
            Arc::clone(&documents.repositories),
        ));
        seed.insert(CoreSeam::<
            dyn SourceContent,
            core_seam::DocumentSourceContent,
        >::new(content));
        seed.insert(CoreSeam::<dyn SourceAccess, core_seam::DocumentSourceAccess>::new(access));
    }
    let mut admitted: Vec<Box<dyn Extension>> =
        vec![Box::new(UsersExtension), Box::new(BrainExtension)];
    admitted.extend(members);
    let admitted = crate::app_context::fixture_admitted_of(admitted);
    crate::composed::resolved_kernel(pool, &admitted, seed)
        .expect("the users and brain members resolve over the fixture seeds")
}

pub fn fixture_brain_slot(kernel: &Kernel) -> Arc<BrainHandleSlot> {
    kernel
        .resolved()
        .get::<Arc<BrainHandleSlot>>()
        .expect("the brain slot build returns its handle")
}

pub fn fixture_brain(
    pool: &DbPool,
    documents: Option<&DocumentsHandles>,
    notifications: DynNotificationSink,
    profile: BrainProfile,
) -> Option<Arc<BrainHandle>> {
    let kernel = fixture_brain_kernel(pool, documents, notifications, profile);
    fixture_brain_slot(&kernel).get().cloned()
}

// A personal decision fact the owner entered, inserted straight into the
// store at the given status.
pub async fn seed_brain_fact(
    pool: &DbPool,
    owner: &UserId,
    status: FactStatus,
    subject: &str,
) -> anyhow::Result<BrainFactId> {
    let subject: SubjectKey = subject.parse()?;
    let candidate = FactCandidate::builder(
        FactKindId::try_new("decision")?,
        subject.clone(),
        FactSource::EnteredBy {
            user_id: owner.clone(),
        },
    )
    .field("owner", json!("billing"))
    .topic(TopicId::try_new("ops")?)
    .extractor("wp7")
    .build();
    let fact = NewFact::builder(
        candidate,
        FactScope::User(owner.clone()),
        "Billing owns the deploy gate.",
        format!("identity:{subject}:{}", uuid::Uuid::new_v4().simple()),
    )
    .status(status)
    .importance_base(0.5)
    .owner(owner.clone())
    .build();
    let mut conn = pool.write_pool().acquire().await?;
    Ok(FactRepository::insert_candidate(&mut conn, &fact).await?.id)
}
