//! The brain as agent tools: each validates its input against its schema,
//! asks the gate under `brain`, calls the [`BrainService`] and answers text
//! the model reads. `Tool` is the engine's `dyn` contract, hence
//! `#[async_trait]` on its impl.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use harness_engine::extension::ExtensionTool;
use harness_tools::{PermissionRequest, Tool, ToolContext, ToolError, ToolOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::operations::{Dispute, Lookup, Propose, Why};
use super::render::failure_text;
use crate::error::BrainError;
use crate::service::BrainService;

pub(super) const PERMISSION: &str = "brain";
const INPUT_REFUSED: &str = "The input does not fit this tool; take fact ids from brain_lookup and write subjects as kind:id.";

pub(super) fn all(service: &BrainService) -> Vec<ExtensionTool> {
    vec![
        BrainTool::<Lookup>::offered(service),
        BrainTool::<Why>::offered(service),
        BrainTool::<Propose>::offered(service),
        BrainTool::<Dispute>::offered(service),
    ]
}

pub(super) trait BrainOperation: Send + Sync + 'static {
    type Input: DeserializeOwned + Send;
    const NAME: &'static str;
    const TITLE: &'static str;
    const DESCRIPTION: &'static str;

    // JSON: a tool's input schema is JSON Schema, handed to the model as it is.
    fn schema() -> Value;

    fn target(input: &Self::Input) -> String;

    fn call(
        service: BrainService,
        input: Self::Input,
    ) -> impl Future<Output = Result<String, BrainError>> + Send;
}

struct BrainTool<O> {
    service: BrainService,
    operation: PhantomData<fn() -> O>,
}

impl<O: BrainOperation> BrainTool<O> {
    fn offered(service: &BrainService) -> ExtensionTool {
        let tool: Arc<dyn Tool> = Arc::new(Self {
            service: service.clone(),
            operation: PhantomData,
        });
        ExtensionTool::asking_itself(tool, PERMISSION)
    }
}

#[async_trait]
impl<O: BrainOperation> Tool for BrainTool<O> {
    fn name(&self) -> &str {
        O::NAME
    }

    fn description(&self) -> String {
        O::DESCRIPTION.to_owned()
    }

    // JSON: the engine's tool contract carries the schema as JSON Schema.
    fn schema(&self) -> Value {
        O::schema()
    }

    // JSON: the model's tool input arrives as JSON and is checked against the
    // schema here.
    async fn run(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        harness_tools::validate(&O::schema(), &mut input)?;
        let input: O::Input = serde_json::from_value(input)
            .map_err(|error| ToolError::caused(INPUT_REFUSED, error))?;
        let target = O::target(&input);
        ctx.ask(PermissionRequest {
            permission: PERMISSION.to_owned(),
            patterns: vec![format!("{} {target}", O::NAME)],
            always: vec![format!("{} *", O::NAME)],
            title: O::TITLE.to_owned(),
            metadata: json!({ "tool": O::NAME, "target": target }),
        })
        .await?;
        let answer = tokio::select! {
            () = ctx.cancel.cancelled() => return Err(ToolError::Aborted),
            answer = O::call(self.service.clone(), input) => answer,
        };
        let output = answer.map_err(|error| ToolError::caused(failure_text(&error), error))?;
        Ok(ToolOutput::new(O::TITLE, output))
    }
}
