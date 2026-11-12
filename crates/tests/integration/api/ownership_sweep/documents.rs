use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use serde_json::json;
use systemprompt_api::routes::documents::{DocumentsState, folders_router};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use uuid::Uuid;

use super::engine::{Fixture, send, sweep};
use super::table::Family;
use crate::common::{browser_page, request_context};
use crate::documents::{app, create_markdown, fixture, id_of, json_request};

struct Documents {
    state: DocumentsState,
    owner: AuthedFixture,
    stranger: AuthedFixture,
    id: String,
}

impl Fixture for Documents {
    fn owner(&self) -> Router {
        app(&self.state, &self.owner)
    }

    fn stranger(&self) -> Router {
        app(&self.state, &self.stranger)
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/{}", self.id))
    }

    fn body_limit(&self) -> usize {
        self.state.max_body_bytes()
    }

    fn stranger_id(&self) -> String {
        self.stranger.user_id.as_str().to_owned()
    }
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

fn folders(state: &DocumentsState, user: &AuthedFixture) -> Router {
    browser_page(folders_router(state.clone())).layer(Extension(
        request_context(user.user_id.as_str()).with_browser_session(),
    ))
}

struct Folders {
    state: DocumentsState,
    owner: AuthedFixture,
    stranger: AuthedFixture,
    id: String,
}

impl Fixture for Folders {
    fn owner(&self) -> Router {
        folders(&self.state, &self.owner)
    }

    fn stranger(&self) -> Router {
        folders(&self.state, &self.stranger)
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/{}/grants", self.id))
    }

    fn body_limit(&self) -> usize {
        self.state.max_body_bytes()
    }
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_document_route() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "sweep-doc-owner").await?;
    let stranger = seed_bridge_credential(&pool, "sweep-doc-stranger").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    sweep(
        &Documents {
            state,
            owner,
            stranger,
            id,
        },
        Family::Documents,
    )
    .await
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_folder_route() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "sweep-folder-owner").await?;
    let stranger = seed_bridge_credential(&pool, "sweep-folder-stranger").await?;
    let body = json!({ "name": format!("sweep-{}", Uuid::new_v4().simple()) });
    let created = send(
        folders(&state, &owner),
        json_request(Method::POST, "/", &body),
    )
    .await?;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let created: serde_json::Value = serde_json::from_str(&created.body)?;
    let id = created["id"].as_str().expect("folder id").to_owned();
    sweep(
        &Folders {
            state,
            owner,
            stranger,
            id,
        },
        Family::Folders,
    )
    .await
}
