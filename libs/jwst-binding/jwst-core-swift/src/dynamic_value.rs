use std::collections::HashMap;

use jwst_core::Any;

pub type DynamicValueMap = HashMap<String, DynamicValue>;

pub struct DynamicValue {
    any: Any,
}

impl DynamicValue {
    pub fn new(any: Any) -> Self {
        Self { any }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.any {
            Any::True => Some(true),
            Any::False => Some(false),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self.any {
            Any::Float32(value) => Some(value.0 as f64),
            Any::Float64(value) => Some(value.0),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self.any {
            Any::Integer(value) => Some(value as i64),
            Any::BigInt64(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match &self.any {
            Any::String(value) => Some(value.to_string()),
            _ => None,
        }
    }

    pub fn as_buffer(&self) -> Option<Vec<u8>> {
        match &self.any {
            Any::Binary(value) => Some(value.clone()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<Vec<DynamicValue>> {
        match &self.any {
            Any::Array(value) => Some(value.iter().map(|a| DynamicValue::new(a.clone())).collect()),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<HashMap<String, DynamicValue>> {
        match &self.any {
            Any::Object(value) => Some(
                value
                    .iter()
                    .map(|(key, value)| (key.clone(), DynamicValue::new(value.clone())))
                    .collect(),
            ),
            _ => None,
        }
    }
}
