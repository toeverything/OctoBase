use std::io::{Error, Write};
use super::*;
use nom::bytes::complete::take;

pub fn read_var_buffer(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (tail, len) = read_var_u64(input)?;
    let (tail, val) = take(len)(tail)?;
    Ok((tail, val))
}

// TODO remove leading underscore after being used
pub fn _write_var_buffer<W: Write>(buffer: &mut W, data: &[u8]) -> Result<(), Error> {
    write_var_u64(buffer, data.len() as u64)?;
    buffer.write_all(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::{error::{Error, ErrorKind}, Err, AsBytes};
    use rand::{Rng, thread_rng};

    #[test]
    fn test_read_var_buffer() {
        // Test case 1: valid input, buffer length = 5
        let input = [0x05, 0x01, 0x02, 0x03, 0x04, 0x05];
        let expected_output = [0x01, 0x02, 0x03, 0x04, 0x05];
        let result = read_var_buffer(&input);
        assert_eq!(result, Ok((&[][..], &expected_output[..])));

        // Test case 2: truncated input, missing buffer
        let input = [0x05, 0x01, 0x02, 0x03];
        let result = read_var_buffer(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[1..], ErrorKind::Eof)))
        );

        // Test case 3: invalid input
        let input = [0xFF, 0x01, 0x02, 0x03];
        let result = read_var_buffer(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[2..], ErrorKind::Eof)))
        );

        // Test case 4: invalid var int encoding
        let input = [0xFF, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        let result = read_var_buffer(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[7..], ErrorKind::Eof)))
        );
    }

    #[test]
    fn test_var_buf_codec() {
        test_var_buf_enc_dec(&[]);
        test_var_buf_enc_dec(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        test_var_buf_enc_dec(b"test_var_buf_enc_dec");
        let mut rng = thread_rng();
        for _ in 0..100 {
            test_var_buf_enc_dec(generate_rand_bytes(rng.gen::<u16>() as usize).as_bytes());
        }
    }

    fn test_var_buf_enc_dec(data: &[u8]) {
        let mut buf = Vec::<u8>::new();
        _write_var_buffer(&mut buf, data).unwrap();
        let result = read_var_buffer(buf.as_bytes());
        assert_eq!(result, Ok((&[][..], &data[..])));
    }

    fn generate_rand_bytes(len: usize) -> Vec<u8> {
        let mut rng = thread_rng();
        let mut bytes = vec![0u8; len];
        rng.fill(&mut bytes[..]);
        bytes
    }
}
