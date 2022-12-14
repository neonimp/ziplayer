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

use crate::structures::{CentralDirectory, EndOfCentralDirectory, LocalFileHeader, ZipEntry};
use crate::{Result, ZipError, CD_SIG, EOCD_SIG, LFH_SIG};
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{BufReader, Read, Seek, SeekFrom};
#[cfg(not(target_os = "windows"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

fn find_next_signature<R: Read + Seek>(
    reader: &mut R,
    signature: u32,
    hint: Option<u64>,
) -> Result<u64> {
    let mut lower = [0u8; 2];
    let mut top = [0u8; 2];
    let offset;
    let start_pos = reader.stream_position()?;

    if let Some(hint) = hint {
        reader.seek(SeekFrom::Start(hint))?;
    }

    loop {
        // Look for the lower 2 bytes of the signature first
        reader.read_exact(&mut lower)?;
        if lower == signature.to_le_bytes()[..2] {
            // If we found it, look for the top 2 bytes
            reader.read_exact(&mut top)?;
            if top == signature.to_le_bytes()[2..] {
                offset = reader.stream_position()? - 4;
                // Rewind to the start position
                reader.seek(SeekFrom::Start(start_pos))?;
                return Ok(offset);
            }
        }
    }
}

pub fn find_eocd<T: Read + Seek>(data: &mut BufReader<T>) -> Result<EndOfCentralDirectory> {
    let sig_candidate;
    let eocd: Option<EndOfCentralDirectory>;

    let offset = find_next_signature(data, EOCD_SIG, None)?;
    data.seek(SeekFrom::Start(offset))?;
    sig_candidate = data.read_u32::<LittleEndian>()?;

    if sig_candidate == EOCD_SIG {
        eocd = Some(EndOfCentralDirectory {
            disk_number: data.read_u16::<LittleEndian>()?,
            disk_with_central_directory: data.read_u16::<LittleEndian>()?,
            number_of_central_directory_records_on_this_disk: data.read_u16::<LittleEndian>()?,
            total_number_of_central_directory_records: data.read_u16::<LittleEndian>()?,
            size_of_central_directory: data.read_u32::<LittleEndian>()?,
            offset_of_start_of_central_directory: data.read_u32::<LittleEndian>()?,
            zip_file_comment: {
                let mut buf = vec![0u8; data.read_u16::<LittleEndian>()? as usize];
                data.read_exact(&mut buf)?;
                buf
            },
        });
        return Ok(eocd.unwrap());
    } else {
        return Err(ZipError::EndOfCentralDirectoryNotFound);
    }
}

/// Using an eocd, parse the central directory.
pub fn parse_central_dir<T: Read + Seek>(
    data: &mut BufReader<T>,
    offset: u64,
) -> Result<CentralDirectory> {
    let sig_candidate;

    data.seek(SeekFrom::Start(offset))?;
    sig_candidate = data.read_u32::<LittleEndian>()?;
    if sig_candidate != CD_SIG {
        return Err(ZipError::InvalidSignature(sig_candidate));
    }

    let version_made_by = data.read_u16::<LittleEndian>()?;
    let version_needed_to_extract = data.read_u16::<LittleEndian>()?;
    let flags = data.read_u16::<LittleEndian>()?;
    let compression = data.read_u16::<LittleEndian>()?;
    let last_mod_time = data.read_u16::<LittleEndian>()?;
    let last_mod_date = data.read_u16::<LittleEndian>()?;
    let crc32 = data.read_u32::<LittleEndian>()?;
    let compressed_size = data.read_u32::<LittleEndian>()?;
    let uncompressed_size = data.read_u32::<LittleEndian>()?;
    // The lengths are stored here but the data is at the end of the structure.
    let fname_len = data.read_u16::<LittleEndian>()? as usize;
    let extra_len = data.read_u16::<LittleEndian>()? as usize;
    let comment_len = data.read_u16::<LittleEndian>()? as usize;
    let disk_number_start = data.read_u16::<LittleEndian>()?;
    let internal_file_attributes = data.read_u16::<LittleEndian>()?;
    let external_file_attributes = data.read_u32::<LittleEndian>()?;
    let relative_offset_of_local_header = data.read_u32::<LittleEndian>()?;
    let filename = {
        let mut buf = vec![0u8; fname_len];
        data.read_exact(&mut buf)?;
        OsStr::from_bytes(&buf).to_os_string()
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
    let is_directory = compressed_size == 0 && uncompressed_size == 0;

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
        relative_offset_of_local_header,
        is_directory,
        len,
    })
}

