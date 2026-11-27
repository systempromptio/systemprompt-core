use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_ids::{CallId, MessageId, SessionId};
use harness_tasks::CancellationToken;
use harness_tools::{PermissionGate, PermissionRequest, ToolContext, ToolError};

#[derive(Debug, Default)]
pub(crate) struct Recorder {
    asked: Mutex<Vec<PermissionRequest>>,
}

impl Recorder {
    pub(crate) fn asked(&self) -> Vec<PermissionRequest> {
        self.asked.lock().unwrap().clone()
    }
}

#[async_trait]
impl PermissionGate for Recorder {
    async fn ask(&self, request: PermissionRequest) -> Result<(), ToolError> {
        self.asked.lock().unwrap().push(request);
        Ok(())
    }
}

pub(crate) fn context(root: &Path, gate: Arc<Recorder>) -> ToolContext {
    ToolContext {
        session_id: SessionId::mint(),
        message_id: MessageId::mint(),
        call_id: CallId::try_new("call").unwrap(),
        agent: "build".to_owned(),
        cwd: root.to_path_buf(),
        worktree: root.to_path_buf(),
        data_dir: root.join(".data"),
        cancel: CancellationToken::new(),
        gate,
        observer: None,
        progress: None,
    }
}

pub(crate) fn recorded(root: &Path) -> (ToolContext, Arc<Recorder>) {
    let gate = Arc::new(Recorder::default());
    (context(root, Arc::clone(&gate)), gate)
}

pub(crate) fn put(root: &Path, rel: &str, body: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

pub(crate) fn invalid_message(error: &ToolError) -> String {
    match error {
        ToolError::InvalidInput { message } => message.clone(),
        other => panic!("expected invalid input, got {other:?}"),
    }
}
