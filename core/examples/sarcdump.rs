//! List a SARC archive (optionally zstd-compressed), or extract its files.
//!
//! cargo run -p mkwtrack --example sarcdump -- <archive> [out_dir]

use mkwtrack::{io, sarc};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: sarcdump <archive> [out_dir]");
        std::process::exit(2);
    };
    let data = io::read_file(std::path::Path::new(path)).expect("read");
    let files = sarc::read(&data).expect("sarc");
    for (name, d) in &files {
        println!("{}\t{name}", d.len());
        if let Some(out) = args.get(1) {
            let p = std::path::Path::new(out).join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, d).unwrap();
        }
    }
}
