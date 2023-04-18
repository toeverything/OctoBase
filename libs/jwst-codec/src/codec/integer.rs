use super::*;
use byteorder::WriteBytesExt;
use nom::{number::complete::be_u8, Needed};
use std::io::{Error, Write};

fn map_nom_error(e: nom::Err<nom::error::Error<&[u8]>>) -> nom::Err<nom::error::Error<&[u8]>> {
    match e {
        nom::Err::Incomplete(Needed::Size(n)) => {
            nom::Err::Incomplete(Needed::Size(n.saturating_add(1)))
        }
        _ => e,
    }
}

pub fn read_var_u64(input: &[u8]) -> IResult<&[u8], u64> {
    let mut shift = 0;
    let mut result: u64 = 0;
    let mut rest = input;
    loop {
        let (tail, byte) = be_u8(rest).map_err(map_nom_error)?;
        let byte_val = byte as u64;
        result |= (byte_val & 0b0111_1111) << shift;
        if byte_val & 0b1000_0000 == 0 {
            return Ok((tail, result));
        }
        shift += 7;
        rest = tail;
    }
}

pub fn write_var_u64<W: Write>(buffer: &mut W, mut num: u64) -> Result<(), Error> {
    // bit or 0b1000_0000 pre 7 bit if has more bits
    while num >= 0b10000000 {
        buffer.write_u8(num as u8 & 0b0111_1111 | 0b10000000)?;
        num >>= 7;
    }

    buffer.write_u8((num & 0b01111111) as u8)?;

    Ok(())
}

pub fn read_var_i64(i: &[u8]) -> IResult<&[u8], i64> {
    // parse the first byte
    let (i, byte1) = be_u8(i)?;
    // get the sign bit and the first 6 bits of the number
    let sign_bit = (byte1 >> 6) & 0b1;
    let mut num = (byte1 & 0b0011_1111) as i64;

    let mut shift = 6;
    let mut curr_byte = byte1;
    let mut rest = i;
    loop {
        // If the sign bit is set, we need more bits
        if curr_byte & 0b1000_0000 != 0 {
            // Parse the next byte
            let (tail, next_byte) = be_u8(rest).map_err(map_nom_error)?;
            // Add the remaining 7 bits to the number
            num |= ((next_byte & 0b0111_1111) as i64) << shift;
            shift += 7;
            rest = tail;
            curr_byte = next_byte;
        } else {
            break;
        }
    }

    // Negate the number if the sign bit is set
    if sign_bit == 1 {
        num = -num;
    }

    Ok((rest, num))
}

pub fn write_var_i64<W: Write>(buffer: &mut W, mut num: i64) -> Result<(), Error> {
    let is_negative = num < 0;
    num = num.saturating_abs();

    buffer.write_u8(
        // bit or 0b1000_0000 if has more bits
        if num > 0b00111111 { 0b10000000 } else { 0 }
            // bit or 0b0100_0000 if negative
            | if is_negative { 0b0100_0000 } else { 0 }
            // store last 6 bits
            | num as u8 & 0b0011_1111,
    )?;
    num >>= 6;
    while num > 0 {
        buffer.write_u8(
            // bit or 0b1000_0000 pre 7 bit if has more bits
            if num > 0b01111111 { 0b10000000 } else { 0 }
            // store last 7 bits
            | num as u8 & 0b0111_1111,
        )?;
        num >>= 7;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lib0::encoding::Write;

    fn test_var_uint_enc_dec(num: u64) {
        let mut buf1 = Vec::new();
        write_var_u64(&mut buf1, num).unwrap();

        let mut buf2 = Vec::new();
        buf2.write_var(num);

        {
            let (rest, decoded_num) = read_var_u64(&buf1).unwrap();
            assert_eq!(num, decoded_num);
            assert_eq!(rest.len(), 0);
        }

        {
            let (rest, decoded_num) = read_var_u64(&buf2).unwrap();
            assert_eq!(num, decoded_num);
            assert_eq!(rest.len(), 0);
        }
    }

    #[test]
    fn test_var_uint_codec() {
        test_var_uint_enc_dec(0);
        test_var_uint_enc_dec(1);
        test_var_uint_enc_dec(127);
        test_var_uint_enc_dec(0b1000_0000);
        test_var_uint_enc_dec(0b1_0000_0000);
        test_var_uint_enc_dec(0b1_1111_1111);
        test_var_uint_enc_dec(0b10_0000_0000);
        test_var_uint_enc_dec(0b11_1111_1111);
        test_var_uint_enc_dec(0x7fff_ffff_ffff_ffff);
        test_var_uint_enc_dec(u64::max_value());
    }

    fn test_var_int_enc_dec(num: i64) {
        {
            let mut buf1: Vec<u8> = Vec::new();
            write_var_i64(&mut buf1, num).unwrap();

            let (rest, decoded_num) = read_var_i64(&buf1).unwrap();
            assert_eq!(num, decoded_num);
            assert_eq!(rest.len(), 0);
        }

        {
            let mut buf2 = Vec::new();
            buf2.write_var(num);

            let (rest, decoded_num) = read_var_i64(&buf2).unwrap();
            assert_eq!(num, decoded_num);
            assert_eq!(rest.len(), 0);
        }
    }

    #[test]
    fn test_var_int() {
        test_var_int_enc_dec(0);
        test_var_int_enc_dec(1);
        test_var_int_enc_dec(-1);
        test_var_int_enc_dec(63);
        test_var_int_enc_dec(-63);
        test_var_int_enc_dec(64);
        test_var_int_enc_dec(-64);
        test_var_int_enc_dec(i64::MAX);
        // TODO: abs(MIN) cannot be represented as i64
        // this is a limitation of the lib0's var int encoding
        // need to discuss whether to improve the coding method in the future
        // test_var_int_enc_dec(i64::MIN);
        test_var_int_enc_dec(((1 << 40) - 1) * 8);
        test_var_int_enc_dec(-((1 << 40) - 1) * 8);
    }
}
