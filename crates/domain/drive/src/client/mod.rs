//! [`DriveClient`]: one account's Drive over the instance's document routes (PROTOCOL section 12).
//!
//! Every call takes a fresh enough JWT from the account's [`CredentialSource`]
//! through [`with_jwt`], which mints once more and retries when the instance
//! refuses the token. A write can name the conversation it belongs to
//! (`x-context-id`, an id the instance issued with
//! [`DriveClient::create_context`]). A file up to
//! [`INLINE_UPLOAD_LIMIT`](sp_protocol::wire::drive::INLINE_UPLOAD_LIMIT) goes
//! up as one multipart request; a bigger one as a resumable upload that is then
//! imported by its blob.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod collaboration;
mod documents;
mod types;
mod uploads;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use sp_client::{CredentialSource, HttpClient, SpClient, with_jwt};
use sp_protocol::ids::DocumentId;
use sp_protocol::wire::paths;
use url::Url;

pub use types::{
    ContentFormat, DocumentBytes, EditOutcome, ImportFile, ListQuery, ReadView, RenderedPage,
};

use crate::error::{DriveError, classify, decode_failure};
use crate::http::{Answer, Call, route, send_once};

const LIST_BODY: usize = 8 * 1024 * 1024;
const READ_BODY: usize = 4 * 1024 * 1024;
const PAGE_BODY: usize = 16 * 1024 * 1024;
const CONTENT_BODY: usize = 64 * 1024 * 1024;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

/// One account's Drive. Cheap to clone; the credential stays with the
/// [`CredentialSource`].
#[derive(Clone)]
pub struct DriveClient {
    inner: Arc<Inner>,
}

struct Inner {
    base: String,
    origin: Url,
    http: HttpClient,
    credentials: Arc<dyn CredentialSource>,
}

impl fmt::Debug for DriveClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriveClient")
            .field("base", &self.inner.base)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "the instance expects {} as the body, not null"
)]
struct EmptyBody {}

impl DriveClient {
    #[must_use]
    pub fn new(
        instance: &SpClient,
        http: HttpClient,
        credentials: Arc<dyn CredentialSource>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                base: instance.base().to_owned(),
                origin: instance.origin().clone(),
                http,
                credentials,
            }),
        }
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.inner.base
    }

    #[must_use]
    pub fn origin(&self) -> &Url {
        &self.inner.origin
    }

    fn url(&self, path: &str, segments: &[&str]) -> Result<Url, DriveError> {
        route(&self.inner.base, path, segments).map_err(DriveError::Client)
    }

    fn document_url(&self, id: &DocumentId, rest: &[&str]) -> Result<Url, DriveError> {
        let id = id.to_string();
        let mut segments = Vec::with_capacity(rest.len() + 1);
        segments.push(id.as_str());
        segments.extend_from_slice(rest);
        self.url(paths::DOCUMENTS, &segments)
    }

    async fn answer(&self, call: Call, expect: &[u16]) -> Result<Answer, DriveError> {
        let http = self.inner.http.reqwest();
        let call = &call;
        let answer = with_jwt(&*self.inner.credentials, |jwt| send_once(http, call, jwt)).await?;
        if !expect.contains(&answer.status) {
            return Err(classify(
                call.endpoint,
                answer.status,
                &answer.headers,
                &answer.body,
            ));
        }
        Ok(answer)
    }

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        call: Call,
        expect: &[u16],
    ) -> Result<T, DriveError> {
        let endpoint = call.endpoint;
        let answer = self.answer(call, expect).await?;
        decode(endpoint, &answer)
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    endpoint: &'static str,
    answer: &Answer,
) -> Result<T, DriveError> {
    serde_json::from_slice(&answer.body)
        .map_err(|error| DriveError::Client(decode_failure(endpoint, answer.status, &error)))
}

fn revision_of(answer: &Answer) -> Option<String> {
    let raw = answer.header("etag")?;
    let tag = raw.trim().trim_start_matches("W/").trim_matches('"');
    (!tag.is_empty()).then(|| tag.to_owned())
}

fn strip_empty_query(url: &mut Url) {
    if url.query().is_some_and(str::is_empty) {
        url.set_query(None);
    }
}
