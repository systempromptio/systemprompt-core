use super::types::{WebhookConfig, WebhookDeliveryResult, WebhookStats, WebhookTestResult};
use super::WebhookService;
use crate::models::external_integrations::{IntegrationError, IntegrationResult};
use serde_json::Value;
use std::collections::HashMap;

impl WebhookService {
    pub async fn send_webhook(
        &self,
        url: &str,
        payload: Value,
        config: Option<WebhookConfig>,
    ) -> IntegrationResult<WebhookDeliveryResult> {
        let config = config.unwrap_or_default();

        let mut request_builder = self
            .http_client
            .post(url)
            .json(&payload)
            .header("Content-Type", "application/json")
            .header("User-Agent", "SystemPrompt-Webhook/1.0");

        for (key, value) in &config.headers {
            request_builder = request_builder.header(key, value);
        }

        if let Some(secret) = &config.secret {
            let signature = self.generate_signature(secret, &payload)?;
            request_builder = request_builder.header("X-Webhook-Signature", signature);
        }

        if let Some(timeout) = config.timeout {
            request_builder = request_builder.timeout(timeout);
        }

        let start_time = std::time::Instant::now();

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers: HashMap<String, String> = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();

                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<error reading response: {}>", e));
                let duration = start_time.elapsed();

                Ok(WebhookDeliveryResult {
                    success: status >= 200 && status < 300,
                    status_code: status,
                    response_body: body,
                    response_headers: headers,
                    duration_ms: duration.as_millis() as u64,
                    error: None,
                })
            },
            Err(e) => {
                let duration = start_time.elapsed();
                Ok(WebhookDeliveryResult {
                    success: false,
                    status_code: 0,
                    response_body: String::new(),
                    response_headers: HashMap::new(),
                    duration_ms: duration.as_millis() as u64,
                    error: Some(e.to_string()),
                })
            },
        }
    }

    pub async fn get_endpoint_stats(&self, endpoint_id: &str) -> IntegrationResult<WebhookStats> {
        let endpoint = {
            let endpoints = self.endpoints.read().await;
            endpoints.get(endpoint_id).cloned().ok_or_else(|| {
                IntegrationError::Webhook(format!("Endpoint not found: {endpoint_id}"))
            })?
        };

        Ok(WebhookStats {
            endpoint_id: endpoint.id,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            last_request_at: None,
            average_response_time_ms: 0,
        })
    }

    pub async fn test_endpoint(&self, endpoint_id: &str) -> IntegrationResult<WebhookTestResult> {
        let endpoint = {
            let endpoints = self.endpoints.read().await;
            endpoints.get(endpoint_id).cloned().ok_or_else(|| {
                IntegrationError::Webhook(format!("Endpoint not found: {endpoint_id}"))
            })?
        };

        let test_payload = serde_json::json!({
            "test": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "endpoint_id": endpoint_id
        });

        let config = WebhookConfig {
            secret: endpoint.secret.clone(),
            headers: endpoint.headers.clone(),
            timeout: Some(std::time::Duration::from_secs(10)),
        };

        let result = self
            .send_webhook(&endpoint.url, test_payload, Some(config))
            .await?;

        Ok(WebhookTestResult {
            endpoint_id: endpoint.id,
            success: result.success,
            status_code: result.status_code,
            response_time_ms: result.duration_ms,
            error: result.error,
        })
    }
}
