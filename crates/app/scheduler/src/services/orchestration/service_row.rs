//! Decoding of a `services` row into a [`DbServiceRecord`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::service_records::DbServiceRecord;
use crate::error::{SchedulerError, SchedulerResult};
use systemprompt_database::JsonRow;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::{ServiceModule, ServiceStatus};
use systemprompt_traits::RepositoryError;

fn required_text<'a>(row: &'a JsonRow, column: &str) -> SchedulerResult<&'a str> {
    row.get(column)
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid_row(format!("row has no `{column}` value")))
}

fn invalid_row(reason: String) -> SchedulerError {
    SchedulerError::Repository(RepositoryError::invalid_data("services", reason))
}

fn decode_error(
    context: &str,
    source: impl std::error::Error + Send + Sync + 'static,
) -> SchedulerError {
    SchedulerError::Repository(RepositoryError::Decode {
        context: context.to_owned(),
        source: Box::new(source),
    })
}

pub(super) fn decode_db_service(row: &JsonRow) -> SchedulerResult<DbServiceRecord> {
    let name = ServiceName::new(required_text(row, "name")?);
    let service_type = required_text(row, "service_type")?
        .parse::<ServiceModule>()
        .map_err(|e| decode_error("services.module_name", e))?;
    let status = required_text(row, "status")?
        .parse::<ServiceStatus>()
        .map_err(|e| decode_error("services.status", e))?;
    let pid = row.get("pid").and_then(serde_json::Value::as_i64);
    let port = row
        .get("port")
        .and_then(serde_json::Value::as_i64)
        .and_then(|p| i32::try_from(p).ok())
        .ok_or_else(|| invalid_row(format!("row `{name}` has no valid `port` value")))?;
    let updated_at_epoch = row
        .get("updated_at_epoch")
        .and_then(serde_json::Value::as_f64);
    Ok(DbServiceRecord {
        name,
        service_type,
        status,
        pid,
        port,
        updated_at_epoch,
    })
}
