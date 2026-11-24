//! The title of the plane listing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::PLANE_NOT_APPLIED_HINT;
use systemprompt_manifest::profile::UnappliedPlaneMode;

pub(super) fn planes_title(unapplied: &[&str], mode: UnappliedPlaneMode) -> String {
    if unapplied.is_empty() {
        return "Config planes".to_owned();
    }
    let effect = if mode.denies() {
        "their requests are denied"
    } else {
        "their requests pass ungoverned"
    };
    format!(
        "Config planes — NOT APPLIED: {} (unapplied_planes: {}, {effect}); {}",
        unapplied.join(", "),
        mode.as_str(),
        PLANE_NOT_APPLIED_HINT
    )
}
