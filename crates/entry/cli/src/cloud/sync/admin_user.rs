use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_cloud::{
    get_cloud_paths, CloudCredentials, CloudPath, ProfilePath, ProjectContext,
};
use systemprompt_core_database::Database;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{PromoteResult, UserAdminService, UserService};

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

impl CloudUser {
    pub fn from_credentials() -> Result<Option<Self>> {
        let cloud_paths = get_cloud_paths()?;
        let creds_path = cloud_paths.resolve(CloudPath::Credentials);

        if !creds_path.exists() {
            return Ok(None);
        }

        let creds = CloudCredentials::load_from_path(&creds_path)?;

        Ok(creds
            .user_email
            .map(|email| Self { email, name: None }))
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
    #[allow(dead_code)]
    pub path: PathBuf,
}

pub fn discover_profiles() -> Result<Vec<ProfileInfo>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();

    for entry in std::fs::read_dir(&profiles_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let profile_yaml = ctx.profile_path(&name, ProfilePath::Config);
            let secrets_json = ctx.profile_path(&name, ProfilePath::Secrets);

            if profile_yaml.exists() && secrets_json.exists() {
                if let Ok(content) = std::fs::read_to_string(&secrets_json) {
                    if let Ok(secrets) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(db_url) = secrets.get("database_url").and_then(|v| v.as_str()) {
                            profiles.push(ProfileInfo {
                                name,
                                database_url: db_url.to_string(),
                                path: path.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(profiles)
}

pub async fn sync_admin_to_database(
    user: &CloudUser,
    database_url: &str,
    profile_name: &str,
) -> SyncResult {
    let db = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Database::new_postgres(database_url),
    )
    .await
    {
        Ok(Ok(db)) => Arc::new(db),
        Ok(Err(e)) => {
            return SyncResult::ConnectionFailed {
                profile: profile_name.to_string(),
                error: e.to_string(),
            };
        },
        Err(_) => {
            return SyncResult::ConnectionFailed {
                profile: profile_name.to_string(),
                error: "Connection timed out".to_string(),
            };
        },
    };

    let user_service = match UserService::new(&db) {
        Ok(s) => s,
        Err(e) => {
            return SyncResult::Failed {
                profile: profile_name.to_string(),
                error: format!("Failed to create user service: {}", e),
            };
        },
    };

    let admin_service = UserAdminService::new(user_service.clone());

    match user_service.find_by_email(&user.email).await {
        Ok(Some(_existing)) => match admin_service.promote_to_admin(&user.email).await {
            Ok(PromoteResult::Promoted(_, _)) => SyncResult::Promoted {
                email: user.email.clone(),
                profile: profile_name.to_string(),
            },
            Ok(PromoteResult::AlreadyAdmin(_)) => SyncResult::AlreadyAdmin {
                email: user.email.clone(),
                profile: profile_name.to_string(),
            },
            Ok(PromoteResult::UserNotFound) => SyncResult::Failed {
                profile: profile_name.to_string(),
                error: "User not found after existence check".to_string(),
            },
            Err(e) => SyncResult::Failed {
                profile: profile_name.to_string(),
                error: format!("Promotion failed: {}", e),
            },
        },
        Ok(None) => {
            let username = user.username();
            let display_name = user.name.as_deref();

            match user_service
                .create(&username, &user.email, display_name, display_name)
                .await
            {
                Ok(_new_user) => match admin_service.promote_to_admin(&user.email).await {
                    Ok(_) => SyncResult::Created {
                        email: user.email.clone(),
                        profile: profile_name.to_string(),
                    },
                    Err(e) => SyncResult::Failed {
                        profile: profile_name.to_string(),
                        error: format!("Created user but promotion failed: {}", e),
                    },
                },
                Err(e) => SyncResult::Failed {
                    profile: profile_name.to_string(),
                    error: format!("User creation failed: {}", e),
                },
            }
        },
        Err(e) => SyncResult::Failed {
            profile: profile_name.to_string(),
            error: format!("Failed to check existing user: {}", e),
        },
    }
}

pub async fn sync_admin_to_all_profiles(user: &CloudUser) -> Vec<SyncResult> {
    let profiles = match discover_profiles() {
        Ok(p) => p,
        Err(e) => {
            CliService::warning(&format!("Failed to discover profiles: {}", e));
            return Vec::new();
        },
    };

    if profiles.is_empty() {
        CliService::info("No profiles found to sync admin user.");
        return Vec::new();
    }

    let mut results = Vec::new();

    for profile in profiles {
        let result = sync_admin_to_database(user, &profile.database_url, &profile.name).await;
        results.push(result);
    }

    results
}

pub fn print_sync_results(results: &[SyncResult]) {
    for result in results {
        match result {
            SyncResult::Created { email, profile } => {
                CliService::success(&format!(
                    "Created admin user '{}' in profile '{}'",
                    email, profile
                ));
            },
            SyncResult::Promoted { email, profile } => {
                CliService::success(&format!(
                    "Promoted existing user '{}' to admin in profile '{}'",
                    email, profile
                ));
            },
            SyncResult::AlreadyAdmin { email, profile } => {
                CliService::info(&format!(
                    "User '{}' is already admin in profile '{}'",
                    email, profile
                ));
            },
            SyncResult::ConnectionFailed { profile, error } => {
                CliService::warning(&format!(
                    "Could not connect to profile '{}': {}",
                    profile, error
                ));
            },
            SyncResult::Failed { profile, error } => {
                CliService::warning(&format!(
                    "Failed to sync admin to profile '{}': {}",
                    profile, error
                ));
            },
        }
    }
}
