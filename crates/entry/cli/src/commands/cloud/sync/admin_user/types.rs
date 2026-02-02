use anyhow::Result;
use std::path::PathBuf;
use systemprompt_cloud::{get_cloud_paths, CloudCredentials, CloudPath};


#[derive(Debug, Clone)]
pub struct CloudUser {
    pub email: String,
    pub name: Option<String>,
}

#[derive(Debug)]
pub enum SyncResult {
    Created { email: String, profile: String },
    Promoted { email: String, profile: String },
    AlreadyAdmin { email: String, profile: String },
    ConnectionFailed { profile: String, error: String },
    Failed { profile: String, error: String },
}

#[derive(Debug)]
pub enum ProfileSkipReason {
    MissingConfig { path: PathBuf },
    MissingSecrets { path: PathBuf },
    SecretsReadError { path: PathBuf, error: String },
    SecretsParseError { path: PathBuf, error: String },
    MissingDatabaseUrl { profile: String },
    InvalidDirectoryName { path: PathBuf },
}

#[derive(Debug)]
pub struct ProfileDiscoveryResult {
    pub profiles: Vec<ProfileInfo>,
    pub skipped: Vec<ProfileSkipReason>,
}

impl CloudUser {
    pub fn from_credentials() -> Result<Option<Self>> {
        let cloud_paths = get_cloud_paths()?;
        let creds_path = cloud_paths.resolve(CloudPath::Credentials);

        if !creds_path.exists() {
            return Ok(None);
        }

        let creds = CloudCredentials::load_from_path(&creds_path)?;

        Ok(Some(Self {
            email: creds.user_email,
            name: None,
        }))
    }

    pub fn username(&self) -> String {
        self.email
            .split('@')
            .next()
            .unwrap_or(&self.email)
            .to_string()
    }
}

#[derive(Debug)]
pub struct ProfileInfo {
    pub name: String,
    pub database_url: String,
}

pub(super) enum ProfileEntryResult {
    Valid(ProfileInfo),
    Skip(ProfileSkipReason),
    NotDirectory,
}
