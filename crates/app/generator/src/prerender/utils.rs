use std::collections::HashSet;

use systemprompt_template_provider::{ComponentContext, RenderedComponent};
use systemprompt_templates::TemplateRegistry;

pub fn merge_json_data(base: &mut serde_json::Value, extension: &serde_json::Value) {
    match (base, extension) {
        (serde_json::Value::Object(base_obj), serde_json::Value::Object(ext_obj)) => {
            for (key, ext_value) in ext_obj {
                match base_obj.get_mut(key) {
                    Some(base_value) => merge_json_data(base_value, ext_value),
                    None => {
                        base_obj.insert(key.clone(), ext_value.clone());
                    },
                }
            }
        },
        (base, extension) => {
            *base = extension.clone();
        },
    }
}

pub async fn render_components(
    template_registry: &TemplateRegistry,
    target_type: &str,
    component_ctx: &ComponentContext<'_>,
    data: &mut serde_json::Value,
) {
    let mut rendered_variables: HashSet<String> = HashSet::new();

    for component in template_registry.components_for(target_type) {
        let variable_name = component.variable_name();

        if rendered_variables.contains(variable_name) {
            tracing::debug!(
                component_id = %component.component_id(),
                variable_name = %variable_name,
                priority = component.priority(),
                "Skipping component, variable already rendered by higher-priority component"
            );
            continue;
        }

        let result = if let Some(partial) = component.partial_template() {
            template_registry
                .render_partial(&partial.name, data)
                .map(|html| RenderedComponent::new(component.variable_name(), html))
                .map_err(|e| anyhow::anyhow!("{}", e))
        } else {
            component.render(component_ctx).await
        };

        match result {
            Ok(rendered) => {
                if let Some(obj) = data.as_object_mut() {
                    rendered_variables.insert(rendered.variable_name.clone());
                    obj.insert(
                        rendered.variable_name,
                        serde_json::Value::String(rendered.html),
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    component_id = %component.component_id(),
                    target_type = %target_type,
                    error = %e,
                    "Component render failed"
                );
            },
        }
    }
}
