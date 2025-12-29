use crate::services::skills::{SkillMetadata, SkillService};
use anyhow::Result;
use std::sync::Arc;
use systemprompt_models::execution::context::RequestContext;

#[derive(Debug)]
pub struct SkillInjector {
    skill_service: Arc<SkillService>,
}

impl SkillInjector {
    pub const fn new(skill_service: Arc<SkillService>) -> Self {
        Self { skill_service }
    }

    pub async fn inject_for_tool(
        &self,
        skill_id: Option<&str>,
        base_prompt: String,
        ctx: &RequestContext,
    ) -> Result<String> {
        if let Some(sid) = skill_id {
            match self.skill_service.load_skill(sid, ctx).await {
                Ok(skill_content) => Ok(format!(
                    "{}\n\n## Writing Guidance\n\nFollow this methodology and style:\n\n{}",
                    base_prompt, skill_content
                )),
                Err(e) => {
                    tracing::warn!(skill_id = %sid, error = %e, "Failed to load skill");
                    Ok(base_prompt)
                },
            }
        } else {
            Ok(base_prompt)
        }
    }

    pub async fn get_metadata(&self, skill_id: &str) -> Result<SkillMetadata> {
        self.skill_service.load_skill_metadata(skill_id).await
    }

    pub async fn inject_with_metadata(
        &self,
        skill_id: &str,
        base_prompt: String,
        ctx: &RequestContext,
    ) -> Result<(String, SkillMetadata)> {
        let skill_content = self.skill_service.load_skill(skill_id, ctx).await?;

        let metadata = self.skill_service.load_skill_metadata(skill_id).await?;

        let enhanced_prompt = format!(
            "{}\n\n## Writing Guidance\n\nFollow this methodology and style:\n\n{}",
            base_prompt, skill_content
        );

        Ok((enhanced_prompt, metadata))
    }
}
