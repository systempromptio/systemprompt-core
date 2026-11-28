//! Shared fixtures: a permission gate that records every question and can
//! refuse one permission, a tool context rooted in a temporary project, and
//! small file helpers.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;
use systemprompt_engine::tools::{
    PermissionGate, PermissionRequest, Tool, ToolContext, ToolError, ToolOutput,
};
use systemprompt_identifiers::engine::{CallId, IdClock, MessageId, SessionId};
use systemprompt_traits::CancellationToken;

#[derive(Debug, Default)]
pub(crate) struct Gate {
    asked: Mutex<Vec<PermissionRequest>>,
    refuse: Option<String>,
}

impl Gate {
    pub(crate) fn refusing(permission: &str) -> Self {
        Self {
            asked: Mutex::default(),
            refuse: Some(permission.to_owned()),
        }
    }

    pub(crate) fn asked(&self) -> Vec<PermissionRequest> {
        self.asked.lock().unwrap().clone()
    }

    pub(crate) fn permissions(&self) -> Vec<String> {
        self.asked()
            .into_iter()
            .map(|request| request.permission)
            .collect()
    }
}

#[async_trait]
impl PermissionGate for Gate {
    async fn ask(&self, request: PermissionRequest) -> Result<(), ToolError> {
        let refused = self.refuse.as_deref() == Some(request.permission.as_str());
        self.asked.lock().unwrap().push(request);
        if refused {
            Err(ToolError::Rejected)
        } else {
            Ok(())
        }
    }
}

pub(crate) struct Project {
    pub(crate) dir: tempfile::TempDir,
    pub(crate) root: PathBuf,
}

impl Project {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        let root = dunce(&root);
        Self { dir, root }
    }

    pub(crate) fn outside(&self) -> PathBuf {
        let path = dunce(self.dir.path()).join("elsewhere");
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    pub(crate) fn put(&self, rel: &str, body: &[u8]) -> PathBuf {
        put(&self.root, rel, body)
    }

    pub(crate) fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.root.join(rel)).unwrap()
    }

    pub(crate) fn text(&self, rel: &str) -> String {
        String::from_utf8(self.read(rel)).unwrap()
    }

    pub(crate) fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    pub(crate) fn context(&self, gate: Arc<Gate>) -> ToolContext {
        context(&self.root, gate)
    }

    pub(crate) async fn call(&self, tool: &dyn Tool, input: Value) -> Called {
        self.call_with(tool, input, Gate::default()).await
    }

    pub(crate) async fn call_with(&self, tool: &dyn Tool, input: Value, gate: Gate) -> Called {
        let gate = Arc::new(gate);
        let ctx = self.context(Arc::clone(&gate));
        let result = tool.run(input, &ctx).await;
        Called { result, gate }
    }
}

pub(crate) struct Called {
    pub(crate) result: Result<ToolOutput, ToolError>,
    pub(crate) gate: Arc<Gate>,
}

impl Called {
    pub(crate) fn ok(self) -> ToolOutput {
        match self.result {
            Ok(output) => output,
            Err(err) => panic!("expected the call to succeed, got {err:?}"),
        }
    }

    pub(crate) fn err(self) -> ToolError {
        match self.result {
            Ok(output) => panic!("expected the call to fail, got {output:?}"),
            Err(err) => err,
        }
    }

    pub(crate) fn message(self) -> String {
        self.err().to_string()
    }
}

pub(crate) fn dunce(path: &Path) -> PathBuf {
    let canonical = std::fs::canonicalize(path).unwrap();
    let text = canonical.to_string_lossy().into_owned();
    text.strip_prefix(r"\\?\")
        .map_or(canonical.clone(), PathBuf::from)
}

pub(crate) fn context(root: &Path, gate: Arc<Gate>) -> ToolContext {
    let clock = IdClock::new();
    ToolContext {
        session_id: SessionId::mint(&clock),
        message_id: MessageId::mint(&clock),
        call_id: CallId::try_new("call").unwrap(),
        agent: "build".to_owned(),
        cwd: root.to_path_buf(),
        worktree: root.to_path_buf(),
        data_dir: root.join(".data"),
        cancel: CancellationToken::new(),
        gate,
        observer: None,
        progress: None,
        files: systemprompt_engine::tools::FileLocks::default(),
    }
}

pub(crate) fn put(root: &Path, rel: &str, body: &[u8]) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, body).unwrap();
    path
}

pub(crate) fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
