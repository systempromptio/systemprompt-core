//! [`FailureStage`]: the pipeline step a `notification_delivery_failures` row
//! records, stored as its `stage` column text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const DELIVER_PREFIX: &str = "deliver:";
const DELIVER_RETRY_PREFIX: &str = "deliver_retry:";

/// A stored stage that no pipeline step writes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("'{0}' is not a notification failure stage")]
pub struct FailureStageError(String);

/// Where in the pipeline a failure happened.
///
/// `Deliver` is a channel that failed permanently and is never asked again;
/// `DeliverRetry` is one transient failure of a delivery pass; the bare
/// `DeliverAbandoned` closes a notification whose retries ran out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum FailureStage {
    Decode,
    Process,
    Route,
    Rules,
    Audience,
    Visibility,
    Deliver { surface: String },
    DeliverRetry { surface: String },
    DeliverAbandoned,
}

impl fmt::Display for FailureStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode => f.write_str("decode"),
            Self::Process => f.write_str("process"),
            Self::Route => f.write_str("route"),
            Self::Rules => f.write_str("rules"),
            Self::Audience => f.write_str("audience"),
            Self::Visibility => f.write_str("visibility"),
            Self::Deliver { surface } => write!(f, "{DELIVER_PREFIX}{surface}"),
            Self::DeliverRetry { surface } => write!(f, "{DELIVER_RETRY_PREFIX}{surface}"),
            Self::DeliverAbandoned => f.write_str("deliver"),
        }
    }
}

impl FromStr for FailureStage {
    type Err = FailureStageError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let unknown = || FailureStageError(text.to_owned());
        if let Some(surface) = text.strip_prefix(DELIVER_RETRY_PREFIX) {
            if surface.is_empty() {
                return Err(unknown());
            }
            return Ok(Self::DeliverRetry {
                surface: surface.to_owned(),
            });
        }
        if let Some(surface) = text.strip_prefix(DELIVER_PREFIX) {
            if surface.is_empty() {
                return Err(unknown());
            }
            return Ok(Self::Deliver {
                surface: surface.to_owned(),
            });
        }
        match text {
            "decode" => Ok(Self::Decode),
            "process" => Ok(Self::Process),
            "route" => Ok(Self::Route),
            "rules" => Ok(Self::Rules),
            "audience" => Ok(Self::Audience),
            "visibility" => Ok(Self::Visibility),
            "deliver" => Ok(Self::DeliverAbandoned),
            _ => Err(unknown()),
        }
    }
}

impl TryFrom<String> for FailureStage {
    type Error = FailureStageError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<FailureStage> for String {
    fn from(stage: FailureStage) -> Self {
        stage.to_string()
    }
}
