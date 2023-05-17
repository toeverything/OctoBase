mod reader;
mod writer;

pub use reader::{CrdtRead, CrdtReader};
pub use writer::{CrdtWrite, CrdtWriter};

use super::*;
use std::io::Cursor;

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

impl CrdtReader for RawDecoder {
    fn get_buffer(&self) -> &Cursor<Vec<u8>> {
        &self.buffer
    }

    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.buffer
    }

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
pub struct RawEncoder {
    pub(super) buffer: Cursor<Vec<u8>>,
}

impl RawEncoder {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            buffer: Cursor::new(buffer),
        }
    }
}

impl CrdtWriter for RawEncoder {
    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.buffer
    }

    // ydoc specific write functions
    #[inline(always)]
    fn write_info(&mut self, num: u8) -> JwstCodecResult<()> {
        self.write_u8(num)
    }

    fn write_item_id(&mut self, id: &Id) -> JwstCodecResult<()> {
        self.write_var_u64(id.client)?;
        self.write_var_u64(id.clock)?;
        Ok(())
    }
}
