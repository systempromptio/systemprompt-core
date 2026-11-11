use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_api::routes::documents::DocumentsState;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_models::documents::content_hash;
use systemprompt_test_fixtures::{AuthedFixture, seed_drive_credential};
use uuid::Uuid;

use super::engine::{Fixture, get, send, sweep, with_bearer};
use super::table::Family;
use crate::common::setup_ctx;
use crate::documents::{app, commit_meta, drive_commit};

struct Drive {
    gateway: Router,
    documents: DocumentsState,
    owner: AuthedFixture,
    stranger: AuthedFixture,
    id: String,
    sha: String,
}

impl Fixture for Drive {
    fn owner(&self) -> Router {
        with_bearer(self.gateway.clone(), self.owner.jwt.as_str())
    }

    fn stranger(&self) -> Router {
        with_bearer(self.gateway.clone(), self.stranger.jwt.as_str())
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/{}", self.id))
    }

    fn view_router(&self) -> Router {
        app(&self.documents, &self.owner)
    }

    fn body_limit(&self) -> usize {
        self.documents.max_body_bytes()
    }

    fn primary_for(&self, template: &str) -> (String, String) {
        if template.contains("/blob/") {
            let absent = content_hash(Uuid::new_v4().as_bytes());
            (self.sha.clone(), absent.as_str().to_owned())
        } else {
            (self.id.clone(), self.absent())
        }
    }
}

#[tokio::test]
async fn a_second_device_user_is_refused_every_owned_drive_route() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let gateway = gateway_router(&ctx)?;
    let documents = DocumentsState::build(&ctx)?;
    let owner = seed_drive_credential(&pool, "sweep-drive-owner").await?;
    let stranger = seed_drive_credential(&pool, "sweep-drive-stranger").await?;
    let path = format!("sweep-{}.md", Uuid::new_v4().simple());
    let blob = format!("# {path}\n");
    let committed = drive_commit(
        &gateway,
        &owner,
        &commit_meta(&path, blob.as_bytes(), None),
        blob.as_bytes(),
        None,
    )
    .await?;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.json());
    let manifest = send(
        with_bearer(gateway.clone(), owner.jwt.as_str()),
        get("/bridge/drive/manifest"),
    )
    .await?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest.body)?;
    let entry = manifest["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|e| e["path"] == path.as_str()))
        .expect("committed entry in the owner's manifest");
    let id = entry["document_id"]
        .as_str()
        .expect("document id")
        .to_owned();
    let sha = content_hash(blob.as_bytes()).as_str().to_owned();
    sweep(
        &Drive {
            gateway,
            documents,
            owner,
            stranger,
            id,
            sha,
        },
        Family::Drive,
    )
    .await
}
