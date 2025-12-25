//! JSON conversion utilities for Val types

use anyhow::Result;
use serde_json::Value as JsonValue;

use super::types::Val;

pub fn json_to_val_map(json: &JsonValue) -> Result<std::collections::HashMap<String, Val>> {
    match json {
        JsonValue::Object(map) => {
            let mut result = std::collections::HashMap::new();
            for (key, value) in map {
                result.insert(key.clone(), json_to_val(value)?);
            }
            Ok(result)
        }
        _ => Err(anyhow::anyhow!("Expected JSON object for inputs")),
    }
}

pub fn json_to_val(json: &JsonValue) -> Result<Val> {
    let val = match json {
        JsonValue::Null => Val::Null,
        JsonValue::Bool(b) => Val::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Val::Num(i as f64)
            } else if let Some(f) = n.as_f64() {
                Val::Num(f)
            } else {
                return Err(anyhow::anyhow!("Invalid number in JSON"));
            }
        }
        JsonValue::String(s) => Val::Str(s.clone()),
        JsonValue::Array(arr) => {
            let vals: Result<Vec<Val>> = arr.iter().map(json_to_val).collect();
            Val::List(vals?)
        }
        JsonValue::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (key, value) in obj {
                map.insert(key.clone(), json_to_val(value)?);
            }
            Val::Obj(map)
        }
    };
    Ok(val)
}

pub fn val_to_json(val: &Val) -> Result<JsonValue> {
    let json = match val {
        Val::Null => JsonValue::Null,
        Val::Bool(b) => JsonValue::Bool(*b),
        Val::Num(n) => serde_json::Number::from_f64(*n)
            .map(JsonValue::Number)
            .ok_or_else(|| anyhow::anyhow!("Invalid number"))?,
        Val::Str(s) => JsonValue::String(s.clone()),
        Val::List(arr) => {
            let vals: Result<Vec<JsonValue>> = arr.iter().map(val_to_json).collect();
            JsonValue::Array(vals?)
        }
        Val::Obj(obj) => {
            let mut map = serde_json::Map::new();
            for (key, value) in obj {
                map.insert(key.clone(), val_to_json(value)?);
            }
            JsonValue::Object(map)
        }
        Val::Promise(awaitable) => {
            return Err(anyhow::anyhow!(
                "Cannot convert Promise value to JSON ({:?})",
                awaitable
            ));
        }
        Val::Error(error_info) => serde_json::to_value(error_info)?,
        Val::NativeFunc(_) => JsonValue::Null,
    };
    Ok(json)
}

pub fn val_map_to_json(map: &std::collections::HashMap<String, Val>) -> Result<JsonValue> {
    let mut json_map = serde_json::Map::new();
    for (key, value) in map {
        json_map.insert(key.clone(), val_to_json(value)?);
    }
    Ok(JsonValue::Object(json_map))
}
