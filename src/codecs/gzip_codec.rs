use std::io::{BufRead, Write};
use crate::compression_codecs::{CompressionCodec, MemoryStream};

pub struct GzipCodec {
    level: i32,
}

impl GzipCodec {
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl CompressionCodec for GzipCodec {
    fn int_id(&self) -> u16 {
        14
    }

    fn compress(&self, data: MemoryStream) -> crate::Result<Vec<u8>> {
        todo!()
    }

    fn expand(&self, data: MemoryStream) -> crate::Result<Vec<u8>> {
        todo!()
    }

    fn streamed_expansion(&self, reader: &mut impl BufRead, writer: &mut impl Write) {
        todo!()
    }
}
