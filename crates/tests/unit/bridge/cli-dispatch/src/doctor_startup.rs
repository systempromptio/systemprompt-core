use super::sandbox::{Sandbox, argv};
use systemprompt_bridge::cli::doctor::{Status, run_checks};
use systemprompt_bridge::cli::run_with_args;
use systemprompt_bridge::context::{BridgeContext, ProxyMode};

fn corrupt_portfile(sb: &Sandbox) {
    let dir = sb.config.path().join("systemprompt");
    std::fs::create_dir_all(&dir).expect("config dir");
    std::fs::write(dir.join("bridge-proxy.json"), "{ this is not json").expect("write port file");
}

#[test]
fn a_corrupt_proxy_port_file_starts_the_context_and_is_reported_as_a_startup_fault() {
    let sb = Sandbox::new();
    corrupt_portfile(&sb);
    let faults = sb.run(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach)
            .expect("a corrupt port file must not stop the bridge from starting");
        ctx.startup_faults.clone()
    });

    let fault = faults
        .iter()
        .find(|f| f.component == "proxy port file")
        .unwrap_or_else(|| panic!("the unreadable port file is recorded, got {faults:?}"));
    assert!(
        fault.error.contains("bridge-proxy.json"),
        "the fault names the file that could not be parsed: {}",
        fault.error
    );
}

#[test]
fn doctor_prepends_one_failing_startup_check_per_startup_fault() {
    let sb = Sandbox::new();
    corrupt_portfile(&sb);
    let (names, startup): (Vec<&'static str>, Vec<String>) = sb.run(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("context starts");
        let (checks, any_fail) = ctx.block_on(run_checks(&ctx));
        assert!(any_fail, "a startup fault fails the doctor run");
        let names = checks.iter().map(|c| c.name).collect();
        let startup = checks
            .iter()
            .filter(|c| c.name == "startup")
            .inspect(|c| assert_eq!(c.status, Status::Fail, "a startup fault is never a warning"))
            .map(|c| c.detail.clone())
            .collect();
        (names, startup)
    });

    assert_eq!(
        names.first().copied(),
        Some("startup"),
        "the startup faults are reported before anything else, got {names:?}"
    );
    assert_eq!(startup.len(), 1, "one check per fault, got {startup:?}");
    assert!(
        startup[0].starts_with("proxy port file: "),
        "the check detail carries the component and the error: {}",
        startup[0]
    );
}

#[test]
fn a_corrupt_port_file_does_not_make_dispatch_report_a_runtime_init_failure() {
    let clean = Sandbox::new();
    let baseline = clean.run(|| format!("{:?}", run_with_args(&argv(&["status"]))));

    let sb = Sandbox::new();
    corrupt_portfile(&sb);
    let code = sb.run(|| format!("{:?}", run_with_args(&argv(&["status"]))));

    assert!(
        !code.contains("70"),
        "70 is the runtime-init failure code; a recorded fault must not use it, got {code}"
    );
    assert_eq!(
        code, baseline,
        "a corrupt port file must not change what an unrelated command exits with"
    );
}
