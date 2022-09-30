use std::{fs::File, process::exit};
use ziplayer::read::{find_eocd, index_archive};

fn main() {
    let files;

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage: dump_structures <zipfile>");
        exit(1);
    }
    let filename = &args[1];
    let file = File::open(filename).unwrap();
    let mut reader = std::io::BufReader::new(&file);
    let eocd = find_eocd(&mut reader);

    if let Ok(eocd) = eocd {
        println!(" EOCD: \n{:#X?}", eocd);
        files = index_archive(
            &mut reader,
            Some(eocd.offset_of_start_of_central_directory as u64),
        )
        .unwrap();
    } else {
        println!("No EOCD found, this usually means the file is not a zip file or is corrupted.");
        println!("The tool will try to find entries anyway. The results may be incomplete, or incorrect.");
        files = index_archive(&mut reader, None).unwrap();
    }

    println!("Found {:X} entries", files.len());

    let mut dc = 0;
    let mut fc = 0;

    for file in files {
        if file.1.is_directory {
            println!("Directory: {}", file.0.to_string_lossy());
            dc += 1;
        } else {
            println!(
                "File: {}, {}/us, {}/cs, {:X}",
                file.0.to_string_lossy(),
                file.1.uncompressed_size,
                file.1.compressed_size,
                file.1.relative_offset_of_local_header
            );
            fc += 1;
        }
    }
    println!("Total Directories: {}", dc);
    println!("Total Files: {}", fc);
}
