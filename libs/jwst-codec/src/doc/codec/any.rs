use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Any {
    Undefined,
    Null,
    Integer(u64),
    Float32(f32),
    Float64(f64),
    BigInt64(i64),
    False,
    True,
    String(String),
    Object(HashMap<String, Any>),
    Array(Vec<Any>),
    Binary(Vec<u8>),
}

impl Any {
    pub(crate) fn from<R: CrdtReader>(decoder: &mut R) -> JwstCodecResult<Self> {
        let index = decoder.read_u8()?;
        match 127 - index {
            0 => Ok(Any::Undefined),
            1 => Ok(Any::Null),
            2 => Ok(Any::Integer(decoder.read_var_u64()?)), // Integer
            3 => Ok(Any::Float32(decoder.read_f32_be()?)),  // Float32
            4 => Ok(Any::Float64(decoder.read_f64_be()?)),  // Float64
            5 => Ok(Any::BigInt64(decoder.read_i64_be()?)), // BigInt64
            6 => Ok(Any::False),
            7 => Ok(Any::True),
            8 => Ok(Any::String(decoder.read_var_string()?)), // String
            9 => {
                let len = decoder.read_var_u64()?;
                let object = (0..len)
                    .map(|_| Self::read_key_value(decoder))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Any::Object(object.into_iter().collect()))
            } // Object
            10 => {
                let len = decoder.read_var_u64()?;
                let any = (0..len)
                    .map(|_| Self::from(decoder))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Any::Array(any))
            } // Array
            11 => {
                let binary = decoder.read_var_buffer()?;
                Ok(Any::Binary(binary.to_vec()))
            } // Binary
            _ => Ok(Any::Undefined),
        }
    }

    fn read_key_value<R: CrdtReader>(decoder: &mut R) -> JwstCodecResult<(String, Any)> {
        let key = decoder.read_var_string()?;
        let value = Self::from(decoder)?;

        Ok((key, value))
    }

    pub(crate) fn from_multiple<R: CrdtReader>(decoder: &mut R) -> JwstCodecResult<Vec<Any>> {
        let len = decoder.read_var_u64()?;
        let any = (0..len)
            .map(|_| Any::from(decoder))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(any)
    }
}
