use chrono::Utc;

use crate::messages::{Command, ScrollDirection};
use crate::state::{ArtifactDisplay, FocusedPanel};
use systemprompt_identifiers::ArtifactId;

use super::super::TuiApp;

impl TuiApp {
    pub(crate) fn handle_tool_approve(&mut self, id: uuid::Uuid) -> Vec<Command> {
        if let Some(_tool_call) = self.state.tools.approve(id) {
            self.state.focus = FocusedPanel::Chat;
        }
        vec![Command::None]
    }

    pub(crate) fn handle_tool_reject(&mut self, id: uuid::Uuid) -> Vec<Command> {
        self.state.tools.reject(id);
        self.state.focus = FocusedPanel::Chat;
        vec![Command::None]
    }

    pub(crate) fn handle_tool_execution_complete(
        &mut self,
        id: uuid::Uuid,
        result: crate::messages::ToolExecutionResult,
    ) -> Vec<Command> {
        self.state
            .tools
            .complete_execution(id, result.success, Some(result.output));
        vec![Command::None]
    }

    pub(crate) fn handle_artifacts_select(&mut self, index: usize) -> Vec<Command> {
        self.state.artifacts.selected_index = index;
        vec![Command::None]
    }

    pub(crate) fn handle_artifacts_scroll(&mut self, direction: ScrollDirection) -> Vec<Command> {
        match direction {
            ScrollDirection::Up => self.state.artifacts.scroll_up(1),
            ScrollDirection::Down => self.state.artifacts.scroll_down(1),
            _ => {},
        }
        vec![Command::None]
    }

    pub(crate) fn handle_artifacts_select_next(&mut self) -> Vec<Command> {
        self.state.artifacts.select_next();
        vec![Command::None]
    }

    pub(crate) fn handle_artifacts_select_previous(&mut self) -> Vec<Command> {
        self.state.artifacts.select_previous();
        vec![Command::None]
    }

    pub(crate) fn handle_artifacts_loaded(
        &mut self,
        artifacts: Vec<systemprompt_models::a2a::Artifact>,
    ) -> Vec<Command> {
        self.state.artifacts.clear();
        for artifact in artifacts {
            self.state.artifacts.add_artifact(ArtifactDisplay {
                artifact_id: artifact.id.clone(),
                name: artifact.name.clone(),
                artifact_type: Some(artifact.metadata.artifact_type.clone()),
                task_id: artifact.metadata.task_id.clone(),
                context_id: artifact.metadata.context_id.clone(),
                created_at: chrono::DateTime::parse_from_rfc3339(&artifact.metadata.created_at)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
            });
        }
        self.state.artifacts.needs_initial_load = false;
        vec![Command::None]
    }

    pub(crate) fn handle_artifact_deleted(&mut self, artifact_id: &ArtifactId) -> Vec<Command> {
        self.state.artifacts.remove_artifact(artifact_id);
        vec![Command::None]
    }
}
