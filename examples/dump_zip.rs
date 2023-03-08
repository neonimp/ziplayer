use argh::FromArgs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{fs, fs::File, process::exit};
use crc::Crc;
use indicatif::ProgressBar;
use ziplayer::reader::ZipReader;
use ziplayer::structures::CentralDirectory;

/// List or dump the contents of a zip file without decompressing them
#[derive(FromArgs)]
struct Args {
    /// the zip file to dump
    #[argh(positional)]
    filename: String,
    /// dump the files without decompressing them to the given directory
    #[argh(option, short = 'd')]
    dump_to_files: Option<PathBuf>,
    /// list the files in the zip file
    #[argh(switch, short = 'l')]
    list_files: bool,
    /// extract the files to the given directory
    /// (this will decompress the files)
    #[argh(option, short = 'x')]
    extract_to: Option<PathBuf>,
}

fn main() {
    println!("Zip file dumper (c) 2023 neonimp <mxavier[at]neonimp[dot]com>");
    let args: Args = argh::from_env();
    let filename = args.filename;

    let mut file = File::open(filename).unwrap();
    println!("Parsing zip file...");
    let mut zip = ZipReader::new(&mut file).unwrap_or_else(|e| {
        println!("Error: ({:0X}):{}", e.error_code(), e);
        exit(1);
    });

    if args.list_files {
        list_files(&zip);
    }

    if let Some(where_to) = args.dump_to_files {
        dump_files(&mut zip, where_to);
    }

    if let Some(where_to) = args.extract_to {
        // extract_files(&mut zip, where_to);
    }
}

// fn extract_files(zip: &mut ZipReader<&mut File>, where_to: PathBuf) {
//     let index = zip.index().iter()
//         .map(|(p, cd)| (p.to_owned(), cd.to_owned()))
//         .collect::<Vec<(PathBuf, CentralDirectory)>>();
//     if where_to.exists() {
//         fs::remove_dir_all(&where_to).unwrap();
//     }
//
//     if !where_to.exists() {
//         fs::create_dir(&where_to).unwrap();
//     }
//
//     let
// }

fn list_files(zip: &ZipReader<&mut File>) {
    for entry in zip.index().iter() {
        if entry.1.is_directory {
            println!("directory: {:?}", entry.0);
        } else {
            println!(
                "file: {:?}, size: {}, comp.size: {}, comp.method: {}, ratio: {:.2}",
                entry.0,
                entry.1.uncompressed_size,
                entry.1.compressed_size,
                entry.1.compression,
                entry.1.compressed_size as f64 / entry.1.uncompressed_size as f64
            );
        }
    }
}

fn dump_files(zip: &mut ZipReader<&mut File>, where_to: PathBuf) {
    let index = zip.index().iter()
        .map(|(p, cd)| (p.to_owned(), cd.to_owned()))
        .collect::<Vec<(PathBuf, CentralDirectory)>>();
    if where_to.exists() {
        fs::remove_dir_all(&where_to).unwrap();
    }

    if !where_to.exists() {
        fs::create_dir(&where_to).unwrap();
    }
    let pb = ProgressBar::new(index.len() as u64);
    let mut file = File::create(where_to.join("index")).unwrap();
    for entry in index.iter() {
        file.write_all(
            format!("{} {:0X}\n", entry.1.filename.to_str().unwrap(), entry.1.crc32).as_bytes()
        ).unwrap();
    }
    file.flush().unwrap();

    for entry in index.iter() {
        let path = where_to.join(entry.0.to_str().unwrap());
        if entry.1.is_directory {
            fs::create_dir(&path).unwrap();
            continue;
        }

        if !path.parent().unwrap().exists() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
        }

        let mut file = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    panic!("Error: {} {}", e, path.to_str().unwrap());
                } else {
                    panic!("Error: {}", e);
                }
            }
        };
        let buf = zip.dump_file_from_cd(&entry.1).unwrap();
        file.write_all(&buf).unwrap();
        file.flush().unwrap();
        pb.inc(1);
    }
}

fn check_crc<P: AsRef<Path>>(file: P) -> u32 {
    let buffer = &mut [0; 8192];
    let mut file = File::open(file).unwrap();

    let crc = Crc::<u32>::new(&crc::CRC_32_CKSUM);
    let mut digest = crc.digest();
    while let Ok(n) = file.read(buffer) {
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    digest.finalize()
}
