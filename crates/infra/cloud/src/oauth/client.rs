//! Browser-driven OAuth login flow.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use reqwest::Client;
use systemprompt_logging::CliService;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};
use tokio::sync::{Mutex, oneshot};

use crate::OAuthProvider;
use crate::constants::oauth::{CALLBACK_PORT, CALLBACK_TIMEOUT_SECS};
use crate::error::{CloudError, CloudResult};

#[derive(serde::Deserialize)]
struct CallbackParams {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(serde::Deserialize)]
struct AuthorizeResponse {
    authorize_url: String,
}

#[derive(Debug, Clone, Copy)]
pub struct OAuthTemplates {
    pub success_html: &'static str,
    pub error_html: &'static str,
}

struct CallbackState {
    tx: Mutex<Option<oneshot::Sender<CloudResult<String>>>>,
    success_html: String,
    error_html: String,
}

async fn callback_handler(
    State(state): State<Arc<CallbackState>>,
    Query(params): Query<CallbackParams>,
) -> Html<String> {
    let result: CloudResult<String> = if let Some(error) = params.error {
        let desc = params
            .error_description
            .unwrap_or_else(|| "(no description provided)".into());
        Err(CloudError::OAuthFlow {
            message: format!("OAuth error: {error} - {desc}"),
        })
    } else if let Some(token) = params.access_token {
        Ok(token)
    } else {
        Err(CloudError::OAuthFlow {
            message: "No token received in callback".to_owned(),
        })
    };

    let sender = state.tx.lock().await.take();
    let Some(sender) = sender else {
        return Html(state.error_html.clone());
    };

    let is_success = result.is_ok();
    if sender.send(result).is_err() {
        tracing::warn!("OAuth result receiver dropped before result could be sent");
    }

    if is_success {
        Html(state.success_html.clone())
    } else {
        Html(state.error_html.clone())
    }
}

pub async fn run_oauth_flow(
    api_url: &str,
    provider: OAuthProvider,
    templates: OAuthTemplates,
) -> CloudResult<String> {
    let (tx, rx) = oneshot::channel::<CloudResult<String>>();
    let state = Arc::new(CallbackState {
        tx: Mutex::new(Some(tx)),
        success_html: templates.success_html.to_owned(),
        error_html: templates.error_html.to_owned(),
    });

    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(state);
    let addr = format!("127.0.0.1:{CALLBACK_PORT}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    CliService::info(&format!("Starting authentication server on http://{addr}"));

    let redirect_uri = format!("http://127.0.0.1:{CALLBACK_PORT}/callback");

    CliService::info("Fetching authorization URL...");

    let client = Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_DEFAULT_TIMEOUT)
        .build()?;
    let oauth_endpoint = format!(
        "{}/api/v1/auth/oauth/{}?redirect_uri={}",
        api_url,
        provider.as_str(),
        urlencoding::encode(&redirect_uri)
    );

    let response = client.get(&oauth_endpoint).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to read OAuth error response body");
            format!("(body unreadable: {e})")
        });
        return Err(CloudError::OAuthFlow {
            message: format!("Failed to get authorization URL ({status}): {body}"),
        });
    }

    let auth_response: AuthorizeResponse = response.json().await?;

    let auth_url = auth_response.authorize_url;

    CliService::info(&format!(
        "Opening browser for {} authentication...",
        provider.display_name()
    ));
    CliService::info(&format!("URL: {auth_url}"));

    if let Err(e) = open::that(&auth_url) {
        CliService::warning(&format!("Could not open browser automatically: {e}"));
        CliService::info("Please open this URL manually:");
        CliService::key_value("URL", &auth_url);
    }

    CliService::info("Waiting for authentication...");
    CliService::info(&format!("(timeout in {CALLBACK_TIMEOUT_SECS} seconds)"));

    let server = axum::serve(listener, app);

    tokio::select! {
        result = rx => {
            result.map_err(|_e| CloudError::OAuthFlow { message: "Authentication cancelled".to_owned() })?
        }
        _ = server => {
            Err(CloudError::OAuthFlow { message: "Server stopped unexpectedly".to_owned() })
        }
        () = tokio::time::sleep(std::time::Duration::from_secs(CALLBACK_TIMEOUT_SECS)) => {
            Err(CloudError::OAuthFlow { message: format!("Authentication timed out after {CALLBACK_TIMEOUT_SECS} seconds") })
        }
    }
}
