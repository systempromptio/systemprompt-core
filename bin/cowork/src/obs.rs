use crate::types::HelperOutput;
use std::io::Write;

pub fn emit(output: &HelperOutput) -> std::io::Result<()> {
    let json = serde_json::to_string(output)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(json.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

pub fn diag(msg: &str) {
    tracing::warn!(target: "systemprompt_cowork", "{msg}");
}
