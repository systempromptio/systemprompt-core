//! The agent access policy an installation registers to let the Drive sync
//! save whole files: `replace_content` on the `drive_sync` channel alone,
//! the trash on the same channel, every other capability left to core's
//! default refusal.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_files::documents::agents::{
    AgentAccessPolicy, AgentActor, AgentCap, AgentCapability, AgentChannel, DocumentContext,
};

struct DriveSyncSaves;

#[async_trait]
impl AgentAccessPolicy for DriveSyncSaves {
    fn name(&self) -> &'static str {
        "drive-sync-saves"
    }

    async fn cap(&self, _doc: &DocumentContext, _agent: &AgentActor) -> AgentCap {
        AgentCap::Edit
    }

    async fn allows(
        &self,
        capability: AgentCapability,
        _doc: &DocumentContext,
        agent: &AgentActor,
    ) -> bool {
        matches!(
            capability,
            AgentCapability::ReplaceContent | AgentCapability::Trash
        ) && agent.channel == AgentChannel::DriveSync
    }
}

systemprompt_files::register_agent_access_policy!("drive-sync-saves", |_| Arc::new(DriveSyncSaves));
