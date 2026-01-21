pub mod pkce {
    pub const CODE_CHALLENGE_MIN_LENGTH: usize = 43;
    pub const CODE_CHALLENGE_MAX_LENGTH: usize = 128;
}

pub mod token {
    pub const COOKIE_MAX_AGE_SECONDS: i64 = 3600;
    pub const SECONDS_PER_DAY: i64 = 86400;
    pub const REFRESH_TOKEN_EXPIRY_DAYS: i64 = 30;
    pub const ANONYMOUS_TOKEN_EXPIRY_SECONDS: i64 = 24 * 3600;
}

pub mod webauthn {
    pub const CHALLENGE_EXPIRY_SECONDS: u64 = 300;
    pub const CLEANUP_INTERVAL_SECONDS: u64 = 300;
}

pub mod validation {
    pub const MIN_SEQUENTIAL_RUN: usize = 6;
    pub const DIVERSITY_THRESHOLD: f64 = 0.5;
    pub const MIN_UNIQUE_CHARS: usize = 20;
}
