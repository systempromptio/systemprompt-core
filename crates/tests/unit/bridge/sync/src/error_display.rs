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

fn code(actual: std::process::ExitCode) -> String {
    format!("{actual:?}")
}

#[test]
fn an_authentication_failure_carries_the_chain_s_own_exit_code_through_sync() {
    // Why: sync wraps the credential chain, and a signed-out install (5) has
    // to stay distinguishable from a transient failure worth retrying (10).
    // Flattening both onto 1 is what makes an automated retry impossible.
    use systemprompt_bridge::auth::ChainError;
    use systemprompt_bridge::auth::providers::AuthFailedSource;

    assert_eq!(
        code(SyncError::Authentication(ChainError::NoneSucceeded).exit_code()),
        code(std::process::ExitCode::from(5)),
    );
    assert_eq!(
        code(
            SyncError::Authentication(ChainError::PreferredTransient {
                provider: "mtls",
                source: AuthFailedSource::SignInRequired,
            })
            .exit_code()
        ),
        code(std::process::ExitCode::from(10)),
    );
    assert_eq!(
        code(
            SyncError::NoCredential {
                bin: "systemprompt-bridge"
            }
            .exit_code()
        ),
        code(std::process::ExitCode::from(5)),
        "no credential at all reports the same signed-out code as an empty chain"
    );
}

#[test]
fn provisioning_and_elevation_failures_are_plain_failures_that_still_say_why() {
    let provision = SyncError::Provision(std::io::Error::other(
        "create /Library/Application Support/ClaudeCode: permission denied",
    ));
    let elevation = SyncError::Elevation("the administrator prompt was declined".to_owned());

    for error in [&provision, &elevation] {
        assert_eq!(
            code(error.exit_code()),
            code(std::process::ExitCode::FAILURE),
            "{error}"
        );
    }
    assert!(
        provision.to_string().contains("permission denied"),
        "the underlying io failure reaches the operator verbatim: {provision}"
    );
    assert_eq!(
        elevation.to_string(),
        "the administrator prompt was declined",
        "an elevation failure is reported as written, not wrapped in boilerplate"
    );
}
