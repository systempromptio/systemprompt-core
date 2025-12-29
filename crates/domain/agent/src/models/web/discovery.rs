use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentCounts {
    pub total: usize,
    pub active: usize,
    pub enabled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDiscoveryEntry {
    pub uuid: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub url: String,
    pub status: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDiscoveryResponse {
    pub agents: Vec<AgentDiscoveryEntry>,
    pub total: usize,
}
