use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId};

#[derive(Debug)]
pub struct ArtifactsState {
    pub artifacts: Vec<ArtifactDisplay>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub filter_type: Option<String>,
    pub needs_initial_load: bool,
}

#[derive(Debug, Clone)]
pub struct ArtifactDisplay {
    pub artifact_id: ArtifactId,
    pub name: Option<String>,
    pub artifact_type: Option<String>,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub created_at: DateTime<Utc>,
}

impl ArtifactsState {
    pub const fn new() -> Self {
        Self {
            artifacts: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            filter_type: None,
            needs_initial_load: true,
        }
    }

    pub fn add_artifact(&mut self, artifact: ArtifactDisplay) {
        if let Some(existing) = self
            .artifacts
            .iter_mut()
            .find(|a| a.artifact_id.as_ref() == artifact.artifact_id.as_ref())
        {
            *existing = artifact;
        } else {
            let pos = self
                .artifacts
                .iter()
                .position(|a| a.created_at < artifact.created_at)
                .unwrap_or(self.artifacts.len());
            self.artifacts.insert(pos, artifact);
        }
    }

    pub fn select_next(&mut self) {
        let filtered = self.filtered_artifacts();
        if !filtered.is_empty() {
            self.selected_index = (self.selected_index + 1).min(filtered.len() - 1);
            self.ensure_visible();
        }
    }

    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        let viewport_height = 20;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected_index - viewport_height + 1;
        }
    }

    pub fn filtered_artifacts(&self) -> Vec<&ArtifactDisplay> {
        self.filter_type.as_ref().map_or_else(
            || self.artifacts.iter().collect(),
            |filter| {
                self.artifacts
                    .iter()
                    .filter(|a| a.artifact_type.as_deref() == Some(filter.as_str()))
                    .collect()
            },
        )
    }

    pub fn selected_artifact(&self) -> Option<&ArtifactDisplay> {
        let filtered = self.filtered_artifacts();
        filtered.get(self.selected_index).copied()
    }

    pub fn set_filter(&mut self, filter: Option<String>) {
        self.filter_type = filter;
        self.selected_index = 0;
    }

    pub fn clear(&mut self) {
        self.artifacts.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn load_artifacts(&mut self, artifacts: Vec<systemprompt_models::a2a::Artifact>) {
        for artifact in artifacts {
            self.add_artifact(ArtifactDisplay {
                artifact_id: artifact.id.clone(),
                name: artifact.name.clone(),
                artifact_type: Some(artifact.metadata.artifact_type.clone()),
                task_id: artifact.metadata.task_id.clone(),
                context_id: artifact.metadata.context_id.clone(),
                created_at: DateTime::parse_from_rfc3339(&artifact.metadata.created_at)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
            });
        }
        self.artifacts
            .sort_by(|a, b| b.created_at.cmp(&a.created_at));
        self.needs_initial_load = false;
    }

    pub fn artifact_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self
            .artifacts
            .iter()
            .filter_map(|a| a.artifact_type.clone())
            .collect();
        types.sort();
        types.dedup();
        types
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max_offset = self.filtered_artifacts().len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + amount).min(max_offset);
    }

    pub fn selected_artifact_id(&self) -> Option<ArtifactId> {
        self.selected_artifact().map(|a| a.artifact_id.clone())
    }

    pub fn delete_selected(&mut self) -> Option<ArtifactId> {
        if self.artifacts.is_empty() {
            return None;
        }

        let artifact_id = self.selected_artifact_id()?;

        self.artifacts
            .retain(|a| a.artifact_id.as_ref() != artifact_id.as_ref());

        if self.selected_index >= self.artifacts.len() && !self.artifacts.is_empty() {
            self.selected_index = self.artifacts.len() - 1;
        }

        Some(artifact_id)
    }

    pub fn remove_artifact(&mut self, artifact_id: &ArtifactId) {
        self.artifacts
            .retain(|a| a.artifact_id.as_ref() != artifact_id.as_ref());

        if self.selected_index >= self.artifacts.len() && !self.artifacts.is_empty() {
            self.selected_index = self.artifacts.len() - 1;
        }
    }
}

impl Default for ArtifactsState {
    fn default() -> Self {
        Self::new()
    }
}
