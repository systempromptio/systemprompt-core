//! The content checks an assembled upload passes before it becomes a blob.
//!
//! [`inspect`] sniffs the first bytes against the declared MIME type, then
//! runs the check the bytes call for, chosen from the bytes themselves: any
//! upload that opens a zip archive passes the zip guard whatever it was
//! declared as, and the kind of check passed is recorded on the upload so
//! an import can insist on it. The checks: the `office` zip-bomb guard, which
//! inflates every entry into a counter as the archive streams past, the `image`
//! dimension cap and metadata strip, or the `svg` active-content refusal.
//! Images and SVG are read whole, bounded by their own size limits; every
//! other kind is checked without being buffered. A
//! type with no check of its own is stored as an opaque blob.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod directory;
mod image;
mod office;
mod reader;
pub(crate) mod sniff;
mod svg;

use futures_util::StreamExt;
use systemprompt_traits::{ByteRange, FileStorage, StoredFileId};

use self::sniff::{Kind, declared_kind, detected_mime, is_zip, matches, sniff};
use super::error::{RefusalReason, UploadError, UploadResult};
use crate::config::UploadLimits;

const SNIFF_BYTES: u64 = 8 * 1024;

/// What an upload passed as: the MIME type recorded for it and, when the
/// check rewrote it, the bytes to store in place of the upload.
#[derive(Debug)]
pub(crate) struct Inspected {
    pub(crate) detected_mime: String,
    pub(crate) checked: Kind,
    pub(crate) replacement: Option<Vec<u8>>,
}

pub(crate) struct Subject<'a> {
    pub(crate) storage: &'a dyn FileStorage,
    pub(crate) id: &'a StoredFileId,
    pub(crate) size: u64,
    pub(crate) declared_mime: &'a str,
}

pub(crate) async fn read_range(
    storage: &dyn FileStorage,
    id: &StoredFileId,
    range: ByteRange,
) -> UploadResult<Vec<u8>> {
    let mut read = storage.open(id, Some(range)).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = read.body.next().await {
        bytes.extend_from_slice(&chunk?);
    }
    Ok(bytes)
}

async fn read_bounded(
    subject: &Subject<'_>,
    limit: u64,
    what: RefusalReason,
) -> UploadResult<Vec<u8>> {
    if subject.size > limit {
        return Err(UploadError::refused(
            what,
            format!(
                "{} bytes exceed the {limit}-byte limit for this type",
                subject.size
            ),
        ));
    }
    read_range(subject.storage, subject.id, ByteRange::From(0)).await
}

pub(crate) async fn inspect(
    subject: &Subject<'_>,
    limits: &UploadLimits,
) -> UploadResult<Inspected> {
    let expected = declared_kind(subject.declared_mime).ok_or_else(|| {
        UploadError::refused(
            RefusalReason::UnsupportedType,
            format!("{} is not a MIME type", subject.declared_mime),
        )
    })?;
    let head = read_range(
        subject.storage,
        subject.id,
        ByteRange::Bounded {
            start: 0,
            end: SNIFF_BYTES.min(subject.size) - 1,
        },
    )
    .await?;
    if !matches(expected, &head) {
        return Err(UploadError::refused(
            RefusalReason::MimeMismatch,
            format!(
                "declared {} but the content is {}",
                subject.declared_mime,
                sniff(&head).map_or("an unrecognised type", |kind| detected_mime(kind, "text"))
            ),
        ));
    }
    let checked = if is_zip(&head) {
        Kind::Zip
    } else if expected == Kind::Opaque && sniff(&head) == Some(Kind::Text) {
        Kind::Text
    } else {
        expected
    };
    let replacement = match checked {
        Kind::Zip => {
            office::check(subject.storage, subject.id, subject.size, limits).await?;
            None
        },
        Kind::Png | Kind::Jpeg | Kind::Gif | Kind::Webp => {
            let bytes =
                read_bounded(subject, limits.image_max_bytes, RefusalReason::ImageLimits).await?;
            let stripped = image::sanitize(checked, &bytes, limits)?;
            (stripped != bytes).then_some(stripped)
        },
        Kind::Svg => {
            let bytes =
                read_bounded(subject, limits.svg_max_bytes, RefusalReason::UnsafeSvg).await?;
            svg::check(&bytes)?;
            None
        },
        Kind::Pdf | Kind::Text | Kind::Opaque => None,
    };
    Ok(Inspected {
        detected_mime: detected_mime(expected, subject.declared_mime).to_owned(),
        checked,
        replacement,
    })
}
