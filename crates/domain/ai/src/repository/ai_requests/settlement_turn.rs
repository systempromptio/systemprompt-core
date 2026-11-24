//! The assistant turn a settled completion records: its tool calls and text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Postgres, Transaction};
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::text::without_nul;
use systemprompt_traits::RepositoryError;

use super::settlement::tokens;
use super::settlement_outcome::SettleCompletion;

pub(super) async fn turn_recorded(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &AiRequestId,
    completion: &SettleCompletion<'_>,
) -> Result<bool, RepositoryError> {
    let recorded = sqlx::query_scalar!(
        r#"
        SELECT (
            EXISTS (SELECT 1 FROM ai_request_tool_calls WHERE request_id = $1)
            OR EXISTS (
                SELECT 1 FROM ai_request_messages
                WHERE request_id = $1 AND role = 'assistant' AND sequence_number = $2
            )
        ) AS "recorded!"
        "#,
        request_id.as_str(),
        completion.next_message_sequence,
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(recorded)
}

pub(super) async fn insert_turn(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &AiRequestId,
    completion: &SettleCompletion<'_>,
) -> Result<(), RepositoryError> {
    for (index, tool) in completion.tool_calls.iter().enumerate() {
        let name = without_nul(&tool.name);
        let input = without_nul(&tool.input);
        let error_path = tool
            .args
            .as_ref()
            .and_then(|args| args.error_path.as_deref())
            .map(without_nul);
        sqlx::query!(
            r#"
            INSERT INTO ai_request_tool_calls (
                request_id, ai_tool_call_id, tool_name, tool_input, sequence_number,
                args_validity, args_error_path
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            request_id.as_str(),
            tool.id.as_str(),
            name.as_ref(),
            input.as_ref(),
            tokens(u32::try_from(index + 1).unwrap_or(u32::MAX))?,
            tool.args.as_ref().map(|args| args.validity.as_str()),
            error_path.as_deref()
        )
        .execute(&mut **tx)
        .await?;
    }
    match (completion.assistant_text, completion.next_message_sequence) {
        (Some(text), Some(sequence)) => {
            let text = without_nul(text);
            sqlx::query!(
                r#"
                INSERT INTO ai_request_messages (request_id, role, content, sequence_number)
                VALUES ($1, 'assistant', $2, $3)
                "#,
                request_id.as_str(),
                text.as_ref(),
                sequence
            )
            .execute(&mut **tx)
            .await?;
        },
        (Some(_), None) => tracing::warn!(
            request_id = %request_id,
            "assistant text not recorded: the receipt predates the carried message count"
        ),
        (None, _) => {},
    }
    Ok(())
}
