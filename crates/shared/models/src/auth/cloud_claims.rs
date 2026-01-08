use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAuthClaims {
    pub sub: String,
    pub exp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl CloudAuthClaims {
    pub fn subject(&self) -> &str {
        &self.sub
    }

    pub fn expires_at(&self) -> i64 {
        self.exp
    }

    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.exp < now
    }
}
