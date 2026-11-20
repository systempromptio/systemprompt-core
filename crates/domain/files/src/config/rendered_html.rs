//! The `files.documents.renderedHtml` section: what the rendered view of an
//! HTML document may do inside its sandbox, at two tiers.
//!
//! The top level (`scripts`, `remoteOrigins`) applies to a document no agent
//! has written; `agentAuthored` applies to one that carries agent content
//! (the sticky `documents.agent_content` flag) and may only narrow the top
//! level: a wider `agentAuthored` is refused when the profile loads. Every
//! switch is off and every list empty by default.
//!
//! Each tier's `Content-Security-Policy` is built once, when the section is
//! loaded (see `rendered_csp`), and handed out by `RenderedHtmlConfig::csp`;
//! every tier's policy carries `form-action 'none'` and `base-uri 'none'`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::HeaderValue;
use http::header::InvalidHeaderValue;
use serde::{Deserialize, Serialize};

use super::remote_origin::RemoteOrigin;
use super::rendered_csp::LOCKED_CSP;

#[derive(Debug, thiserror::Error)]
pub enum RenderedHtmlError {
    #[error("remote origin {origin:?} is not a URL")]
    Unparsable {
        origin: String,
        #[source]
        source: url::ParseError,
    },
    #[error("remote origin {origin:?} {reason}")]
    NotAnOrigin {
        origin: String,
        reason: &'static str,
    },
    #[error("renderedHtml.agentAuthored.scripts is on while renderedHtml.scripts is off")]
    AgentScriptsWider,
    #[error(
        "renderedHtml.agentAuthored.remoteOrigins lists {origin} which renderedHtml.remoteOrigins \
         does not"
    )]
    AgentOriginWider { origin: RemoteOrigin },
    #[error("the rendered-HTML policy is not a header value")]
    Csp(#[from] InvalidHeaderValue),
}

/// What one tier of rendered pages may do: run its scripts, and load from
/// the listed origins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderedHtmlTier {
    #[serde(default)]
    pub scripts: bool,
    #[serde(default)]
    pub remote_origins: Vec<RemoteOrigin>,
}

impl RenderedHtmlTier {
    fn wider_part_than(&self, top: &Self) -> Option<RenderedHtmlError> {
        if self.scripts && !top.scripts {
            return Some(RenderedHtmlError::AgentScriptsWider);
        }
        self.remote_origins
            .iter()
            .find(|origin| !top.remote_origins.contains(origin))
            .map(|origin| RenderedHtmlError::AgentOriginWider {
                origin: origin.clone(),
            })
    }
}

/// The rendered view's policy: the top-level tier, and the tier for a
/// document that carries agent content, never wider than the top level,
/// each with its CSP built once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RenderedHtmlYaml", into = "RenderedHtmlYaml")]
pub struct RenderedHtmlConfig {
    top: RenderedHtmlTier,
    agent_authored: RenderedHtmlTier,
    top_csp: HeaderValue,
    agent_csp: HeaderValue,
}

impl Default for RenderedHtmlConfig {
    fn default() -> Self {
        Self {
            top: RenderedHtmlTier::default(),
            agent_authored: RenderedHtmlTier::default(),
            top_csp: HeaderValue::from_static(LOCKED_CSP),
            agent_csp: HeaderValue::from_static(LOCKED_CSP),
        }
    }
}

impl RenderedHtmlConfig {
    pub fn new(
        top: RenderedHtmlTier,
        agent_authored: RenderedHtmlTier,
    ) -> Result<Self, RenderedHtmlError> {
        if let Some(wider) = agent_authored.wider_part_than(&top) {
            return Err(wider);
        }
        Ok(Self {
            top_csp: top.csp()?,
            agent_csp: agent_authored.csp()?,
            top,
            agent_authored,
        })
    }

    #[must_use]
    pub const fn top(&self) -> &RenderedHtmlTier {
        &self.top
    }

    #[must_use]
    pub const fn agent_authored(&self) -> &RenderedHtmlTier {
        &self.agent_authored
    }

    #[must_use]
    pub const fn tier(&self, agent_content: bool) -> &RenderedHtmlTier {
        if agent_content {
            &self.agent_authored
        } else {
            &self.top
        }
    }

    #[must_use]
    pub fn csp(&self, agent_content: bool) -> HeaderValue {
        if agent_content {
            self.agent_csp.clone()
        } else {
            self.top_csp.clone()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenderedHtmlYaml {
    #[serde(default)]
    scripts: bool,
    #[serde(default)]
    remote_origins: Vec<RemoteOrigin>,
    #[serde(default)]
    agent_authored: RenderedHtmlTier,
}

impl TryFrom<RenderedHtmlYaml> for RenderedHtmlConfig {
    type Error = RenderedHtmlError;

    fn try_from(yaml: RenderedHtmlYaml) -> Result<Self, Self::Error> {
        Self::new(
            RenderedHtmlTier {
                scripts: yaml.scripts,
                remote_origins: yaml.remote_origins,
            },
            yaml.agent_authored,
        )
    }
}

impl From<RenderedHtmlConfig> for RenderedHtmlYaml {
    fn from(config: RenderedHtmlConfig) -> Self {
        Self {
            scripts: config.top.scripts,
            remote_origins: config.top.remote_origins,
            agent_authored: config.agent_authored,
        }
    }
}
