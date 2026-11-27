use sp_client::{ClientError, SpClient};
use sp_protocol::view::SignInFailure;

use crate::support::{http, jwt};

#[test]
fn base_urls_follow_the_origin_rules() {
    assert!(SpClient::new("https://sp.example.com/", http()).is_ok());
    assert!(SpClient::new("http://localhost:8080", http()).is_ok());
    assert!(matches!(
        SpClient::new("http://sp.example.com", http()),
        Err(ClientError::Origin(_))
    ));
    assert!(matches!(
        SpClient::new("https://user:pw@sp.example.com", http()),
        Err(ClientError::Origin(_))
    ));
}

#[test]
fn the_base_and_origin_are_exposed() {
    let client = SpClient::new("https://sp.example.com/tenant/", http()).unwrap();
    assert_eq!(client.origin().as_str(), "https://sp.example.com/");
    assert_eq!(client.base(), "https://sp.example.com/tenant");
    assert_eq!(client.base_url().as_str(), "https://sp.example.com/tenant");
}

#[tokio::test]
async fn a_silent_address_is_unreachable() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let client = SpClient::new(&format!("http://127.0.0.1:{port}"), http()).unwrap();
    let error = client.whoami(&jwt()).await.unwrap_err();
    assert!(matches!(error, ClientError::Unreachable { .. }));
    assert_eq!(error.sign_in_failure(), SignInFailure::Unreachable);
    assert!(!format!("{error:?}").contains("header.claims.signature"));
}
