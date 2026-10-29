//! Stale loopback-secret handling: which remedy a rejection points the user
//! at, and first-sighting dedup of stale fingerprint pairs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::proxy::secret;
use crate::proxy::server::ProxyContext;

pub(super) fn first_stale_pair(ctx: &ProxyContext, presented_fp: &str, expected_fp: &str) -> bool {
    ctx.stats
        .stale_secret_pairs
        .lock()
        .map_or(true, |mut seen| {
            seen.insert((presented_fp.to_owned(), expected_fp.to_owned()))
        })
}

/// Which fix a stale-secret rejection points the user at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleSecretRemedy {
    RestartClaudeDesktop,
    ReapplyMachinePolicy,
    ReapplyProfile,
}

impl StaleSecretRemedy {
    pub(super) fn text(self) -> String {
        match self {
            Self::RestartClaudeDesktop => "the Claude Desktop machine policy already carries this \
                 bridge's credential; Claude Desktop is still using the value it read at launch, \
                 so quit and restart Claude Desktop"
                .to_owned(),
            Self::ReapplyMachinePolicy => format!(
                "the Claude Desktop machine policy still carries the credential Claude Desktop \
                 presented, which is not this bridge's; re-apply the profile and approve the \
                 administrator prompt (the machine policy needs it), then restart Claude \
                 Desktop. {}",
                secret::reapply_hint()
            ),
            Self::ReapplyProfile => secret::reapply_hint(),
        }
    }
}

#[must_use]
pub fn stale_secret_remedy(
    is_claude_desktop: bool,
    presented_fp: &str,
    bridge_desktop_fp: &str,
    policy_fp: Option<&str>,
) -> StaleSecretRemedy {
    match policy_fp {
        Some(policy) if is_claude_desktop && policy == bridge_desktop_fp => {
            StaleSecretRemedy::RestartClaudeDesktop
        },
        Some(policy) if is_claude_desktop && policy == presented_fp => {
            StaleSecretRemedy::ReapplyMachinePolicy
        },
        _ => StaleSecretRemedy::ReapplyProfile,
    }
}

pub(super) fn remediation_for(
    ctx: &ProxyContext,
    user_agent: &str,
    presented_fp: &str,
) -> StaleSecretRemedy {
    let is_claude_desktop = user_agent.contains("Claude/") && user_agent.contains("Electron/");
    if !is_claude_desktop {
        return StaleSecretRemedy::ReapplyProfile;
    }
    let ours = crate::ids::LoopbackSecret::new(ctx.secret.as_ref().as_str());
    let bridge_desktop_fp = secret::fingerprint(
        crate::proxy::scoped_token::host_token(
            &ours,
            systemprompt_models::bridge::host::HostKind::ClaudeDesktop,
        )
        .as_str(),
    );
    let policy_fp = (ctx.deps.desktop_policy_key_fp)();
    stale_secret_remedy(true, presented_fp, &bridge_desktop_fp, policy_fp.as_deref())
}
