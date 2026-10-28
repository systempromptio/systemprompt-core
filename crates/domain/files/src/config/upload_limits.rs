//! The `files.documents.uploads` section: the resumable upload API's part
//! size, session lifetime, per-user quota and the content-safety limits an
//! upload must pass before it becomes a blob, each with a floor and a ceiling
//! an operator may set between.
//!
//! The quota is per user over a rolling day; the install is the tenant, so
//! these limits are the tenant's. The largest single upload is
//! `files.documents.maxUploadBytes`, the same limit an import enforces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

const PART_SIZE_BOUNDS: (u64, u64) = (MIB, 64 * MIB);
const SESSION_TTL_HOURS_BOUNDS: (u64, u64) = (1, 168);
const DAILY_QUOTA_BOUNDS: (u64, u64) = (64 * MIB, 1024 * GIB);
const OPEN_SESSIONS_BOUNDS: (u64, u64) = (1, 256);
const INLINE_BOUNDS: (u64, u64) = (64 * KIB, 16 * MIB);
const OFFICE_ENTRIES_BOUNDS: (u64, u64) = (16, 1_000_000);
const OFFICE_UNCOMPRESSED_BOUNDS: (u64, u64) = (MIB, 8 * GIB);
const OFFICE_RATIO_BOUNDS: (u64, u64) = (2, 1_000);
const IMAGE_DIMENSION_BOUNDS: (u64, u64) = (256, 65_535);
const IMAGE_BYTES_BOUNDS: (u64, u64) = (MIB, 256 * MIB);
const SVG_BYTES_BOUNDS: (u64, u64) = (KIB, 16 * MIB);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UploadLimits {
    pub part_size_bytes: u64,
    pub session_ttl_hours: u64,
    pub daily_quota_bytes: u64,
    pub max_open_sessions: u64,
    pub inline_max_bytes: u64,
    pub office_max_entries: u64,
    pub office_max_uncompressed_bytes: u64,
    pub office_max_ratio: u64,
    pub image_max_dimension: u64,
    pub image_max_bytes: u64,
    pub svg_max_bytes: u64,
}

impl Default for UploadLimits {
    fn default() -> Self {
        Self {
            part_size_bytes: 8 * MIB,
            session_ttl_hours: 24,
            daily_quota_bytes: 4 * GIB,
            max_open_sessions: 8,
            inline_max_bytes: 5 * MIB,
            office_max_entries: 10_000,
            office_max_uncompressed_bytes: 512 * MIB,
            office_max_ratio: 100,
            image_max_dimension: 16_384,
            image_max_bytes: 64 * MIB,
            svg_max_bytes: 2 * MIB,
        }
    }
}

impl UploadLimits {
    #[must_use]
    pub const fn bounded(&self) -> [(&'static str, u64, (u64, u64)); 11] {
        [
            ("partSizeBytes", self.part_size_bytes, PART_SIZE_BOUNDS),
            (
                "sessionTtlHours",
                self.session_ttl_hours,
                SESSION_TTL_HOURS_BOUNDS,
            ),
            (
                "dailyQuotaBytes",
                self.daily_quota_bytes,
                DAILY_QUOTA_BOUNDS,
            ),
            (
                "maxOpenSessions",
                self.max_open_sessions,
                OPEN_SESSIONS_BOUNDS,
            ),
            ("inlineMaxBytes", self.inline_max_bytes, INLINE_BOUNDS),
            (
                "officeMaxEntries",
                self.office_max_entries,
                OFFICE_ENTRIES_BOUNDS,
            ),
            (
                "officeMaxUncompressedBytes",
                self.office_max_uncompressed_bytes,
                OFFICE_UNCOMPRESSED_BOUNDS,
            ),
            ("officeMaxRatio", self.office_max_ratio, OFFICE_RATIO_BOUNDS),
            (
                "imageMaxDimension",
                self.image_max_dimension,
                IMAGE_DIMENSION_BOUNDS,
            ),
            ("imageMaxBytes", self.image_max_bytes, IMAGE_BYTES_BOUNDS),
            ("svgMaxBytes", self.svg_max_bytes, SVG_BYTES_BOUNDS),
        ]
    }
}
