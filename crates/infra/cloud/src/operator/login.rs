//! Browser authorization-code flow with PKCE against an OIDC issuer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use base64::prelude::*;
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{CloudAuthToken, Email};
use systemprompt_logging::CliService;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;

use super::endpoints::{AuthorizeParams, CodeRedemption, authorize_url, discover, redeem_code};
use super::{OperatorCredentials, identity_claims};
use crate::OAuthTemplates;
use crate::constants::oauth::{CALLBACK_PORT, CALLBACK_TIMEOUT_SECS};
use crate::error::{CloudError, CloudResult};

#[derive(Debug, Clone)]
pub struct OperatorLoginRequest {
    pub issuer: String,
    pub client_id: String,
    pub scopes: Vec<String>,
}

#[derive(Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

struct CallbackState {
    expected_state: String,
    tx: Mutex<Option<oneshot::Sender<CloudResult<String>>>>,
    success_html: &'static str,
    error_html: &'static str,
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn pkce_challenge(verifier: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

async fn callback_handler(
    State(state): State<Arc<CallbackState>>,
    Query(params): Query<CallbackParams>,
) -> Html<&'static str> {
    let result = if let Some(error) = params.error {
        Err(CloudError::operator_auth(format!(
            "{error}: {}",
            params.error_description.unwrap_or_default()
        )))
    } else if params.state.as_deref() != Some(state.expected_state.as_str()) {
        Err(CloudError::operator_auth("callback state did not match"))
    } else {
        params
            .code
            .ok_or_else(|| CloudError::operator_auth("callback carried no authorization code"))
    };
    let is_success = result.is_ok();
    let sender = state.tx.lock().await.take();
    if let Some(sender) = sender
        && sender.send(result).is_err()
    {
        tracing::warn!("Operator login receiver dropped before the callback arrived");
    }
    Html(if is_success {
        state.success_html
    } else {
        state.error_html
    })
}

async fn await_code(
    listener: tokio::net::TcpListener,
    app: Router,
    rx: oneshot::Receiver<CloudResult<String>>,
) -> CloudResult<String> {
    let server = axum::serve(listener, app);
    tokio::select! {
        result = rx => {
            result.map_err(|_e| CloudError::operator_auth("sign-in cancelled"))?
        }
        _ = server => Err(CloudError::operator_auth("callback server stopped unexpectedly")),
        () = tokio::time::sleep(Duration::from_secs(CALLBACK_TIMEOUT_SECS)) => {
            Err(CloudError::operator_auth(format!(
                "sign-in timed out after {CALLBACK_TIMEOUT_SECS} seconds"
            )))
        }
    }
}

pub async fn run_operator_login(
    request: &OperatorLoginRequest,
    templates: OAuthTemplates,
) -> CloudResult<OperatorCredentials> {
    let client = reqwest::Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_DEFAULT_TIMEOUT)
        .build()?;
    let discovery = discover(&client, &request.issuer).await?;

    let verifier = random_token();
    let state = random_token();
    let nonce = random_token();
    let redirect_uri = format!("http://127.0.0.1:{CALLBACK_PORT}/callback");

    let (tx, rx) = oneshot::channel();
    let callback = Arc::new(CallbackState {
        expected_state: state.clone(),
        tx: Mutex::new(Some(tx)),
        success_html: templates.success_html,
        error_html: templates.error_html,
    });
    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(callback);
    let listener = crate::callback_listener::bind_callback_listener(CALLBACK_PORT)?;

    let url = authorize_url(
        &discovery,
        request,
        AuthorizeParams {
            redirect_uri: &redirect_uri,
            state: &state,
            nonce: &nonce,
            challenge: &pkce_challenge(&verifier),
        },
    );
    CliService::info(&format!("Opening browser to sign in at {}", request.issuer));
    if let Err(e) = open::that(&url) {
        CliService::warning(&format!("Could not open browser automatically: {e}"));
        CliService::key_value("URL", &url);
    }
    CliService::info("Waiting for sign-in...");

    let code = await_code(listener, app, rx).await?;

    let id_token = redeem_code(
        &client,
        &discovery,
        request,
        CodeRedemption {
            code: &code,
            redirect_uri: &redirect_uri,
            verifier: &verifier,
        },
    )
    .await?;

    let claims = identity_claims(&id_token)?;
    if claims.nonce.as_deref() != Some(nonce.as_str()) {
        return Err(CloudError::operator_auth("id_token nonce did not match"));
    }
    let email = claims
        .email
        .map(Email::try_new)
        .transpose()
        .map_err(|source| CloudError::OperatorEmail { source })?;

    Ok(OperatorCredentials {
        issuer: request.issuer.trim_end_matches('/').to_owned(),
        client_id: request.client_id.clone(),
        id_token: CloudAuthToken::new(id_token),
        email,
        authenticated_at: Utc::now(),
    })
}
