/**!
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

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use thiserror::Error;

pub mod structures;
pub mod read;

pub const EOCD_SIG: u32 = 0x06054b50;
pub const EOCD64_SIG: u32 = 0x06064b50;
pub const CD_SIG: u32 = 0x02014b50;
pub const LFH_SIG: u32 = 0x04034b50;
pub const DD_SIG: u32 = 0x08074b50;

#[derive(Debug, Error)]
pub enum ZipError {
    #[error("IO exception: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Invalid signature: {0}")]
    InvalidSignature(u32),
    #[error("Invalid zip file")]
    EndOfCentralDirectoryNotFound,
    #[error("Invalid entry in archive at offset {0}")]
    InvalidEntry(u64),
    #[error("Fatal Error: {0}, {1}")]
    UnknownError(u64, String),
}

impl PartialEq for ZipError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ZipError::IOError(a), ZipError::IOError(b)) => a.kind() == b.kind(),
            (ZipError::InvalidSignature(a), ZipError::InvalidSignature(b)) => a == b,
            (ZipError::EndOfCentralDirectoryNotFound, ZipError::EndOfCentralDirectoryNotFound) => true,
            (ZipError::InvalidEntry(a), ZipError::InvalidEntry(b)) => a == b,
            (ZipError::UnknownError(a, b), ZipError::UnknownError(c, d)) => a == c && b == d,
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, ZipError>;

pub struct ZipObject {
    pub path: OsString,
    pub fptr: File,
    pub files: HashMap<OsString, structures::CentralDirectory>,
}

#[cfg(test)]
mod tests {
    use super::*;
}
