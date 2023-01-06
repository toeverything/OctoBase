//! Progress 1/10:
//!  * There are many kinds of errors related to mapping between [yrs::types::Value] & [lib0::any::Any] to the
//!    user's expected result from querying a specific attribute. Getting this wrong can lead to corrupt data.
//!  * Improve: I think we should make value errors and context more pervasive somehow.
//!    * It would be great to have a single way to manage and parse values, as it seems
//!      important to avoid corruption and identify where corruption happens through clear errors.

use lib0::any::Any;
use yrs::types::Value;
/// Value assigned to a block or metadata
pub struct OctoValue(Value);

impl OctoValue {
    pub fn try_as_string(&self) -> Result<String, OctoValueError> {
        match &self.0 {
            Value::Any(Any::String(box_str)) => return Ok(box_str.to_string()),
            _ => Err(OctoValueError::WrongType {
                actual: self.actual_type(),
                expected: "basic string",
                details: None,
            }),
        }
    }

    // this is for constructing error only, not for stabilizing
    fn actual_type(&self) -> &'static str {
        match &self.0 {
            Value::Any(any_value) => match any_value {
                Any::Null => "null",
                Any::Undefined => "undefined",
                Any::Bool(_) => "boolean",
                Any::Number(_) => "number",
                Any::BigInt(_) => "bigint",
                Any::String(_) => "string",
                Any::Buffer(_) => "buffer",
                Any::Array(_) => "array",
                Any::Map(_) => "map",
            },
            Value::YText(_) => "YText",
            Value::YArray(_) => "YArray",
            Value::YMap(_) => "YMap",
            Value::YXmlElement(_) => "YXmlElement",
            Value::YXmlFragment(_) => "YXmlFragment",
            Value::YXmlText(_) => "YXmlText",
            Value::YDoc(_) => "YDoc",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OctoValueError {
    // TODO: Can we reveal the underlying error in display (e.g. a parsing error?)
    #[error("expected value of type `{expected}`, but found `{actual}`")]
    WrongType {
        actual: &'static str,
        expected: &'static str,
        details: Option<Box<dyn std::error::Error>>,
    },
}

impl From<Value> for OctoValue {
    fn from(yrs_value: Value) -> Self {
        OctoValue(yrs_value)
    }
}
