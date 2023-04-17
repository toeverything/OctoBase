use super::*;
use nom::combinator::map_res;

pub fn read_var_string(input: &[u8]) -> IResult<&[u8], String> {
    map_res(read_var_buffer, |s| String::from_utf8(s.to_vec()))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::{
        error::{Error, ErrorKind},
        Err,
    };

    #[test]
    fn test_read_var_string() {
        // Test case 1: valid input, string length = 5
        let input = [0x05, 0x68, 0x65, 0x6C, 0x6C, 0x6F];
        let expected_output = "hello".to_string();
        let result = read_var_string(&input);
        assert_eq!(result, Ok((&[][..], expected_output)));

        // Test case 2: missing string length
        let input = [0x68, 0x65, 0x6C, 0x6C, 0x6F];
        let result = read_var_string(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[1..], ErrorKind::Eof)))
        );

        // Test case 3: truncated input
        let input = [0x05, 0x68, 0x65, 0x6C, 0x6C];
        let result = read_var_string(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[1..], ErrorKind::Eof)))
        );

        // Test case 4: invalid input
        let input = [0xFF, 0x01, 0x02, 0x03, 0x04];
        let result = read_var_string(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[2..], ErrorKind::Eof)))
        );

        // Test case 5: invalid var int encoding
        let input = [0xFF, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        let result = read_var_string(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[7..], ErrorKind::Eof)))
        );

        // Test case 6: invalid input, invalid UTF-8 encoding
        let input = [0x05, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let result = read_var_string(&input);
        assert_eq!(
            result,
            Err(Err::Error(Error::new(&input[..], ErrorKind::MapRes)))
        );
    }
}
