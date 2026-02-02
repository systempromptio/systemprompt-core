use std::sync::Arc;
use systemprompt_database::Database;
use systemprompt_logging::CliService;
use systemprompt_users::{PromoteResult, UserAdminService, UserService};

use super::discovery::{discover_profiles, print_discovery_summary};
use super::types::{CloudUser, SyncResult};

async fn promote_existing_user(
    admin_service: &UserAdminService,
    email: &str,
    profile_name: &str,
) -> SyncResult {
    match admin_service.promote_to_admin(email).await {
        Ok(PromoteResult::Promoted(_, _)) => SyncResult::Promoted {
            email: email.to_string(),
            profile: profile_name.to_string(),
        },
        Ok(PromoteResult::AlreadyAdmin(_)) => SyncResult::AlreadyAdmin {
            email: email.to_string(),
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
    }
}

async fn create_and_promote_user(
    user_service: &UserService,
    admin_service: &UserAdminService,
    user: &CloudUser,
    profile_name: &str,
) -> SyncResult {
    let username = user.username();
    let display_name = user.name.as_deref();

    match user_service
        .create(&username, &user.email, display_name, display_name)
        .await
    {
        Ok(_) => match admin_service.promote_to_admin(&user.email).await {
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
        Err(e) => {
            tracing::warn!(profile = %profile_name, error = %e, "Database connection timed out");
            return SyncResult::ConnectionFailed {
                profile: profile_name.to_string(),
                error: "Connection timed out (5s)".to_string(),
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
        Ok(Some(_)) => promote_existing_user(&admin_service, &user.email, profile_name).await,
        Ok(None) => {
            create_and_promote_user(&user_service, &admin_service, user, profile_name).await
        },
        Err(e) => SyncResult::Failed {
            profile: profile_name.to_string(),
            error: format!("Failed to check existing user: {}", e),
        },
    }
}

pub async fn sync_admin_to_all_profiles(user: &CloudUser, verbose: bool) -> Vec<SyncResult> {
    let discovery = match discover_profiles() {
        Ok(d) => d,
        Err(e) => {
            CliService::warning(&format!("Failed to discover profiles: {}", e));
            return Vec::new();
        },
    };

    print_discovery_summary(&discovery, verbose);

    if discovery.profiles.is_empty() {
        if discovery.skipped.is_empty() {
            CliService::info("No profiles found to sync admin user.");
        } else {
            CliService::warning(
                "No profiles available for sync (all skipped due to configuration issues).",
            );
        }
        return Vec::new();
    }

    let mut results = Vec::new();

    for profile in discovery.profiles {
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
