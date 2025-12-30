use anyhow::{anyhow, Result};
use axum::extract::{Path, Query, State};
use axum::response::{Html, Json};
use axum::routing::get;
use axum::Router;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use systemprompt_core_logging::CliService;
use tokio::sync::{oneshot, Mutex};

use crate::api_client::{CheckoutEvent, ProvisioningEventType};
use crate::constants::checkout::{CALLBACK_PORT, CALLBACK_TIMEOUT_SECS};
use crate::CloudApiClient;

#[derive(Debug, Deserialize)]
struct CallbackParams {
    transaction_id: Option<String>,
    tenant_id: Option<String>,
    status: Option<String>,
    error: Option<String>,
    checkout_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct StatusResponse {
    status: String,
    message: Option<String>,
    app_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CheckoutCallbackResult {
    pub transaction_id: String,
    pub tenant_id: String,
    pub fly_app_name: Option<String>,
    pub needs_deploy: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct CheckoutTemplates {
    pub success_html: &'static str,
    pub error_html: &'static str,
    pub waiting_html: &'static str,
}

#[allow(clippy::struct_field_names)]
struct AppState {
    tx: Arc<Mutex<Option<oneshot::Sender<Result<CheckoutCallbackResult>>>>>,
    api_client: Arc<CloudApiClient>,
    success_html: String,
    error_html: String,
    waiting_html: String,
}

pub async fn run_checkout_callback_flow(
    api_client: &CloudApiClient,
    checkout_url: &str,
    templates: CheckoutTemplates,
) -> Result<CheckoutCallbackResult> {
    let (tx, rx) = oneshot::channel::<Result<CheckoutCallbackResult>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let state = AppState {
        tx: Arc::clone(&tx),
        api_client: Arc::new(CloudApiClient::new(
            api_client.api_url(),
            api_client.token(),
        )),
        success_html: templates.success_html.to_string(),
        error_html: templates.error_html.to_string(),
        waiting_html: templates.waiting_html.to_string(),
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
            result.map_err(|_| anyhow!("Checkout cancelled"))?
        }
        _ = server => {
            Err(anyhow!("Server stopped unexpectedly"))
        }
        () = tokio::time::sleep(Duration::from_secs(CALLBACK_TIMEOUT_SECS)) => {
            Err(anyhow!("Checkout timed out after {CALLBACK_TIMEOUT_SECS} seconds"))
        }
    }
}

async fn callback_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallbackParams>,
) -> Html<String> {
    if let Some(error) = &params.error {
        tracing::error!(error = %error, "Checkout error from callback");
        send_result(&state.tx, Err(anyhow!("Checkout error: {}", error))).await;
        return Html(state.error_html.clone());
    }

    if let (Some(transaction_id), Some(tenant_id)) =
        (params.transaction_id.clone(), params.tenant_id.clone())
    {
        match params.status.as_deref() {
            Some("completed") => {
                let result = Ok(CheckoutCallbackResult {
                    transaction_id,
                    tenant_id: tenant_id.clone(),
                    fly_app_name: None,
                    needs_deploy: false,
                });
                send_result(&state.tx, result).await;
                let html = state.success_html.replace("{{TENANT_ID}}", &tenant_id);
                return Html(html);
            },
            Some(status) => {
                send_result(&state.tx, Err(anyhow!("Checkout status: {}", status))).await;
                return Html(state.error_html.clone());
            },
            None => {
                send_result(
                    &state.tx,
                    Err(anyhow!(
                        "Checkout callback missing required 'status' parameter"
                    )),
                )
                .await;
                return Html(state.error_html.clone());
            },
        }
    }

    if params.status.as_deref() == Some("pending") {
        if let Some(checkout_session_id) = params.checkout_session_id.clone() {
            CliService::info("Payment confirmed, waiting for provisioning...");

            let api_client = Arc::clone(&state.api_client);
            let tx = Arc::clone(&state.tx);
            let transaction_id = params
                .transaction_id
                .clone()
                .unwrap_or_else(|| checkout_session_id.clone());

            tokio::spawn(async move {
                match wait_for_checkout_provisioning(&api_client, &checkout_session_id).await {
                    Ok(prov_result) => {
                        let result = Ok(CheckoutCallbackResult {
                            transaction_id,
                            tenant_id: prov_result.event.tenant_id,
                            fly_app_name: prov_result.event.fly_app_name,
                            needs_deploy: prov_result.needs_deploy,
                        });
                        send_result(&tx, result).await;
                    },
                    Err(e) => {
                        send_result(&tx, Err(e)).await;
                    },
                }
            });

            return Html(state.waiting_html.clone());
        }

        send_result(
            &state.tx,
            Err(anyhow!("Pending status but no checkout_session_id")),
        )
        .await;
        return Html(state.error_html.clone());
    }

    send_result(
        &state.tx,
        Err(anyhow!("Missing transaction_id or tenant_id in callback")),
    )
    .await;
    Html(state.error_html.clone())
}

async fn send_result(
    tx: &Arc<Mutex<Option<oneshot::Sender<Result<CheckoutCallbackResult>>>>>,
    result: Result<CheckoutCallbackResult>,
) {
    if let Some(sender) = tx.lock().await.take() {
        if sender.send(result).is_err() {
            tracing::warn!("Checkout result receiver dropped");
        }
    }
}

struct CheckoutProvisioningResult {
    event: CheckoutEvent,
    needs_deploy: bool,
}

async fn wait_for_checkout_provisioning(
    client: &CloudApiClient,
    checkout_session_id: &str,
) -> Result<CheckoutProvisioningResult> {
    let mut stream = client.subscribe_checkout_events(checkout_session_id);

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                if let Some(msg) = &event.message {
                    CliService::info(msg);
                }

                match event.event_type {
                    ProvisioningEventType::InfrastructureReady => {
                        return Ok(CheckoutProvisioningResult {
                            event,
                            needs_deploy: true,
                        });
                    },
                    ProvisioningEventType::TenantReady => {
                        return Ok(CheckoutProvisioningResult {
                            event,
                            needs_deploy: false,
                        });
                    },
                    ProvisioningEventType::ProvisioningFailed => {
                        return Err(anyhow!(
                            "Provisioning failed: {}",
                            event.message.as_deref().unwrap_or("Unknown error")
                        ));
                    },
                    _ => {},
                }
            },
            Err(e) => {
                return Err(anyhow!("SSE stream error: {}", e));
            },
        }
    }

    Err(anyhow!("SSE stream closed unexpectedly"))
}

async fn status_handler(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
) -> Json<StatusResponse> {
    match state.api_client.get_tenant_status(&tenant_id).await {
        Ok(status) => Json(StatusResponse {
            status: status.status,
            message: status.message,
            app_url: status.app_url,
        }),
        Err(e) => Json(StatusResponse {
            status: "error".to_string(),
            message: Some(e.to_string()),
            app_url: None,
        }),
    }
}
