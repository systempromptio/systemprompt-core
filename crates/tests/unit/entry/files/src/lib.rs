//! Tests for `systemprompt-files-entry`: the `core files` commands, their
//! database-scope guard, the state they resolve and the validation report a
//! failing `validate` carries; and the document routes' same-origin actor
//! scope and the collab update forwarding path.

#[cfg(test)]
mod common;
#[cfg(test)]
mod core_dispatchers_db;
#[cfg(test)]
mod core_files_ai_count_db;
#[cfg(test)]
mod core_files_ai_delete_db;
#[cfg(test)]
mod core_files_ai_output_lifecycle;
#[cfg(test)]
mod core_files_dispatcher_db;
#[cfg(test)]
mod core_files_show_db;
#[cfg(test)]
mod core_files_upload_db;
#[cfg(test)]
mod documents_actor_scope;
#[cfg(test)]
mod documents_collab_forward_path;
