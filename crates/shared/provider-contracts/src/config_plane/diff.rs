//! Generic declared-versus-current drift over two [`PlaneSnapshot`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use super::PlaneSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
    Missing,
    Changed,
    Extra,
}

impl DriftKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Changed => "changed",
            Self::Extra => "extra",
        }
    }
}

/// One entity whose database value differs from the declaration.
///
/// `declared` is `None` for a [`DriftKind::Extra`] row (only the database has
/// it) and `current` is `None` for a [`DriftKind::Missing`] row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftRow {
    pub entity: String,
    pub kind: DriftKind,
    pub declared: Option<String>,
    pub current: Option<String>,
}

#[must_use]
pub fn diff_snapshots(declared: &PlaneSnapshot, current: &PlaneSnapshot) -> Vec<DriftRow> {
    let mut rows = Vec::new();
    for (entity, want) in declared {
        match current.get(entity) {
            None => rows.push(DriftRow {
                entity: entity.clone(),
                kind: DriftKind::Missing,
                declared: Some(want.clone()),
                current: None,
            }),
            Some(have) if have != want => rows.push(DriftRow {
                entity: entity.clone(),
                kind: DriftKind::Changed,
                declared: Some(want.clone()),
                current: Some(have.clone()),
            }),
            Some(_) => {},
        }
    }
    for (entity, have) in current {
        if !declared.contains_key(entity) {
            rows.push(DriftRow {
                entity: entity.clone(),
                kind: DriftKind::Extra,
                declared: None,
                current: Some(have.clone()),
            });
        }
    }
    rows
}
