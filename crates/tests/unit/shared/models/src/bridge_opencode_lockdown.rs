//! `lockdown_policies` ordering with allowed plugins, and the optional model
//! display name on a provider's `models` entry.

use systemprompt_models::bridge::opencode::{
    Lockdown, ModelEntry, PolicyAction, PolicyEffect, lockdown_policies,
};

fn lockdown<'a>(plugins: &'a [String], mcp: &'a [String]) -> Lockdown<'a> {
    Lockdown {
        provider: "systemprompt",
        allowed_tools: &[],
        denied_tools: &[],
        allowed_mcp: mcp,
        allowed_plugins: plugins,
    }
}

fn integration(
    policies: &[systemprompt_models::bridge::opencode::Policy],
) -> Vec<(PolicyEffect, String)> {
    policies
        .iter()
        .filter(|p| p.action == PolicyAction::Integration)
        .map(|p| (p.effect, p.resource.clone()))
        .collect()
}

#[test]
fn allowed_plugin_follows_the_plugin_deny() {
    let plugins = vec!["/etc/opencode/plugins/*".to_owned()];
    let mcp = vec!["odoo".to_owned()];
    let got = integration(&lockdown_policies(&lockdown(&plugins, &mcp)));
    assert_eq!(
        got,
        vec![
            (PolicyEffect::Deny, "mcp:*".to_owned()),
            (PolicyEffect::Deny, "plugin:*".to_owned()),
            (
                PolicyEffect::Allow,
                "plugin:/etc/opencode/plugins/*".to_owned()
            ),
            (PolicyEffect::Allow, "mcp:odoo".to_owned()),
        ]
    );
}

#[test]
fn no_allowed_plugins_keeps_every_plugin_denied() {
    let got = integration(&lockdown_policies(&lockdown(&[], &[])));
    assert!(
        got.iter()
            .all(|(effect, res)| *effect == PolicyEffect::Deny || !res.starts_with("plugin:"))
    );
}

#[test]
fn model_name_is_written_only_when_set() {
    let named = ModelEntry {
        model: "dyn-fast".to_owned(),
        name: Some("DynWork".to_owned()),
        limit: None,
    };
    let bare = ModelEntry {
        name: None,
        ..named.clone()
    };
    assert_eq!(
        serde_json::to_value(&named).unwrap(),
        serde_json::json!({ "modelID": "dyn-fast", "name": "DynWork" })
    );
    assert_eq!(
        serde_json::to_value(&bare).unwrap(),
        serde_json::json!({ "modelID": "dyn-fast" })
    );
}
