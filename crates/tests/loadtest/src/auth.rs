use std::process::Command;

pub fn acquire_token(web_dir: &str) -> Result<String, String> {
    let profile_path = format!("{web_dir}/.systemprompt/profiles/local/profile.yaml");

    let output = Command::new(format!("{web_dir}/target/debug/systemprompt"))
        .args(["admin", "session", "login", "--token-only"])
        .env("SYSTEMPROMPT_PROFILE", &profile_path)
        .current_dir(web_dir)
        .output()
        .map_err(|e| format!("Failed to run CLI: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let token = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with("eyJ"))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!("No JWT in CLI output. stderr: {stderr}")
        })?;

    Ok(token)
}
