use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use uuid::Uuid;

use super::engine::{Fixture, get, send, sweep, with_bearer};
use super::table::Family;
use crate::gateway_provider_credentials::{app, request, token};

struct Credentials {
    gateway: Router,
    owner: String,
    stranger: String,
    label: String,
}

impl Fixture for Credentials {
    fn owner(&self) -> Router {
        with_bearer(self.gateway.clone(), &self.owner)
    }

    fn stranger(&self) -> Router {
        with_bearer(self.gateway.clone(), &self.stranger)
    }

    fn resource(&self) -> &str {
        &self.label
    }

    fn owner_view(&self) -> Request<Body> {
        get("/me/provider-credentials")
    }
}

#[tokio::test]
async fn a_second_user_cannot_read_replace_or_remove_another_users_provider_key() -> Result<()> {
    let gateway = app().await?;
    let owner = token(&format!(
        "sweep-byok-owner-{}@example.invalid",
        Uuid::new_v4().simple()
    ))
    .await?;
    let stranger = token(&format!(
        "sweep-byok-stranger-{}@example.invalid",
        Uuid::new_v4().simple()
    ))
    .await?;
    let label = format!("sweep-{}", Uuid::new_v4().simple());
    let put = serde_json::json!({ "api_key": "good-key", "label": label }).to_string();
    let stored = send(
        gateway.clone(),
        request(
            "PUT",
            "/me/provider-credentials/byok",
            Some(&owner),
            Some(put),
        ),
    )
    .await?;
    assert_eq!(stored.status, StatusCode::OK, "{}", stored.body);
    sweep(
        &Credentials {
            gateway,
            owner,
            stranger,
            label,
        },
        Family::ProviderCredentials,
    )
    .await
}
