

use crate::Result;

/// Trait for valid compression codecs.
/// Compression codecs are used to compress and decompress data.
pub trait CompressionCodec {
    /// Returns the int identifier for the compression codec.
    fn int_id(&self) -> u16;
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn expand(&self, data: &[u8]) -> Result<Vec<u8>>;
}

/// No compression codec.
/// Just returns the data as is.
struct NoCompressionCodec;

impl CompressionCodec for NoCompressionCodec {
    fn int_id(&self) -> u16 {
        0
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn expand(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}
