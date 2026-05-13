use serde_json::Value;

/// Validates `instance` against a JSON Schema value using the default validator (draft-aware).
pub fn validate_json_schema(instance: &Value, schema: &Value) -> Result<(), String> {
    jsonschema::validate(schema, instance).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_matching_instance() {
        let schema = json!({ "type": "object", "required": ["x"], "properties": { "x": { "type": "number" } } });
        let instance = json!({ "x": 1 });
        validate_json_schema(&instance, &schema).unwrap();
    }

    #[test]
    fn rejects_bad_instance() {
        let schema = json!({ "type": "object", "required": ["x"], "properties": { "x": { "type": "number" } } });
        let instance = json!({ "x": "nope" });
        assert!(validate_json_schema(&instance, &schema).is_err());
    }
}
