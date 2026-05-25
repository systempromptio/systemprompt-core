//! Axum handlers and provisioning watcher for the checkout flow.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::{Html, Json};
use futures::StreamExt;
use systemprompt_logging::CliService;
use tokio::sync::{Mutex, oneshot};

use super::{AppState, CallbackParams, CheckoutCallbackResult, StatusResponse};
use crate::CloudApiClient;
use crate::api_client::{CheckoutEvent, ProvisioningEventType};
use crate::error::{CloudError, CloudResult};

pub(super) async fn callback_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallbackParams>,
) -> Html<String> {
    if let Some(error) = &params.error {
        tracing::error!(error = %error, "Checkout error from callback");
        send_result(
            &state.tx,
            Err(CloudError::CheckoutFlow {
                message: format!("Checkout error: {error}"),
            }),
        )
        .await;
        return Html(state.error_template.clone());
    }

    if let (Some(transaction_id), Some(tenant_id)) =
        (params.transaction_id.clone(), params.tenant_id.clone())
    {
        match params.status.as_deref() {
            Some("completed") => {
                let html = state
                    .success_template
                    .replace("{{TENANT_ID}}", tenant_id.as_str());
                let result = Ok(CheckoutCallbackResult {
                    transaction_id,
                    tenant_id,
                    fly_app_name: None,
                    needs_deploy: false,
                });
                send_result(&state.tx, result).await;
                return Html(html);
            },
            Some(status) => {
                send_result(
                    &state.tx,
                    Err(CloudError::CheckoutFlow {
                        message: format!("Checkout status: {status}"),
                    }),
                )
                .await;
                return Html(state.error_template.clone());
            },
            None => {
                send_result(
                    &state.tx,
                    Err(CloudError::CheckoutFlow {
                        message: "Checkout callback missing required 'status' parameter"
                            .to_string(),
                    }),
                )
                .await;
                return Html(state.error_template.clone());
            },
        }
    }

    if params.status.as_deref() == Some("pending") {
        if let Some(checkout_session_id) = params.checkout_session_id.clone() {
            CliService::info("Payment confirmed, waiting for provisioning...");

            let api_client = Arc::clone(&state.api_client);
            let tx = Arc::clone(&state.tx);
            let transaction_id = params.transaction_id.clone().unwrap_or_else(|| {
                systemprompt_identifiers::TransactionId::new(checkout_session_id.as_str())
            });

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

            return Html(state.waiting_template.clone());
        }

        send_result(
            &state.tx,
            Err(CloudError::CheckoutFlow {
                message: "Pending status but no checkout_session_id".to_owned(),
            }),
        )
        .await;
        return Html(state.error_template.clone());
    }

    send_result(
        &state.tx,
        Err(CloudError::CheckoutFlow {
            message: "Missing transaction_id or tenant_id in callback".to_owned(),
        }),
    )
    .await;
    Html(state.error_template.clone())
}

pub(super) async fn send_result(
    tx: &Arc<Mutex<Option<oneshot::Sender<CloudResult<CheckoutCallbackResult>>>>>,
    result: CloudResult<CheckoutCallbackResult>,
) {
    let sender = tx.lock().await.take();
    if let Some(sender) = sender {
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
    checkout_session_id: &systemprompt_identifiers::CheckoutSessionId,
) -> CloudResult<CheckoutProvisioningResult> {
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
                        return Err(CloudError::ProvisioningFailed {
                            message: event.message.unwrap_or_else(|| "Unknown error".to_owned()),
                        });
                    },
                    _ => {},
                }
            },
            Err(e) => {
                return Err(e);
            },
        }
    }

    Err(CloudError::SseStream {
        message: "SSE stream closed unexpectedly".to_owned(),
    })
}

pub(super) async fn status_handler(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
) -> Json<StatusResponse> {
    let tenant_id = systemprompt_identifiers::TenantId::new(tenant_id);
    match state.api_client.get_tenant_status(&tenant_id).await {
        Ok(status) => Json(StatusResponse {
            status: status.status,
            message: status.message,
            app_url: status.app_url,
        }),
        Err(e) => Json(StatusResponse {
            status: "error".to_owned(),
            message: Some(e.to_string()),
            app_url: None,
        }),
    }
}