/// Parse a local file header.
/// the offset is relative to the start of the file.
pub fn parse_header<T: Read + Seek>(
    data: &mut BufReader<T>,
    offset: u64,
) -> Result<LocalFileHeader> {
    let sig_candidate;

    // Rewind the reader
    data.seek(SeekFrom::Start(offset))?;

    sig_candidate = match data.read_u32::<LittleEndian>() {
        Ok(sig) => sig,
        Err(e) => {
            return if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Err(ZipError::InvalidEntry(offset))
            } else {
                Err(ZipError::IOError(e))
            }
        }
    };

    if sig_candidate == LFH_SIG {
        let offset = data.stream_position()? - 4;
        let version = data.read_u16::<LittleEndian>()?;
        let flags = data.read_u16::<LittleEndian>()?;
        let compression = data.read_u16::<LittleEndian>()?;
        let last_mod_time = data.read_u16::<LittleEndian>()?;
        let last_mod_date = data.read_u16::<LittleEndian>()?;
        let mut crc32 = !0;
        let mut compressed_size = !0;
        let mut uncompressed_size = !0;
        // Do we need to look for the data descriptor?
        if flags & 1 << 3 == 0 {
            crc32 = data.read_u32::<LittleEndian>()?;
            compressed_size = data.read_u32::<LittleEndian>()?;
            uncompressed_size = data.read_u32::<LittleEndian>()?;
        }
        let fname_len = data.read_u16::<LittleEndian>()? as usize;
        let extra_len = data.read_u16::<LittleEndian>()? as usize;
        let filename = {
            let mut buf = vec![0u8; fname_len];
            data.read_exact(&mut buf)?;
            OsStr::from_bytes(&buf).to_os_string()
        };
        let extra_field = {
            let mut buf = vec![0u8; extra_len];
            data.read_exact(&mut buf)?;
            buf
        };
        let data_offset = data.stream_position()?;

        return Ok(LocalFileHeader {
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
        });
    } else {
        return Err(ZipError::InvalidSignature(sig_candidate));
    }
}

pub fn index_archive<R: Read + Seek>(
    reader: &mut BufReader<R>,
    hint: Option<u64>,
) -> Result<HashMap<OsString, CentralDirectory>> {
    let mut index = HashMap::new();
    let mut hint = hint.unwrap_or(0);

    reader.seek(SeekFrom::Start(0))?;

    loop {
        let offset = match find_next_signature(reader, CD_SIG, Some(hint)) {
            Ok(offset) => offset,
            Err(e) => {
                if e == ZipError::IOError(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)) {
                    return Ok(index);
                } else {
                    return Err(e);
                }
            }
        };

        let header = parse_central_dir(reader, offset)?;
        hint = hint + header.len;
        index.insert(header.filename.clone(), header);
    }
}

/// Index the archive forcibly.
///
/// This can be used to try to forcibly index a corrupt archive,
/// however it is not guaranteed to work, thus you should 
/// not rely on unwrapping the result.
///
/// This method MUST ONLY be used as a last resort.
/// 
/// 
pub fn intensive_index_archive<R: Read + Seek>(
    reader: &mut BufReader<R>,
) -> Result<HashMap<OsString, ZipEntry>> {
    let mut lh_index = HashMap::new();
    let cd_index;

    reader.seek(SeekFrom::Start(0))?;

    // Try to index by central directory first
    cd_index = index_archive(reader, None)?;

    // Now try to index by local file headers, this is against the spec
    // and has a somewhat high chance of false positives.
    loop {
        let offset = match find_next_signature(reader, LFH_SIG, None) {
            Ok(offset) => offset,
            Err(e) => {
                if e == ZipError::IOError(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)) {
                    break;
                } else {
                    return Err(e);
                }
            }
        };

        let header = parse_header(reader, offset)?;
        lh_index.insert(header.filename.clone(), header);
    }

    // Now we have two indexes, we need to merge them.
    let mut index = HashMap::new();
    for (filename, cd_header) in cd_index {
        index.insert(filename.clone(), ZipEntry::CentralDirectory(cd_header));
    }

    for (filename, lh_header) in lh_index {
        if index.contains_key(&filename) {
            continue;
        }

        index.insert(filename.clone(), ZipEntry::LocalFileHeader(lh_header));
    }

    Ok(index)
}

/// Dump the file as it's stored in the zip file.
pub fn dump_file<T: Read + Seek>(
    data: &mut BufReader<T>,
    file: &CentralDirectory,
) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; file.compressed_size as usize];
    data.seek(SeekFrom::Start(file.relative_offset_of_local_header as u64))?;
    let header = parse_header(data, file.relative_offset_of_local_header as u64)?;
    data.seek(SeekFrom::Start(header.data_offset))?;
    data.read_exact(&mut buf)?;
    Ok(buf)
}
