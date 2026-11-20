//! Names that collide: a unique-constraint failure on a document's or a
//! folder's name read back as `NameTaken` or `FolderNameTaken`, and the
//! extension split the free-name suffixes are inserted before.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{ConstraintKind, RepositoryError};

use crate::documents::error::DocumentsError;

const FOLDER_NAME_INDEX: &str = "uq_document_folders_parent_name";

pub(super) fn split_extension(name: &str) -> (&str, &str) {
    name.rfind('.')
        .filter(|&dot| dot > 0)
        .map_or((name, ""), |dot| name.split_at(dot))
}

pub(super) fn name_taken(error: RepositoryError, name: &str) -> DocumentsError {
    if is_name_taken(&error) {
        DocumentsError::NameTaken {
            name: name.to_owned(),
        }
    } else {
        DocumentsError::Repository(error)
    }
}

pub(super) fn folder_name_taken(error: RepositoryError, name: &str) -> DocumentsError {
    if is_folder_name_taken(&error) {
        DocumentsError::FolderNameTaken {
            name: name.to_owned(),
        }
    } else {
        DocumentsError::Repository(error)
    }
}

pub(super) fn is_folder_name_taken(error: &RepositoryError) -> bool {
    matches!(
        error,
        RepositoryError::Constraint {
            kind: ConstraintKind::Unique,
            constraint,
            ..
        } if constraint == FOLDER_NAME_INDEX
    )
}

pub(super) const fn is_name_taken(error: &RepositoryError) -> bool {
    matches!(
        error,
        RepositoryError::Constraint {
            kind: ConstraintKind::Unique,
            ..
        }
    )
}
