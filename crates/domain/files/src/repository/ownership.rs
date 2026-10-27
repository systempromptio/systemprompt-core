//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the files
//! domain: moves every file, document, folder, revision authorship and grant
//! attribution from one user to another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct FilesOwnerReassignment;
    domain = "files";
    update "files" => "UPDATE files SET user_id = $2 WHERE user_id = $1";
    update "documents.owner_user_id" => "UPDATE documents SET owner_user_id = $2 WHERE owner_user_id = $1";
    update "documents.created_by" => "UPDATE documents SET created_by = $2 WHERE created_by = $1";
    update "document_folders.owner_user_id" => "UPDATE document_folders SET owner_user_id = $2 WHERE owner_user_id = $1";
    update "document_folders.created_by" => "UPDATE document_folders SET created_by = $2 WHERE created_by = $1";
    update "document_revisions.author_user_id" => "UPDATE document_revisions SET author_user_id = $2 WHERE author_user_id = $1";
    update "document_grants.granted_by" => "UPDATE document_grants SET granted_by = $2 WHERE granted_by = $1";
}
