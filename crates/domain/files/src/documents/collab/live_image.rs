//! A picture placed in a live session, before any checkpoint holds it.
//!
//! Peers draw an inserted image straight away. It is served under the check a
//! checkpoint applies: the reference has an inserter in this session's update
//! log who uploaded those bytes, and the caller may view the document. Only
//! the node holding the session can answer; elsewhere the picture appears
//! with the next checkpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::CollabService;
use crate::documents::model::MediaBlob;

impl CollabService {
    pub async fn live_image(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        reference: &str,
        trace: &TraceId,
    ) -> Option<MediaBlob> {
        self.authorized(who, document, DocumentAction::View, trace)
            .await
            .ok()?;
        let entry = self.cached(document)?;
        let owners = entry.lock().await.images.clone();
        let uploads = self.uploads.as_deref()?;
        let owners_of = |wanted: &str| owners.inserters(wanted);
        let mut admitted = self
            .documents
            .admit_media(uploads, &owners_of, vec![reference.to_owned()], &[])
            .await;
        admitted.media.remove(reference)
    }
}
