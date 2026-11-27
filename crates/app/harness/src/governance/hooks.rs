//! Governance hooks: the instance calls they need, the hook token they carry,
//! and the bodies of the events.
//!
//! The hook client is provisioned lazily with the user's JWT and kept in memory
//! with the instance; one hook token per plugin is minted with
//! `client_credentials` and reused until five minutes before it lapses. A mint
//! refused with 401 means another client of the same user rotated the
//! secret: the client is provisioned once more and the mint tried again. Every
//! body carries the context the protocol defines: the session's v5 UUID, its
//! native id, its directory and the host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use systemprompt_engine::run::{ToolCall, ToolOutcome, UserPrompt};
use systemprompt_identifiers::engine::SessionId;
use serde_json::json;
use systemprompt_device::client::{ClientError, HookClient, HookGrant, SpClient, with_jwt};
use sp_protocol::ids::{PluginId, SkillId};
use sp_protocol::names::{native_session_id, skill_dir_name};
use sp_protocol::wire::hooks::{GovernAnswer, HookContext, HookEvent, ToolResponse, skill_ref};
use sp_protocol::{DeviceCredential, HOST_ID, HookToken, Jwt};
use uuid::Uuid;

use crate::governance::error::GovernanceError;
use crate::governance::instance::Instance;

/// The instance routes governance calls.
///
/// Hook-client provisioning, hook-token minting, event tracking and tool
/// verdicts. An applied instance holds it as `Arc<dyn HookApi>`, hence
/// `#[async_trait]`; [`SpClient`] is the production implementation and
/// [`crate::governance::Governance::install`] accepts any other.
#[async_trait::async_trait]
pub trait HookApi: Send + Sync {
    async fn provision_client(&self, jwt: &Jwt) -> Result<HookClient, ClientError>;

    async fn mint_token(
        &self,
        client: &HookClient,
        plugin: &PluginId,
    ) -> Result<HookGrant, ClientError>;

    async fn send_event(
        &self,
        token: &HookToken,
        plugin: &PluginId,
        event: &HookEvent,
        device: Option<&DeviceCredential>,
    ) -> Result<(), ClientError>;

    async fn ask_verdict(
        &self,
        token: &HookToken,
        plugin: &PluginId,
        event: &HookEvent,
    ) -> Result<GovernAnswer, ClientError>;
}

#[async_trait::async_trait]
impl HookApi for SpClient {
    async fn provision_client(&self, jwt: &Jwt) -> Result<HookClient, ClientError> {
        self.provision_hook_client(jwt).await
    }

    async fn mint_token(
        &self,
        client: &HookClient,
        plugin: &PluginId,
    ) -> Result<HookGrant, ClientError> {
        self.mint_hook_token(client, plugin).await
    }

    async fn send_event(
        &self,
        token: &HookToken,
        plugin: &PluginId,
        event: &HookEvent,
        device: Option<&DeviceCredential>,
    ) -> Result<(), ClientError> {
        self.track(token, plugin, event, device).await
    }

    async fn ask_verdict(
        &self,
        token: &HookToken,
        plugin: &PluginId,
        event: &HookEvent,
    ) -> Result<GovernAnswer, ClientError> {
        self.govern(token, plugin, event).await
    }
}

pub(crate) async fn hook_token(
    instance: &Instance,
    plugin: &PluginId,
) -> Result<HookToken, GovernanceError> {
    if let Some(token) = instance.cached_token(plugin, Utc::now()) {
        return Ok(token);
    }
    let _minting = instance.mint_gate.lock().await;
    if let Some(token) = instance.cached_token(plugin, Utc::now()) {
        return Ok(token);
    }
    let client = match instance.cached_client() {
        Some(client) => client,
        None => provision(instance).await?,
    };
    let grant = match instance.api.mint_token(&client, plugin).await {
        Ok(grant) => grant,
        Err(error) if error.is_unauthorized() => {
            tracing::debug!(
                plugin = %plugin.as_str(),
                "hook token refused; provisioning the hook client again"
            );
            let client = provision(instance).await?;
            instance
                .api
                .mint_token(&client, plugin)
                .await
                .map_err(GovernanceError::Hook)?
        },
        Err(error) => return Err(GovernanceError::Hook(error)),
    };
    let token = grant.token.clone();
    instance.keep_grant(plugin, grant);
    Ok(token)
}

