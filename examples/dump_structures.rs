use std::{fs::File, process::exit};
use ziplayer::reader::ZipReader;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage: dump_structures <zipfile>");
        exit(1);
    }
    let filename = &args[1];
    let mut file = File::open(filename).unwrap();
    let zip = ZipReader::new(&mut file).unwrap();

    for entry in zip.index() {
        if entry.1.is_directory {
            println!("directory: {:?}", entry.0);
        } else {
            println!(
                "file: {:?}, size: {}, comp.size: {}, comp.method: {}",
                entry.0, entry.1.uncompressed_size, entry.1.compressed_size, entry.1.compression
            );
        }
    }
}
