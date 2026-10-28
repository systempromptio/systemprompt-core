use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::RevisionAttribution;
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::service::{Author, CreateRequest, RevisionOrigin};
use systemprompt_files::documents::templates::TemplateRegistry;
use systemprompt_files::documents::{
    DocumentAccessRepositories, DocumentAccessService, DocumentEvents, DocumentService,
    DocumentServiceParts, SubjectDirectory, SubjectDirectoryError, SubjectMatch,
};
use systemprompt_files::uploads::{UploadService, UploadServiceParts};
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
use systemprompt_traits::{BackgroundTasks, FileStorage};
use tempfile::TempDir;
use uuid::Uuid;

pub(crate) const GROUP: RuleType = RuleType::extension_static("group");

pub(crate) struct GroupSource {
    dimensions: Vec<SubjectDimension>,
    groups: HashMap<UserId, Vec<String>>,
    known: Option<Vec<String>>,
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
            known: None,
        }
    }

    pub(crate) fn enumerable(mut self, known: &[&str]) -> Self {
        self.known = Some(known.iter().map(|value| (*value).to_owned()).collect());
        self
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

    async fn known_values(
        &self,
        _dimension: &SubjectDimension,
    ) -> Result<Option<Vec<String>>, SubjectSourceError> {
        Ok(self.known.clone())
    }
}

#[derive(Default)]
pub(crate) struct RoleDirectory {
    roles: HashMap<String, Vec<UserId>>,
    users: Vec<(UserId, String)>,
}

impl RoleDirectory {
    pub(crate) fn with_role(mut self, role: &str, members: &[&UserId]) -> Self {
        self.roles.insert(
            role.to_owned(),
            members.iter().map(|user| (*user).clone()).collect(),
        );
        self
    }

    pub(crate) fn with_user(mut self, user: &UserId, display: &str) -> Self {
        self.users.push((user.clone(), display.to_owned()));
        self
    }
}

#[async_trait]
impl SubjectDirectory for RoleDirectory {
    async fn roles_of(&self, user: &UserId) -> Result<Option<Vec<String>>, SubjectDirectoryError> {
        Ok(Some(
            self.roles
                .iter()
                .filter(|(_, members)| members.contains(user))
                .map(|(role, _)| role.clone())
                .collect(),
        ))
    }

    async fn members(
        &self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<UserId>, SubjectDirectoryError> {
        Ok(match kind {
            "user" => vec![UserId::new(value)],
            "role" => self
                .roles
                .get(value)
                .map(|members| members.iter().take(limit).cloned().collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        })
    }

    async fn search_users(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SubjectMatch>, SubjectDirectoryError> {
        let query = query.to_lowercase();
        Ok(self
            .users
            .iter()
            .filter(|(_, display)| display.to_lowercase().starts_with(&query))
            .take(limit)
            .map(|(user, display)| SubjectMatch {
                value: user.as_str().to_owned(),
                display: display.clone(),
                detail: None,
            })
            .collect())
    }

    async fn roles(&self) -> Result<Vec<String>, SubjectDirectoryError> {
        let mut roles: Vec<String> = self.roles.keys().cloned().collect();
        roles.sort();
        Ok(roles)
    }
}

pub(crate) struct Harness {
    pub(crate) service: DocumentService,
    pub(crate) repos: Arc<DocumentRepositories>,
    pub(crate) access_entities: Arc<AccessControlRepository>,
    pub(crate) storage: Arc<TempDir>,
    pub(crate) broadcasters: Arc<EventBroadcasters>,
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
    harness_with(hook, subjects, storage, RoleDirectory::default()).await
}

pub(crate) async fn harness_with(
    hook: SharedAuthzHook,
    subjects: GroupSource,
    storage: Arc<TempDir>,
    directory: RoleDirectory,
) -> Harness {
    harness_configured(
        hook,
        subjects,
        storage,
        directory,
        DocumentsConfig::default(),
    )
    .await
}

pub(crate) async fn harness_configured(
    hook: SharedAuthzHook,
    subjects: GroupSource,
    storage: Arc<TempDir>,
    directory: RoleDirectory,
    config: DocumentsConfig,
) -> Harness {
    let db = test_db_pool().await;
    harness_on_db(db, hook, subjects, storage, directory, config)
}

pub(crate) fn harness_on_db(
    db: DbPool,
    hook: SharedAuthzHook,
    subjects: GroupSource,
    storage: Arc<TempDir>,
    directory: RoleDirectory,
    config: DocumentsConfig,
) -> Harness {
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
        Arc::new(directory),
    ));
    let file_storage: Arc<dyn FileStorage> =
        Arc::new(LocalFileStorage::new(storage.path().to_path_buf()));
    let blobs = Arc::new(BlobStore::new(
        Arc::clone(&file_storage),
        repos.blobs.clone(),
    ));
    let uploads = Arc::new(UploadService::new(UploadServiceParts {
        storage: file_storage,
        uploads: Arc::clone(&repos.uploads),
        blobs: Arc::clone(&blobs),
        max_upload_bytes: config.max_upload_bytes,
        limits: config.uploads,
        tasks: BackgroundTasks::new(),
    }));
    let broadcasters = Arc::new(EventBroadcasters::new());
    let events = Arc::new(DocumentEvents::new(Arc::new(EventRouter::local_only(
        Arc::clone(&broadcasters),
    ))));
    let service = DocumentService::new(DocumentServiceParts {
        repos: Arc::clone(&repos),
        blobs,
        codecs: Arc::new(CodecRegistry::discover()),
        templates: Arc::new(TemplateRegistry::discover(Vec::new())),
        access,
        events,
        config,
    });
    service.attach_uploads(&uploads);
    Harness {
        service,
        repos,
        access_entities,
        storage,
        broadcasters,
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
        attribution: RevisionAttribution::default(),
    }
}

pub(crate) fn create(template: &str, folder: Option<&DocumentFolderId>) -> CreateRequest {
    CreateRequest {
        template_id: DocumentTemplateId::new(template),
        folder: folder.cloned(),
        name: unique("doc"),
        params: TemplateParams::default(),
        ops: Vec::new(),
        author: api_author(),
        message: None,
        content: None,
        media: Media::new(),
    }
}
