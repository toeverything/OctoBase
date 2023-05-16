use super::*;
use byteorder::{BigEndian, ReadBytesExt};
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
    fn get_buffer(&self) -> &Cursor<Vec<u8>>;
    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>>;
    fn read_var_u64(&mut self) -> JwstCodecResult<u64> {
        read_with_cursor(self.get_buffer_mut(), read_var_u64)
    }
    fn read_var_string(&mut self) -> JwstCodecResult<String> {
        read_with_cursor(self.get_buffer_mut(), read_var_string)
    }
    fn read_var_buffer<'a>(&mut self) -> JwstCodecResult<Vec<u8>> {
        read_with_cursor(self.get_buffer_mut(), |i| {
            read_var_buffer(i).map(|(tail, val)| (tail, val.to_vec()))
        })
    }
    fn read_u8(&mut self) -> JwstCodecResult<u8> {
        let buffer = self.get_buffer_mut();
        Ok(buffer
            .read_u8()
            .map_err(|_| JwstCodecError::IncompleteDocument)?)
    }
    fn read_f32_be(&mut self) -> JwstCodecResult<f32> {
        let buffer = self.get_buffer_mut();
        Ok(buffer
            .read_f32::<BigEndian>()
            .map_err(|_| JwstCodecError::IncompleteDocument)?)
    }
    fn read_f64_be(&mut self) -> JwstCodecResult<f64> {
        let buffer = self.get_buffer_mut();
        Ok(buffer
            .read_f64::<BigEndian>()
            .map_err(|_| JwstCodecError::IncompleteDocument)?)
    }
    fn read_i64_be(&mut self) -> JwstCodecResult<i64> {
        let buffer = self.get_buffer_mut();
        Ok(buffer
            .read_i64::<BigEndian>()
            .map_err(|_| JwstCodecError::IncompleteDocument)?)
    }
    fn is_empty(&self) -> bool {
        let buffer = self.get_buffer();
        buffer.position() >= buffer.get_ref().len() as u64
    }
    fn len(&self) -> u64 {
        let buffer = self.get_buffer();
        buffer.get_ref().len() as u64 - buffer.position()
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
    fn get_buffer(&self) -> &Cursor<Vec<u8>> {
        &self.buffer
    }

    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.buffer
    }
}
