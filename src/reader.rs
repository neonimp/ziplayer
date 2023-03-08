/*
   Zip file reader and writer, in pure Rust.
   Copyright (C) 2022 Matheus Xavier <mxavier@neonimp.com>

   This program is free software: you can redistribute it and/or modify
   it under the terms of the GNU Lesser General Public License as published by
   the Free Software Foundation, either version 3 of the License, or
   (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
   GNU General Public License for more details.

   You should have received a copy of the GNU Lesser General Public License
   along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use neoncore::int_util::Endianness::LittleEndian;
use neoncore::int_util::StreamReadInt;
#[cfg(feature = "multi-thread")]
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::compression_codecs::CompressionCodec;
use crate::structures::{CentralDirectory, EndOfCentralDirectory, EndOfCentralDirectory64, LocalFileHeader, ZipEntry};
use crate::{Result, ZipError, CD_SIG, EOCD_SIG, LFH_SIG, EOCD64_SIG};

pub struct ZipIndex(BTreeMap<PathBuf, CentralDirectory>);

impl ZipIndex {
    pub fn new(map: BTreeMap<PathBuf, CentralDirectory>) -> Self {
        ZipIndex(map)
    }

    pub fn files(&self) -> impl Iterator<Item = &CentralDirectory> {
        self.0.iter().filter_map(
            |(_path, info)| {
                if !info.is_directory {
                    Some(info)
                } else {
                    None
                }
            },
        )
    }

    pub fn dirs(&self) -> impl Iterator<Item = &CentralDirectory> {
        self.0.iter().filter_map(
            |(_path, info)| {
                if info.is_directory {
                    Some(info)
                } else {
                    None
                }
            },
        )
    }

    pub fn get(&self, path: &Path) -> Option<&CentralDirectory> {
        self.0.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, info: CentralDirectory) -> Option<CentralDirectory> {
        self.0.insert(path, info)
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.0.contains_key(path)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Path, &CentralDirectory)> {
        self.0.iter().map(|(path, info)| (path.as_path(), info))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Path> {
        self.0.keys().map(|path| path.as_path())
    }

    pub fn values(&self) -> impl Iterator<Item = &CentralDirectory> {
        self.0.values()
    }

    pub fn into_keys(self) -> impl Iterator<Item = PathBuf> {
        self.0.into_keys()
    }

    pub fn into_values(self) -> impl Iterator<Item = CentralDirectory> {
        self.0.into_values()
    }
}

impl IntoIterator for ZipIndex {
    type Item = (PathBuf, CentralDirectory);
    type IntoIter = std::collections::btree_map::IntoIter<PathBuf, CentralDirectory>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub struct ZipReader<R: Read + Seek> {
    reader: BufReader<R>,
    index: ZipIndex,
    is_zip64: bool,
}

pub struct ZipEntryInfo {
    pub name: PathBuf,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub is_compressed: bool,
    pub size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub compression_method: u16,
    pub last_modified: u32,
    pub last_accessed: u32,
    pub comment: Option<String>,
    pub offset: u64,
}

impl ZipEntryInfo {
    pub(crate) fn from_central_dir(entry: &CentralDirectory) -> Self {
        ZipEntryInfo {
            name: entry.filename.clone(),
            is_dir: entry.external_file_attributes & 0x10 == 0x10,
            is_file: entry.external_file_attributes & 0x20 == 0x20,
            is_symlink: entry.external_file_attributes & 0x40000000 == 0x40000000,
            is_compressed: entry.compression != 0,
            size: entry.uncompressed_size as u64,
            compressed_size: entry.compressed_size as u64,
            crc32: entry.crc32,
            compression_method: entry.compression,
            last_modified: entry.last_mod_date as u32,
            last_accessed: entry.last_mod_date as u32,
            offset: entry.local_header_rel_offset as u64,
            comment: None,
        }
    }
}

fn find_next_signature<R: Read + Seek>(
    reader: &mut R,
    signature: u32,
    hint: Option<u64>,
) -> Result<u64> {
    let offset;
    let start_pos = reader.stream_position()?;
    // Split the signature into top and bottom u16s
    let sig_lower = (signature & 0x0000FFFF) as u16;
    let sig_upper = ((signature & 0xFFFF0000) >> 16) as u16;
    let sig_fbyte = (signature & 0x000000FF) as u8;

    if let Some(hint) = hint {
        reader.seek(SeekFrom::Start(hint))?;
    }

    // Scan byte by byte until we find the first byte of the signature
    loop {
        let mut byte = [0u8];
        let mut next_byte = [0u8];
        reader.read_exact(&mut byte)?;
        if byte[0] == sig_fbyte {
            // If the first byte matches, read the next byte and check if it matches the lower
            // u16 of the signature
            reader.read_exact(&mut next_byte)?;
            let lower = ((next_byte[0] as u16) << 8) | byte[0] as u16;
            if lower == sig_lower {
                // get the upper u16 of the signature candidate
                let upper = reader.read_u16(LittleEndian)?;
                if upper == sig_upper {
                    // If the upper u16 matches, we found the signature
                    offset = reader.stream_position()? - 4;
                    break;
                }
            }
        }
    }

    // Rewind the reader to the original position
    reader.seek(SeekFrom::Start(start_pos))?;
    Ok(offset)
}

fn find_eocd<T: Read + Seek>(data: &mut BufReader<T>) -> Result<EndOfCentralDirectory> {
    let eocd: Option<EndOfCentralDirectory>;
    let offset = match find_next_signature(data, EOCD_SIG, None) {
        Ok(offset) => offset,
        Err(_) => return Err(ZipError::EndOfCentralDirectoryNotFound),
    };
    data.seek(SeekFrom::Start(offset))?;
    let sig_candidate = data.read_u32(LittleEndian)?;

    if sig_candidate == EOCD_SIG {
        eocd = Some(EndOfCentralDirectory {
            disk_number: data.read_u16(LittleEndian)?,
            disk_with_central_directory: data.read_u16(LittleEndian)?,
            number_of_central_directory_records_on_this_disk: data.read_u16(LittleEndian)?,
            total_number_of_central_directory_records: data.read_u16(LittleEndian)?,
            size_of_central_directory: data.read_u32(LittleEndian)?,
            offset_of_start_of_central_directory: data.read_u32(LittleEndian)?,
            zip_file_comment: {
                let mut buf = vec![0u8; data.read_u16(LittleEndian)? as usize];
                data.read_exact(&mut buf)?;
                buf
            },
        });
        Ok(eocd.unwrap())
    } else {
        Err(ZipError::EndOfCentralDirectoryNotFound)
    }
}

/// Using an eocd, parse the central directory.
fn parse_central_dir<T: Read + Seek>(
    data: &mut BufReader<T>,
    offset: u64,
) -> Result<CentralDirectory> {
    data.seek(SeekFrom::Start(offset))?;
    let sig_candidate = data.read_u32(LittleEndian)?;
    if sig_candidate != CD_SIG {
        return Err(ZipError::InvalidSignature(sig_candidate));
    }

    let version_made_by = data.read_u16(LittleEndian)?;
    let version_needed_to_extract = data.read_u16(LittleEndian)?;
    let flags = data.read_u16(LittleEndian)?;
    let compression = data.read_u16(LittleEndian)?;
    let last_mod_time = data.read_u16(LittleEndian)?;
    let last_mod_date = data.read_u16(LittleEndian)?;
    let crc32 = data.read_u32(LittleEndian)?;
    let compressed_size = data.read_u32(LittleEndian)?;
    let uncompressed_size = data.read_u32(LittleEndian)?;
    // The lengths are stored here but the data is at the end of the structure.
    let fname_len = data.read_u16(LittleEndian)? as usize;
    let extra_len = data.read_u16(LittleEndian)? as usize;
    let comment_len = data.read_u16(LittleEndian)? as usize;
    let disk_number_start = data.read_u16(LittleEndian)?;
    let internal_file_attributes = data.read_u16(LittleEndian)?;
    let external_file_attributes = data.read_u32(LittleEndian)?;
    let relative_offset_of_local_header = data.read_u32(LittleEndian)?;
    let filename = {
        let mut buf = vec![0u8; fname_len];
        data.read_exact(&mut buf)?;
        PathBuf::from(String::from_utf8(buf)?)
    };
    let extra_field = {
        let mut buf = vec![0u8; extra_len];
        data.read_exact(&mut buf)?;
        buf
    };
    let file_comment = {
        let mut buf = vec![0u8; comment_len];
        data.read_exact(&mut buf)?;
        buf
    };
    let len = data.stream_position()? - offset;
    let is_directory = uncompressed_size == 0;

    Ok(CentralDirectory {
        offset,
        version_made_by,
        version_needed_to_extract,
        flags,
        compression,
        last_mod_time,
        last_mod_date,
        crc32,
        compressed_size,
        uncompressed_size,
        filename,
        extra_field,
        file_comment,
        disk_number_start,
        internal_file_attributes,
        external_file_attributes,
        local_header_rel_offset: relative_offset_of_local_header,
        is_directory,
        len,
    })
}

/// Parse a local file header.
/// the offset is relative to the start of the file.
fn parse_header<T: Read + Seek>(data: &mut BufReader<T>, offset: u64) -> Result<LocalFileHeader> {
    // Rewind the reader
    data.seek(SeekFrom::Start(offset))?;

    let sig_candidate = match data.read_u32(LittleEndian) {
        Ok(sig) => sig,
        Err(e) => {
            return if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Err(ZipError::InvalidEntry(offset))
            } else {
                Err(ZipError::IOError(e))
            };
        }
    };

    if sig_candidate == LFH_SIG {
        let offset = data.stream_position()? - 4;
        let version = data.read_u16(LittleEndian)?;
        let flags = data.read_u16(LittleEndian)?;
        let compression = data.read_u16(LittleEndian)?;
        let last_mod_time = data.read_u16(LittleEndian)?;
        let last_mod_date = data.read_u16(LittleEndian)?;
        let mut crc32 = 0;
        let mut compressed_size = 0;
        let mut uncompressed_size = 0;
        // Do we need to look for the data descriptor?
        if flags & 1 << 3 == 0 {
            crc32 = data.read_u32(LittleEndian)?;
            compressed_size = data.read_u32(LittleEndian)?;
            uncompressed_size = data.read_u32(LittleEndian)?;
        }
        let fname_len = data.read_u16(LittleEndian)? as usize;
        let extra_len = data.read_u16(LittleEndian)? as usize;
        let filename = {
            let mut buf = vec![0u8; fname_len];
            data.read_exact(&mut buf)?;
            PathBuf::from(String::from_utf8(buf)?)
        };
        let extra_field = {
            let mut buf = vec![0u8; extra_len];
            data.read_exact(&mut buf)?;
            buf
        };
        let data_offset = data.stream_position()?;

        Ok(LocalFileHeader {
            offset,
            version,
            flags,
            compression,
            last_mod_time,
            last_mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            filename,
            extra_field,
            data_offset,
        })
    } else {
        Err(ZipError::InvalidSignature(sig_candidate))
    }
}

pub fn index_archive<R: Read + Seek>(
    reader: &mut BufReader<R>,
    hint: Option<u64>,
) -> Result<ZipIndex> {
    let mut index = BTreeMap::new();
    let mut hint = hint.unwrap_or(0);

    reader.rewind()?;

    loop {
        let offset = match find_next_signature(reader, CD_SIG, Some(hint)) {
            Ok(offset) => offset,
            Err(e) => {
                return if e
                    == ZipError::IOError(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
                {
                    Ok(ZipIndex::new(index))
                } else {
                    Err(e)
                };
            }
        };

        let header = parse_central_dir(reader, offset)?;
        hint += header.len;
        index.insert(header.filename.clone(), header);
    }
}

/// Dump the file as it's stored in the zip file.
pub fn dump_file<T: Read + Seek>(
    data: &mut BufReader<T>,
    CentralDirectory {
        local_header_rel_offset,
        compressed_size,
        ..
    }: &CentralDirectory,
) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; *compressed_size as usize];
    data.seek(SeekFrom::Start(*local_header_rel_offset as u64))?;
    let header = parse_header(data, *local_header_rel_offset as u64)?;
    data.seek(SeekFrom::Start(header.data_offset))?;
    data.read_exact(&mut buf)?;
    Ok(buf)
}

/// Get the local file header for a file from a central directory entry.
pub fn get_local_file_header<T: Read + Seek>(
    data: &mut BufReader<T>,
    CentralDirectory {
        local_header_rel_offset: relative_offset_of_local_header,
        ..
    }: &CentralDirectory,
) -> Result<LocalFileHeader> {
    data.seek(SeekFrom::Start(*relative_offset_of_local_header as u64))?;
    parse_header(data, *relative_offset_of_local_header as u64)
}

/// Extract a file from `reader` to `where_to` using `codec` and the info in `cd`.
pub fn extract_file<R, P>(
    reader: &mut R,
    cd: &CentralDirectory,
    where_to: P,
    codec: &mut impl CompressionCodec,
) -> Result<()>
where
    R: Read + Seek,
    P: AsRef<Path>,
{
    let mut reader = BufReader::new(reader);
    let where_to = where_to.as_ref();
    let dest_path = where_to.join(&cd.filename);
    if !where_to.exists() {
        return Err(ZipError::IOError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Destination path does not exist",
        )));
    }

    if dest_path.exists() {
        return Err(ZipError::IOError(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "File already exists",
        )));
    }

    let mut file = File::create(&dest_path)?;

    if codec.int_id() != cd.compression {
        return Err(ZipError::MismatchedCompressionMethod(
            cd.compression,
            codec.int_id(),
        ));
    }
    codec.streamed_expansion(&mut reader, &mut file);
    Ok(())
}

impl<R: Read + Seek> ZipReader<R> {
    /// Read and index a ZIP archive.
    pub fn new(reader: R) -> Result<ZipReader<R>> {
        let mut reader = BufReader::new(reader);
        let eocd = find_eocd(&mut reader)?;
        let index = index_archive(
            &mut reader,
            Some(eocd.offset_of_start_of_central_directory as u64),
        )?;

        Ok(ZipReader { reader, index, is_zip64: false })
    }

    /// Dump a file from the archive, without decompressing it.
    pub fn dump_file<T: AsRef<Path>>(&mut self, filename: &T) -> Result<Vec<u8>> {
        let entry = self
            .index
            .get(filename.as_ref())
            .ok_or(ZipError::EntryNotFound(filename.as_ref().into()))?;
        dump_file(&mut self.reader, entry)
    }

    /// Dump a file from the archive, without decompressing it from a central directory entry.
    pub fn dump_file_from_cd(&mut self, cd: &CentralDirectory) -> Result<Vec<u8>> {
        dump_file(&mut self.reader, cd)
    }

    /// Get the index of the archive.
    pub fn index(&self) -> &ZipIndex {
        &self.index
    }

    pub fn file_info<T: AsRef<Path>>(&self, filename: &T) -> Result<ZipEntryInfo> {
        let entry = self
            .index
            .get(filename.as_ref())
            .ok_or(ZipError::EntryNotFound(filename.as_ref().into()))?;
        Ok(ZipEntryInfo::from_central_dir(entry))
    }

    /// Extract a file from the archive.
    pub fn extract_file<T: AsRef<Path>>(
        &mut self,
        filename: &T,
        codec: &mut impl CompressionCodec,
    ) -> Result<Vec<u8>> {
        let entry = self
            .index
            .get(filename.as_ref())
            .ok_or(ZipError::EntryNotFound(filename.as_ref().into()))?
            .clone();
        self.extract_data_from_cd(&entry, codec)
    }

    /// Internal function to extract data from a central directory entry.
    /// This is used by `extract_file` and `extract_all_files`.
    fn extract_data_from_cd(
        &mut self,
        cd: &CentralDirectory,
        codec: &mut impl CompressionCodec,
    ) -> Result<Vec<u8>> {
        if cd.compression != codec.int_id() {
            return Err(ZipError::MismatchedCompressionMethod(
                cd.compression,
                codec.int_id(),
            ));
        }
        let data = dump_file(&mut self.reader, cd)?;
        codec.expand((&data, data.len()))
    }

    /// Extract all files to the given directory.
    pub fn extract_all_files<T: AsRef<Path>>(
        &mut self,
        dir: &T,
        codec: &mut impl CompressionCodec,
    ) -> Result<()> {
        let files = self
            .index
            .files()
            .cloned()
            .collect::<Vec<CentralDirectory>>();
        self.build_directories(dir)?;
        for file in files {
            extract_file(&mut self.reader, &file, dir, codec)?;
        }

        Ok(())
    }

    fn build_directories<T: AsRef<Path>>(&mut self, base: &T) -> Result<()> {
        let dirs = self
            .index
            .dirs()
            .cloned()
            .collect::<Vec<CentralDirectory>>();
        for dir in dirs {
            let mut path = base.as_ref().to_path_buf();
            path.push(&dir.filename);
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Test that we can find the EOCD signature. when it's aligned, this is the best case scenario.
    #[test]
    fn test_find_sig_aligned() {
        // Generated by wxHexEditor //
        let data: [u8; 168] = [
            0x00, 0x2F, 0x6D, 0x61, 0x78, 0x5F, 0x73, 0x69, 0x7A, 0x65, 0x2E, 0x72, 0x73, 0x55,
            0x54, 0x05, 0x00, 0x01, 0xA9, 0xBA, 0xEE, 0x63, 0x50, 0x4B, 0x01, 0x02, 0x00, 0x00,
            0x0A, 0x00, 0x00, 0x00, 0x08, 0x00, 0xC8, 0x7A, 0x50, 0x56, 0xDB, 0x87, 0xEE, 0xBA,
            0x1A, 0x02, 0x00, 0x00, 0x8C, 0x09, 0x00, 0x00, 0x1D, 0x00, 0x09, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF5, 0xEC, 0x00, 0x00, 0x70, 0x6F,
            0x73, 0x74, 0x63, 0x61, 0x72, 0x64, 0x2D, 0x6D, 0x61, 0x69, 0x6E, 0x2F, 0x74, 0x65,
            0x73, 0x74, 0x73, 0x2F, 0x73, 0x63, 0x68, 0x65, 0x6D, 0x61, 0x2E, 0x72, 0x73, 0x55,
            0x54, 0x05, 0x00, 0x01, 0xA9, 0xBA, 0xEE, 0x63, 0x50, 0x4B, 0x05, 0x06, 0x00, 0x00,
            0x00, 0x00, 0x2C, 0x00, 0x2C, 0x00, 0x82, 0x0E, 0x00, 0x00, 0x53, 0xEF, 0x00, 0x00,
            0x28, 0x00, 0x61, 0x31, 0x63, 0x33, 0x61, 0x66, 0x34, 0x37, 0x61, 0x65, 0x63, 0x34,
            0x33, 0x33, 0x61, 0x34, 0x30, 0x30, 0x62, 0x39, 0x38, 0x37, 0x31, 0x38, 0x64, 0x36,
            0x37, 0x65, 0x32, 0x62, 0x38, 0x38, 0x33, 0x61, 0x36, 0x36, 0x38, 0x64, 0x37, 0x37,
        ];

        let mut reader = Cursor::new(data);
        let mut buf_reader = BufReader::new(&mut reader);
        let eocd = find_next_signature(&mut buf_reader, EOCD_SIG, None).unwrap();
        println!("EOCD: {}", eocd);
        assert_eq!(eocd, 0x6A);
    }
}
