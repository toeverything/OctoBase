use super::*;
use std::io::Cursor;

// compatible with ydoc v1
#[derive(Clone)]
pub struct RawDecoder {
    pub(super) buffer: Cursor<Vec<u8>>,
}

impl RawDecoder {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            buffer: Cursor::new(buffer),
        }
    }

    pub fn rest_ref(&self) -> &[u8] {
        let pos = self.buffer.position();
        let buf = self.buffer.get_ref();

        if pos == 0 {
            buf
        } else {
            &buf[(pos as usize).min(buf.len())..]
        }
    }

    pub fn drain(self) -> Vec<u8> {
        let pos = self.buffer.position();
        let mut buf = self.buffer.into_inner();

        if pos == 0 {
            buf
        } else {
            buf.split_off(pos as usize)
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
#[derive(Default)]
pub struct RawEncoder {
    buffer: Cursor<Vec<u8>>,
}

impl RawEncoder {
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer.into_inner()
    }
}

impl CrdtWriter for RawEncoder {
    fn get_buffer_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.buffer
    }

    // ydoc specific write functions
    #[inline(always)]
    fn write_info(&mut self, num: u8) -> JwstCodecResult {
        self.write_u8(num)
    }

    fn write_item_id(&mut self, id: &Id) -> JwstCodecResult {
        self.write_var_u64(id.client)?;
        self.write_var_u64(id.clock)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crdt_reader() {
        {
            let mut reader = RawDecoder::new(vec![0xf2, 0x5]);
            assert_eq!(reader.read_var_u64().unwrap(), 754);
        }
        {
            let buffer = vec![0x5, b'h', b'e', b'l', b'l', b'o'];
            let mut reader = RawDecoder::new(buffer.clone());

            assert_eq!(reader.clone().read_var_string().unwrap(), "hello");
            assert_eq!(
                reader.clone().read_var_buffer().unwrap().as_slice(),
                b"hello"
            );

            assert_eq!(reader.read_u8().unwrap(), 5);
            assert_eq!(reader.read_u8().unwrap(), b'h');
            assert_eq!(reader.read_u8().unwrap(), b'e');
            assert_eq!(reader.read_u8().unwrap(), b'l');
            assert_eq!(reader.read_u8().unwrap(), b'l');
            assert_eq!(reader.read_u8().unwrap(), b'o');
        }
        {
            let mut reader = RawDecoder::new(vec![0x40, 0x49, 0x0f, 0xdb]);
            assert_eq!(reader.read_f32_be().unwrap(), 3.1415927);
        }
        {
            let mut reader = RawDecoder::new(vec![0x40, 0x09, 0x21, 0xfb, 0x54, 0x44, 0x2d, 0x18]);
            assert_eq!(reader.read_f64_be().unwrap(), 3.141592653589793);
        }
        {
            let mut reader = RawDecoder::new(vec![0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
            assert_eq!(reader.read_i64_be().unwrap(), i64::MAX);
        }
        {
            let mut reader = RawDecoder::new(vec![0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
            assert_eq!(reader.read_i64_be().unwrap(), i64::MIN);
        }
        {
            let mut reader = RawDecoder::new(vec![0x80]);
            assert_eq!(reader.read_info().unwrap(), 0x80);
        }
        {
            let mut reader = RawDecoder::new(vec![0x1, 0x2]);
            assert_eq!(reader.read_item_id().unwrap(), Id::new(1, 2));
        }
    }

    #[test]
    fn test_crdt_writer() {
        {
            let mut writer = RawEncoder::default();
            writer.write_var_u64(754).unwrap();
            assert_eq!(writer.into_inner(), vec![0xf2, 0x5]);
        }
        {
            let ret = vec![0x5, b'h', b'e', b'l', b'l', b'o'];
            let mut writer = RawEncoder::default();
            writer.write_var_string("hello").unwrap();
            assert_eq!(writer.into_inner(), ret);

            let mut writer = RawEncoder::default();
            writer.write_var_buffer(b"hello").unwrap();
            assert_eq!(writer.into_inner(), ret);

            let mut writer = RawEncoder::default();
            writer.write_u8(5).unwrap();
            writer.write_u8(b'h').unwrap();
            writer.write_u8(b'e').unwrap();
            writer.write_u8(b'l').unwrap();
            writer.write_u8(b'l').unwrap();
            writer.write_u8(b'o').unwrap();
            assert_eq!(writer.into_inner(), ret);
        }
        {
            let mut writer = RawEncoder::default();
            writer.write_f32_be(3.1415927).unwrap();
            assert_eq!(writer.into_inner(), vec![0x40, 0x49, 0x0f, 0xdb]);
        }
        {
            let mut writer = RawEncoder::default();
            writer.write_f64_be(3.141592653589793).unwrap();
            assert_eq!(
                writer.into_inner(),
                vec![0x40, 0x09, 0x21, 0xfb, 0x54, 0x44, 0x2d, 0x18]
            );
        }
        {
            let mut writer = RawEncoder::default();
            writer.write_i64_be(i64::MAX).unwrap();
            assert_eq!(
                writer.into_inner(),
                vec![0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
            );
        }
        {
            let mut writer = RawEncoder::default();
            writer.write_i64_be(i64::MIN).unwrap();
            assert_eq!(
                writer.into_inner(),
                vec![0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
            );
        }
        {
            let mut writer = RawEncoder::default();
            writer.write_info(0x80).unwrap();
            assert_eq!(writer.into_inner(), vec![0x80]);
        }
        {
            let mut writer = RawEncoder::default();
            writer.write_item_id(&Id::new(1, 2)).unwrap();
            assert_eq!(writer.into_inner(), vec![0x1, 0x2]);
        }
    }
}
