use serde_json::{json, Value as JsonValue};

#[must_use]
pub fn users_input_schema_with_roles(role_names: &[String]) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["list", "assign_role", "remove_role", "delete"],
                "description": "Operation to perform. Defaults to 'list' if not specified."
            },
            "user_id": {
                "type": "string",
                "description": "User ID. Optional for list, required for assign_role, remove_role, delete."
            },
            "role": {
                "type": "string",
                "enum": role_names,
                "description": "Role to assign or remove. Required for assign_role and remove_role actions."
            }
        }
    })
}

#[must_use]
pub fn users_input_schema() -> JsonValue {
    users_input_schema_with_roles(&default_role_names())
}

fn default_role_names() -> Vec<String> {
    vec![
        "anonymous".to_string(),
        "user".to_string(),
        "admin".to_string(),
    ]
}

#[must_use]
pub fn users_output_schema() -> JsonValue {
    list_users_output_schema()
}

fn list_users_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "description": "List of users with session statistics",
        "properties": {
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "name": {"type": "string"},
                        "email": {"type": "string"},
                        "roles": {"type": "array", "items": {"type": "string"}},
                        "status": {"type": "string"},
                        "created_at": {"type": "string"},
                        "total_sessions": {"type": "integer"},
                        "last_active": {"type": ["string", "null"]}
                    }
                }
            },
            "count": {"type": "integer"}
        },
        "x-artifact-type": "table",
        "x-table-hints": {
            "columns": ["id", "name", "email", "roles", "status", "total_sessions", "last_active"],
            "sortable_columns": ["name", "email", "total_sessions", "last_active", "created_at"],
            "default_sort": {"column": "last_active", "order": "desc"},
            "filterable": true,
            "page_size": 25,
            "column_types": {
                "id": "string",
                "name": "string",
                "email": "string",
                "roles": "array",
                "status": "string",
                "total_sessions": "integer",
                "last_active": "datetime"
            }
        }
    })
}
