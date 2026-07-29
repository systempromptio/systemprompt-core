//! The built-in secret-pattern table, split from the scanner so each stays
//! under the file-size ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// One built-in plaintext-credential pattern.
///
/// `id` is the persistent referent written into
/// `governance_decisions.evaluated_rules.pattern_id` and used by dashboards /
/// SQL aggregations — it must never change. `name` is the human-readable label
/// rendered in deny messages. `expr` is an anchored regular expression seeded
/// from the gitleaks (MIT) ruleset: a bare vendor prefix in prose passes, a
/// full-length credential denies.
#[derive(Debug, Clone, Copy)]
pub struct SecretPattern {
    pub id: &'static str,
    pub name: &'static str,
    pub expr: &'static str,
}

pub const SECRET_PATTERNS: &[SecretPattern] = &[
    SecretPattern {
        id: "aws-access-key",
        name: "AWS Access Key",
        expr: r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
    },
    SecretPattern {
        id: "aws-secret-key",
        name: "AWS Secret Key",
        expr: r"(?i)aws_secret_access_key",
    },
    SecretPattern {
        id: "github-token-classic",
        name: "GitHub Token (classic)",
        expr: r"\bghp_[A-Za-z0-9]{36,}",
    },
    SecretPattern {
        id: "github-token-fine-grained",
        name: "GitHub Token (fine-grained)",
        expr: r"\bgithub_pat_[A-Za-z0-9_]{22,}",
    },
    SecretPattern {
        id: "github-oauth",
        name: "GitHub OAuth",
        expr: r"\bgho_[A-Za-z0-9]{36,}",
    },
    SecretPattern {
        id: "github-app-user-to-server",
        name: "GitHub App User-to-Server",
        expr: r"\bghu_[A-Za-z0-9]{36,}",
    },
    SecretPattern {
        id: "github-app-server-to-server",
        name: "GitHub App Server-to-Server",
        expr: r"\bghs_[A-Za-z0-9]{36,}",
    },
    SecretPattern {
        id: "github-app-refresh",
        name: "GitHub App Refresh",
        expr: r"\bghr_[A-Za-z0-9]{36,}",
    },
    SecretPattern {
        id: "gitlab-token",
        name: "GitLab Token",
        expr: r"\bglpat-[A-Za-z0-9_\-]{20,}",
    },
    SecretPattern {
        id: "slack-bot-token",
        name: "Slack Bot Token",
        expr: r"\bxoxb-[0-9A-Za-z\-]{10,}",
    },
    SecretPattern {
        id: "slack-user-token",
        name: "Slack User Token",
        expr: r"\bxoxp-[0-9A-Za-z\-]{10,}",
    },
    SecretPattern {
        id: "slack-webhook",
        name: "Slack Webhook",
        expr: r"hooks\.slack\.com/services/T[0-9A-Za-z_]+/B[0-9A-Za-z_]+/[0-9A-Za-z_]+",
    },
    SecretPattern {
        id: "stripe-secret-key",
        name: "Stripe Secret Key",
        expr: r"\bsk_live_[A-Za-z0-9]{16,}",
    },
    SecretPattern {
        id: "stripe-restricted-key",
        name: "Stripe Restricted Key",
        expr: r"\brk_live_[A-Za-z0-9]{16,}",
    },
    SecretPattern {
        id: "google-api-key",
        name: "Google API Key",
        expr: r"\bAIza[0-9A-Za-z_\-]{35}",
    },
    SecretPattern {
        id: "anthropic-api-key",
        name: "Anthropic API Key",
        expr: r"\bsk-ant-[A-Za-z0-9_\-]{20,}",
    },
    SecretPattern {
        id: "openai-api-key",
        name: "OpenAI API Key",
        expr: r"\bsk-proj-[A-Za-z0-9_\-]{20,}",
    },
    SecretPattern {
        id: "twilio-auth-token",
        name: "Twilio Auth Token",
        expr: r"(?i)twilio_auth_token",
    },
    SecretPattern {
        id: "sendgrid-api-key",
        name: "SendGrid API Key",
        expr: r"\bSG\.[A-Za-z0-9_\-]{16,}\.[A-Za-z0-9_\-]{16,}",
    },
    SecretPattern {
        id: "mailgun-api-key",
        name: "Mailgun API Key",
        expr: r"\bkey-[a-f0-9]{32}\b",
    },
    SecretPattern {
        id: "heroku-api-key",
        name: "Heroku API Key",
        expr: r"(?i)heroku_api_key",
    },
    SecretPattern {
        id: "pem-private-key-rsa",
        name: "RSA Private Key",
        expr: r"-----BEGIN RSA PRIVATE KEY-----",
    },
    SecretPattern {
        id: "pem-private-key-ec",
        name: "EC Private Key",
        expr: r"-----BEGIN EC PRIVATE KEY-----",
    },
    SecretPattern {
        id: "pem-private-key",
        name: "PEM Private Key",
        expr: r"-----BEGIN PRIVATE KEY-----",
    },
    SecretPattern {
        id: "generic-password-field",
        name: "Generic password field",
        expr: r#"(?i)\bpassword=[^\s&"']+"#,
    },
    SecretPattern {
        id: "generic-secret-field",
        name: "Generic secret field",
        expr: r#"(?i)\bsecret=[^\s&"']+"#,
    },
    SecretPattern {
        id: "bearer-token-jwt",
        name: "Bearer token (JWT)",
        expr: r"Bearer eyJ[A-Za-z0-9_\-]+",
    },
    SecretPattern {
        id: "jwt-raw",
        name: "JWT token (raw)",
        expr: r"\beyJhbGciOi[A-Za-z0-9_\-]+",
    },
    SecretPattern {
        id: "postgres-url-with-password",
        name: "Postgres URL with password",
        expr: r"\bpostgres(?:ql)?://[^\s:@/]+:[^\s@/]+@",
    },
    SecretPattern {
        id: "mysql-url-with-password",
        name: "MySQL URL with password",
        expr: r"\bmysql://[^\s:@/]+:[^\s@/]+@",
    },
    SecretPattern {
        id: "mongodb-srv-url",
        name: "MongoDB connection string",
        expr: r"\bmongodb\+srv://[^\s:@/]+:[^\s@/]+@",
    },
    SecretPattern {
        id: "redis-url-with-auth",
        name: "Redis URL with auth",
        expr: r"\bredis://[^\s:@/]+:[^\s@/]+@",
    },
];

// Why: reported when `find_high_entropy_token` fires; `expr` is empty because
// the detector is procedural, so this must never be added to SECRET_PATTERNS.
pub(super) const HIGH_ENTROPY_PATTERN: SecretPattern = SecretPattern {
    id: "high-entropy-token",
    name: "High-entropy token (possible credential)",
    expr: "",
};
