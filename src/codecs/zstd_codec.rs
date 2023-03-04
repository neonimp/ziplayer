use std::io::{BufRead, BufReader, Read, Write};
use zstd::Decoder;
use crate::compression_codecs::{CompressionCodec, MemoryStream};
use crate::{Result, ZipError};

pub struct ZstdCodec {
    pub level: i32,
}

impl ZstdCodec {
    pub fn new(level: i32) -> Result<Self> {
        if !zstd::compression_level_range().contains(&level) {
            return Err(ZipError::InvalidCompressionLevel(level));
        }
        Ok(Self { level })
    }
}

impl CompressionCodec for ZstdCodec {

    fn int_id(&self) -> u16 {
        0
    }

    fn compress(&self, data: MemoryStream) -> Result<Vec<u8>> {
        let mut encoder = zstd::Encoder::new(Vec::new(), self.level)?;
        encoder.write_all(data.0)?;
        Ok(encoder.finish()?)
    }

    fn expand(&self, data: MemoryStream) -> Result<Vec<u8>> {
        let mut cursor = std::io::Cursor::new(data.0);
        let mut data_reader = BufReader::new(cursor);
        let mut buf = Vec::with_capacity(data.1);
        let mut decoder = zstd::Decoder::new(data_reader)?;
        decoder.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn streamed_expansion(&self, reader: &mut impl BufRead, writer: &mut impl Write) {
        let mut decoder = Decoder::new(reader).unwrap();
        std::io::copy(&mut decoder, writer).unwrap();
    }
}
