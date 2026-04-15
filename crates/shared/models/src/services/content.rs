use crate::{ContentConfigRaw, ContentSourceConfigRaw};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentConfig {
    #[serde(flatten)]
    pub raw: ContentConfigRaw,
    #[serde(default)]
    pub sources: HashMap<String, ContentSourceConfigRaw>,
}
