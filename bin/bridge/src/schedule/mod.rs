use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "macos" | "darwin" | "mac" => Some(Os::Mac),
            "windows" | "win" => Some(Os::Windows),
            "linux" => Some(Os::Linux),
            _ => None,
        }
    }
}

#[must_use]
pub fn template(os: Os, binary: &Path) -> String {
    match os {
        Os::Mac => launchd_plist(binary),
        Os::Windows => task_scheduler_xml(binary),
        Os::Linux => systemd_user_unit(binary),
    }
}

#[must_use]
pub fn template_filename(os: Os) -> &'static str {
    match os {
        Os::Mac => "io.systemprompt.bridge-sync.plist",
        Os::Windows => "systemprompt-bridge-sync.xml",
        Os::Linux => "systemprompt-bridge-sync.service+timer",
    }
}

#[must_use]
pub fn install_hint(os: Os) -> &'static str {
    match os {
        Os::Mac => {
            "Save to ~/Library/LaunchAgents/io.systemprompt.bridge-sync.plist, then: launchctl \
             bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.systemprompt.bridge-sync.plist"
        },
        Os::Windows => {
            "Save as systemprompt-bridge-sync.xml, then: schtasks /Create /TN \
             \"SystempromptBridgeSync\" /XML systemprompt-bridge-sync.xml"
        },
        Os::Linux => {
            "Split into ~/.config/systemd/user/systemprompt-bridge-sync.{service,timer}, then: \
             systemctl --user daemon-reload && systemctl --user enable --now \
             systemprompt-bridge-sync.timer"
        },
    }
}

const LAUNCHD_PLIST_TMPL: &str = include_str!("templates/launchd.plist.tmpl");
const TASK_SCHEDULER_XML_TMPL: &str = include_str!("templates/task-scheduler.xml.tmpl");
const SYSTEMD_UNIT_TMPL: &str = include_str!("templates/systemd.unit.tmpl");

fn launchd_plist(binary: &Path) -> String {
    LAUNCHD_PLIST_TMPL.replace("{binary}", &binary.display().to_string())
}

fn task_scheduler_xml(binary: &Path) -> String {
    TASK_SCHEDULER_XML_TMPL.replace("{binary}", &binary.display().to_string())
}

fn systemd_user_unit(binary: &Path) -> String {
    SYSTEMD_UNIT_TMPL.replace("{binary}", &binary.display().to_string())
}
