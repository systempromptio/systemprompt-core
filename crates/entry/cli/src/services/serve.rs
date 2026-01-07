use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_scheduler::ProcessCleanup;
use systemprompt_loader::ModuleLoader;
use systemprompt_runtime::{
    install_module_with_db, validate_system, AppContext, Modules, ServiceCategory,
};
use systemprompt_traits::{ModuleInfo, Phase, StartupEvent, StartupEventExt, StartupEventSender};

pub async fn execute_with_events(
    foreground: bool,
    events: Option<&StartupEventSender>,
) -> Result<String> {
    let port = 8080u16;

    if events.is_none() {
        CliService::startup_banner(Some("Starting services..."));
    }

    if let Some(pid) = check_port_available(port) {
        if let Some(tx) = events {
            let _ = tx.send(StartupEvent::PortConflict { port, pid });
        }
        handle_port_conflict(port, pid, events).await?;
        if let Some(tx) = events {
            let _ = tx.send(StartupEvent::PortConflictResolved { port });
        }
    } else if let Some(tx) = events {
        tx.port_available(port);
    } else {
        CliService::phase_success(&format!("Port {} available", port), None);
    }

    register_modules(events);

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    run_migrations(&ctx, events).await?;

    systemprompt_core_logging::init_logging(Arc::clone(ctx.db_pool()));

    if let Some(tx) = events {
        tx.phase_started(Phase::Database);
        let _ = tx.send(StartupEvent::DatabaseValidated);
        tx.phase_completed(Phase::Database);
    } else {
        CliService::phase("Validation");
        CliService::phase_info("Running system validation...", None);
    }

    validate_system(&ctx)
        .await
        .context("System validation failed")?;

    if events.is_none() {
        CliService::phase_success("System validation complete", None);
    }

    if events.is_none() {
        CliService::phase("Server");
        if !foreground {
            CliService::phase_warning("Daemon mode not supported", Some("running in foreground"));
        }
    } else if let Some(tx) = events {
        tx.phase_started(Phase::ApiServer);
        if !foreground {
            tx.warning("Daemon mode not supported, running in foreground");
        }
    }

    if foreground {
        systemprompt_core_api::services::server::run_server(
            Arc::unwrap_or_clone(ctx),
            events.cloned(),
        )
        .await?;
    }

    Ok(format!("http://127.0.0.1:{}", port))
}

pub async fn execute(foreground: bool) -> Result<()> {
    execute_with_events(foreground, None).await.map(|_| ())
}

fn check_port_available(port: u16) -> Option<u32> {
    ProcessCleanup::check_port(port)
}

fn kill_process(pid: u32) {
    ProcessCleanup::kill_process(pid);
}

async fn handle_port_conflict(
    port: u16,
    pid: u32,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if events.is_none() {
        CliService::warning(&format!("Port {} is already in use by PID {}", port, pid));
    }

    let is_interactive = atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout);

    if is_interactive {
        print!("Kill process {} and restart? [y/N] ", pid);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let response = input.trim().to_lowercase();
            if response == "y" || response == "yes" {
                if events.is_none() {
                    CliService::info(&format!("Killing process {}...", pid));
                }
                kill_process(pid);
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                if check_port_available(port).is_some() {
                    return Err(anyhow::anyhow!(
                        "Failed to free port {} after killing PID {}",
                        port,
                        pid
                    ));
                }
                if events.is_none() {
                    CliService::success(&format!("Port {} is now available", port));
                }
                return Ok(());
            }
        }
        return Err(anyhow::anyhow!(
            "Port {} is occupied by PID {}. Aborted by user.",
            port,
            pid
        ));
    }

    if events.is_none() {
        CliService::error(&format!("Port {} is already in use by PID {}", port, pid));
        CliService::info("To restart cleanly, run one of:");
        CliService::info("   - just api-rebuild    (rebuild and restart)");
        CliService::info("   - just api-nuke       (nuclear option - kill everything)");
        CliService::info(&format!(
            "   - kill {}             (manually kill the process)",
            pid
        ));
    }
    Err(anyhow::anyhow!(
        "Port {} is occupied by PID {}. Cannot start server.\n\nRun interactively (in a TTY) to \
         be prompted to kill the process.",
        port,
        pid
    ))
}

fn register_modules(events: Option<&StartupEventSender>) {
    let api_registrations: Vec<_> =
        inventory::iter::<systemprompt_runtime::ModuleApiRegistration>().collect();

    if let Some(tx) = events {
        let modules: Vec<_> = api_registrations
            .iter()
            .map(|r| ModuleInfo {
                name: r.module_name.to_string(),
                category: format!("{:?}", r.category),
            })
            .collect();
        tx.modules_loaded(modules.len(), modules);
    } else {
        CliService::phase_info(
            &format!("Loading {} route modules", api_registrations.len()),
            None,
        );

        for registration in &api_registrations {
            let category_name = match registration.category {
                ServiceCategory::Core => "Core",
                ServiceCategory::Agent => "Agent",
                ServiceCategory::Mcp => "Mcp",
                ServiceCategory::Meta => "Meta",
            };
            CliService::phase_success(
                registration.module_name,
                Some(&format!("{} routes", category_name)),
            );
        }
    }
}

async fn run_migrations(ctx: &AppContext, events: Option<&StartupEventSender>) -> Result<()> {
    let modules = Modules::from_vec(ModuleLoader::all())?;

    for module in modules.all() {
        install_module_with_db(module, ctx.db_pool().as_ref())
            .await
            .with_context(|| format!("Failed to install module '{}'", module.name))?;
    }

    if events.is_none() {
        CliService::phase_success("Database schemas installed", None);
    }

    Ok(())
}
