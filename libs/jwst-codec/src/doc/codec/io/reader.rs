use super::*;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Error};

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

#[inline]
fn map_io_error(e: Error) -> JwstCodecError {
    JwstCodecError::IncompleteDocument(e.to_string())
}

pub trait CrdtReader {
    // basic read functions
    fn get_buffer(&self) -> &Cursor<Vec<u8>>;
    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>>;
    fn read_var_u64(&mut self) -> JwstCodecResult<u64> {
        read_with_cursor(self.get_buffer_mut(), read_var_u64)
    }
    fn read_var_i64(&mut self) -> JwstCodecResult<i64> {
        read_with_cursor(self.get_buffer_mut(), read_var_i64)
    }
    fn read_var_string(&mut self) -> JwstCodecResult<String> {
        read_with_cursor(self.get_buffer_mut(), read_var_string)
    }
    fn read_var_buffer(&mut self) -> JwstCodecResult<Vec<u8>> {
        read_with_cursor(self.get_buffer_mut(), |i| {
            read_var_buffer(i).map(|(tail, val)| (tail, val.to_vec()))
        })
    }
    fn read_u8(&mut self) -> JwstCodecResult<u8> {
        self.get_buffer_mut().read_u8().map_err(map_io_error)
    }
    fn read_f32_be(&mut self) -> JwstCodecResult<f32> {
        self.get_buffer_mut()
            .read_f32::<BigEndian>()
            .map_err(map_io_error)
    }
    fn read_f64_be(&mut self) -> JwstCodecResult<f64> {
        self.get_buffer_mut()
            .read_f64::<BigEndian>()
            .map_err(map_io_error)
    }
    fn read_i64_be(&mut self) -> JwstCodecResult<i64> {
        self.get_buffer_mut()
            .read_i64::<BigEndian>()
            .map_err(map_io_error)
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
    fn read_info(&mut self) -> JwstCodecResult<u8>;
    fn read_item_id(&mut self) -> JwstCodecResult<Id>;
}

pub trait CrdtRead<R: CrdtReader> {
    fn read(reader: &mut R) -> JwstCodecResult<Self>
    where
        Self: Sized;
}
