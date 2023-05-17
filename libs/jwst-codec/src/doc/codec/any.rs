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

impl<R: CrdtReader> CrdtRead<R> for Any {
    fn read(reader: &mut R) -> JwstCodecResult<Self> {
        let index = reader.read_u8()?;
        match 127 - index {
            0 => Ok(Any::Undefined),
            1 => Ok(Any::Null),
            2 => Ok(Any::Integer(reader.read_var_u64()?)), // Integer
            3 => Ok(Any::Float32(reader.read_f32_be()?)),  // Float32
            4 => Ok(Any::Float64(reader.read_f64_be()?)),  // Float64
            5 => Ok(Any::BigInt64(reader.read_i64_be()?)), // BigInt64
            6 => Ok(Any::False),
            7 => Ok(Any::True),
            8 => Ok(Any::String(reader.read_var_string()?)), // String
            9 => {
                let len = reader.read_var_u64()?;
                let object = (0..len)
                    .map(|_| Self::read_key_value(reader))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Any::Object(object.into_iter().collect()))
            } // Object
            10 => {
                let len = reader.read_var_u64()?;
                let any = (0..len)
                    .map(|_| Self::read(reader))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Any::Array(any))
            } // Array
            11 => {
                let binary = reader.read_var_buffer()?;
                Ok(Any::Binary(binary.to_vec()))
            } // Binary
            _ => Ok(Any::Undefined),
        }
    }
}

impl Any {
    fn read_key_value<R: CrdtReader>(reader: &mut R) -> JwstCodecResult<(String, Any)> {
        let key = reader.read_var_string()?;
        let value = Self::read(reader)?;

        Ok((key, value))
    }

    pub(crate) fn from_multiple<R: CrdtReader>(reader: &mut R) -> JwstCodecResult<Vec<Any>> {
        let len = reader.read_var_u64()?;
        let any = (0..len)
            .map(|_| Any::read(reader))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(any)
    }
}
