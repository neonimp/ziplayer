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

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
#[cfg(not(target_os = "windows"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use byteorder::{LittleEndian, ReadBytesExt};
use thiserror::Error;

pub const EOCD_SIG: u32 = 0x06054b50;
pub const EOCD64_SIG: u32 = 0x06064b50;
pub const CD_SIG: u32 = 0x02014b50;

#[derive(Debug, Error)]
pub enum ZipError {
    #[error("IO exception: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Invalid signature: {0}")]
    InvalidSignature(u32),
    #[error("Invalid zip file")]
    EndOfCentralDirectoryNotFound,
}

pub type Result<T> = std::result::Result<T, ZipError>;

pub struct ZipObject {
    pub name: OsString,
    pub fptr: File,
    pub central_directory: Vec<u8>,
}

/// Describes a file in the zip archive.
pub struct LocalFileHeader {
    pub version: u16,
    pub flags: u16,
    pub compression: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename: OsString,
    pub extra_field: Vec<u8>,
}

/// This comes after the file data if the bit 3 in the flags field is set.
/// this means the values for the crc32, compressed_size, and uncompressed_size
/// are stored here instead of in the LocalFileHeader.
pub struct DataDescriptor {
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}

/// Due to the way the zip format is designed, the central directory is
/// placed at the end of the file.
#[derive(Debug)]
pub struct CentralDirectory {
    pub version_made_by: u16,
    pub version_needed_to_extract: u16,
    pub flags: u16,
    pub compression: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename: OsString,
    pub extra_field: Vec<u8>,
    pub file_comment: Vec<u8>,
    pub disk_number_start: u16,
    pub internal_file_attributes: u16,
    pub external_file_attributes: u32,
    pub relative_offset_of_local_header: u32,
}

/// Very last structure in a zip archive, it has information that
/// helps the reader find the central directory.
#[derive(Debug)]
pub struct EndOfCentralDirectory {
    pub disk_number: u16,
    pub disk_with_central_directory: u16,
    pub number_of_central_directory_records_on_this_disk: u16,
    pub total_number_of_central_directory_records: u16,
    pub size_of_central_directory: u32,
    pub offset_of_start_of_central_directory: u32,
    pub zip_file_comment: Vec<u8>,
}

pub fn find_eocd<T: Read + Seek>(data: &mut BufReader<T>) -> Result<EndOfCentralDirectory> {
    let mut sig_candidate;
    let eocd: Option<EndOfCentralDirectory>;

    loop {
        sig_candidate = match data.read_u32::<LittleEndian>(){
            Ok(sig) => sig,
            Err(e) => {
                return if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    Err(ZipError::EndOfCentralDirectoryNotFound)
                } else {
                    Err(ZipError::IOError(e))
                }
            },
        };
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
        }
    }
}

/// Using an eocd, parse the central directory.
pub fn parse_central_dir<T: Read+Seek>(eocd: &EndOfCentralDirectory, data: &mut BufReader<T>) -> Result<CentralDirectory> {
    let sig_candidate;

    data.seek(SeekFrom::Start(eocd.offset_of_start_of_central_directory as u64))?;
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
    // The lenghts are stored here
    let fname_len = data.read_u16::<LittleEndian>()? as usize;
    let extra_len = data.read_u16::<LittleEndian>()? as usize;
    let comment_len = data.read_u16::<LittleEndian>()? as usize;

}

#[cfg(test)]
mod tests {
    use super::*;
}
