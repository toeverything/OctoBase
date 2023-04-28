use nom::{
    multi::count,
    number::complete::{be_f32, be_f64, be_i64, be_u8},
};

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

fn read_any_item(input: &[u8]) -> IResult<&[u8], Any> {
    let (tail, index) = be_u8(input)?;
    match 127 - index {
        0 => Ok((tail, Any::Undefined)),
        1 => Ok((tail, Any::Null)),
        2 => {
            let (tail, int) = read_var_u64(tail)?;
            Ok((tail, Any::Integer(int)))
        } // Integer
        3 => {
            let (tail, float) = be_f32(tail)?;
            Ok((tail, Any::Float32(float)))
        } // Float32
        4 => {
            let (tail, float) = be_f64(tail)?;
            Ok((tail, Any::Float64(float)))
        } // Float64
        5 => {
            let (tail, int) = be_i64(tail)?;
            Ok((tail, Any::BigInt64(int)))
        } // BigInt64
        6 => Ok((tail, Any::False)),
        7 => Ok((tail, Any::True)),
        8 => {
            let (tail, string) = read_var_string(tail)?;
            Ok((tail, Any::String(string)))
        } // String
        9 => {
            let (tail, len) = read_var_u64(tail)?;
            let (tail, object) = count(read_key_value, len as usize)(tail)?;
            Ok((tail, Any::Object(object.into_iter().collect())))
        } // Object
        10 => {
            let (tail, len) = read_var_u64(tail)?;
            let (tail, any) = count(read_any_item, len as usize)(tail)?;
            Ok((tail, Any::Array(any)))
        } // Array
        11 => {
            let (tail, binary) = read_var_buffer(tail)?;
            Ok((tail, Any::Binary(binary.to_vec())))
        } // Binary
        _ => Ok((tail, Any::Undefined)),
    }
}

pub fn read_key_value(input: &[u8]) -> IResult<&[u8], (String, Any)> {
    let (tail, key) = read_var_string(input)?;
    let (tail, value) = read_any_item(tail)?;

    Ok((tail, (key, value)))
}

pub fn read_any(input: &[u8]) -> IResult<&[u8], Vec<Any>> {
    let (tail, len) = read_var_u64(input)?;
    let (tail, any) = count(read_any_item, len as usize)(tail)?;

    Ok((tail, any))
}
