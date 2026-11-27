//! The Drive's engine half: the agent tools, offered only while the instance
//! enables the Drive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use harness_engine::extension::{EngineExtension, ExtensionMetadata, ExtensionTool};

use super::DriveExtension;
use super::handle::DriveHandle;
use super::tools::{PERMISSION, drive_tools};

/// What the Drive adds to sessions: search, list, read, propose an edit,
/// create and comment, each asking the permission gate itself under
/// [`PERMISSION`].
#[derive(Debug, Clone)]
pub struct DriveEngine {
    handle: DriveHandle,
}

impl DriveEngine {
    pub const fn new(handle: DriveHandle) -> Self {
        Self { handle }
    }
}

impl EngineExtension for DriveEngine {
    fn metadata(&self) -> ExtensionMetadata {
        DriveExtension::describe()
    }

    fn tools(&self) -> Vec<ExtensionTool> {
        drive_tools(&self.handle)
            .into_iter()
            .map(|tool| ExtensionTool::asking_itself(tool, PERMISSION))
            .collect()
    }
}
