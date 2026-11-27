//! The question tool: asks the user one to four multiple-choice questions
//! through the shell and waits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use harness_ids::QuestionId;
use harness_prompt::engine_tools::{
    QUESTION_DESCRIPTION, QUESTION_MAX_OPTIONS, QUESTION_MAX_QUESTIONS, QUESTION_MIN_OPTIONS,
    question_schema,
};
use harness_prompt::tool_names::QUESTION;
use harness_tools::{PermissionRequest, Tool, ToolContext, ToolError, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::events::{Question, QuestionAsk};
use crate::inner::Inner;

pub(crate) struct QuestionTool {
    inner: Arc<Inner>,
}

impl QuestionTool {
    pub(crate) const fn new(inner: Arc<Inner>) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    questions: Vec<Question>,
}

#[async_trait]
impl Tool for QuestionTool {
    fn name(&self) -> &str {
        QUESTION
    }

    fn description(&self) -> String {
        QUESTION_DESCRIPTION.to_owned()
    }

    // JSON: the tool's input schema is JSON Schema, sent to the model as it is.
    fn schema(&self) -> Value {
        question_schema()
    }

    // JSON: the object the model wrote, read into this tool's input.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input =
            serde_json::from_value(input).map_err(|error| ToolError::malformed("", error))?;
        check(&input.questions)?;
        ctx.ask(PermissionRequest {
            permission: QUESTION.to_owned(),
            patterns: vec!["*".to_owned()],
            always: vec!["*".to_owned()],
            title: "Ask the user".to_owned(),
            metadata: json!({ "questions": input.questions.len() }),
        })
        .await?;
        let ask = QuestionAsk {
            id: QuestionId::mint(),
            session: ctx.session_id.clone(),
            call_id: ctx.call_id.clone(),
            questions: input.questions.clone(),
        };
        let answers = match self
            .inner
            .asks
            .question(&self.inner, ask, &ctx.cancel)
            .await
        {
            None => return Err(ToolError::Aborted),
            Some(None) => return Err(ToolError::Rejected),
            Some(Some(answers)) => answers,
        };
        let text = render(&input.questions, &answers);
        Ok(ToolOutput::new("Asked the user", text).with_metadata(json!({ "answers": answers })))
    }
}

fn check(questions: &[Question]) -> Result<(), ToolError> {
    if questions.is_empty() || questions.len() > QUESTION_MAX_QUESTIONS {
        let message = format!("ask between 1 and {QUESTION_MAX_QUESTIONS} questions");
        return Err(ToolError::invalid(message));
    }
    let allowed = QUESTION_MIN_OPTIONS..=QUESTION_MAX_OPTIONS;
    if let Some(question) = questions
        .iter()
        .find(|question| !allowed.contains(&question.options.len()))
    {
        return Err(ToolError::invalid(format!(
            "\"{}\" needs between {QUESTION_MIN_OPTIONS} and {QUESTION_MAX_OPTIONS} options",
            question.question
        )));
    }
    Ok(())
}

fn render(questions: &[Question], answers: &[Vec<String>]) -> String {
    questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let answer = answers
                .get(index)
                .filter(|chosen| !chosen.is_empty())
                .map(|chosen| chosen.join(", "));
            let shown = answer.as_deref().unwrap_or("(no answer)");
            format!("{}\nAnswer: {shown}", question.question)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
