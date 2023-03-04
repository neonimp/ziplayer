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

//! This module contains the C interface for the library.
//! the functions are exported as C symbols and can be used from C/C++.
//! this is an inherently unsafe module, as it is interfacing with C code.


use std::io::{Cursor};

use std::ptr::{null, null_mut};

use libc::{c_char, c_uchar, size_t};

use crate::reader::{ZipEntryInfo, ZipReader};

use crate::ZipError;

#[repr(C)]
pub struct IZipReader<'buff> {
    reader: Option<ZipReader<Cursor<&'buff mut [u8]>>>,
    error: Option<ZipError>,
}

#[repr(C)]
pub struct IZipEntry {
    filename: *const c_uchar,
    filename_len: size_t,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    is_dir: bool,
    offset: u64,
}

/// Opens a zip file from a buffer passed in from C.
/// The buffer is converted to a slice and then to a SeekableCursor.
/// The cursor is then passed to ZipReader::new.
/// If the reader is successfully created, it is boxed and a pointer to it is returned.
/// If the reader is not successfully created, an IZipReader is created with the error field set.
/// The C code can the use the C helper function to get the error.
#[no_mangle]
pub unsafe extern "C" fn zip_open_buffer(buf: &mut c_uchar, buf_len: size_t) -> *mut IZipReader {
    // Convert the buffer to a slice
    let buf = std::slice::from_raw_parts_mut(buf, buf_len);
    // Make a SeekableCursor from the buffer
    let cursor = Cursor::new(buf);
    let reader = match ZipReader::new(cursor) {
        Ok(reader) => Box::new(IZipReader {
            reader: Some(reader),
            error: None,
        }),
        Err(e) => Box::new(IZipReader {
            reader: None,
            error: Some(e),
        }),
    };
    Box::leak(reader)
}

#[no_mangle]
pub unsafe extern "C" fn zip_find_file(reader: *mut IZipReader, filename: *const c_uchar, filename_len: size_t) -> *mut ZipEntryInfo {
    let reader = if !reader.is_null() {
        &mut *reader
    } else { return null_mut(); };

    let filename = if !filename.is_null() {
        std::slice::from_raw_parts(filename, filename_len)
    } else { return null_mut(); };

    let filename = std::str::from_utf8(filename).unwrap();

    if let Some(reader) = &mut reader.reader {
        let finfo = Box::new(reader.file_info(filename).unwrap());
        Box::leak(finfo)
    } else { null_mut() }
}

#[no_mangle]
pub unsafe extern "C" fn zip_close(reader: *mut IZipReader) {
    let reader = if !reader.is_null() {
        Box::from_raw(reader)
    } else { return; };

    // Drop the reader
    drop(reader);
}

#[no_mangle]
#[inline]
pub unsafe extern "C" fn zip_get_error(reader: *const IZipReader) -> *const ZipError {
    let reader = if !reader.is_null() {
        &*reader
    } else { return null(); };
    let error = &reader.error;

    if let Some(error) = error {
        error
    } else { null() }
}

/// Gets the error message from a ZipError.
/// The error message is copied to the buffer passed in from C.
///
/// If the buffer is not large enough, the function returns the required size.]
///
/// If the message is successfully copied, the function returns the size of the message.
///
/// On error the function returns u32::MAX.
#[no_mangle]
#[inline]
pub unsafe extern "C" fn zip_error_get_message(error: *const ZipError, out_buf: *mut c_char, out_max: usize) -> usize {
    let error = if !error.is_null() {
        &*error
    } else { return !0; };
    let error_str = error.to_string();
    let error = error_str.as_bytes();
    let out_buf = if !out_buf.is_null() {
        std::slice::from_raw_parts_mut(out_buf, out_max)
    } else { return !0; };

    if error.len() > out_max {
        return error.len();
    }

    for (i, b) in error.iter().enumerate() {
        out_buf[i] = *b as c_char;
    }

    return out_max;
}
