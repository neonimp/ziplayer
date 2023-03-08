use std::env::args;

fn main() {
    if args().len() != 3 {
        println!("Usage: extract_all <zipfile> <output_dir>");
        std::process::exit(1);
    }

    let filename = args().nth(1).unwrap();
    let output_dir = args().nth(2).unwrap();

    let mut file = std::fs::File::open(filename).unwrap();
    let mut zip = ziplayer::reader::ZipReader::new(&mut file).unwrap_or_else(|e| {
        println!("Error: ({:0X}):{}", e.error_code(), e);
        std::process::exit(1);
    });

    // Canonicalize the output directory
    let _output_dir = std::fs::canonicalize(output_dir).unwrap();

    let deflate_codec = ziplayer::codecs::gzip_codec::new();

    // Extract all files
    zip.extract_all_files(&output_dir, deflate_codec).unwrap_or_else(|e| {
        println!("Error: ({:0X}):{}", e.error_code(), e);
        std::process::exit(1);
    });
}
