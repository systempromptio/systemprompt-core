mod delivery;
mod types;

pub use delivery::send_webhook;
pub use types::{
    RetryPolicy, WebhookConfig, WebhookDeliveryResult, WebhookStats, WebhookTestResult,
};

use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::external_integrations::{
    IntegrationError, IntegrationResult, WebhookEndpoint, WebhookRequest, WebhookResponse,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug)]
pub struct WebhookService {
    pub(crate) endpoints: RwLock<HashMap<String, WebhookEndpoint>>,
    pub(crate) http_client: Client,
}

impl WebhookService {
    pub fn new() -> Self {
        Self {
            endpoints: RwLock::new(HashMap::new()),
            http_client: Client::new(),
        }
    }

    pub async fn register_endpoint(
        &self,
        mut endpoint: WebhookEndpoint,
    ) -> IntegrationResult<String> {
        if endpoint.id.is_empty() {
            endpoint.id = Uuid::new_v4().to_string();
        }

        let endpoint_id = endpoint.id.clone();

        {
            let mut endpoints = self.endpoints.write().await;
            endpoints.insert(endpoint_id.clone(), endpoint);
        }

        Ok(endpoint_id)
    }

    pub async fn update_endpoint(&self, endpoint: WebhookEndpoint) -> IntegrationResult<()> {
        {
            let mut endpoints = self.endpoints.write().await;
            endpoints.insert(endpoint.id.clone(), endpoint);
        }
        Ok(())
    }

    pub async fn get_endpoint(
        &self,
        endpoint_id: &str,
    ) -> IntegrationResult<Option<WebhookEndpoint>> {
        let endpoints = self.endpoints.read().await;
        Ok(endpoints.get(endpoint_id).cloned())
    }

    pub async fn list_endpoints(&self) -> IntegrationResult<Vec<WebhookEndpoint>> {
        let endpoints = self.endpoints.read().await;
        Ok(endpoints.values().cloned().collect())
    }

    pub async fn remove_endpoint(&self, endpoint_id: &str) -> IntegrationResult<bool> {
        let mut endpoints = self.endpoints.write().await;
        Ok(endpoints.remove(endpoint_id).is_some())
    }

    pub async fn handle_webhook(
        &self,
        endpoint_id: &str,
        request: WebhookRequest,
    ) -> IntegrationResult<WebhookResponse> {
        let endpoint = {
            let endpoints = self.endpoints.read().await;
            endpoints.get(endpoint_id).cloned().ok_or_else(|| {
                IntegrationError::Webhook(format!("Endpoint not found: {endpoint_id}"))
            })?
        };

        if !endpoint.active {
            return Ok(WebhookResponse {
                status: 404,
                body: Some(serde_json::json!({"error": "Endpoint is inactive"})),
            });
        }

        if let (Some(_secret), Some(signature)) = (&endpoint.secret, &request.signature) {
            if !self.verify_signature_internal(&endpoint, &request.body, signature)? {
                return Ok(WebhookResponse {
                    status: 401,
                    body: Some(serde_json::json!({"error": "Invalid signature"})),
                });
            }
        }

        let event_type = request
            .headers
            .get("x-webhook-event")
            .or_else(|| request.headers.get("x-event-type"))
            .or_else(|| request.headers.get("x-github-event"))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        if !endpoint.events.is_empty()
            && !endpoint.events.contains(&event_type)
            && !endpoint.events.contains(&"*".to_string())
        {
            return Ok(WebhookResponse {
                status: 200,
                body: Some(serde_json::json!({"message": "Event type not subscribed"})),
            });
        }

        Ok(WebhookResponse {
            status: 200,
            body: Some(serde_json::json!({
                "message": "Webhook processed successfully",
                "event_type": event_type,
                "endpoint_id": endpoint_id
            })),
        })
    }

    pub async fn verify_signature(
        &self,
        endpoint_id: &str,
        payload: &Value,
        signature: &str,
    ) -> IntegrationResult<bool> {
        let endpoint = {
            let endpoints = self.endpoints.read().await;
            endpoints.get(endpoint_id).cloned().ok_or_else(|| {
                IntegrationError::Webhook(format!("Endpoint not found: {endpoint_id}"))
            })?
        };

        self.verify_signature_internal(&endpoint, payload, signature)
    }

    pub(crate) fn verify_signature_internal(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &Value,
        signature: &str,
    ) -> IntegrationResult<bool> {
        let secret = endpoint.secret.as_ref().ok_or_else(|| {
            IntegrationError::Webhook("No secret configured for endpoint".to_string())
        })?;

        let expected_signature = self.generate_signature(secret, payload)?;

        Ok(self.secure_compare(&expected_signature, signature))
    }

    pub(crate) fn generate_signature(
        &self,
        secret: &str,
        payload: &Value,
    ) -> IntegrationResult<String> {
        let payload_bytes = serde_json::to_vec(payload)?;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| IntegrationError::Webhook(format!("Invalid secret: {e}")))?;

        mac.update(&payload_bytes);
        let result = mac.finalize();
        let hex_result = hex::encode(result.into_bytes());

        Ok(format!("sha256={hex_result}"))
    }

    fn secure_compare(&self, a: &str, b: &str) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for (byte_a, byte_b) in a.bytes().zip(b.bytes()) {
            result |= byte_a ^ byte_b;
        }

        result == 0
    }
}

impl Default for WebhookService {
    fn default() -> Self {
        Self::new()
    }
}
