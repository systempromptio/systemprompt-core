use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::prelude::*;
use systemprompt_cloud::OAuthTemplates;
use systemprompt_cloud::error::CloudError;
use systemprompt_cloud::operator::{LoginIo, OperatorLoginRequest, run_operator_login_with};
use systemprompt_identifiers::ClientId;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn templates() -> OAuthTemplates {
    OAuthTemplates {
        success_html: "ok",
        error_html: "err",
    }
}

fn request(issuer: String) -> OperatorLoginRequest {
    OperatorLoginRequest {
        issuer,
        client_id: ClientId::new("cli"),
        scopes: vec!["openid".to_owned(), "email".to_owned()],
    }
}

fn id_token(nonce: &str, email: &str) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let claims = BASE64_URL_SAFE_NO_PAD.encode(format!(
        r#"{{"nonce":"{nonce}","email":"{email}","exp":4102444800}}"#
    ));
    format!("{header}.{claims}.sig")
}

struct TokenFor {
    nonce: Arc<Mutex<String>>,
    tamper: bool,
}

impl Respond for TokenFor {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let seen = self.nonce.lock().unwrap().clone();
        let nonce = if self.tamper {
            format!("{seen}x")
        } else {
            seen
        };
        ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({ "id_token": id_token(&nonce, "op@example.com") }))
    }
}

async fn idp(nonce: &Arc<Mutex<String>>, tamper: bool) -> MockServer {
    let server = MockServer::start().await;
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issuer": uri,
            "authorization_endpoint": format!("{uri}/authorize"),
            "token_endpoint": format!("{uri}/token"),
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("code=granted"))
        .and(body_string_contains("code_verifier="))
        .respond_with(TokenFor {
            nonce: Arc::clone(nonce),
            tamper,
        })
        .mount(&server)
        .await;
    server
}

fn query(url: &str) -> HashMap<String, String> {
    reqwest::Url::parse(url)
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

fn browser(
    nonce: Arc<Mutex<String>>,
    callback: impl Fn(&HashMap<String, String>) -> String + Send + Sync + 'static,
    opened: std::io::Result<()>,
) -> impl Fn(&str) -> std::io::Result<()> + Sync {
    let callback = Arc::new(callback);
    let failed = opened.is_err();
    move |url: &str| {
        let params = query(url);
        *nonce.lock().unwrap() = params["nonce"].clone();
        let target = format!("{}?{}", params["redirect_uri"], callback(&params));
        tokio::spawn(async move {
            reqwest::get(target).await.expect("callback request");
        });
        if failed {
            Err(std::io::Error::other("no browser"))
        } else {
            Ok(())
        }
    }
}

fn granted(params: &HashMap<String, String>) -> String {
    format!("code=granted&state={}", params["state"])
}

#[tokio::test]
async fn a_completed_browser_sign_in_yields_the_operator_identity() {
    let nonce = Arc::new(Mutex::new(String::new()));
    let server = idp(&nonce, false).await;
    let open = browser(Arc::clone(&nonce), granted, Ok(()));
    let io = LoginIo {
        callback_port: 0,
        open_browser: &open,
    };
    let creds = run_operator_login_with(&request(format!("{}/", server.uri())), templates(), io)
        .await
        .unwrap();
    assert_eq!(creds.issuer, server.uri());
    assert_eq!(creds.client_id.as_str(), "cli");
    assert_eq!(creds.email.unwrap().as_str(), "op@example.com");
    assert!(!creds.id_token.as_str().is_empty());
}

#[tokio::test]
async fn a_browser_that_cannot_open_still_completes_through_the_printed_url() {
    let nonce = Arc::new(Mutex::new(String::new()));
    let server = idp(&nonce, false).await;
    let open = browser(Arc::clone(&nonce), granted, Err(std::io::Error::other("x")));
    let io = LoginIo {
        callback_port: 0,
        open_browser: &open,
    };
    let creds = run_operator_login_with(&request(server.uri()), templates(), io)
        .await
        .unwrap();
    assert_eq!(creds.email.unwrap().as_str(), "op@example.com");
}

async fn refused(
    callback: impl Fn(&HashMap<String, String>) -> String + Send + Sync + 'static,
    tamper: bool,
) -> CloudError {
    let nonce = Arc::new(Mutex::new(String::new()));
    let server = idp(&nonce, tamper).await;
    let open = browser(Arc::clone(&nonce), callback, Ok(()));
    let io = LoginIo {
        callback_port: 0,
        open_browser: &open,
    };
    run_operator_login_with(&request(server.uri()), templates(), io)
        .await
        .unwrap_err()
}

#[tokio::test]
async fn an_identity_provider_error_on_the_callback_is_refused_with_its_reason() {
    let error = refused(
        |_| "error=access_denied&error_description=user%20said%20no".to_owned(),
        false,
    )
    .await;
    assert!(
        error.to_string().contains("access_denied: user said no"),
        "{error}"
    );
}

#[tokio::test]
async fn a_callback_with_a_foreign_state_is_refused() {
    let error = refused(|_| "code=granted&state=forged".to_owned(), false).await;
    assert!(
        error.to_string().contains("callback state did not match"),
        "{error}"
    );
}

#[tokio::test]
async fn a_callback_without_a_code_is_refused() {
    let error = refused(|params| format!("state={}", params["state"]), false).await;
    assert!(
        error
            .to_string()
            .contains("callback carried no authorization code"),
        "{error}"
    );
}

#[tokio::test]
async fn an_id_token_with_a_different_nonce_is_refused() {
    let error = refused(granted, true).await;
    assert!(
        error.to_string().contains("id_token nonce did not match"),
        "{error}"
    );
}
