use super::*;
use nom::bytes::complete::take;

pub fn read_var_buffer(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (tail, len) = read_var_u64(input)?;
    let (tail, val) = take(len)(tail)?;
    Ok((tail, val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::{
        error::{Error, ErrorKind},
        Err,
    };

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
}
