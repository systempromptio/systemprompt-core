use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "macos" | "darwin" | "mac" => Some(Os::Mac),
            "windows" | "win" => Some(Os::Windows),
            "linux" => Some(Os::Linux),
            _ => None,
        }
    }
}

pub fn template(os: Os, binary: &Path) -> String {
    match os {
        Os::Mac => launchd_plist(binary),
        Os::Windows => task_scheduler_xml(binary),
        Os::Linux => systemd_user_unit(binary),
    }
}

pub fn template_filename(os: Os) -> &'static str {
    match os {
        Os::Mac => "io.systemprompt.cowork-sync.plist",
        Os::Windows => "systemprompt-cowork-sync.xml",
        Os::Linux => "systemprompt-cowork-sync.service+timer",
    }
}

pub fn install_hint(os: Os) -> &'static str {
    match os {
        Os::Mac => {
            "Save to ~/Library/LaunchAgents/io.systemprompt.cowork-sync.plist, then: launchctl \
             bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.systemprompt.cowork-sync.plist"
        },
        Os::Windows => {
            "Save as systemprompt-cowork-sync.xml, then: schtasks /Create /TN \
             \"SystempromptCoworkSync\" /XML systemprompt-cowork-sync.xml"
        },
        Os::Linux => {
            "Split into ~/.config/systemd/user/systemprompt-cowork-sync.{service,timer}, then: \
             systemctl --user daemon-reload && systemctl --user enable --now \
             systemprompt-cowork-sync.timer"
        },
    }
}

fn launchd_plist(binary: &Path) -> String {
    let binary = binary.display();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.systemprompt.cowork-sync</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>sync</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StartInterval</key>
    <integer>1800</integer>
    <key>StandardOutPath</key>
    <string>/tmp/systemprompt-cowork-sync.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/systemprompt-cowork-sync.err</string>
</dict>
</plist>
"#
    )
}

fn task_scheduler_xml(binary: &Path) -> String {
    let binary = binary.display();
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>systemprompt.io Cowork plugin + MCP allowlist sync</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
    <TimeTrigger>
      <Repetition>
        <Interval>PT30M</Interval>
      </Repetition>
      <StartBoundary>2026-01-01T00:00:00</StartBoundary>
      <Enabled>true</Enabled>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>true</RunOnlyIfNetworkAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{binary}</Command>
      <Arguments>sync</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn systemd_user_unit(binary: &Path) -> String {
    let binary = binary.display();
    format!(
        r#"# === systemprompt-cowork-sync.service ===
[Unit]
Description=systemprompt.io Cowork plugin + MCP allowlist sync
After=network-online.target

[Service]
Type=oneshot
ExecStart={binary} sync
Nice=10

[Install]
WantedBy=default.target

# === systemprompt-cowork-sync.timer ===
[Unit]
Description=Periodic systemprompt-cowork sync

[Timer]
OnBootSec=2min
OnUnitActiveSec=30min
Persistent=true

[Install]
WantedBy=timers.target
"#
    )
}
