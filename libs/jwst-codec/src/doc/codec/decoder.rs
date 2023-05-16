use super::*;
use byteorder::ReadBytesExt;
use std::io::Cursor;

#[inline]
fn read_with_cursor<T, F>(buffer: &mut Cursor<Vec<u8>>, f: F) -> JwstCodecResult<T>
where
    F: FnOnce(&[u8]) -> IResult<&[u8], T>,
{
    // TODO: use remaining_slice() instead after it is stabilized
    let input = buffer.get_ref();
    let rest_pos = buffer.position().min(input.len() as u64) as usize;
    let input = &input[rest_pos..];

    let (tail, result) = f(input).map_err(|e| e.map_input(|u| u.len()))?;

    buffer.set_position((rest_pos + input.len() - tail.len()) as u64);
    Ok(result)
}

pub trait CrdtReader {
    // basic read functions
    fn get_buffer(&mut self) -> &mut Cursor<Vec<u8>>;
    fn read_var_u64(&mut self) -> JwstCodecResult<u64> {
        read_with_cursor(self.get_buffer(), read_var_u64)
    }
    fn read_var_string(&mut self) -> JwstCodecResult<String> {
        read_with_cursor(self.get_buffer(), read_var_string)
    }
    fn read_u8(&mut self) -> JwstCodecResult<u8> {
        let buffer = self.get_buffer();
        Ok(buffer
            .read_u8()
            .map_err(|_| JwstCodecError::IncompleteDocument)?)
    }

    // ydoc specific read functions
    #[inline(always)]
    fn read_info(&mut self) -> JwstCodecResult<u8> {
        self.read_u8()
    }

    fn read_item_id(&mut self) -> JwstCodecResult<Id> {
        let client = self.read_var_u64()?;
        let clock = self.read_var_u64()?;
        Ok(Id::new(client, clock))
    }

    fn read_content(&mut self, tag_type: u8) -> JwstCodecResult<Content> {
        read_with_cursor(self.get_buffer(), |input| read_content(input, tag_type))
    }
}

// compatible with ydoc v1
pub struct RawDecoder {
    pub(super) buffer: Cursor<Vec<u8>>,
}

impl RawDecoder {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            buffer: Cursor::new(buffer),
        }
    }
}

impl<'d> CrdtReader for RawDecoder {
    fn get_buffer(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.buffer
    }
}
