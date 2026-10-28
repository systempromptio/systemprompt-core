//! Stored write acknowledgements: an edit that carries an `op_id` stores
//! its ack, and a retry with the same `op_id` returns it marked `replayed`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::StoredWriteAck;

use super::outputs::EditOutput;
use super::support::tool_error;
use super::write::EditCall;

pub(super) async fn replay(call: &EditCall<'_>) -> Result<Option<(EditOutput, String)>, McpError> {
    let Some(op_id) = call.op_id.as_ref() else {
        return Ok(None);
    };
    let Some(stored) = call
        .tools
        .service()
        .find_write_ack(call.who, &call.document, op_id, call.ctx.trace_id())
        .await
        .map_err(tool_error)?
    else {
        return Ok(None);
    };
    let mut output: EditOutput = serde_json::from_value(stored.ack).map_err(|error| {
        tracing::error!(error = %error, op_id = %op_id, "Stored write ack does not decode");
        McpError::internal_error("the stored acknowledgement could not be read", None)
    })?;
    output.replayed = true;
    let summary = format!(
        "revision {}: op {op_id} already applied",
        output.revision_id
    );
    Ok(Some((output, summary)))
}

pub(super) async fn remember(call: &EditCall<'_>, output: &EditOutput) {
    let Some(op_id) = call.op_id.as_ref() else {
        return;
    };
    let ack = match serde_json::to_value(output) {
        Ok(ack) => ack,
        Err(error) => {
            tracing::warn!(error = %error, op_id = %op_id, "Write ack not stored");
            return;
        },
    };
    let stored = StoredWriteAck {
        revision_id: output.revision_id.clone(),
        ack,
    };
    if let Err(error) = call
        .tools
        .service()
        .record_write_ack(&call.document, op_id, &stored)
        .await
    {
        tracing::warn!(error = %error, op_id = %op_id, "Write ack not stored");
    }
}
