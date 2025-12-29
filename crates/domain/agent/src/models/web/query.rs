use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAgentsQuery {
    pub page: Option<i32>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub capability: Option<String>,
}

impl Default for ListAgentsQuery {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(20),
            offset: Some(0),
            search: None,
            status: None,
            capability: None,
        }
    }
}
