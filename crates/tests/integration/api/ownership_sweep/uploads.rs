use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use systemprompt_api::routes::documents::DocumentsState;
use systemprompt_api::routes::uploads::router;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};

use super::engine::{Fixture, get, sweep};
use super::table::Family;
use crate::common::{request_context, setup_ctx};
use crate::routes_uploads::open;

struct Uploads {
    state: DocumentsState,
    owner: AuthedFixture,
    stranger: AuthedFixture,
    id: String,
    part_limit: usize,
}

impl Fixture for Uploads {
    fn owner(&self) -> Router {
        router(self.state.clone()).layer(Extension(request_context(self.owner.user_id.as_str())))
    }

    fn stranger(&self) -> Router {
        router(self.state.clone()).layer(Extension(request_context(self.stranger.user_id.as_str())))
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/{}", self.id))
    }

    fn oversize_status_for(&self, template: &str) -> StatusCode {
        if template.contains("/parts/") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::PAYLOAD_TOO_LARGE
        }
    }

    fn body_limit_for(&self, _method: &str, template: &str) -> usize {
        if template.contains("/parts/") {
            self.part_limit
        } else {
            self.body_limit()
        }
    }
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_upload_route() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "sweep-upload-owner").await?;
    let stranger = seed_bridge_credential(&pool, "sweep-upload-stranger").await?;
    let id = open(&state, &owner).await?;
    let part_size = ctx.documents().uploads().limits().part_size_bytes;
    let part_limit = usize::try_from(part_size + 64 * 1024)?;
    sweep(
        &Uploads {
            state,
            owner,
            stranger,
            id,
            part_limit,
        },
        Family::Uploads,
    )
    .await
}
