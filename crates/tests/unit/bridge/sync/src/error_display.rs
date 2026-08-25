use systemprompt_bridge::sync::{CredentialRejection, SyncError};

fn unauthorized() -> SyncError {
    SyncError::GatewayUnauthorized(Box::new(CredentialRejection {
        bin: "systemprompt-bridge",
        endpoint: "manifest",
        status: 401,
        gateway: "https://gw.example.com".to_owned(),
        credential: "both the cached credential and a freshly minted replacement",
        identity: " for oliver@example.com (user_abc)".to_owned(),
        config_file: "/home/o/.config/systemprompt/systemprompt-bridge.toml".to_owned(),
        pat_file: "/home/o/.config/systemprompt/systemprompt-bridge.pat".to_owned(),
        override_note: " — note the credential location for this process is redirected by \
                         XDG_CONFIG_HOME; a bridge launched from the desktop resolves the \
                         default location instead"
            .to_owned(),
    }))
}

#[test]
fn unauthorized_error_names_gateway_identity_and_credential_paths() {
    let msg = unauthorized().to_string();
    assert!(msg.contains("https://gw.example.com"), "{msg}");
    assert!(msg.contains("oliver@example.com (user_abc)"), "{msg}");
    assert!(msg.contains("HTTP 401 from manifest"), "{msg}");
    assert!(
        msg.contains("/home/o/.config/systemprompt/systemprompt-bridge.pat"),
        "{msg}"
    );
    assert!(
        msg.contains("both the cached credential and a freshly minted replacement"),
        "{msg}"
    );
    assert!(msg.contains("XDG_CONFIG_HOME"), "{msg}");
    assert!(
        msg.contains("login"),
        "the message must state the fix: {msg}"
    );
}

#[test]
fn unauthorized_error_exit_code_is_stable() {
    assert_eq!(
        format!("{:?}", unauthorized().exit_code()),
        format!("{:?}", std::process::ExitCode::from(10)),
    );
}
