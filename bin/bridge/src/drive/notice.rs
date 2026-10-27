//! The words of a document notice toast.
//!
//! The document is named by its Drive path when this device has synced it,
//! else by the name the event carries, else by its id; the sender by the
//! display name the event carries, else by user id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::status::DriveStatus;
use crate::proxy::drive_signal::{DocumentNotice, NoticeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeText {
    pub title_key: &'static str,
    pub body_key: &'static str,
    pub args: Vec<(&'static str, String)>,
}

#[must_use]
pub fn notice_text(status: &DriveStatus, notice: &DocumentNotice) -> NoticeText {
    let document = status
        .proposals
        .iter()
        .find(|p| p.document_id == notice.document_id)
        .and_then(|p| p.path.clone())
        .or_else(|| {
            status
                .editors
                .iter()
                .find(|e| e.document_id == notice.document_id)
                .and_then(|e| e.path.clone())
        })
        .or_else(|| notice.document_name.clone())
        .unwrap_or_else(|| notice.document_id.to_string());
    let from = notice
        .from_name
        .clone()
        .unwrap_or_else(|| notice.from.to_string());
    match notice.kind {
        NoticeKind::Shared => NoticeText {
            title_key: "drive-shared-title",
            body_key: "drive-shared-body",
            args: vec![
                ("from", from),
                ("document", document),
                ("level", notice.detail.clone().unwrap_or_default()),
            ],
        },
        NoticeKind::ProposalOpened => NoticeText {
            title_key: "drive-proposal-title",
            body_key: "drive-proposal-body",
            args: vec![("from", from), ("document", document)],
        },
    }
}
