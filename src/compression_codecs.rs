use crate::Result;
use std::io::{BufRead, Read, Write};

pub type MemoryStream<'stream> = (&'stream Vec<u8>, usize);

/// Trait for valid compression codecs.
/// Compression codecs are used to compress and decompress data.
pub trait CompressionCodec: Sync + Send {
    /// Returns the int identifier for the compression codec.
    fn int_id(&self) -> u16;
    fn compress(&self, data: MemoryStream) -> Result<Vec<u8>>;
    fn expand(&self, data: MemoryStream) -> Result<Vec<u8>>;

    fn compress_to_writer(&self, data: MemoryStream, writer: &mut dyn Write) -> Result<()> {
        let compressed = self.compress(data)?;
        writer.write_all(&compressed)?;
        Ok(())
    }

    fn expand_from_reader(&self, reader: &mut dyn Read) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        self.expand((&buf, buf.len()))
    }

    fn streamed_expansion(&self, reader: &mut impl BufRead, writer: &mut impl Write);
}

/// No compression codec.
/// Just returns the data as is.
struct NoCompressionCodec;

impl CompressionCodec for NoCompressionCodec {
    fn int_id(&self) -> u16 {
        0
    }

    fn compress(&self, data: MemoryStream) -> Result<Vec<u8>> {
        Ok(data.0.to_vec())
    }

    fn expand(&self, data: MemoryStream) -> Result<Vec<u8>> {
        Ok(data.0.to_vec())
    }

    fn streamed_expansion(&self, reader: &mut impl BufRead, writer: &mut impl Write) {
        std::io::copy(reader, writer).unwrap();
    }
}
