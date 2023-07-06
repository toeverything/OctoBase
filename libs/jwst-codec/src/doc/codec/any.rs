use super::*;
use ordered_float::OrderedFloat;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum Any {
    Undefined,
    Null,
    Integer(u64),
    Float32(OrderedFloat<f32>),
    Float64(OrderedFloat<f64>),
    BigInt64(i64),
    False,
    True,
    String(String),
    // FIXME: due to macro's overflow evaluating, we can't use proptest here
    #[cfg_attr(test, proptest(skip))]
    Object(HashMap<String, Any>),
    #[cfg_attr(test, proptest(skip))]
    Array(Vec<Any>),
    Binary(Vec<u8>),
}

impl<R: CrdtReader> CrdtRead<R> for Any {
    fn read(reader: &mut R) -> JwstCodecResult<Self> {
        let index = reader.read_u8()?;
        match 127u8.overflowing_sub(index).0 {
            0 => Ok(Any::Undefined),
            1 => Ok(Any::Null),
            2 => Ok(Any::Integer(reader.read_var_u64()?)), // Integer
            3 => Ok(Any::Float32(reader.read_f32_be()?.into())), // Float32
            4 => Ok(Any::Float64(reader.read_f64_be()?.into())), // Float64
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

impl<W: CrdtWriter> CrdtWrite<W> for Any {
    fn write(&self, writer: &mut W) -> JwstCodecResult {
        match self {
            Any::Undefined => writer.write_u8(127)?,
            Any::Null => writer.write_u8(127 - 1)?,
            Any::Integer(value) => {
                writer.write_u8(127 - 2)?;
                writer.write_var_u64(*value)?;
            }
            Any::Float32(value) => {
                writer.write_u8(127 - 3)?;
                writer.write_f32_be(value.into_inner())?;
            }
            Any::Float64(value) => {
                writer.write_u8(127 - 4)?;
                writer.write_f64_be(value.into_inner())?;
            }
            Any::BigInt64(value) => {
                writer.write_u8(127 - 5)?;
                writer.write_i64_be(*value)?;
            }
            Any::False => writer.write_u8(127 - 6)?,
            Any::True => writer.write_u8(127 - 7)?,
            Any::String(value) => {
                writer.write_u8(127 - 8)?;
                writer.write_var_string(value)?;
            }
            Any::Object(value) => {
                writer.write_u8(127 - 9)?;
                writer.write_var_u64(value.len() as u64)?;
                for (key, value) in value {
                    Self::write_key_value(writer, key, value)?;
                }
            }
            Any::Array(values) => {
                writer.write_u8(127 - 10)?;
                writer.write_var_u64(values.len() as u64)?;
                for value in values {
                    value.write(writer)?;
                }
            }
            Any::Binary(value) => {
                writer.write_u8(127 - 11)?;
                writer.write_var_buffer(value)?;
            }
        }

        Ok(())
    }
}

impl Any {
    fn read_key_value<R: CrdtReader>(reader: &mut R) -> JwstCodecResult<(String, Any)> {
        let key = reader.read_var_string()?;
        let value = Self::read(reader)?;

        Ok((key, value))
    }

    fn write_key_value<W: CrdtWriter>(writer: &mut W, key: &str, value: &Any) -> JwstCodecResult {
        writer.write_var_string(key)?;
        value.write(writer)?;

        Ok(())
    }

    pub(crate) fn read_multiple<R: CrdtReader>(reader: &mut R) -> JwstCodecResult<Vec<Any>> {
        let len = reader.read_var_u64()?;
        let any = (0..len)
            .map(|_| Any::read(reader))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(any)
    }

    pub(crate) fn write_multiple<W: CrdtWriter>(writer: &mut W, any: &[Any]) -> JwstCodecResult {
        writer.write_var_u64(any.len() as u64)?;
        for value in any {
            value.write(writer)?;
        }

        Ok(())
    }
}

macro_rules! impl_primitive_from {
    (unsigned, $($ty: ty),*) => {
        $(
            impl From<$ty> for Any {
                fn from(value: $ty) -> Self {
                    Self::Integer(value.into())
                }
            }
        )*
    };
    (signed, $($ty: ty),*) => {
        $(
            impl From<$ty> for Any {
                fn from(value: $ty) -> Self {
                    Self::BigInt64(value.into())
                }
            }
        )*
    };
    (string, $($ty: ty),*) => {
        $(
            impl From<$ty> for Any {
                fn from(value: $ty) -> Self {
                    Self::String(value.into())
                }
            }
        )*
    };
}

impl_primitive_from!(unsigned, u8, u16, u32, u64);
impl_primitive_from!(signed, i8, i16, i32, i64);
impl_primitive_from!(string, String, &str);

impl From<usize> for Any {
    fn from(value: usize) -> Self {
        Self::Integer(value as u64)
    }
}

impl From<isize> for Any {
    fn from(value: isize) -> Self {
        Self::BigInt64(value as i64)
    }
}

impl From<f32> for Any {
    fn from(value: f32) -> Self {
        Self::Float32(value.into())
    }
}

impl From<f64> for Any {
    fn from(value: f64) -> Self {
        Self::Float64(value.into())
    }
}

impl From<bool> for Any {
    fn from(value: bool) -> Self {
        if value {
            Self::True
        } else {
            Self::False
        }
    }
}

impl FromIterator<Any> for Any {
    fn from_iter<I: IntoIterator<Item = Any>>(iter: I) -> Self {
        Self::Array(iter.into_iter().collect())
    }
}

impl<'a> FromIterator<&'a Any> for Any {
    fn from_iter<I: IntoIterator<Item = &'a Any>>(iter: I) -> Self {
        Self::Array(iter.into_iter().cloned().collect())
    }
}

impl FromIterator<(String, Any)> for Any {
    fn from_iter<I: IntoIterator<Item = (String, Any)>>(iter: I) -> Self {
        let mut map = HashMap::new();
        map.extend(iter);
        Self::Object(map)
    }
}

impl From<HashMap<String, Any>> for Any {
    fn from(value: HashMap<String, Any>) -> Self {
        Self::Object(value)
    }
}

impl From<Vec<u8>> for Any {
    fn from(value: Vec<u8>) -> Self {
        Self::Binary(value)
    }
}

impl From<&[u8]> for Any {
    fn from(value: &[u8]) -> Self {
        Self::Binary(value.into())
    }
}

// TODO: impl for Any::Undefined
impl<T: Into<Any>> From<Option<T>> for Any {
    fn from(value: Option<T>) -> Self {
        if let Some(val) = value {
            val.into()
        } else {
            Any::Null
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{collection::vec, prelude::*};

    #[test]
    fn test_any_codec() {
        let any = Any::Object(
            vec![
                ("name".to_string(), Any::String("Alice".to_string())),
                ("age".to_string(), Any::Integer(25)),
                (
                    "contacts".to_string(),
                    Any::Array(vec![
                        Any::Object(
                            vec![
                                ("type".to_string(), Any::String("Mobile".to_string())),
                                ("number".to_string(), Any::String("1234567890".to_string())),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                        Any::Object(
                            vec![
                                ("type".to_string(), Any::String("Email".to_string())),
                                (
                                    "address".to_string(),
                                    Any::String("alice@example.com".to_string()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                        Any::Undefined,
                    ]),
                ),
                (
                    "standard_data".to_string(),
                    Any::Array(vec![
                        Any::Undefined,
                        Any::Null,
                        Any::Integer(1145141919810),
                        Any::Float32(114.514.into()),
                        Any::Float64(115.514.into()),
                        Any::BigInt64(-1145141919810),
                        Any::False,
                        Any::True,
                        Any::Object(
                            vec![
                                ("name".to_string(), Any::String("tadokoro".to_string())),
                                ("age".to_string(), Any::String("24".to_string())),
                                ("profession".to_string(), Any::String("student".to_string())),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                        Any::Binary(vec![1, 2, 3, 4, 5]),
                    ]),
                ),
            ]
            .into_iter()
            .collect(),
        );

        let mut encoder = RawEncoder::default();
        any.write(&mut encoder).unwrap();
        let encoded = encoder.into_inner();

        let mut decoder = RawDecoder::new(encoded);
        let decoded = Any::read(&mut decoder).unwrap();

        assert_eq!(any, decoded);
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn test_random_any(any in vec(any::<Any>(), 0..100)) {
            for any in &any {
                let mut encoder = RawEncoder::default();
                any.write(&mut encoder).unwrap();
                let encoded = encoder.into_inner();

                let mut decoder = RawDecoder::new(encoded);
                let decoded = Any::read(&mut decoder).unwrap();

                assert_eq!(any, &decoded);
            }
        }
    }

    #[test]
    fn test_convert_to_any() {
        let any: Vec<Any> = vec![
            42u8.into(),
            42u16.into(),
            42u32.into(),
            42u64.into(),
            114.514f32.into(),
            1919.810f64.into(),
            (-42i8).into(),
            (-42i16).into(),
            (-42i32).into(),
            (-42i64).into(),
            false.into(),
            true.into(),
            "JWST".to_string().into(),
            "OctoBase".into(),
            vec![1u8, 9, 1, 9].into(),
            (&[8u8, 1, 0][..]).into(),
            [Any::True, 42u8.into()].iter().collect(),
        ];
        assert_eq!(
            any,
            vec![
                Any::Integer(42),
                Any::Integer(42),
                Any::Integer(42),
                Any::Integer(42),
                Any::Float32(114.514.into()),
                Any::Float64(1919.810.into()),
                Any::BigInt64(-42),
                Any::BigInt64(-42),
                Any::BigInt64(-42),
                Any::BigInt64(-42),
                Any::False,
                Any::True,
                Any::String("JWST".to_string()),
                Any::String("OctoBase".to_string()),
                Any::Binary(vec![1, 9, 1, 9]),
                Any::Binary(vec![8, 1, 0]),
                Any::Array(vec![Any::True, Any::Integer(42)])
            ]
        );

        assert_eq!(
            vec![("key".to_string(), 10u64.into())]
                .into_iter()
                .collect::<Any>(),
            Any::Object(HashMap::from_iter(vec![(
                "key".to_string(),
                Any::Integer(10)
            )]))
        );

        let any: Any = 10u64.into();
        assert_eq!(
            [any].iter().collect::<Any>(),
            Any::Array(vec![Any::Integer(10)])
        );
    }
}
