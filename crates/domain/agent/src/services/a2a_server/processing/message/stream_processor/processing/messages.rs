use std::sync::Arc;

use systemprompt_models::{AiContentPart, AiMessage, MessageRole, RequestContext};

use crate::models::AgentRuntimeInfo;
use crate::services::SkillService;

pub(super) struct BuildAiMessagesParams<'a> {
    pub agent_runtime: &'a AgentRuntimeInfo,
    pub conversation_history: Vec<AiMessage>,
    pub user_text: String,
    pub user_parts: Vec<AiContentPart>,
    pub skill_service: &'a Arc<SkillService>,
    pub request_ctx: &'a RequestContext,
}

pub(super) async fn build_ai_messages(params: BuildAiMessagesParams<'_>) -> Vec<AiMessage> {
    let BuildAiMessagesParams {
        agent_runtime,
        conversation_history,
        user_text,
        user_parts,
        skill_service,
        request_ctx,
    } = params;
    let mut ai_messages = Vec::new();

    if !agent_runtime.skills.is_empty() {
        tracing::info!(
            skill_count = agent_runtime.skills.len(),
            skills = ?agent_runtime.skills,
            "Loading skills for agent"
        );

        let mut skills_prompt = String::from(
            "# Your Skills\n\nYou have the following skills that define your capabilities and \
             writing style:\n\n",
        );

        for skill_id in &agent_runtime.skills {
            let skill_id_typed = systemprompt_identifiers::SkillId::new(skill_id);
            match skill_service.load_skill(&skill_id_typed, request_ctx).await {
                Ok(skill_content) => {
                    tracing::info!(
                        skill_id = %skill_id,
                        content_len = skill_content.len(),
                        "Loaded skill"
                    );
                    skills_prompt.push_str(&format!(
                        "## {} Skill\n\n{}\n\n---\n\n",
                        skill_id, skill_content
                    ));
                },
                Err(e) => {
                    tracing::warn!(skill_id = %skill_id, error = %e, "Failed to load skill");
                },
            }
        }

        ai_messages.push(AiMessage {
            role: MessageRole::System,
            content: skills_prompt,
            parts: Vec::new(),
        });

        tracing::info!("Skills injected into agent context");
    }

    if let Some(system_prompt) = &agent_runtime.system_prompt {
        ai_messages.push(AiMessage {
            role: MessageRole::System,
            content: system_prompt.clone(),
            parts: Vec::new(),
        });
    }

    ai_messages.extend(conversation_history);

    ai_messages.push(AiMessage {
        role: MessageRole::User,
        content: user_text,
        parts: user_parts,
    });

    ai_messages
}
