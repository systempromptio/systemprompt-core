//! The agent access policy an installation contributes to let the Drive sync
//! save whole files: `replace_content` on the `drive_sync` channel alone,
//! the trash on the same channel, every other capability left to core's
//! default refusal. [`DriveSyncPolicyMember`] contributes it to the
//! single-valued `AgentAccessSlot`; the harness composes it after the
//! reference members.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, FactoryError, Resolved,
};
use systemprompt_files::contract::AgentAccessSlot;
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

pub(crate) struct DriveSyncPolicyMember;

impl Extension for DriveSyncPolicyMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "drive-sync-saves",
            name: "Drive sync saves",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.slot::<AgentAccessSlot>().policy(
            |_: &Resolved<'_>| -> Result<Arc<dyn AgentAccessPolicy>, FactoryError> {
                Ok(Arc::new(DriveSyncSaves))
            },
        );
        Ok(())
    }
}
