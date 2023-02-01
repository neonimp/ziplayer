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

use std::ffi::OsString;

/// Describes a file in the zip archive.
#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    pub offset: u64,
    pub version: u16,
    pub flags: u16,
    pub compression: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename: String,
    pub extra_field: Vec<u8>,
    pub data_offset: u64,
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
#[derive(Debug, Clone)]
pub struct CentralDirectory {
    pub offset: u64,
    pub version_made_by: u16,
    pub version_needed_to_extract: u16,
    pub flags: u16,
    pub compression: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename: String,
    pub extra_field: Vec<u8>,
    pub file_comment: Vec<u8>,
    pub disk_number_start: u16,
    pub internal_file_attributes: u16,
    pub external_file_attributes: u32,
    pub relative_offset_of_local_header: u32,
    pub is_directory: bool,
    pub len: u64,
}

/// Very last structure in a zip archive, it has information that
/// helps the reader find the central directory.
#[derive(Debug, Clone)]
pub struct EndOfCentralDirectory {
    pub disk_number: u16,
    pub disk_with_central_directory: u16,
    pub number_of_central_directory_records_on_this_disk: u16,
    pub total_number_of_central_directory_records: u16,
    pub size_of_central_directory: u32,
    pub offset_of_start_of_central_directory: u32,
    pub zip_file_comment: Vec<u8>,
}

pub enum ZipEntry {
    LocalFileHeader(LocalFileHeader),
    CentralDirectory(CentralDirectory),
    EndOfCentralDirectory(EndOfCentralDirectory),
}
