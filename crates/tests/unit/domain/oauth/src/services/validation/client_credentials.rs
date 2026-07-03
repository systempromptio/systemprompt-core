//! Tests for client credential verification
//!
//! Tests `verify_client_authentication` which handles auth_method dispatch,
//! bcrypt secret verification, and timing-safe error paths.

use systemprompt_oauth::services::verify_client_authentication;

#[test]
fn auth_method_none_returns_ok() {
    verify_client_authentication("none", None, None).expect("auth_method none passes");
}

#[test]
fn auth_method_none_ignores_secret_and_hash() {
    let hash = bcrypt::hash("some_secret", 4).unwrap();

    verify_client_authentication("none", Some(&hash), Some("some_secret"))
        .expect("auth_method none ignores secret and hash");
}

#[test]
fn valid_secret_and_hash_returns_ok() {
    let hash = bcrypt::hash("test_secret", 4).unwrap();

    verify_client_authentication("client_secret_post", Some(&hash), Some("test_secret"))
        .expect("valid secret and hash pass");
}

#[test]
fn client_secret_post_method_with_valid_creds() {
    let secret = "my_application_secret_value";
    let hash = bcrypt::hash(secret, 4).unwrap();

    verify_client_authentication("client_secret_post", Some(&hash), Some(secret))
        .expect("client_secret_post with valid creds passes");
}

#[test]
fn wrong_secret_returns_err() {
    let hash = bcrypt::hash("correct_secret", 4).unwrap();

    let result =
        verify_client_authentication("client_secret_post", Some(&hash), Some("wrong_secret"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Invalid client secret"),
        "Expected 'Invalid client secret' but got: {err_msg}"
    );
}

#[test]
fn hash_present_no_secret_returns_err() {
    let hash = bcrypt::hash("test_secret", 4).unwrap();

    let result = verify_client_authentication("client_secret_post", Some(&hash), None);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Client secret required"),
        "Expected 'Client secret required' but got: {err_msg}"
    );
}

#[test]
fn no_hash_secret_present_returns_err() {
    let result = verify_client_authentication("client_secret_post", None, Some("orphan_secret"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Client has no secret hash configured"),
        "Expected 'Client has no secret hash configured' but got: {err_msg}"
    );
}

#[test]
fn no_hash_no_secret_returns_err() {
    let result = verify_client_authentication("client_secret_post", None, None);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Client secret required"),
        "Expected 'Client secret required' but got: {err_msg}"
    );
}

#[test]
fn error_messages_are_descriptive() {
    let hash = bcrypt::hash("secret", 4).unwrap();

    let no_secret_err = verify_client_authentication("client_secret_basic", Some(&hash), None)
        .unwrap_err()
        .to_string();
    assert!(no_secret_err.contains("Client secret required"));

    let no_hash_err = verify_client_authentication("client_secret_basic", None, Some("secret"))
        .unwrap_err()
        .to_string();
    assert!(no_hash_err.contains("Client has no secret hash configured"));

    let wrong_secret_err =
        verify_client_authentication("client_secret_basic", Some(&hash), Some("wrong"))
            .unwrap_err()
            .to_string();
    assert!(wrong_secret_err.contains("Invalid client secret"));
}

#[test]
fn various_auth_methods_require_secret() {
    let methods = [
        "client_secret_basic",
        "client_secret_post",
        "client_secret_jwt",
    ];

    for method in methods {
        let result = verify_client_authentication(method, None, None);
        assert!(
            result.is_err(),
            "auth_method '{method}' should require credentials"
        );
    }
}
