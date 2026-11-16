//! What `FilesExtension` contributes: its metadata, schemas, user-purge rows,
//! jobs and job run scope, and the three files slots it opens.

use std::any::TypeId;

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_files::FilesExtension;
use systemprompt_files::contract::{AgentAccessSlot, BlobHolderSlot, DocumentsSlot};
use systemprompt_identifiers::ExtensionId;

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("files"), &mut sink);
    FilesExtension
        .contribute(&mut c)
        .expect("the files member contributes");
    sink
}

fn table_names(sink: &ContributionSink) -> Vec<&str> {
    sink.schemas()
        .iter()
        .filter_map(|s| s.table.as_deref())
        .collect()
}

#[test]
fn test_files_extension_metadata() {
    let metadata = FilesExtension.metadata();
    assert_eq!(metadata.id, "files");
    assert_eq!(metadata.name, "Files");
    assert!(!metadata.version.is_empty());
}

#[test]
fn test_files_extension_schemas_count() {
    assert_eq!(contributed().schemas().len(), 33);
}

#[test]
fn test_files_extension_schemas_names() {
    let sink = contributed();
    let names = table_names(&sink);
    for table in [
        "files",
        "content_files",
        "issue_reports",
        "issue_report_attachments",
        "document_collab_peers",
        "document_upload_objects",
        "document_media",
        "document_blob_collections",
    ] {
        assert!(names.contains(&table), "missing schema {table}");
    }
}

#[test]
fn test_files_extension_required_columns() {
    let sink = contributed();
    let required = |table: &str| {
        sink.schemas()
            .iter()
            .find(|s| s.table.as_deref() == Some(table))
            .map(|s| s.required_columns.clone())
            .expect("schema present")
    };
    let files = required("files");
    for column in ["id", "path", "mime_type", "created_at"] {
        assert!(files.contains(&column.to_owned()));
    }
    let content_files = required("content_files");
    for column in ["id", "content_id", "file_id"] {
        assert!(content_files.contains(&column.to_owned()));
    }
}

#[test]
fn test_files_extension_schemas_have_sql() {
    for schema in contributed().schemas() {
        assert!(
            !schema.sql.is_empty(),
            "Schema {} has empty SQL",
            schema.table.as_deref().unwrap_or("<table-less>")
        );
    }
}

#[test]
fn test_files_extension_user_purge_rows() {
    let sink = contributed();
    let purged: Vec<(&str, &str)> = sink
        .user_purges()
        .iter()
        .map(|p| (p.table, p.column))
        .collect();
    assert!(purged.contains(&("issue_reports", "reporter_user_id")));
    assert!(purged.contains(&("document_locks", "holder_user_id")));
    let grants = sink
        .user_purges()
        .iter()
        .find(|p| p.table == "document_grants")
        .expect("document grants are purged");
    assert_eq!(grants.predicate, Some("subject_kind = 'user'"));
}

#[test]
fn test_files_extension_contributes_jobs_and_run_scope() {
    let sink = contributed();
    assert!(sink.jobs().len() >= 10, "the files jobs are contributed");
    assert_eq!(sink.job_run_scopes().len(), 1);
}

#[test]
fn test_files_extension_opens_its_three_slots() {
    let sink = contributed();
    let opened: Vec<TypeId> = sink.slots().iter().map(|record| record.slot()).collect();
    for slot in [
        TypeId::of::<BlobHolderSlot>(),
        TypeId::of::<AgentAccessSlot>(),
        TypeId::of::<DocumentsSlot>(),
    ] {
        assert!(opened.contains(&slot));
    }
}
