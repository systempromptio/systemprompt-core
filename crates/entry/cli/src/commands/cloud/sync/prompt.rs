use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_sync::SyncDirection;

#[allow(dead_code)]
pub fn prompt_direction() -> Result<SyncDirection> {
    let options = &[
        "Local -> Cloud (push your changes to cloud)",
        "Cloud -> Local (pull cloud state to local)",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Sync direction")
        .items(options)
        .default(0)
        .interact()?;

    Ok(match selection {
        0 => SyncDirection::Push,
        _ => SyncDirection::Pull,
    })
}
