//! The `files.documents.externalSharing` section: whether a document may be
//! shared outside the instance, as a read-only public link
//! (`publicLinks`) or an invite addressed to an email (`invites`). Both are
//! off by default.
//!
//! `denyFormats` and `denyFolders` withhold external sharing from
//! documents of those formats and from every document beneath those folders
//! (a folder deny is inherited by its whole subtree). The policy is
//! re-checked on every open of a link or invite, so tightening it closes
//! links already handed out. Core sends no mail: an invite's link is
//! returned once to the sharer, who delivers it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::documents::DocumentFormat;

const DEFAULT_MAX_LINK_DAYS: u32 = 30;
const MAX_LINK_DAYS_CEILING: u32 = 365;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSharingConfig {
    #[serde(default)]
    pub public_links: bool,
    #[serde(default)]
    pub invites: bool,
    #[serde(default)]
    pub deny_formats: Vec<DocumentFormat>,
    #[serde(default)]
    pub deny_folders: Vec<DocumentFolderId>,
    #[serde(default = "default_max_link_days")]
    pub max_link_days: u32,
}

/// Which external channel a check is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalChannel {
    PublicLink,
    Invite,
}

const fn default_max_link_days() -> u32 {
    DEFAULT_MAX_LINK_DAYS
}

impl Default for ExternalSharingConfig {
    fn default() -> Self {
        Self {
            public_links: false,
            invites: false,
            deny_formats: Vec::new(),
            deny_folders: Vec::new(),
            max_link_days: DEFAULT_MAX_LINK_DAYS,
        }
    }
}

impl ExternalSharingConfig {
    #[must_use]
    pub const fn channel_enabled(&self, channel: ExternalChannel) -> bool {
        match channel {
            ExternalChannel::PublicLink => self.public_links,
            ExternalChannel::Invite => self.invites,
        }
    }

    #[must_use]
    pub fn allows(
        &self,
        channel: ExternalChannel,
        format: DocumentFormat,
        folder_path: &[DocumentFolderId],
    ) -> bool {
        self.channel_enabled(channel)
            && !self.deny_formats.contains(&format)
            && !folder_path.iter().any(|f| self.deny_folders.contains(f))
    }

    #[must_use]
    pub fn link_days(&self, requested: Option<u32>) -> u32 {
        let ceiling = self.max_link_days.clamp(1, MAX_LINK_DAYS_CEILING);
        requested.map_or(ceiling, |days| days.clamp(1, ceiling))
    }
}
