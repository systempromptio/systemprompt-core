use systemprompt_engine::prompt::ShellKind;

#[test]
fn detects_the_shell_from_its_program() {
    assert_eq!(ShellKind::from_program("pwsh"), ShellKind::Pwsh);
    assert_eq!(
        ShellKind::from_program(r"C:\Program Files\PowerShell\7\pwsh.exe"),
        ShellKind::Pwsh
    );
    assert_eq!(
        ShellKind::from_program("\"C:\\Program Files\\PowerShell\\7\\PWSH.EXE\""),
        ShellKind::Pwsh
    );
    assert_eq!(
        ShellKind::from_program(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
        ShellKind::WindowsPowerShell
    );
    assert_eq!(
        ShellKind::from_program(r"C:\Windows\System32\cmd.exe"),
        ShellKind::Cmd
    );
    assert_eq!(
        ShellKind::from_program(r"C:\Program Files\Git\bin\bash.exe"),
        ShellKind::Bash
    );
    assert_eq!(ShellKind::from_program("/usr/bin/zsh"), ShellKind::Bash);
}

#[test]
fn guidance_matches_the_shell() {
    assert!(ShellKind::Pwsh.guidance().contains("PowerShell 7"));
    assert!(
        ShellKind::WindowsPowerShell
            .guidance()
            .contains("&& and || do not exist")
    );
    assert!(ShellKind::Bash.guidance().contains("/c/Users/me"));
    assert!(ShellKind::Cmd.guidance().contains("%NAME%"));
    for kind in [
        ShellKind::Pwsh,
        ShellKind::WindowsPowerShell,
        ShellKind::Bash,
        ShellKind::Cmd,
    ] {
        assert!(kind.guidance().contains("background jobs"));
    }
}
