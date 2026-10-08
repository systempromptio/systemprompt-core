//! The Vertex Model Garden listing call.
//!
//! `GET {host}/v1beta1/publishers/{publisher}/models?pageSize=300`, bearer
//! token, following `nextPageToken` until the catalog is exhausted. The page
//! size is Vertex's maximum — 301 is rejected with HTTP 400 — so it is written
//! as a constant rather than tuned. A page is followed at most `MAX_PAGES`
//! times, so a `nextPageToken` that never clears cannot hold boot open until
//! the caller's timeout fires.
//!
//! `host` is the origin only (`https://us-central1-aiplatform.googleapis.com`).
//! A failed listing is a [`ListingError`] whose Display is the reason the
//! discovery report carries. `list_all` collects failures per
//! publisher rather than stopping at the first — a publisher we are not
//! entitled to answers 403, and that must not cost us the publishers we are
//! entitled to — formatted in exactly the shape
//! [`DiscoveryReport::failed_publishers`](systemprompt_manifest::services::DiscoveryReport)
//! carries.
//!
//! What comes back is Google's *global* catalog, not "what this project may
//! call": there is no project-scoped listing endpoint (the obvious
//! `projects/{p}/locations/{l}/publishers/...` path is a 404). Entitlement is
//! proven only by invoking a model, which is why discovery publishes from the
//! rate card and never claims the listing is an access check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

use super::classify::PublisherModel;

const PAGE_SIZE: &str = "300";

const MAX_PAGES: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum ListingError {
    #[error("listing request failed: {0}")]
    Request(#[source] reqwest::Error),

    #[error("listing returned an unreadable body: {0}")]
    Body(#[source] reqwest::Error),

    #[error("listing returned {status}: {body}")]
    Status {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("listing returned an unreadable body: {0}")]
    Decode(#[from] serde_json::Error),

    #[error(
        "listing did not terminate after {} pages; {read} models read are discarded",
        MAX_PAGES
    )]
    Unterminated { read: usize },
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListPage {
    #[serde(default)]
    publisher_models: Vec<PublisherModel>,

    #[serde(default)]
    next_page_token: Option<String>,
}

pub async fn list_publisher_models(
    http: &reqwest::Client,
    host: &str,
    token: &str,
    publisher: &str,
) -> Result<Vec<PublisherModel>, ListingError> {
    let url = format!(
        "{}/v1beta1/publishers/{publisher}/models",
        host.trim_end_matches('/')
    );
    let mut models = Vec::new();
    let mut page_token: Option<String> = None;

    for _ in 0..MAX_PAGES {
        let mut request = http
            .get(&url)
            .bearer_auth(token)
            .query(&[("pageSize", PAGE_SIZE)]);
        if let Some(token) = page_token.as_deref() {
            request = request.query(&[("pageToken", token)]);
        }

        let response = request.send().await.map_err(ListingError::Request)?;
        let status = response.status();
        let body = response.text().await.map_err(ListingError::Body)?;
        if !status.is_success() {
            return Err(ListingError::Status {
                status,
                body: body.trim().to_owned(),
            });
        }

        let page: ListPage = serde_json::from_str(&body)?;
        models.extend(page.publisher_models);

        match page.next_page_token {
            Some(next) if !next.is_empty() => page_token = Some(next),
            _ => return Ok(models),
        }
    }

    let read = models.len();
    Err(ListingError::Unterminated { read })
}

pub async fn list_all(
    http: &reqwest::Client,
    host: &str,
    token: &str,
    provider: &str,
    publishers: &[String],
) -> (Vec<PublisherModel>, Vec<String>) {
    let mut models = Vec::new();
    let mut failures = Vec::new();
    for publisher in publishers {
        match list_publisher_models(http, host, token, publisher).await {
            Ok(page) => models.extend(page),
            Err(reason) => failures.push(format!("{provider}/{publisher}: {reason}")),
        }
    }
    (models, failures)
}
