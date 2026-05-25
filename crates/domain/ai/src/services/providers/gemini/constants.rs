use std::time::Duration;

pub(super) mod timeout {
    use super::Duration;

    pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
}

pub(super) mod tokens {
    pub(crate) const THINKING_BUDGET: u32 = 8192;
}

pub(super) mod defaults {
    pub(crate) const RELEVANCE_SCORE: f32 = 0.85;
    pub(crate) const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta";
}
