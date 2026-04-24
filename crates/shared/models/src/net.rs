use std::time::Duration;

pub const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub const HTTP_DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub const HTTP_HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

pub const HTTP_AUTH_VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

pub const HTTP_SYNC_DEPLOY_TIMEOUT: Duration = Duration::from_secs(60);

pub const HTTP_STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub const HTTP_KEEPALIVE: Duration = Duration::from_secs(60);

pub const HTTP_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub const AGENT_MONITOR_TCP_TIMEOUT: Duration = Duration::from_secs(15);

pub const AGENT_READINESS_TCP_TIMEOUT: Duration = Duration::from_secs(2);

pub const IMAGE_GEN_LONG_POLL_TIMEOUT: Duration = Duration::from_secs(300);

pub const IMAGE_GEN_OPENAI_TIMEOUT: Duration = Duration::from_secs(120);
