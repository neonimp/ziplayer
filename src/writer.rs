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

use crate::{CD_SIG, EOCD_SIG, LFH_SIG, Result, ZipError};
use crate::structures::{CentralDirectory, EndOfCentralDirectory, LocalFileHeader, ZipEntry};

use std::collections::BTreeMap;
use std::io::Write;

pub struct ZipWriter<'a, W: Write> {
    writer: &'a mut W,
    entries: BTreeMap<String, ZipEntry>,
    cd: CentralDirectory,
}


