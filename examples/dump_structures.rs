use std::fs::File;
use ziplayer::find_eocd;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let filename = &args[1];
    let file = File::open(filename).unwrap();
    let mut reader = std::io::BufReader::new(file);
    let eocd = find_eocd(&mut reader).unwrap();
    println!("{:#?}", eocd);
    let central_dir = ziplayer::parse_central_dir(&eocd, &mut reader).unwrap();
    println!("{:#?}", central_dir);
}