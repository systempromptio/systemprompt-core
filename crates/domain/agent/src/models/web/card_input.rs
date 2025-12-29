use crate::models::a2a::{AgentCapabilities, SecurityScheme, TransportProtocol};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct AgentCardInput {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub url: Option<String>,
    pub version: String,
    #[serde(default)]
    pub preferred_transport: Option<TransportProtocol>,
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    #[serde(default)]
    pub default_input_modes: Vec<String>,
    #[serde(default)]
    pub default_output_modes: Vec<String>,
    #[serde(default)]
    pub skills: Vec<crate::models::a2a::AgentSkill>,
    #[serde(default)]
    pub security_schemes: Option<HashMap<String, SecurityScheme>>,
    #[serde(default)]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
}

pub fn default_protocol_version() -> String {
    "0.3.0".to_string()
}
