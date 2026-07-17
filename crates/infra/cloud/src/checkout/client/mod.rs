//! Browser-driven Paddle checkout flow used by `systemprompt cloud
//! checkout`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handler;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CheckoutSessionId, TenantId, TransactionId};
use systemprompt_logging::CliService;
use tokio::sync::{Mutex, oneshot};

use handler::{callback_handler, status_handler};

use crate::CloudApiClient;
use crate::constants::checkout::{CALLBACK_PORT, CALLBACK_TIMEOUT_SECS};
use crate::error::{CloudError, CloudResult};

#[derive(Debug, Deserialize)]
pub(super) struct CallbackParams {
    pub(super) transaction_id: Option<TransactionId>,
    pub(super) tenant_id: Option<TenantId>,
    pub(super) status: Option<String>,
    pub(super) error: Option<String>,
    pub(super) checkout_session_id: Option<CheckoutSessionId>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct StatusResponse {
    pub(super) status: String,
    pub(super) message: Option<String>,
    pub(super) app_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CheckoutCallbackResult {
    pub transaction_id: TransactionId,
    pub tenant_id: TenantId,
    pub fly_app_name: Option<String>,
    pub needs_deploy: bool,
}

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "All three fields are static HTML payloads; the `_html` suffix disambiguates them at \
              the call site."
)]
pub struct CheckoutTemplates {
    pub success_html: &'static str,
    pub error_html: &'static str,
    pub waiting_html: &'static str,
}

pub(super) struct AppState {
    pub(super) tx: Arc<Mutex<Option<oneshot::Sender<CloudResult<CheckoutCallbackResult>>>>>,
    pub(super) api_client: Arc<CloudApiClient>,
    pub(super) success_template: String,
    pub(super) error_template: String,
    pub(super) waiting_template: String,
}

pub async fn run_checkout_callback_flow(
    api_client: &CloudApiClient,
    checkout_url: &str,
    templates: CheckoutTemplates,
) -> CloudResult<CheckoutCallbackResult> {
    let (tx, rx) = oneshot::channel::<CloudResult<CheckoutCallbackResult>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let state = AppState {
        tx: Arc::clone(&tx),
        api_client: Arc::new(CloudApiClient::new(
            api_client.api_url(),
            api_client.token(),
        )?),
        success_template: templates.success_html.to_owned(),
        error_template: templates.error_html.to_owned(),
        waiting_template: templates.waiting_html.to_owned(),
    };

    let app = Router::new()
        .route("/callback", get(callback_handler))
        .route("/status/{tenant_id}", get(status_handler))
        .with_state(Arc::new(state));

    let addr = format!("127.0.0.1:{CALLBACK_PORT}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    CliService::info(&format!(
        "Starting checkout callback server on http://{addr}"
    ));

    CliService::info("Opening Paddle checkout in your browser...");
    CliService::info(&format!("URL: {checkout_url}"));

    if let Err(e) = open::that(checkout_url) {
        CliService::warning(&format!("Could not open browser automatically: {e}"));
        CliService::info("Please open this URL manually:");
        CliService::key_value("URL", checkout_url);
    }

    CliService::info("Waiting for checkout completion...");
    CliService::info(&format!("(timeout in {CALLBACK_TIMEOUT_SECS} seconds)"));

    let server = axum::serve(listener, app);

    tokio::select! {
        result = rx => {
            result.map_err(|_e| CloudError::CheckoutFlow { message: "Checkout cancelled".to_owned() })?
        }
        _ = server => {
            Err(CloudError::CheckoutFlow { message: "Server stopped unexpectedly".to_owned() })
        }
        () = tokio::time::sleep(Duration::from_secs(CALLBACK_TIMEOUT_SECS)) => {
            Err(CloudError::CheckoutFlow { message: format!("Checkout timed out after {CALLBACK_TIMEOUT_SECS} seconds") })
        }
    }
}
