use std::process::Command;

// `admin_email`, when set, is forwarded as `--email`. This is required on
// cloud-less deployments (e.g. air-gapped): without it the CLI falls back to
// resolving the admin identity from cloud credentials, which an air-gapped box
// will never have. When `None`, the CLI's cloud-credential path is used.
pub fn acquire_token(web_dir: &str, admin_email: Option<&str>) -> Result<String, String> {
    let profile_path = format!("{web_dir}/.systemprompt/profiles/local/profile.yaml");

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
