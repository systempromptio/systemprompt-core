//! The Drive as agent tools (search, list, read, propose an edit, create,
//! comment): each asks the gate under `drive` and answers
//! `crate::members::drive::agent`'s text. `Tool` is the engine's `dyn`
//! contract, hence `#[async_trait]` on its impl.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod read;
mod write;

use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use systemprompt_engine::tools::{PermissionRequest, Tool, ToolContext, ToolError, ToolOutput};

use self::read::{List, Read, Search};
use self::write::{Comment, Create, Propose};
use super::handle::DriveHandle;
use crate::members::drive::agent;
use systemprompt_drive::client::DriveClient;
use systemprompt_drive::error::DriveError;

pub const PERMISSION: &str = "drive";
pub const TOOL_NAMES: [&str; 6] = [
    Search::NAME,
    List::NAME,
    Read::NAME,
    Propose::NAME,
    Create::NAME,
    Comment::NAME,
];
const INPUT_REFUSED: &str =
    "An id in the input is not a Drive id; take ids from a listing or a read.";
const UNAVAILABLE: &str = "No instance is signed in, so the Drive cannot be reached.";
const SCOPES: [&str; 3] = ["mine", "shared", "recent"];

pub(crate) fn drive_tools(handle: &DriveHandle) -> Vec<Arc<dyn Tool>> {
    vec![
        DriveTool::<Search>::shared(handle),
        DriveTool::<List>::shared(handle),
        DriveTool::<Read>::shared(handle),
        DriveTool::<Propose>::shared(handle),
        DriveTool::<Create>::shared(handle),
        DriveTool::<Comment>::shared(handle),
    ]
}

trait DriveOperation: Send + Sync + 'static {
    type Input: DeserializeOwned + Send;
    const NAME: &'static str;
    const TITLE: &'static str;
    const DESCRIPTION: &'static str;

    // JSON: a tool's input schema is JSON Schema, handed to the model as it is.
    fn schema() -> Value;

    fn target(_input: &Self::Input) -> String {
        "*".to_owned()
    }

    fn call(
        client: DriveClient,
        input: Self::Input,
    ) -> impl Future<Output = Result<String, DriveError>> + Send;
}

struct DriveTool<O> {
    handle: DriveHandle,
    operation: PhantomData<fn() -> O>,
}

impl<O: DriveOperation> DriveTool<O> {
    fn shared(handle: &DriveHandle) -> Arc<dyn Tool> {
        Arc::new(Self {
            handle: handle.clone(),
            operation: PhantomData,
        })
    }
}

#[async_trait]
impl<O: DriveOperation> Tool for DriveTool<O> {
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
        systemprompt_engine::tools::validate(&O::schema(), &mut input)?;
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
        let client = self
            .handle
            .client()
            .map_err(|error| ToolError::caused(UNAVAILABLE, error))?;
        let answer = tokio::select! {
            () = ctx.cancel.cancelled() => return Err(ToolError::Aborted),
            answer = O::call(client, input) => answer,
        };
        let output =
            answer.map_err(|error| ToolError::caused(agent::failure_text(&error), error))?;
        Ok(ToolOutput::new(O::TITLE, output))
    }
}
