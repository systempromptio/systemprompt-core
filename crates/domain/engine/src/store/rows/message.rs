//! Rows of the `message` table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rusqlite::{Connection, OptionalExtension, Params, Row, params};

use crate::error::{Result, corrupt};
use crate::id::{MessageId, SessionId, column, optional_column, optional_param, param};
use crate::model::{AssistantMessage, FinishReason, Message, Usage, UserMessage};
use crate::rows::{to_i64, to_u64};

const MESSAGE_COLUMNS: &str = "id, session_id, role, agent, model, parent_id, is_system, error, \
     finish, input_tokens, output_tokens, reasoning_tokens, cache_read_tokens, \
     cache_write_tokens, created_at, completed_at";

const UPSERT_MESSAGE: &str = "INSERT INTO message (id, session_id, role, agent, model, \
       parent_id, is_system, error, finish, input_tokens, output_tokens, reasoning_tokens, \
       cache_read_tokens, cache_write_tokens, created_at, completed_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16) \
     ON CONFLICT(id) DO UPDATE SET agent = excluded.agent, model = excluded.model, \
       parent_id = excluded.parent_id, is_system = excluded.is_system, error = excluded.error, \
       finish = excluded.finish, input_tokens = excluded.input_tokens, \
       output_tokens = excluded.output_tokens, reasoning_tokens = excluded.reasoning_tokens, \
       cache_read_tokens = excluded.cache_read_tokens, \
       cache_write_tokens = excluded.cache_write_tokens, completed_at = excluded.completed_at";

struct MessageRow {
    id: MessageId,
    session_id: SessionId,
    role: String,
    agent: String,
    model: String,
    parent_id: Option<MessageId>,
    system: bool,
    error: Option<String>,
    finish: Option<String>,
    usage: [i64; 5],
    created_at: i64,
    completed_at: Option<i64>,
}

fn message_row(row: &Row<'_>) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: column(row, 0)?,
        session_id: column(row, 1)?,
        role: row.get(2)?,
        agent: row.get(3)?,
        model: row.get(4)?,
        parent_id: optional_column(row, 5)?,
        system: row.get(6)?,
        error: row.get(7)?,
        finish: row.get(8)?,
        usage: [
            row.get(9)?,
            row.get(10)?,
            row.get(11)?,
            row.get(12)?,
            row.get(13)?,
        ],
        created_at: row.get(14)?,
        completed_at: row.get(15)?,
    })
}

impl MessageRow {
    fn decode(self) -> Result<Message> {
        let is_user = match self.role.as_str() {
            "user" => true,
            "assistant" => false,
            other => return Err(corrupt("message.role", other)),
        };
        if is_user {
            Ok(Message::User(self.into_user()))
        } else {
            Ok(Message::Assistant(self.into_assistant()?))
        }
    }

    fn into_user(self) -> UserMessage {
        UserMessage {
            id: self.id,
            session_id: self.session_id,
            agent: self.agent,
            model: self.model,
            system: self.system,
            created_at: self.created_at,
        }
    }

    fn into_assistant(self) -> Result<AssistantMessage> {
        let Some(parent_id) = self.parent_id else {
            return Err(corrupt("message.parent_id", self.id.to_string()));
        };
        let finish = self
            .finish
            .map(|value| {
                FinishReason::parse(&value).ok_or_else(|| corrupt("message.finish", value.as_str()))
            })
            .transpose()?;
        let [input, output, reasoning, cache_read, cache_write] = self.usage;
        Ok(AssistantMessage {
            id: self.id,
            session_id: self.session_id,
            parent_id,
            agent: self.agent,
            model: self.model,
            system: self.system,
            error: self
                .error
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            usage: Usage {
                input: to_u64(input),
                output: to_u64(output),
                reasoning: to_u64(reasoning),
                cache_read: to_u64(cache_read),
                cache_write: to_u64(cache_write),
            },
            finish,
            created_at: self.created_at,
            completed_at: self.completed_at,
        })
    }
}

pub(crate) fn query_messages(
    connection: &Connection,
    tail: &str,
    params: impl Params,
) -> Result<Vec<Message>> {
    let mut statement =
        connection.prepare(&format!("SELECT {MESSAGE_COLUMNS} FROM message {tail}"))?;
    let rows = statement
        .query_map(params, message_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(MessageRow::decode).collect()
}

pub(crate) fn load_message(connection: &Connection, id: &MessageId) -> Result<Option<Message>> {
    connection
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM message WHERE id = ?1"),
            params![param(id)],
            message_row,
        )
        .optional()?
        .map(MessageRow::decode)
        .transpose()
}

pub(crate) fn write_message(connection: &Connection, message: &Message) -> Result<bool> {
    let existed: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM message WHERE id = ?1)",
        params![param(message.id())],
        |row| row.get(0),
    )?;
    let (parent_id, error, finish, usage, completed_at) = match message {
        Message::User(_) => (None, None, None, Usage::default(), None),
        Message::Assistant(assistant) => (
            Some(&assistant.parent_id),
            assistant
                .error
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            assistant.finish.map(FinishReason::as_str),
            assistant.usage,
            assistant.completed_at,
        ),
    };
    connection.execute(
        UPSERT_MESSAGE,
        params![
            param(message.id()),
            param(message.session_id()),
            message.role().as_str(),
            message.agent(),
            message.model(),
            optional_param(parent_id),
            message.is_system(),
            error,
            finish,
            to_i64(usage.input),
            to_i64(usage.output),
            to_i64(usage.reasoning),
            to_i64(usage.cache_read),
            to_i64(usage.cache_write),
            message.created_at(),
            completed_at,
        ],
    )?;
    Ok(!existed)
}
