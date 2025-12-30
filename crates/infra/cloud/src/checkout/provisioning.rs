use anyhow::{anyhow, Result};
use futures::StreamExt;
use std::time::Duration;

use crate::api_client::{ProvisioningEvent, ProvisioningEventType};
use crate::CloudApiClient;

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
                        fly_app_name: None,
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
