use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_lsp::{
    DiagnosticScope, LspConfig, LspError, LspManager, LspTimeouts, ServerId, ServerSpec,
    ServerState,
};

fn spec(id: &str, command: &str, extensions: &[&str]) -> ServerSpec {
    ServerSpec {
        id: ServerId::new(id),
        command: command.to_owned(),
        args: Vec::new(),
        env: BTreeMap::new(),
        extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
        root_markers: Vec::new(),
        initialization_options: None,
    }
}

fn configured(servers: Vec<ServerSpec>) -> LspConfig {
    let timeouts = LspTimeouts {
        initialize: Duration::from_secs(5),
        request: Duration::from_secs(5),
        shutdown: Duration::from_millis(500),
        ..LspTimeouts::default()
    };
    LspConfig {
        builtins: false,
        servers,
        timeouts,
        ..LspConfig::default()
    }
}

#[test]
fn normalizes_the_worktree_lexically() {
    let normalized = |path: &str| {
        LspManager::new(path, configured(Vec::new()))
            .worktree()
            .to_path_buf()
    };
    assert_eq!(normalized("/a/./b/../c"), PathBuf::from("/a/c"));
    assert_eq!(normalized("/../a"), PathBuf::from("/a"));
    assert_eq!(normalized("a/../../b"), PathBuf::from("../b"));
}

#[test]
fn presents_a_client_identity_by_default() {
    let config = LspConfig::default();
    assert_ne!(config.client_name, "");
    assert_ne!(config.client_version, "");
}

#[tokio::test]
async fn a_server_whose_command_is_missing_stays_failed_with_its_reason() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.x"), "text").unwrap();
    let manager = LspManager::new(
        dir.path(),
        configured(vec![spec("missing-ls", "missing-ls", &["x"])]),
    );

    let error = manager.touch(Path::new("a.x"), false).await.unwrap_err();
    match error {
        LspError::Unavailable { server, reason } => {
            assert_eq!(server.as_str(), "missing-ls");
            assert_eq!(reason, "command `missing-ls` was not found");
        },
        other => panic!("expected an unavailable server, got {other:?}"),
    }
    let status = manager.status();
    assert_eq!(status.len(), 1);
    assert_eq!(
        status[0].state,
        ServerState::Failed {
            reason: "command `missing-ls` was not found".to_owned()
        }
    );
    assert_eq!(
        manager.diagnostics(DiagnosticScope::All),
        Vec::<harness_lsp::Diagnostic>::new()
    );
}

#[tokio::test]
async fn files_no_server_handles_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.txt"), "text").unwrap();
    let manager = LspManager::new(dir.path(), configured(vec![spec("rust", "rust", &["rs"])]));
    let error = manager
        .touch(Path::new("notes.txt"), false)
        .await
        .unwrap_err();
    assert!(matches!(error, LspError::NoServer { .. }), "got {error:?}");
}

#[tokio::test]
async fn touching_a_deleted_file_succeeds_without_starting_a_server() {
    let dir = tempfile::tempdir().unwrap();
    let manager = LspManager::new(
        dir.path(),
        configured(vec![spec("missing-ls", "missing-ls", &["x"])]),
    );
    manager.touch(Path::new("gone.x"), true).await.unwrap();
    assert_eq!(manager.status(), Vec::<harness_lsp::ServerStatus>::new());
}

#[tokio::test]
async fn a_stopped_manager_refuses_new_work() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.x"), "text").unwrap();
    let manager = LspManager::new(
        dir.path(),
        configured(vec![spec("missing-ls", "missing-ls", &["x"])]),
    );
    manager.shutdown().await;
    let error = manager.touch(Path::new("a.x"), false).await.unwrap_err();
    assert!(matches!(error, LspError::Stopped), "got {error:?}");
}

#[tokio::test]
async fn workspace_symbols_need_a_running_server() {
    let dir = tempfile::tempdir().unwrap();
    let manager = LspManager::new(dir.path(), configured(Vec::new()));
    let error = manager.workspace_symbols("main").await.unwrap_err();
    assert!(matches!(error, LspError::NoRunningServer), "got {error:?}");
}

#[cfg(unix)]
fn write_program(path: &Path, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
    let mode = if executable { 0o755 } else { 0o644 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
async fn start_failure(manager: &LspManager, file: &str) -> String {
    match manager.touch(Path::new(file), false).await.unwrap_err() {
        LspError::Unavailable { reason, .. } => reason,
        other => panic!("expected an unavailable server, got {other:?}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn commands_resolve_on_the_search_path_relative_to_the_worktree_and_by_absolute_path() {
    let bin = tempfile::tempdir().unwrap();
    write_program(&bin.path().join("fake-ls"), true);
    write_program(&bin.path().join("not-exec"), false);
    let worktree = tempfile::tempdir().unwrap();
    write_program(&worktree.path().join("local-ls"), true);
    for name in ["a.on-path", "a.not-exec", "a.relative", "a.absolute"] {
        std::fs::write(worktree.path().join(name), "text").unwrap();
    }
    let absolute = bin.path().join("fake-ls");
    let config = LspConfig {
        search_path: Some(std::env::join_paths([bin.path()]).unwrap()),
        ..configured(vec![
            spec("on-path", "fake-ls", &["on-path"]),
            spec("not-exec", "not-exec", &["not-exec"]),
            spec("relative", "./local-ls", &["relative"]),
            spec("absolute", absolute.to_str().unwrap(), &["absolute"]),
        ])
    };
    let manager = LspManager::new(worktree.path(), config);

    let not_found = "was not found";
    assert!(
        start_failure(&manager, "a.not-exec")
            .await
            .contains(not_found)
    );
    for file in ["a.on-path", "a.relative", "a.absolute"] {
        let reason = start_failure(&manager, file).await;
        assert!(
            !reason.contains(not_found),
            "{file} was not resolved: {reason}"
        );
    }
}

#[cfg(windows)]
#[test]
fn builtins_are_found_through_the_executable_extensions() {
    let bin = tempfile::tempdir().unwrap();
    std::fs::write(bin.path().join("rust-analyzer.CMD"), "").unwrap();
    std::fs::write(bin.path().join("gopls"), "").unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let config = LspConfig {
        search_path: Some(std::env::join_paths([bin.path()]).unwrap()),
        path_ext: Some(".EXE; .Cmd;;bat".into()),
        ..LspConfig::default()
    };
    let manager = LspManager::new(worktree.path(), config);
    let ids: Vec<String> = manager
        .available_servers()
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    assert_eq!(ids, vec!["rust-analyzer"]);
}
