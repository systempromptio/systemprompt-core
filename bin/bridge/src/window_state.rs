//! Persisted settings-window placement.
//!
//! Lives outside `gui` — which only compiles on macOS and Windows — so the
//! placement rules stay reachable from the Linux test workspace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::paths;
use crate::fsutil;

pub const MIN_WIDTH: u32 = 800;
pub const MIN_HEIGHT: u32 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WorkArea {
    #[must_use]
    pub const fn intersects(&self, geom: &WindowGeometry) -> bool {
        let (ax0, ay0) = (self.x, self.y);
        let ax1 = self.x.saturating_add_unsigned(self.width);
        let ay1 = self.y.saturating_add_unsigned(self.height);
        let (bx0, by0) = (geom.x, geom.y);
        let bx1 = geom.x.saturating_add_unsigned(geom.width);
        let by1 = geom.y.saturating_add_unsigned(geom.height);
        ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
    }
}

#[must_use]
pub const fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (
        if width < MIN_WIDTH { MIN_WIDTH } else { width },
        if height < MIN_HEIGHT {
            MIN_HEIGHT
        } else {
            height
        },
    )
}

// Why: returning `None` is what makes an unplugged second monitor fall back to
// OS centring instead of restoring the window off-screen, where it cannot be
// reached.
#[must_use]
pub fn restore(saved: WindowGeometry, work_areas: &[WorkArea]) -> Option<WindowGeometry> {
    if work_areas.is_empty() {
        return None;
    }
    if !work_areas.iter().any(|a| a.intersects(&saved)) {
        return None;
    }
    let (width, height) = clamp_size(saved.width, saved.height);
    Some(WindowGeometry {
        width,
        height,
        ..saved
    })
}

fn path() -> Option<PathBuf> {
    paths::bridge_metadata_dir().map(|d| d.join(paths::WINDOW_STATE_SENTINEL))
}

#[must_use]
pub fn load() -> Option<WindowGeometry> {
    let bytes = std::fs::read(path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(geom: WindowGeometry) {
    let Some(path) = path() else {
        return;
    };
    let Ok(bytes) = serde_json::to_vec_pretty(&geom) else {
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::debug!(error = %e, "window-state dir unavailable");
        return;
    }
    if let Err(e) = fsutil::atomic_write_0600(&path, &bytes) {
        tracing::debug!(error = %e, "window geometry not persisted");
    }
}
