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

use std::ffi::c_void;
use std::fs::File;
use libc::{size_t, c_uchar};
use std::io::{Seek, Cursor, BufReader, Read};
use std::mem::size_of;
use crate::reader::ZipReader;
use crate::ZipError;

#[repr(C)]
pub struct IZipReader<'c_buf, T: Read + Seek> {
    reader: Option<ZipReader<&'c_buf mut T>>,
    error: Option<ZipError>,
}

#[no_mangle]
/// Opens a zip file from a buffer passed in from C.
/// The buffer is converted to a slice and then to a SeekableCursor.
/// The cursor is then passed to ZipReader::new.
/// If the reader is successfully created, it is boxed and a pointer to it is returned.
/// If the reader is not successfully created, an IZipReader is created with the error field set.
/// The C code can the use the C helper function to get the error.
unsafe extern "C" fn zip_open_buffer(buf: &mut c_uchar, buf_len: size_t) -> *mut c_void {
    // Convert the buffer to a slice
    let buf = std::slice::from_raw_parts_mut(buf, buf_len);
    // Make a SeekableCursor from the buffer
    let mut cursor = Cursor::new(buf);
    let mut reader = match ZipReader::new(&mut cursor) {
        Ok(reader) => IZipReader {
            reader: Some(reader),
            error: None,
        },
        Err(e) => IZipReader {
            reader: None,
            error: Some(e),
        },
    };
    std::mem::forget(reader);
    std::mem::transmute(&mut reader)
}

#[no_mangle]
unsafe extern "C" fn zip_close(reader: *mut IZipReader<BufReader<File>>) {
    let reader = if !reader.is_null() {
        Box::from_raw(reader)
    } else { return; };

    // Drop the reader
    drop(reader);
}

#[no_mangle]
#[inline]
/// Returns the size required by `IZipReader<Cursor<&mut [u8]>>`.
unsafe extern "C" fn zip_req_buf_len() -> usize {
    size_of::<IZipReader<Cursor<&mut [u8]>>>()
}
