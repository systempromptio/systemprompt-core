use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum JsonPatchOperation {
    Add { path: String, value: Value },
    Remove { path: String },
    Replace { path: String, value: Value },
    Move { from: String, path: String },
    Copy { from: String, path: String },
    Test { path: String, value: Value },
}

impl JsonPatchOperation {
    pub fn add(path: impl Into<String>, value: Value) -> Self {
        Self::Add {
            path: path.into(),
            value,
        }
    }

    pub fn remove(path: impl Into<String>) -> Self {
        Self::Remove { path: path.into() }
    }

    pub fn replace(path: impl Into<String>, value: Value) -> Self {
        Self::Replace {
            path: path.into(),
            value,
        }
    }

    pub fn move_op(from: impl Into<String>, path: impl Into<String>) -> Self {
        Self::Move {
            from: from.into(),
            path: path.into(),
        }
    }

    pub fn copy(from: impl Into<String>, path: impl Into<String>) -> Self {
        Self::Copy {
            from: from.into(),
            path: path.into(),
        }
    }

    pub fn test(path: impl Into<String>, value: Value) -> Self {
        Self::Test {
            path: path.into(),
            value,
        }
    }
}

#[derive(Debug)]
pub struct StateDeltaBuilder {
    operations: Vec<JsonPatchOperation>,
}

impl StateDeltaBuilder {
    pub const fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    pub fn add(mut self, path: &str, value: Value) -> Self {
        self.operations.push(JsonPatchOperation::add(path, value));
        self
    }

    pub fn replace(mut self, path: &str, value: Value) -> Self {
        self.operations
            .push(JsonPatchOperation::replace(path, value));
        self
    }

    pub fn remove(mut self, path: &str) -> Self {
        self.operations.push(JsonPatchOperation::remove(path));
        self
    }

    pub fn build(self) -> Vec<JsonPatchOperation> {
        self.operations
    }
}

impl Default for StateDeltaBuilder {
    fn default() -> Self {
        Self::new()
    }
}
