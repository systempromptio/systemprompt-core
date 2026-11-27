//! The closed set of host applications the bridge integrates with.
//!
//! [`HostKind`] is the one definition of host identity shared by the gateway
//! (which advertises the hosts it serves and attests their traffic) and the
//! bridge (which enrols, syncs and mints loopback credentials for every host
//! whose [`HostKind::config_owner`] is [`ConfigOwner::Bridge`]). Its
//! wire form is the kebab-case host id (`claude-code`, `codex-cli`, …), the
//! same string every bridge config file and loopback token scope already
//! carries. This module is the only place those ids are spelled.
//!
//! A host is one packaging of one product: Codex ships as a desktop app
//! (`codex`) and a CLI (`codex-cli`), `OpenCode` likewise (`opencode`,
//! `opencode-cli`). Both packagings of a product read one configuration tree,
//! so "same product, different packaging" is the typed relation
//! [`HostKind::product`] / [`HostKind::kind`], never a prefix test on the id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostKind {
    ClaudeCode,
    ClaudeDesktop,
    Codex,
    CodexCli,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "opencode-cli")]
    OpenCodeCli,
    Hermes,
    Harness,
}

/// How a host is packaged: a desktop application with a bundle or installer
/// the bridge can locate and launch, or a command-line tool found on `PATH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostAppKind {
    DesktopApp,
    CliTool,
}

/// Who writes a host's configuration and holds its loopback credential.
///
/// The bridge enrols, syncs and mints a loopback credential for a
/// [`ConfigOwner::Bridge`] host. A [`ConfigOwner::Host`] host reads and
/// verifies the signed manifest itself: the bridge writes it no profile,
/// lists it nowhere and issues it no credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigOwner {
    Bridge,
    Host,
}

/// The product behind one or more [`HostKind`] packagings. Every packaging of
/// a product reads the same configuration tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostProduct {
    Claude,
    Codex,
    OpenCode,
    Hermes,
    Harness,
}

impl HostKind {
    pub const ALL: [Self; 8] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Codex,
        Self::CodexCli,
        Self::OpenCode,
        Self::OpenCodeCli,
        Self::Hermes,
        Self::Harness,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Codex => "codex",
            Self::CodexCli => "codex-cli",
            Self::OpenCode => "opencode",
            Self::OpenCodeCli => "opencode-cli",
            Self::Hermes => "hermes",
            Self::Harness => "harness",
        }
    }

    #[must_use]
    pub const fn kind(self) -> HostAppKind {
        match self {
            Self::ClaudeDesktop | Self::Codex | Self::OpenCode | Self::Harness => {
                HostAppKind::DesktopApp
            },
            Self::ClaudeCode | Self::CodexCli | Self::OpenCodeCli | Self::Hermes => {
                HostAppKind::CliTool
            },
        }
    }

    #[must_use]
    pub const fn product(self) -> HostProduct {
        match self {
            Self::ClaudeCode | Self::ClaudeDesktop => HostProduct::Claude,
            Self::Codex | Self::CodexCli => HostProduct::Codex,
            Self::OpenCode | Self::OpenCodeCli => HostProduct::OpenCode,
            Self::Hermes => HostProduct::Hermes,
            Self::Harness => HostProduct::Harness,
        }
    }

    #[must_use]
    pub const fn config_owner(self) -> ConfigOwner {
        match self {
            Self::ClaudeCode
            | Self::ClaudeDesktop
            | Self::Codex
            | Self::CodexCli
            | Self::OpenCode
            | Self::OpenCodeCli
            | Self::Hermes => ConfigOwner::Bridge,
            Self::Harness => ConfigOwner::Host,
        }
    }

    // Why: a bridge released before a host joined this vocabulary parses every
    // host id in the manifest strictly, so it fails a manifest naming that
    // host. `None` is a host every supported bridge already parses.
    #[must_use]
    pub const fn min_bridge_version(self) -> Option<semver::Version> {
        match self {
            Self::ClaudeCode
            | Self::ClaudeDesktop
            | Self::Codex
            | Self::CodexCli
            | Self::OpenCode
            | Self::OpenCodeCli
            | Self::Hermes => None,
            Self::Harness => Some(semver::Version::new(0, 88, 0)),
        }
    }

    // Why: Claude Code and Claude Desktop are one product but keep separate
    // configuration (managed settings file versus policy hive), so only the
    // Codex and OpenCode pairs share a written config tree.
    #[must_use]
    pub const fn shares_config_with(self) -> Option<Self> {
        match self {
            Self::Codex => Some(Self::CodexCli),
            Self::CodexCli => Some(Self::Codex),
            Self::OpenCode => Some(Self::OpenCodeCli),
            Self::OpenCodeCli => Some(Self::OpenCode),
            Self::ClaudeCode | Self::ClaudeDesktop | Self::Hermes | Self::Harness => None,
        }
    }

    pub fn parse_list<S: AsRef<str>>(hosts: &[S]) -> Result<Vec<Self>, UnknownHostKind> {
        hosts.iter().map(|host| host.as_ref().parse()).collect()
    }

    pub fn validate_targets<S: AsRef<str>>(hosts: &[S]) -> Result<(), UnknownHostKind> {
        hosts
            .iter()
            .map(S::as_ref)
            .filter(|host| *host != COWORK_TARGET)
            .try_for_each(|host| host.parse::<Self>().map(drop))
    }
}

/// The hosts an instance serves, and the opt-in subset that reaches only the
/// users who turn it on.
///
/// A served host whose [`HostKind::config_owner`] is [`ConfigOwner::Host`]
/// also reaches only the users who turn it on, whether or not `opt_in` names
/// it: the "served to every user by default" rule is about the hosts the
/// bridge configures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstanceHosts {
    pub enabled: Vec<HostKind>,
    pub opt_in: BTreeSet<HostKind>,
}

impl InstanceHosts {
    #[must_use]
    pub fn is_opt_in(&self, host: HostKind) -> bool {
        self.opt_in.contains(&host)
    }

    pub fn by_default(&self) -> impl Iterator<Item = HostKind> + '_ {
        self.enabled
            .iter()
            .copied()
            .filter(|host| !self.is_opt_in(*host))
    }
}

// Why: Cowork is not a HostKind; it receives the Claude Desktop plugin bundle,
// so a `hosts:` list may name it beside the host ids.
pub const COWORK_TARGET: &str = "cowork";

impl HostProduct {
    #[must_use]
    pub fn targeted_by<S: AsRef<str>>(self, hosts: &[S]) -> bool {
        hosts.iter().any(|host| {
            host.as_ref()
                .parse::<HostKind>()
                .is_ok_and(|kind| kind.product() == self)
        })
    }

    pub fn hosts(self) -> impl Iterator<Item = HostKind> {
        HostKind::ALL
            .into_iter()
            .filter(move |host| host.product() == self)
    }
}

impl fmt::Display for HostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for HostAppKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DesktopApp => "desktop-app",
            Self::CliTool => "cli-tool",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown host id `{0}`")]
pub struct UnknownHostKind(pub String);

impl FromStr for HostKind {
    type Err = UnknownHostKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == s)
            .ok_or_else(|| UnknownHostKind(s.to_owned()))
    }
}
