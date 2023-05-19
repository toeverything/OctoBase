use super::*;
use byteorder::{BigEndian, WriteBytesExt};
use std::io::{Cursor, Error, Write};

#[inline]
fn map_io_error(e: Error) -> JwstCodecError {
    JwstCodecError::InvalidWriteBuffer(e.to_string())
}

pub trait CrdtWriter {
    // basic write functions
    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>>;
    fn write_var_u64(&mut self, num: u64) -> JwstCodecResult<()> {
        write_var_u64(self.get_buffer_mut(), num).map_err(map_io_error)
    }
    fn write_var_string<S: AsRef<str>>(&mut self, s: S) -> JwstCodecResult<()> {
        write_var_string(self.get_buffer_mut(), s).map_err(map_io_error)
    }
    fn write_var_buffer(&mut self, buf: &[u8]) -> JwstCodecResult<()> {
        write_var_buffer(self.get_buffer_mut(), buf).map_err(map_io_error)
    }
    fn write_u8(&mut self, num: u8) -> JwstCodecResult<()> {
        self.get_buffer_mut().write(&[num]).map_err(map_io_error)?;
        Ok(())
    }
    fn write_f32_be(&mut self, num: f32) -> JwstCodecResult {
        self.get_buffer_mut()
            .write_f32::<BigEndian>(num)
            .map_err(map_io_error)
    }
    fn write_f64_be(&mut self, num: f64) -> JwstCodecResult {
        self.get_buffer_mut()
            .write_f64::<BigEndian>(num)
            .map_err(map_io_error)
    }
    fn write_i64_be(&mut self, num: i64) -> JwstCodecResult {
        self.get_buffer_mut()
            .write_i64::<BigEndian>(num)
            .map_err(map_io_error)
    }

    fn write_info(&mut self, num: u8) -> JwstCodecResult<()>;
    fn write_item_id(&mut self, id: &Id) -> JwstCodecResult<()>;
}

pub trait CrdtWrite<W: CrdtWriter> {
    fn write(&self, writer: &mut W) -> JwstCodecResult<()>
    where
        Self: Sized;
}
