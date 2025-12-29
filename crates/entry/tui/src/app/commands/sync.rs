use crate::messages::{Message, SyncSubcommand};
use tokio::sync::mpsc::UnboundedSender;

use super::super::TuiApp;

fn send_or_log(sender: &UnboundedSender<Message>, msg: Message) -> bool {
    if sender.send(msg).is_err() {
        tracing::debug!("Command output receiver dropped");
        false
    } else {
        true
    }
}

impl TuiApp {
    pub(crate) fn spawn_sync(&self, subcommand: SyncSubcommand) {
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            let Ok(app_name) = std::env::var("FLY_APP") else {
                send_or_log(
                    &sender,
                    Message::CommandOutput(
                        "\nError: FLY_APP environment variable not set\n".to_string(),
                    ),
                );
                return;
            };

            match subcommand {
                SyncSubcommand::All => {
                    if !send_or_log(
                        &sender,
                        Message::CommandOutput(
                            "\n=== Full Sync to Production (Fly) ===\n".to_string(),
                        ),
                    ) {
                        return;
                    }
                    fly_deploy_impl(&app_name, &sender).await;
                    send_or_log(
                        &sender,
                        Message::CommandOutput("\n=== Sync Complete ===\n".to_string()),
                    );
                },
                SyncSubcommand::Code => {
                    if !send_or_log(
                        &sender,
                        Message::CommandOutput("\n=== Deploying to Fly ===\n".to_string()),
                    ) {
                        return;
                    }
                    fly_deploy_impl(&app_name, &sender).await;
                },
                SyncSubcommand::Migrate => {
                    if !send_or_log(
                        &sender,
                        Message::CommandOutput("\n=== Running Migrations on Fly ===\n".to_string()),
                    ) {
                        return;
                    }
                    fly_ssh_console_impl(&app_name, "systemprompt db migrate", &sender).await;
                },
                SyncSubcommand::Restart => {
                    if !send_or_log(
                        &sender,
                        Message::CommandOutput("\n=== Restarting Fly App ===\n".to_string()),
                    ) {
                        return;
                    }
                    fly_restart_impl(&app_name, &sender).await;
                },
            }
        });
    }
}

async fn fly_deploy_impl(app_name: &str, sender: &UnboundedSender<Message>) {
    if !send_or_log(
        sender,
        Message::CommandOutput("  Deploying to Fly...\n".to_string()),
    ) {
        return;
    }
    match tokio::process::Command::new("fly")
        .args(["deploy", "--app", app_name])
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty()
                && !send_or_log(
                    sender,
                    Message::CommandOutput(format!("  {}\n", stdout.trim())),
                )
            {
                return;
            }
            if output.status.success() {
                send_or_log(
                    sender,
                    Message::CommandOutput("  Deploy complete\n".to_string()),
                );
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                send_or_log(
                    sender,
                    Message::CommandOutput(format!("  Error: {}\n", stderr)),
                );
            }
        },
        Err(e) => {
            send_or_log(sender, Message::CommandOutput(format!("  Error: {}\n", e)));
        },
    }
}

async fn fly_ssh_console_impl(app_name: &str, command: &str, sender: &UnboundedSender<Message>) {
    if !send_or_log(
        sender,
        Message::CommandOutput(format!("  Running: {}\n", command)),
    ) {
        return;
    }
    match tokio::process::Command::new("fly")
        .args(["ssh", "console", "--app", app_name, "-C", command])
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty()
                && !send_or_log(
                    sender,
                    Message::CommandOutput(format!("  {}\n", stdout.trim())),
                )
            {
                return;
            }
            if output.status.success() {
                send_or_log(
                    sender,
                    Message::CommandOutput("  Command complete\n".to_string()),
                );
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                send_or_log(
                    sender,
                    Message::CommandOutput(format!("  Error: {}\n", stderr)),
                );
            }
        },
        Err(e) => {
            send_or_log(sender, Message::CommandOutput(format!("  Error: {}\n", e)));
        },
    }
}

async fn fly_restart_impl(app_name: &str, sender: &UnboundedSender<Message>) {
    if !send_or_log(
        sender,
        Message::CommandOutput("  Restarting Fly app...\n".to_string()),
    ) {
        return;
    }
    match tokio::process::Command::new("fly")
        .args(["apps", "restart", app_name])
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty()
                && !send_or_log(
                    sender,
                    Message::CommandOutput(format!("  {}\n", stdout.trim())),
                )
            {
                return;
            }
            if output.status.success() {
                send_or_log(
                    sender,
                    Message::CommandOutput("  App restarted\n".to_string()),
                );
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                send_or_log(
                    sender,
                    Message::CommandOutput(format!("  Error: {}\n", stderr)),
                );
            }
        },
        Err(e) => {
            send_or_log(sender, Message::CommandOutput(format!("  Error: {}\n", e)));
        },
    }
}
