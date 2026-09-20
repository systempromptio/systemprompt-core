use systemprompt_cloud::{CloudPath, CloudPaths, clear_cloud_state};

#[test]
fn cloud_logout_removes_credentials_even_when_session_cleanup_cannot_be_read() {
    let root = tempfile::TempDir::new().expect("owned cloud state");
    let paths = CloudPaths::new(root.path());
    let credentials = paths.resolve(CloudPath::Credentials);
    let tenants = paths.resolve(CloudPath::Tenants);
    let sessions = paths.resolve(CloudPath::SessionsDir);
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(&credentials, b"owned credentials").unwrap();
    std::fs::write(&tenants, b"owned tenants").unwrap();
    let session_index = sessions.join("index.json");
    std::fs::write(&session_index, b"not json").unwrap();
    let corrupt_evidence = std::fs::read(&session_index).unwrap();

    let cleared = clear_cloud_state(&paths)
        .expect("unreadable session state does not block credential-file cleanup");

    assert_eq!(
        cleared.credentials_path.as_deref(),
        Some(credentials.as_path())
    );
    assert_eq!(cleared.tenants_path.as_deref(), Some(tenants.as_path()));
    assert_eq!(cleared.tenant_sessions_removed, 0);
    assert!(!credentials.exists());
    assert!(!tenants.exists());
    assert_eq!(std::fs::read(&session_index).unwrap(), corrupt_evidence);
    assert!(
        session_index.exists(),
        "logout reports success after removing credential files, but cannot revoke or rewrite tokens in an unreadable session store"
    );
}
