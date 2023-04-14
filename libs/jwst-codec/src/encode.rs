use byteorder::WriteBytesExt;
use std::io::{Error, Write};

pub fn write_var_u64<W: Write>(buffer: &mut W, mut num: u64) -> Result<(), Error> {
    // bit or 0b1000_0000 pre 7 bit if has more bits
    while num >= 0b10000000 {
        buffer.write_u8(num as u8 & 0b0111_1111 | 0b10000000)?;
        num >>= 7;
    }

    buffer.write_u8((num & 0b01111111) as u8)?;

    Ok(())
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

    #[test]
    fn test_var_i64() {
        let i = -9223372036854775807i64;

        let mut buf1 = Vec::new();
        buf1.write_var(i);

        let mut buf2 = Vec::new();
        write_var_i64(&mut buf2, i).unwrap();

        assert_eq!(buf1, buf2);
    }

    #[test]
    fn test_var_u64() {
        let i = 3371165085774383656u64;

        let mut buf1 = Vec::new();
        buf1.write_var(i);

        let mut buf2 = Vec::new();
        write_var_u64(&mut buf2, i as u64).unwrap();

        assert_eq!(buf1, buf2);
    }
}
