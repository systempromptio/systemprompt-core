use systemprompt_bridge::cli::doctor::hooks::{check_hook_urls, hook_urls_in, stale_ports};
use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::proxy::DEFAULT_PROXY_PORT;

fn hooks_json(port: u16) -> String {
    format!(
        r#"{{
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "*",
        "hooks": [
          {{
            "type": "http",
            "url": "http://127.0.0.1:{port}/api/public/hooks/govern?plugin_id=demo",
            "headers": {{ "Authorization": "Bearer x" }},
            "allowedEnvVars": [],
            "timeout": 10
          }}
        ]
      }}
    ],
    "PostToolUse": [
      {{
        "matcher": "*",
        "hooks": [
          {{
            "type": "http",
            "url": "http://127.0.0.1:{port}/api/public/hooks/track?plugin_id=demo",
            "headers": {{ "Authorization": "Bearer x" }},
            "allowedEnvVars": [],
            "timeout": 10,
            "async": true,
            "event": "PostToolUse"
          }}
        ]
      }}
    ]
  }}
}}"#
    )
}

#[test]
fn finds_every_nested_hook_url() {
    let urls = hook_urls_in(&hooks_json(48217));
    assert_eq!(urls.len(), 2);
    assert!(urls.iter().any(|u| u.contains("/govern")));
    assert!(urls.iter().any(|u| u.contains("/track")));
}

#[test]
fn empty_hooks_file_yields_no_urls() {
    assert!(hook_urls_in(r#"{"hooks":{}}"#).is_empty());
}

#[test]
fn malformed_hooks_file_yields_no_urls() {
    assert!(hook_urls_in("not json at all").is_empty());
}

#[test]
fn matching_port_is_not_stale() {
    let urls = hook_urls_in(&hooks_json(48217));
    assert!(stale_ports(&urls, 48217).is_empty());
}

#[test]
fn moved_proxy_makes_every_url_stale() {
    let urls = hook_urls_in(&hooks_json(48217));
    let stale = stale_ports(&urls, 48219);
    assert_eq!(stale.len(), 1);
    assert!(stale.contains(&48217));
}

#[test]
fn non_loopback_urls_are_left_alone() {
    let urls = vec!["https://gateway.example.com/api/public/hooks/govern".to_owned()];
    assert!(stale_ports(&urls, 48217).is_empty());
}

#[test]
fn reports_nothing_when_no_plugins_are_installed() {
    let home = tempfile::tempdir().unwrap();
    temp_env::with_var("HOME", Some(home.path().to_str().unwrap()), || {
        assert!(check_hook_urls().is_none());
    });
}

#[test]
fn flags_hook_urls_that_name_a_port_the_proxy_does_not_hold() {
    let home = tempfile::tempdir().unwrap();
    let plugin = home
        .path()
        .join(".claude/plugins/marketplaces/org-provisioned/plugins/demo/hooks");
    std::fs::create_dir_all(&plugin).unwrap();
    let drifted = DEFAULT_PROXY_PORT + 7;
    std::fs::write(plugin.join("hooks.json"), hooks_json(drifted)).unwrap();

    temp_env::with_var("HOME", Some(home.path().to_str().unwrap()), || {
        let check = check_hook_urls().expect("a hooks.json is present");
        assert_eq!(check.status, Status::Fail);
        assert!(
            check.detail.contains(&drifted.to_string()),
            "detail = {}",
            check.detail
        );
    });
}
