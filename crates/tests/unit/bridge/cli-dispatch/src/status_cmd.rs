use super::sandbox::{Sandbox, argv};
use systemprompt_bridge::cli::run_with_args;
use systemprompt_bridge::config::paths;
use systemprompt_bridge::integration::cowork_plugins;

fn populate(sb: &Sandbox) {
    let plugins = sb.org_plugins();
    let acme = plugins.join("acme-plugin");
    std::fs::create_dir_all(acme.join("skills").join("triage")).expect("skill dir");
    std::fs::create_dir_all(acme.join("skills").join("review")).expect("skill dir");
    std::fs::create_dir_all(acme.join("skills").join(".hidden")).expect("hidden skill dir");
    std::fs::create_dir_all(acme.join("agents")).expect("agents dir");
    std::fs::write(acme.join("agents").join("reviewer.md"), "# reviewer\n").expect("agent");
    std::fs::write(acme.join("agents").join("notes.txt"), "not an agent\n").expect("non-agent");
    std::fs::create_dir_all(plugins.join(".metadata")).expect("dot dir is skipped");

    let meta = sb.metadata();
    std::fs::create_dir_all(&meta).expect("metadata dir");
    std::fs::write(meta.join("last-sync.json"), "{}").expect("last sync");
    std::fs::write(
        meta.join("user.json"),
        serde_json::json!({ "email": "person@example.com" }).to_string(),
    )
    .expect("user fragment");

    let session_org = sb
        .config
        .path()
        .join("Claude-3p")
        .join("local-agent-mode-sessions")
        .join("session-1")
        .join("org-1");
    std::fs::create_dir_all(session_org.join("cowork_plugins")).expect("cowork session org dir");
}

#[test]
fn status_reports_a_fully_provisioned_machine() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
        ]));
        let _ = run_with_args(&argv(&["install"]));
        populate(&sb);
        let _ = run_with_args(&argv(&["status"]));

        let location = paths::org_plugins_effective().expect("org plugins resolvable");
        assert_eq!(location.path, sb.org_plugins());
        let target = cowork_plugins::resolve_target().expect("the seeded Cowork session is found");
        assert!(
            target.session_org_dir.ends_with("org-1"),
            "the newest session org dir is picked: {}",
            target.session_org_dir.display()
        );
        assert_eq!(
            cowork_plugins::enabled_plugins_key("acme-plugin", "org-provisioned"),
            "acme-plugin@org-provisioned"
        );
    });

    assert!(
        sb.metadata().join("last-sync.json").exists()
            && sb.org_plugins().join("acme-plugin").is_dir(),
        "status leaves the provisioned state untouched"
    );
}

#[test]
fn status_without_a_cowork_install_still_reports_the_org_plugins_tree() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["install"]));
        assert!(
            cowork_plugins::resolve_target().is_none(),
            "no Cowork session directory exists in this sandbox"
        );
        let _ = run_with_args(&argv(&["status"]));
    });
    assert!(sb.org_plugins().is_dir());
}