async fn provision(instance: &Instance) -> Result<HookClient, GovernanceError> {
    let api = Arc::clone(&instance.api);
    let client = with_jwt(instance.credentials.as_ref(), |jwt: Jwt| {
        let api = Arc::clone(&api);
        async move { api.provision_client(&jwt).await }
    })
    .await
    .map_err(GovernanceError::Hook)?;
    instance.keep_client(client.clone());
    tracing::debug!("hook client provisioned");
    Ok(client)
}

fn context(native_session: &SessionId, cwd: &Path) -> HookContext {
    HookContext {
        session_id: native_session_id(native_session.as_str()),
        native_session_id: native_session.as_str().to_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
        native_host: HOST_ID.to_owned(),
    }
}

fn event_id() -> String {
    Uuid::new_v4().hyphenated().to_string()
}

pub(crate) fn session_start(native_session: &SessionId, cwd: &Path) -> HookEvent {
    HookEvent::SessionStart {
        context: context(native_session, cwd),
        event_id: event_id(),
    }
}

pub(crate) fn session_end(native_session: &SessionId, cwd: &Path) -> HookEvent {
    HookEvent::SessionEnd {
        context: context(native_session, cwd),
        event_id: event_id(),
    }
}

pub(crate) fn stop(native_session: &SessionId, cwd: &Path) -> HookEvent {
    HookEvent::Stop {
        context: context(native_session, cwd),
        event_id: event_id(),
    }
}

pub(crate) fn prompt(prompt: &UserPrompt) -> Option<HookEvent> {
    if prompt.text.trim().is_empty() {
        return None;
    }
    Some(HookEvent::UserPromptSubmit {
        context: context(&prompt.session_id, &prompt.directory),
        prompt: prompt.text.clone(),
        prompt_id: prompt.message_id.as_str().to_owned(),
    })
}

pub(crate) fn pre_tool(call: &ToolCall) -> HookEvent {
    HookEvent::PreToolUse {
        context: context(&call.session_id, &call.directory),
        tool_name: call.tool.clone(),
        tool_input: call.input.clone(),
        tool_use_id: call.call_id.as_str().to_owned(),
    }
}

pub(crate) fn post_tool(
    call: &ToolCall,
    outcome: &ToolOutcome,
    owners: &BTreeMap<SkillId, PluginId>,
) -> HookEvent {
    let (tool_input, skill) = call.skill.as_deref().map_or_else(
        || (call.input.clone(), None),
        |name| {
            let owner = owners
                .iter()
                .find(|(id, _)| skill_dir_name(id.as_str()) == name)
                .map(|(_, plugin)| plugin);
            (
                json!({ "name": name }),
                Some(skill_ref(owner.map(PluginId::as_str), HOST_ID, name)),
            )
        },
    );
    let tool_response = outcome.error.as_ref().map_or_else(
        || ToolResponse {
            title: Some(outcome.title.clone()),
            output: outcome.output.clone(),
            metadata: (!outcome.metadata.is_null()).then(|| outcome.metadata.clone()),
        },
        |error| ToolResponse {
            title: None,
            output: error.clone(),
            metadata: None,
        },
    );
    HookEvent::PostToolUse {
        context: context(&call.session_id, &call.directory),
        tool_name: call.tool.clone(),
        tool_input,
        tool_use_id: call.call_id.as_str().to_owned(),
        tool_response,
        skill_ref: skill,
    }
}
