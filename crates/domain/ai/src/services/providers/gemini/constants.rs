use std::time::Duration;

pub(super) mod timeout {
    use super::Duration;

    pub(in super::super) const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
}

pub(super) mod tokens {
    pub(in super::super) const THINKING_BUDGET: u32 = 8192;
}

pub(super) mod defaults {
    pub(in super::super) const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta";
}
