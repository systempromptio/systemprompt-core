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
