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

use crate::api_client::{ProvisioningEvent, ProvisioningEventType};
use crate::constants::checkout::{CALLBACK_PORT, CALLBACK_TIMEOUT_SECS};
use crate::CloudApiClient;

#[derive(Debug, Deserialize)]
struct CallbackParams {
    transaction_id: Option<String>,
    tenant_id: Option<String>,
    status: Option<String>,
    error: Option<String>,
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
}

#[derive(Debug, Clone, Copy)]
pub struct CheckoutTemplates {
    pub success_html: &'static str,
    pub error_html: &'static str,
}

struct AppState {
    tx: Arc<Mutex<Option<oneshot::Sender<Result<CheckoutCallbackResult>>>>>,
    api_client: Arc<CloudApiClient>,
    success_html: String,
    error_html: String,
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
    let result = if let Some(error) = params.error {
        Err(anyhow!("Checkout error: {}", error))
    } else if let (Some(transaction_id), Some(tenant_id)) =
        (params.transaction_id, params.tenant_id)
    {
        match params.status {
            Some(status) if status == "completed" => Ok(CheckoutCallbackResult {
                transaction_id,
                tenant_id,
            }),
            Some(status) => Err(anyhow!("Checkout status: {}", status)),
            None => Err(anyhow!(
                "Checkout callback missing required 'status' parameter"
            )),
        }
    } else {
        Err(anyhow!("Missing transaction_id or tenant_id in callback"))
    };

    state.tx.lock().await.take().map_or_else(
        || Html(state.error_html.clone()),
        |sender| {
            let is_success = result.is_ok();
            let tenant_id = match &result {
                Ok(r) => r.tenant_id.clone(),
                Err(e) => {
                    tracing::error!(error = %e, "Checkout failed, tenant ID unavailable");
                    String::new()
                },
            };

            if sender.send(result).is_err() {
                tracing::warn!(
                    "Checkout result receiver dropped - client may not receive payment \
                     confirmation"
                );
            }

            if is_success {
                let html = state.success_html.replace("{{TENANT_ID}}", &tenant_id);
                Html(html)
            } else {
                Html(state.error_html.clone())
            }
        },
    )
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

pub async fn wait_for_provisioning<F>(
    client: &CloudApiClient,
    tenant_id: &str,
    on_event: F,
) -> Result<ProvisioningEvent>
where
    F: Fn(&ProvisioningEvent),
{
    let mut stream = client.subscribe_provisioning_events(tenant_id);

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                on_event(&event);

                match event.event_type {
                    ProvisioningEventType::TenantReady => return Ok(event),
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
                tracing::warn!(error = %e, "SSE stream error, falling back to polling");
                return wait_for_provisioning_polling(client, tenant_id).await;
            },
        }
    }

    tracing::warn!("SSE stream closed unexpectedly, falling back to polling");
    wait_for_provisioning_polling(client, tenant_id).await
}

async fn wait_for_provisioning_polling(
    client: &CloudApiClient,
    tenant_id: &str,
) -> Result<ProvisioningEvent> {
    const MAX_ATTEMPTS: u32 = 60;
    const POLL_INTERVAL_SECS: u64 = 2;

    for attempt in 0..MAX_ATTEMPTS {
        match client.get_tenant_status(tenant_id).await {
            Ok(status) => match status.status.as_str() {
                "ready" => {
                    return Ok(ProvisioningEvent {
                        tenant_id: tenant_id.to_string(),
                        event_type: ProvisioningEventType::TenantReady,
                        status: "ready".to_string(),
                        message: status.message,
                        app_url: status.app_url,
                    });
                },
                "failed" => {
                    return Err(anyhow!(
                        "Provisioning failed: {}",
                        status.message.as_deref().unwrap_or("Unknown error")
                    ));
                },
                _ => {
                    tracing::debug!(
                        attempt = attempt,
                        status = %status.status,
                        "Polling provisioning status"
                    );
                    tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                },
            },
            Err(e) => {
                tracing::warn!(error = %e, attempt = attempt, "Failed to get tenant status");
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            },
        }
    }

    Err(anyhow!(
        "Provisioning timed out after {} seconds",
        MAX_ATTEMPTS * POLL_INTERVAL_SECS as u32
    ))
}
