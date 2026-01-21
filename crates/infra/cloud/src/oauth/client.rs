use anyhow::{anyhow, Context, Result};
use axum::extract::Query;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use reqwest::Client;
use std::sync::Arc;
use systemprompt_logging::CliService;
use tokio::sync::{oneshot, Mutex};

use crate::constants::oauth::{CALLBACK_PORT, CALLBACK_TIMEOUT_SECS};
use crate::OAuthProvider;

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

pub async fn run_oauth_flow(
    api_url: &str,
    provider: OAuthProvider,
    templates: OAuthTemplates,
) -> Result<String> {
    let (tx, rx) = oneshot::channel::<Result<String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let success_html = templates.success_html.to_string();
    let error_html = templates.error_html.to_string();

    let callback_handler = {
        let tx = tx.clone();
        let success_html = success_html.clone();
        let error_html = error_html.clone();
        move |Query(params): Query<CallbackParams>| {
            let tx = tx.clone();
            let success_html = success_html.clone();
            let error_html = error_html.clone();
            async move {
                let result = if let Some(error) = params.error {
                    let desc = params
                        .error_description
                        .unwrap_or_else(|| "(no description provided)".into());
                    Err(anyhow!("OAuth error: {} - {}", error, desc))
                } else if let Some(token) = params.access_token {
                    Ok(token)
                } else {
                    Err(anyhow!("No token received in callback"))
                };

                let sender = tx.lock().await.take();
                if let Some(sender) = sender {
                    let is_success = result.is_ok();
                    let _ = sender.send(result);

                    if is_success {
                        Html(success_html)
                    } else {
                        Html(error_html)
                    }
                } else {
                    Html(error_html)
                }
            }
        }
    };

    let app = Router::new().route("/callback", get(callback_handler));
    let addr = format!("127.0.0.1:{CALLBACK_PORT}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    CliService::info(&format!("Starting authentication server on http://{addr}"));

    let redirect_uri = format!("http://127.0.0.1:{CALLBACK_PORT}/callback");

    CliService::info("Fetching authorization URL...");

    let client = Client::new();
    let oauth_endpoint = format!(
        "{}/api/v1/auth/oauth/{}?redirect_uri={}",
        api_url,
        provider.as_str(),
        urlencoding::encode(&redirect_uri)
    );

    let response = client
        .get(&oauth_endpoint)
        .send()
        .await
        .context("Failed to connect to API")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to read OAuth error response body");
            format!("(body unreadable: {})", e)
        });
        return Err(anyhow!(
            "Failed to get authorization URL ({}): {}",
            status,
            body
        ));
    }

    let auth_response: AuthorizeResponse = response
        .json()
        .await
        .context("Failed to parse authorization response")?;

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
            result.map_err(|_| anyhow!("Authentication cancelled"))?
        }
        _ = server => {
            Err(anyhow!("Server stopped unexpectedly"))
        }
        () = tokio::time::sleep(std::time::Duration::from_secs(CALLBACK_TIMEOUT_SECS)) => {
            Err(anyhow!("Authentication timed out after {CALLBACK_TIMEOUT_SECS} seconds"))
        }
    }
}
