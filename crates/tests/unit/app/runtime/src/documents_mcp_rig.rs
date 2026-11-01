use std::sync::Arc;

use rmcp::ErrorData as McpError;
use systemprompt_database::DbPool;
use systemprompt_identifiers::McpServerId;
use systemprompt_runtime::documents::DocumentsRuntime;
use systemprompt_runtime::documents::mcp::{DocumentsMcpConfig, DocumentsMcpTools, refusal_code};
use systemprompt_runtime::documents::refusal::RefusalCode;
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::FileStorage;
use systemprompt_users::UserRepository;
use tempfile::TempDir;

use crate::documents_mcp_support::{SERVER, allow};

pub(crate) struct Rig {
    pub(crate) pool: DbPool,
    pub(crate) runtime: Arc<DocumentsRuntime>,
    pub(crate) tools: DocumentsMcpTools,
    _storage: TempDir,
}

pub(crate) async fn rig(with_uploads: bool) -> Rig {
    let pool = systemprompt_test_fixtures::test_db_pool().await;
    let storage = TempDir::new().expect("tempdir");
    let files: Arc<dyn FileStorage> = Arc::new(LocalFileStorage::new(storage.path().to_path_buf()));
    let runtime = systemprompt_test_fixtures::fixture_documents_runtime(&pool, &files, &allow());
    let event_router = systemprompt_events::EventRouter::local_only(Arc::new(
        systemprompt_events::EventBroadcasters::new(),
    ));
    let notifications =
        systemprompt_test_fixtures::fixture_notifications(&pool, &event_router, &runtime);
    let tools = DocumentsMcpTools::new(DocumentsMcpConfig {
        service: Arc::clone(runtime.service()),
        notifications: Arc::clone(&notifications.service),
        uploads: with_uploads.then(|| Arc::clone(runtime.uploads())),
        users: Arc::new(UserRepository::new(&pool)),
        tokens: systemprompt_test_fixtures::test_token_authority("documents-test"),
        server_name: McpServerId::new(SERVER),
        api_external_url: "https://docs.example.test/".to_owned(),
    });
    Rig {
        pool,
        runtime,
        tools,
        _storage: storage,
    }
}

pub(crate) fn code_of(error: &McpError) -> RefusalCode {
    refusal_code(error).unwrap_or_else(|| panic!("not a refusal: {error:?}"))
}
