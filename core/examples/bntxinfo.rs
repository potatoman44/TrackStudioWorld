//! Print a BNTX texture's header and the mean of its decoded texels.
//!
//! cargo run -p mkwtrack --example bntxinfo -- <file.bntx.zs>...

use mkwtrack::{bntx, io};

fn main() {
    for path in std::env::args().skip(1) {
        let data = io::read_file(std::path::Path::new(&path)).expect("read");
        for tex in bntx::parse(&data).expect("parse") {
            let img = bntx::decode(&data, &tex, 4096).expect("decode");
            let n = (img.width * img.height) as f64;
            let mut sum = [0f64; 4];
            for px in img.rgba.chunks_exact(4) {
                for c in 0..4 {
                    sum[c] += px[c] as f64;
                }
            }
            println!(
                "{path}\n  {} {}x{} format {:#06x} tile {} mips {} block_h {} channels {:?} mean {:.0?}",
                tex.name,
                tex.width,
                tex.height,
                tex.format,
                tex.tile_mode,
                tex.mip_count,
                tex.block_height_log2,
                tex.channels,
                sum.map(|s| s / n)
            );
        }
    }
}
