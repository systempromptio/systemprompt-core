use std::process::Command;

use base64::Engine;

/// Read the `session_id` claim from a JWT *without verifying its signature* —
/// the loadtest only needs the value the gateway will match the `x-session-id`
/// header against. Core 0.11's `/v1/messages` rejects (401) any request whose
/// `x-session-id` header does not equal the token's own `session_id` claim, so
/// gateway scenarios must echo this value rather than fabricate a fresh label.
///
/// Returns `None` if the token is malformed or carries no `session_id` claim,
/// letting callers fall back to a label for non-JWT auth paths.
pub fn session_id_from_jwt(token: &str) -> Option<String> {
    // A JWT is `header.payload.signature`; the payload is base64url (no pad).
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("session_id")?
        .as_str()
        .map(std::string::ToString::to_string)
}

// `admin_email`, when set, is forwarded as `--email`. This is required on
// cloud-less deployments (e.g. air-gapped): without it the CLI falls back to
// resolving the admin identity from cloud credentials, which an air-gapped box
// will never have. When `None`, the CLI's cloud-credential path is used.
pub fn acquire_token(
    web_dir: &str,
    profile: &str,
    admin_email: Option<&str>,
) -> Result<String, String> {
    // Honour the `--profile` arg so the loadtest can self-acquire a token
    // against any profile (air-gap, ci, local) — each has its own DB and
    // jwt_secret, so a hardcoded `local` path would mint a token the target
    // server rejects.
    let profile_path = format!("{web_dir}/.systemprompt/profiles/{profile}/profile.yaml");

    let mut command = Command::new(format!("{web_dir}/target/debug/systemprompt"));
    command
        .args(["admin", "session", "login", "--token-only"])
        .env("SYSTEMPROMPT_PROFILE", &profile_path)
        .current_dir(web_dir);
    if let Some(email) = admin_email {
        command.args(["--email", email]);
    }

    let output = command
        .output()
        .map_err(|e| format!("Failed to run CLI: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .rev()
        .find(|line| line.starts_with("eyJ"))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!(
                "No JWT in CLI output. On a cloud-less deployment, pass --admin-email (or export \
                 SYSTEMPROMPT_ADMIN_EMAIL). stderr: {stderr}"
            )
        })
}
