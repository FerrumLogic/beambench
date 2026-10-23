use serde_json::{Map, Value};

pub(crate) fn resolve(
    schema: &Value,
    command: &str,
    args: Value,
) -> Result<Option<(String, Value)>, String> {
    // These groups edit the document. Machine/quality-test execution is not
    // reachable through this generic dispatcher.
    for group in ["project", "vector", "variable_text"] {
        let Some(operations) = schema[group].as_array() else {
            continue;
        };
        let Some(operation) = operations
            .iter()
            .find(|entry| entry["operation"] == command)
        else {
            continue;
        };
        let fields = operation["fields"]
            .as_array()
            .ok_or("Invalid workflow schema")?;
        let mut body = Map::new();
        if !args.is_null() {
            for (key, value) in args
                .as_object()
                .ok_or("Command arguments must be an object")?
            {
                let key = to_snake_case(key);
                if !fields.iter().any(|field| field["name"] == key) {
                    return Err(format!("Unexpected argument for {command}: {key}"));
                }
                body.insert(key, value.clone());
            }
        }
        body.insert("command".into(), Value::String(command.into()));
        return Ok(Some((format!("workflows/{group}"), Value::Object(body))));
    }
    Ok(None)
}

fn to_snake_case(key: &str) -> String {
    let mut result = String::new();
    for character in key.chars() {
        if character.is_ascii_uppercase() {
            result.push('_');
            result.push(character.to_ascii_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translates_only_top_level_tauri_arguments() {
        let schema = json!({"project":[{"operation":"add_object_atomic","fields":[{"name":"layer_id"},{"name":"object_data"}]}]});
        let (path, value) = resolve(
            &schema,
            "add_object_atomic",
            json!({"layerId":"id","objectData":{"type":"shape","corner_radius":0}}),
        )
        .unwrap()
        .unwrap();
        assert_eq!(path, "workflows/project");
        assert_eq!(
            value,
            json!({"command":"add_object_atomic","layer_id":"id","object_data":{"type":"shape","corner_radius":0}})
        );
    }

    #[test]
    fn rejects_command_injection_and_does_not_expose_machine_workflows() {
        let schema = json!({"project":[{"operation":"add_object_atomic","fields":[]}],"quality_test":[{"operation":"quality_test_start","fields":[]}]});
        assert!(
            resolve(
                &schema,
                "add_object_atomic",
                json!({"command":"quality_test_start"})
            )
            .is_err()
        );
        assert!(
            resolve(&schema, "quality_test_start", json!({}))
                .unwrap()
                .is_none()
        );
    }
}
