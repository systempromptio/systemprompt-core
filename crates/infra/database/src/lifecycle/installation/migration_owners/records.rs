//! The ownership records an install reads before it judges: one per table
//! (`schema_table_owners`) and one per other object (`schema_object_owners`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::StandingTag;
use systemprompt_identifiers::ExtensionId;

/// One table's owner as `schema_table_owners` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedTable {
    pub table: String,
    pub owner: ExtensionId,
    pub standing: StandingTag,
}

/// The kind of non-table object a `schema_object_owners` record names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectKind {
    Object,
    Index,
    Trigger,
}

impl ObjectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Index => "index",
            Self::Trigger => "trigger",
        }
    }

    #[must_use]
    pub fn parse(kind: &str) -> Option<Self> {
        match kind {
            "object" => Some(Self::Object),
            "index" => Some(Self::Index),
            "trigger" => Some(Self::Trigger),
            _ => None,
        }
    }
}

/// One index, routine, view, sequence, type, domain or trigger's owner as
/// `schema_object_owners` records it. A trigger is named `<trigger> ON
/// <schema>.<table>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedObject {
    pub kind: ObjectKind,
    pub name: String,
    pub owner: ExtensionId,
    pub standing: StandingTag,
}

/// The ownership records an install reads before it judges.
#[derive(Debug, Clone, Copy, Default)]
pub struct Recorded<'a> {
    pub tables: &'a [RecordedTable],
    pub objects: &'a [RecordedObject],
}

impl<'a> Recorded<'a> {
    pub const NONE: Recorded<'static> = Recorded {
        tables: &[],
        objects: &[],
    };

    #[must_use]
    pub const fn tables(tables: &'a [RecordedTable]) -> Self {
        Self {
            tables,
            objects: &[],
        }
    }
}
