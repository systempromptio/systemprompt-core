//! Running one step's tool calls: concurrently, results kept in the order the
//! model issued them. Each call is looked up, its input repaired and validated,
//! its permission settled through the gate (rules, then the user, then the
//! Governor), run, and recorded on its part.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use futures_util::future::join_all;
use harness_prompt::{notices, tool_names};
use harness_store::{JsonMap, MessageId, ToolState, now_ms};
use harness_tasks::CancellationToken;
use harness_tools::{PermissionGate, PermissionRequest, ToolContext};
use serde_json::Value;

use super::gate::Gate;
use super::hooks::Observer;
use super::outcome::{self, Finished};
use super::progress::{CallRecord, PartProgress};
use super::registry::ToolBox;
use crate::governor::ToolCall;
use crate::session::run::StepContext;
use crate::session::stream::PendingCall;

const MALFORMED_EXCERPT: usize = 500;

pub(super) struct Invocation<'a> {
    pub(super) ctx: &'a StepContext,
    toolbox: &'a ToolBox,
    message_id: &'a MessageId,
    call: &'a PendingCall,
    pub(super) started: i64,
}

pub(crate) async fn run_calls(
    ctx: &StepContext,
    toolbox: &ToolBox,
    message_id: &MessageId,
    calls: &[PendingCall],
) -> bool {
    let runs = calls
        .iter()
        .map(|call| run_call(ctx, toolbox, message_id, call));
    join_all(runs).await.into_iter().any(|stop| stop)
}

async fn run_call(
    ctx: &StepContext,
    toolbox: &ToolBox,
    message_id: &MessageId,
    call: &PendingCall,
) -> bool {
    let started = now_ms();
    let record = CallRecord::new(ctx.session.id.clone(), message_id.clone(), call, started);
    let input = call.input.clone();
    let running = ToolState::Running {
        input,
        title: None,
        metadata: JsonMap::new(),
        started,
    };
    record.write(&ctx.inner.db, running).await;
    let store = ctx.inner.store.clone();
    let progress = Arc::new(PartProgress::new(
        store,
        record.clone(),
        call.input.clone(),
        started,
    ));
    let invocation = Invocation {
        ctx,
        toolbox,
        message_id,
        call,
        started,
    };
    let finished = invocation.invoke(Arc::clone(&progress)).await;
    progress.finish();
    record.write(&ctx.inner.db, finished.state).await;
    finished.stop
}

impl Invocation<'_> {
    async fn invoke(&self, progress: Arc<PartProgress>) -> Finished {
        let name = self.call.name.as_str();
        let Some(tool) = self.toolbox.get(name) else {
            let error = notices::unknown_tool(name, &self.toolbox.names());
            return Finished::failed(self.call.input.clone(), error, self.started);
        };
        if let Some(raw) = &self.call.malformed {
            let excerpt: String = raw.chars().take(MALFORMED_EXCERPT).collect();
            let reason = format!("the arguments are not valid JSON: {excerpt}");
            let error = notices::invalid_tool_input(name, &reason);
            return Finished::failed(self.call.input.clone(), error, self.started);
        }
        let mut input = self.call.input.clone();
        if let Err(error) = harness_tools::validate(&tool.schema(), &mut input) {
            let error = notices::invalid_tool_input(name, &error.to_string());
            return Finished::failed(input, error, self.started);
        }
        let governed = self.governed(&input);
        let cancel = self.ctx.cancel.child_token();
        let gate = Arc::new(Gate::new(
            Arc::clone(&self.ctx.inner),
            self.ctx.agent.clone(),
            Arc::clone(&self.ctx.config),
            governed.clone(),
            cancel.clone(),
        ));
        if !self.toolbox.asks_itself(name)
            && let Err(error) = gate.ask(self.engine_request(&input)).await
        {
            return outcome::failure(self, &gate, &governed, input, error).await;
        }
        let tool_ctx = self.tool_context(Arc::clone(&gate), cancel, progress);
        match tool.run(input.clone(), &tool_ctx).await {
            Ok(output) => outcome::completed(self, &gate, &governed, input, output).await,
            Err(error) => outcome::failure(self, &gate, &governed, input, error).await,
        }
    }

    // JSON: the validated tool input, carried to the Governor as the model wrote
    // it.
    fn governed(&self, input: &Value) -> ToolCall {
        let name = self.call.name.as_str();
        let skill = if name == tool_names::SKILL {
            input.get("name").and_then(Value::as_str).map(str::to_owned)
        } else {
            None
        };
        ToolCall {
            session_id: self.ctx.session.id.clone(),
            call_id: self.call.id.clone(),
            tool: name.to_owned(),
            input: input.clone(),
            directory: self.ctx.session.directory.clone(),
            skill,
        }
    }

    // JSON: the validated tool input, shown with the permission request.
    fn engine_request(&self, input: &Value) -> PermissionRequest {
        let name = self.call.name.as_str();
        PermissionRequest {
            permission: self.toolbox.permission(name).to_owned(),
            patterns: vec!["*".to_owned()],
            always: vec!["*".to_owned()],
            title: format!("Use {name}"),
            metadata: input.clone(),
        }
    }

    fn tool_context(
        &self,
        gate: Arc<Gate>,
        cancel: CancellationToken,
        progress: Arc<PartProgress>,
    ) -> ToolContext {
        let ctx = self.ctx;
        let worktree = ctx.config.worktree.root.clone();
        let observer = Observer::new(
            ctx.dir.workspace.clone(),
            Arc::clone(&ctx.dir.lsp),
            worktree.clone(),
        );
        ToolContext {
            session_id: ctx.session.id.clone(),
            message_id: self.message_id.clone(),
            call_id: self.call.id.clone(),
            agent: ctx.agent.name.clone(),
            cwd: ctx.session.directory.clone(),
            worktree,
            data_dir: ctx.inner.paths.data.join("tool-output"),
            cancel,
            gate,
            observer: Some(Arc::new(observer)),
            progress: Some(progress),
        }
    }
}
