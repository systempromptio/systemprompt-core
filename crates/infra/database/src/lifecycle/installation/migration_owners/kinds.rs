//! The statement kinds the migration reader judges, and the kinds of SQL it
//! cannot see through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// The kind of statement a migration changes an object with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableMutation {
    Alter,
    Drop,
    Truncate,
    Rename,
    SetSchema,
    MoveAll,
    DropSchema,
    RenameSchema,
    GrantInSchema,
    CreateTrigger,
    CreateRule,
    CreatePolicy,
    AlterPolicy,
    CreateIndex,
    DropTrigger,
    DropPolicy,
    DropRule,
    DropIndex,
    AlterIndex,
    DropObject,
    DropCascade,
    Grant,
    Revoke,
    Insert,
    Update,
    Delete,
    Merge,
    Copy,
}

impl TableMutation {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Alter => "ALTER TABLE",
            Self::Drop => "DROP TABLE",
            Self::Truncate => "TRUNCATE",
            Self::Rename => "RENAME",
            Self::SetSchema => "SET SCHEMA",
            Self::MoveAll => "ALTER TABLE ALL IN TABLESPACE",
            Self::DropSchema => "DROP SCHEMA",
            Self::RenameSchema => "ALTER SCHEMA RENAME",
            Self::GrantInSchema => "GRANT/REVOKE ON ALL IN SCHEMA",
            Self::CreateTrigger => "CREATE TRIGGER",
            Self::CreateRule => "CREATE RULE",
            Self::CreatePolicy => "CREATE POLICY",
            Self::AlterPolicy => "ALTER POLICY",
            Self::CreateIndex => "CREATE INDEX",
            Self::DropTrigger => "DROP TRIGGER",
            Self::DropPolicy => "DROP POLICY",
            Self::DropRule => "DROP RULE",
            Self::DropIndex => "DROP INDEX",
            Self::AlterIndex => "ALTER INDEX",
            Self::DropObject => "DROP",
            Self::DropCascade => "DROP ... CASCADE",
            Self::Grant => "GRANT",
            Self::Revoke => "REVOKE",
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Merge => "MERGE",
            Self::Copy => "COPY",
        }
    }
}

impl std::fmt::Display for TableMutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A statement whose effect the parser cannot attribute to an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HiddenSql {
    Do,
    Call,
    RoutineCall,
    Execute,
    CopyProgram,
    Routine { language: String },
    UnparseableBody,
    Unclassified { kind: String },
}

impl std::fmt::Display for HiddenSql {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Do => f.write_str("DO"),
            Self::Call => f.write_str("CALL"),
            Self::RoutineCall => f.write_str("SELECT of a routine"),
            Self::Execute => f.write_str("EXECUTE"),
            Self::CopyProgram => f.write_str("COPY from a file or program"),
            Self::Routine { language } => write!(f, "CREATE FUNCTION in language {language}"),
            Self::UnparseableBody => f.write_str("sql routine with an unparseable body"),
            Self::Unclassified { kind } => write!(f, "unclassified {kind}"),
        }
    }
}
