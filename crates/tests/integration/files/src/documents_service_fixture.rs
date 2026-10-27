use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::service::{Author, CreateRequest, RevisionOrigin};
use systemprompt_files::documents::templates::TemplateRegistry;
use systemprompt_files::documents::{
    DocumentAccessRepositories, DocumentAccessService, DocumentEvents, DocumentService,
    DocumentServiceParts,
};
use systemprompt_governance::authz::{
    AccessControlRepository, AllowAllHook, DenyAllHook, RuleType, SharedAuthzHook,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentTemplateId, TraceId, UserId};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::authz::{
    SubjectAttributeSource, SubjectAttributes, SubjectDimension, SubjectSourceError,
};
use systemprompt_models::documents::ActorKind;
use systemprompt_provider_contracts::documents::TemplateParams;
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::test_db_pool;
use tempfile::TempDir;
use uuid::Uuid;

pub(crate) const GROUP: RuleType = RuleType::extension_static("group");

pub(crate) struct GroupSource {
    dimensions: Vec<SubjectDimension>,
    groups: HashMap<UserId, Vec<String>>,
}

impl GroupSource {
    pub(crate) fn new(groups: &[(&UserId, &str)]) -> Self {
        let mut by_user: HashMap<UserId, Vec<String>> = HashMap::new();
        for (user, group) in groups {
            by_user
                .entry((*user).clone())
                .or_default()
                .push((*group).to_owned());
        }
        Self {
            dimensions: vec![SubjectDimension {
                rule_type: GROUP,
                label: "Group",
                precedence: 150,
            }],
            groups: by_user,
        }
    }
}

#[async_trait]
impl SubjectAttributeSource for GroupSource {
    fn dimensions(&self) -> &[SubjectDimension] {
        &self.dimensions
    }

    async fn values_for(
        &self,
        user: &UserId,
        _dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        Ok(self.groups.get(user).cloned().unwrap_or_default())
    }

    async fn resolve_all(&self, user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        Ok(
            [(GROUP, self.groups.get(user).cloned().unwrap_or_default())]
                .into_iter()
                .collect(),
        )
    }
}

pub(crate) struct Harness {
    pub(crate) service: DocumentService,
    pub(crate) repos: Arc<DocumentRepositories>,
    pub(crate) access_entities: Arc<AccessControlRepository>,
    pub(crate) storage: Arc<TempDir>,
}

pub(crate) fn allow() -> SharedAuthzHook {
    Arc::new(AllowAllHook::null())
}

pub(crate) fn deny() -> SharedAuthzHook {
    Arc::new(DenyAllHook::null())
}

pub(crate) async fn harness(hook: SharedAuthzHook, subjects: GroupSource) -> Harness {
    let storage = Arc::new(TempDir::new().expect("tempdir"));
    harness_on(hook, subjects, storage).await
}

pub(crate) async fn harness_on(
    hook: SharedAuthzHook,
    subjects: GroupSource,
    storage: Arc<TempDir>,
) -> Harness {
    let db = test_db_pool().await;
    let repos = Arc::new(DocumentRepositories::new(&db));
    let access_entities = Arc::new(AccessControlRepository::new(&db));
    let access = Arc::new(DocumentAccessService::new(
        DocumentAccessRepositories {
            grants: Arc::clone(&repos.grants),
            folders: Arc::clone(&repos.folders),
            documents: Arc::clone(&repos.documents),
            access_entities: Arc::clone(&access_entities),
        },
        hook,
        Arc::new(subjects),
    ));
    let blobs = Arc::new(BlobStore::new(
        Arc::new(LocalFileStorage::new(storage.path().to_path_buf())),
        repos.blobs.clone(),
    ));
    let events = Arc::new(DocumentEvents::new(Arc::new(EventRouter::local_only(
        Arc::new(EventBroadcasters::new()),
    ))));
    let service = DocumentService::new(DocumentServiceParts {
        repos: Arc::clone(&repos),
        blobs,
        codecs: Arc::new(CodecRegistry::discover()),
        templates: Arc::new(TemplateRegistry::discover(Vec::new())),
        access,
        events,
        config: DocumentsConfig::default(),
    });
    Harness {
        service,
        repos,
        access_entities,
        storage,
    }
}

pub(crate) fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

pub(crate) fn person(prefix: &str) -> AuthenticatedUser {
    let id = unique(prefix);
    AuthenticatedUser::new(
        UserId::new(id.clone()),
        id.clone(),
        format!("{id}@example.test"),
        vec![Permission::User],
    )
}

pub(crate) fn trace() -> TraceId {
    TraceId::generate()
}

pub(crate) fn api_author() -> Author {
    Author {
        actor_kind: ActorKind::User,
        origin: RevisionOrigin::Api,
    }
}

pub(crate) fn create(template: &str, folder: Option<&DocumentFolderId>) -> CreateRequest {
    CreateRequest {
        template_id: DocumentTemplateId::new(template),
        folder: folder.cloned(),
        name: unique("doc"),
        params: TemplateParams::default(),
        author: api_author(),
    }
}
