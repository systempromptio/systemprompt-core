use std::io::Write;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::install::xml::escape;

use super::process::now_unix;

const DEFAULT_MODELS: &[&str] = &["claude-opus-4-7", "claude-sonnet-4-6", "claude-haiku-4-5"];

const PROFILE_TMPL: &str =
    include_str!("../templates/claude_desktop_profile.mobileconfig.tmpl");

#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedProfile {
    pub path: String,
    pub bytes: usize,
    pub payload_uuid: String,
    pub profile_uuid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateProfileBody {
    #[serde(default)]
    pub gateway_base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}

pub fn default_models() -> Vec<String> {
    DEFAULT_MODELS.iter().map(|s| (*s).to_string()).collect()
}

pub fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join("systemprompt-cowork");
    std::fs::create_dir_all(&dir)?;
    let payload_uuid = format!(
        "ce0a{}-cwk0-4cwk-cwk0-{}",
        format!("{:08x}", now_unix() & 0xFFFF_FFFF),
        format!("{:012x}", now_unix() ^ 0xDEADBEEF_CAFEBABEu64)
    );
    let profile_uuid = format!(
        "ce0b{}-cwk0-4cwk-cwk0-{}",
        format!("{:08x}", (now_unix() ^ 0x1234_5678) & 0xFFFF_FFFF),
        format!("{:012x}", now_unix() ^ 0xFEEDFACE_DEADC0DEu64)
    );
    let path = dir.join(format!("claude-cowork-{}.mobileconfig", now_unix()));

    let xml = render_profile(inputs, &payload_uuid, &profile_uuid);
    {
        let mut f = std::fs::File::create(&path)?;
        f.write_all(xml.as_bytes())?;
    }

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: xml.len(),
        payload_uuid,
        profile_uuid,
    })
}

fn render_profile(inputs: &ProfileGenInputs, payload_uuid: &str, profile_uuid: &str) -> String {
    let models_xml: String = inputs
        .models
        .iter()
        .map(|m| format!("            <string>{}</string>", escape(m)))
        .collect::<Vec<_>>()
        .join("\n");

    let org_xml = match inputs.organization_uuid.as_deref() {
        Some(uuid) if !uuid.is_empty() => format!(
            "        <key>deploymentOrganizationUuid</key>\n        <string>{}</string>\n",
            escape(uuid)
        ),
        _ => String::new(),
    };

    PROFILE_TMPL
        .replace("{profile_uuid}", &escape(profile_uuid))
        .replace("{payload_uuid}", &escape(payload_uuid))
        .replace("{base_url}", &escape(&inputs.gateway_base_url))
        .replace("{api_key}", &escape(&inputs.api_key))
        .replace("{models_xml}", &models_xml)
        .replace("{org_xml}", &org_xml)
}

pub fn install_profile(path: &str) -> std::io::Result<()> {
    Command::new("/usr/bin/open").arg(path).status()?;
    Ok(())
}
