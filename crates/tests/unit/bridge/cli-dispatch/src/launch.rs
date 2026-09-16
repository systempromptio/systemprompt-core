//! A bare invocation with no subcommand opens the GUI only when the process
//! was launched without a console of its own (a double-click); from a pipe,
//! a scheduler or a shell it is the credential-helper `run` and never starts
//! a server.

use super::sandbox::{Sandbox, argv};
use systemprompt_bridge::cli::{Launch, run_launch, run_with_args};

#[test]
fn a_bare_invocation_without_a_console_launch_is_the_credential_helper_not_the_gui() {
    let sb = Sandbox::new();
    let code = sb.run(|| run_with_args(&argv(&[])));
    assert_eq!(
        format!("{code:?}"),
        format!("{:?}", std::process::ExitCode::from(5)),
        "with nothing signed in the helper reports no credential"
    );
    assert!(
        !sb.config
            .path()
            .join("systemprompt")
            .join("bridge-loopback.key")
            .exists(),
        "no proxy was started, so no loopback secret was minted"
    );
}

#[test]
fn the_launch_context_defaults_to_no_gui_and_is_explicit_on_this_platform() {
    assert!(!Launch::default().gui_by_default);
    let detected = Launch::detect();
    assert!(
        !detected.gui_by_default || cfg!(any(target_os = "windows", target_os = "macos")),
        "only a Windows or macOS app launch can default to the GUI"
    );
    let sb = Sandbox::new();
    let code = sb.run(|| {
        run_launch(
            &argv(&["help"]),
            Launch {
                gui_by_default: true,
            },
        )
    });
    assert_eq!(
        format!("{code:?}"),
        format!("{:?}", std::process::ExitCode::SUCCESS),
        "an explicit subcommand is never overridden by the GUI default"
    );
}
