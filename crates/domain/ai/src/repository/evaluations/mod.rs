use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::a2a::{DataPart, FilePart, FileWithBytes, Message, Part, TextPart};
use systemprompt_models::ConversationEvaluation;

#[derive(sqlx::FromRow)]
struct TaskMessageRow {
    pub message_id: String,
    pub task_id: String,
    pub role: String,
    pub context_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
struct MessagePartRow {
    pub part_kind: String,
    pub text_content: Option<String>,
    pub file_name: Option<String>,
    pub file_mime_type: Option<String>,
    pub file_bytes: Option<String>,
    pub data_content: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct EvaluationRepository {
    pool: Arc<PgPool>,
}

impl EvaluationRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn create_evaluation(&self, eval: &ConversationEvaluation) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO conversation_evaluations (
                context_id, agent_goal, goal_achieved, goal_achievement_confidence,
                goal_achievement_notes, primary_category, topics_discussed, keywords,
                user_satisfied, conversation_quality, quality_notes, issues_encountered,
                agent_name, total_turns, conversation_duration_seconds, user_initiated,
                completion_status, overall_score, evaluation_summary, analysis_version
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            "#,
            eval.context_id.as_str(),
            eval.agent_goal,
            eval.goal_achieved,
            eval.goal_achievement_confidence as f32,
            eval.goal_achievement_notes,
            eval.primary_category,
            eval.topics_discussed,
            eval.keywords,
            eval.user_satisfied,
            eval.conversation_quality,
            eval.quality_notes,
            eval.issues_encountered,
            eval.agent_name,
            eval.total_turns,
            eval.conversation_duration_seconds,
            eval.user_initiated,
            eval.completion_status,
            eval.overall_score as f32,
            eval.evaluation_summary,
            eval.analysis_version
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_evaluation_by_context(
        &self,
        context_id: &ContextId,
    ) -> anyhow::Result<Option<ConversationEvaluation>> {
        sqlx::query_as!(
            ConversationEvaluation,
            r#"
            SELECT id, context_id, agent_goal, goal_achieved,
                   goal_achievement_confidence, goal_achievement_notes,
                   primary_category, topics_discussed, keywords,
                   user_satisfied, conversation_quality, quality_notes, issues_encountered,
                   agent_name, total_turns, conversation_duration_seconds, user_initiated,
                   completion_status, overall_score,
                   evaluation_summary, analyzed_at, analysis_version
            FROM conversation_evaluations
            WHERE context_id = $1
            LIMIT 1
            "#,
            context_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_unevaluated_conversations(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT c.context_id, c.created_at
            FROM user_contexts c
            LEFT JOIN conversation_evaluations e ON c.context_id = e.context_id
            WHERE e.id IS NULL
              AND EXISTS (
                SELECT 1 FROM task_messages m
                JOIN agent_tasks t ON m.task_id = t.task_id
                WHERE t.context_id = c.context_id
              )
            ORDER BY c.created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "context_id": row.context_id,
                    "created_at": row.created_at,
                })
            })
            .collect())
    }

    pub async fn get_messages_by_context(
        &self,
        context_id: &ContextId,
    ) -> anyhow::Result<Vec<Message>> {
        let message_rows: Vec<TaskMessageRow> = sqlx::query_as!(
            TaskMessageRow,
            r#"SELECT
                m.message_id as "message_id!",
                m.task_id as "task_id!",
                m.role as "role!",
                m.context_id,
                m.metadata
            FROM task_messages m
            JOIN agent_tasks t ON m.task_id = t.task_id
            WHERE t.context_id = $1
            ORDER BY m.created_at ASC"#,
            context_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut messages = Vec::new();

        for row in message_rows {
            let parts = self.get_message_parts(&row.message_id).await?;

            messages.push(Message {
                role: row.role,
                id: row.message_id.into(),
                task_id: Some(TaskId::new(row.task_id)),
                context_id: ContextId::new(
                    row.context_id.unwrap_or_else(|| context_id.to_string()),
                ),
                kind: "message".to_string(),
                parts,
                metadata: row.metadata,
                extensions: None,
                reference_task_ids: None,
            });
        }

        Ok(messages)
    }

    async fn get_message_parts(&self, message_id: &str) -> anyhow::Result<Vec<Part>> {
        let part_rows: Vec<MessagePartRow> = sqlx::query_as!(
            MessagePartRow,
            r#"SELECT
                part_kind as "part_kind!",
                text_content,
                file_name,
                file_mime_type,
                file_bytes,
                data_content
            FROM message_parts WHERE message_id = $1 ORDER BY sequence_number ASC"#,
            message_id
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut parts = Vec::new();

        for row in part_rows {
            let part = match row.part_kind.as_str() {
                "text" => {
                    let text = row.text_content.unwrap_or_default();
                    Part::Text(TextPart { text })
                },
                "file" => {
                    let bytes = row.file_bytes.unwrap_or_default();
                    Part::File(FilePart {
                        file: FileWithBytes {
                            name: row.file_name,
                            mime_type: row.file_mime_type,
                            bytes,
                        },
                    })
                },
                "data" => {
                    let data_value = row
                        .data_content
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                    let data = if let serde_json::Value::Object(map) = data_value {
                        map
                    } else {
                        serde_json::Map::new()
                    };
                    Part::Data(DataPart { data })
                },
                _ => continue,
            };

            parts.push(part);
        }

        Ok(parts)
    }
}
