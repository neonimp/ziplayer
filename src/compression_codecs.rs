

use crate::Result;

pub trait CompressionCodec {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn expand(&self, data: &[u8]) -> Result<Vec<u8>>;
}
