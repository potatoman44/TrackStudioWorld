//! BNTX textures (`Tex/*.bntx.zs`): the texture header, block-linear untiling and decoding to
//! RGBA8.
//!
//! The game's textures are on the Switch 2 platform (`Ounc`). Their surfaces are block linear
//! like the Switch's Tegra X1 (512-byte GOBs, 64 bytes by 8 rows, stacked in blocks of
//! 2^`layout` GOBs), but the bytes inside each GOB are laid out the way the Orin TRM describes:
//! 16-byte runs, four rows of them per 128-byte half. Switch 1 (`NX  `) files use the Tegra X1
//! order and are read too.

use crate::error::{Context, Result};
use std::path::Path;

/// One texture of a BNTX file.
#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    /// Switch 2 GOB layout
    pub orin: bool,
    pub tile_mode: u16,
    pub mip_count: u16,
    /// nn::gfx image format: channel layout in the high byte, type (UNORM, SRGB...) in the low
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub array_count: u32,
    /// log2 of the first mip's block height in GOBs
    pub block_height_log2: u32,
    /// component selectors for R, G, B, A: 0 zero, 1 one, 2..5 R, G, B, A
    pub channels: [u8; 4],
    /// file offset of each mip of the first layer
    pub mip_offsets: Vec<usize>,
}

/// A decoded image, rows top to bottom.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// Whether any texel is less than opaque.
    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255)
    }
}

fn rd<const N: usize>(d: &[u8], o: usize) -> Result<[u8; N]> {
    d.get(o..o + N).and_then(|s| s.try_into().ok()).ok_or_else(|| "BNTX: unexpected end of file".into())
}
fn u16_at(d: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(rd(d, o)?))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(rd(d, o)?))
}
fn off_at(d: &[u8], o: usize) -> Result<usize> {
    Ok(u64::from_le_bytes(rd(d, o)?) as usize)
}

/// (block width, block height, bytes per block) of an image format's channel layout.
fn block_info(format: u32) -> Option<(u32, u32, u32)> {
    Some(match format >> 8 {
        0x02 => (1, 1, 1),                       // R8
        0x09 | 0x0a => (1, 1, 2),                // R8G8, R16
        0x0b | 0x0c => (1, 1, 4),                // R8G8B8A8, B8G8R8A8
        0x15 => (1, 1, 8),                       // R16G16B16A16
        0x1a | 0x1d => (4, 4, 8),                // BC1, BC4
        0x1b | 0x1c | 0x1e..=0x20 => (4, 4, 16), // BC2, BC3, BC5, BC6H, BC7
        0x2d => (4, 4, 16),
        0x2e => (5, 4, 16),
        0x2f => (5, 5, 16),
        0x30 => (6, 5, 16),
        0x31 => (6, 6, 16),
        0x32 => (8, 5, 16),
        0x33 => (8, 6, 16),
        0x34 => (8, 8, 16),
        0x35 => (10, 5, 16),
        0x36 => (10, 6, 16),
        0x37 => (10, 8, 16),
        0x38 => (10, 10, 16),
        0x39 => (12, 10, 16),
        0x3a => (12, 12, 16),
        _ => return None,
    })
}

pub fn parse(d: &[u8]) -> Result<Vec<Texture>> {
    if !d.starts_with(b"BNTX") {
        return Err("not a BNTX file".into());
    }
    let orin = rd::<4>(d, 0x20)? == *b"Ounc";
    let count = u32_at(d, 0x24)? as usize;
    let infos = off_at(d, 0x28)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count.min(4096) {
        let p = off_at(d, infos + 8 * i)?;
        if rd::<4>(d, p)? != *b"BRTI" {
            return Err("BNTX: bad texture info".into());
        }
        let mip_count = u16_at(d, p + 0x16)?.max(1);
        let name_at = off_at(d, p + 0x60)?;
        let name_len = u16_at(d, name_at)? as usize;
        let name = String::from_utf8_lossy(d.get(name_at + 2..name_at + 2 + name_len).ok_or("BNTX: bad name")?).into_owned();
        let mips = off_at(d, p + 0x70)?;
        let mip_offsets = (0..mip_count as usize).map(|m| off_at(d, mips + 8 * m)).collect::<Result<Vec<_>>>()?;
        out.push(Texture {
            name,
            orin,
            tile_mode: u16_at(d, p + 0x12)?,
            mip_count,
            format: u32_at(d, p + 0x1c)?,
            width: u32_at(d, p + 0x24)?,
            height: u32_at(d, p + 0x28)?,
            array_count: u32_at(d, p + 0x30)?,
            block_height_log2: u32_at(d, p + 0x34)? & 7,
            channels: rd(d, p + 0x58)?,
            mip_offsets,
        });
    }
    Ok(out)
}

/// Offset of byte (x, y) inside a 64x8-byte GOB.
fn gob_offset(x: usize, y: usize, orin: bool) -> usize {
    if orin {
        (x / 32) * 256 + (y / 4) * 128 + ((x % 32) / 16) * 64 + (y % 4) * 16 + x % 16
    } else {
        (x / 32) * 256 + (y / 2) * 64 + ((x % 32) / 16) * 32 + (y % 2) * 16 + x % 16
    }
}

/// Block-linear surface (`rows` rows of `width_bytes`) to row-major bytes. Bytes past the end
/// of `data` read as zero.
fn untile(data: &[u8], width_bytes: usize, rows: usize, block_height: usize, orin: bool) -> Vec<u8> {
    let mut out = vec![0u8; width_bytes * rows];
    let gobs_wide = width_bytes.div_ceil(64);
    let block_bytes = 512 * block_height;
    for y in 0..rows {
        let row_base = (y / (8 * block_height)) * gobs_wide * block_bytes + ((y % (8 * block_height)) / 8) * 512;
        for x in (0..width_bytes).step_by(16) {
            let src = row_base + (x / 64) * block_bytes + gob_offset(x % 64, y % 8, orin);
            let n = 16.min(width_bytes - x);
            if let Some(s) = data.get(src..src + n) {
                out[y * width_bytes + x..y * width_bytes + x + n].copy_from_slice(s);
            }
        }
    }
    out
}

/// Decode one texture to RGBA8 at the largest mip no bigger than `max_size` on either side (the
/// smallest mip when none is), with its component selectors applied.
pub fn decode(d: &[u8], t: &Texture, max_size: u32) -> Result<Image> {
    let (bw, bh, bpb) = block_info(t.format).ok_or_else(|| format!("{}: unsupported format {:#06x}", t.name, t.format))?;
    let level = (0..t.mip_count as u32)
        .find(|&l| (t.width >> l).max(1) <= max_size && (t.height >> l).max(1) <= max_size)
        .unwrap_or(t.mip_count as u32 - 1);
    let (w, h) = ((t.width >> level).max(1), (t.height >> level).max(1));
    let (bx, by) = (w.div_ceil(bw) as usize, h.div_ceil(bh) as usize);
    let width_bytes = bx * bpb as usize;
    let start = t.mip_offsets[level as usize];
    let data = d.get(start..).ok_or("BNTX: mip outside the file")?;
    let lin = if t.tile_mode == 1 {
        let pitch = width_bytes.div_ceil(32) * 32;
        let mut v = vec![0u8; width_bytes * by];
        for (y, row) in v.chunks_exact_mut(width_bytes).enumerate() {
            if let Some(s) = data.get(y * pitch..y * pitch + width_bytes) {
                row.copy_from_slice(s);
            }
        }
        v
    } else {
        let mut block_height = 1usize << t.block_height_log2;
        while block_height > 1 && by <= (block_height / 2) * 8 {
            block_height /= 2;
        }
        untile(data, width_bytes, by, block_height, t.orin)
    };
    let (wu, hu) = (w as usize, h as usize);
    let mut px = vec![0u32; wu * hu]; // texture2ddecoder writes BGRA
    let kind = t.format >> 8;
    let ty = t.format & 0xff;
    // the block decoders come from texture2ddecoder; a panic on odd data becomes an error
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match kind {
        0x2d..=0x3a => texture2ddecoder::decode_astc(&lin, wu, hu, bw as usize, bh as usize, &mut px),
        0x1a => texture2ddecoder::decode_bc1(&lin, wu, hu, &mut px),
        0x1b => texture2ddecoder::decode_bc2(&lin, wu, hu, &mut px),
        0x1c => texture2ddecoder::decode_bc3(&lin, wu, hu, &mut px),
        0x1d => texture2ddecoder::decode_bc4(&lin, wu, hu, &mut px),
        0x1e => {
            let mut lin = lin;
            if ty == 2 {
                // signed endpoints: re-bias so the unsigned decoder gives the same ramp
                for b in lin.as_chunks_mut::<8>().0 {
                    b[0] ^= 0x80;
                    b[1] ^= 0x80;
                }
            }
            texture2ddecoder::decode_bc5(&lin, wu, hu, &mut px)
        }
        0x1f => texture2ddecoder::decode_bc6(&lin, wu, hu, &mut px, ty == 2),
        0x20 => texture2ddecoder::decode_bc7(&lin, wu, hu, &mut px),
        _ => {
            let bgra = |r: u8, g: u8, b: u8, a: u8| u32::from_le_bytes([b, g, r, a]);
            for (i, p) in px.iter_mut().enumerate() {
                *p = match kind {
                    0x02 => bgra(lin[i], 0, 0, 255),
                    0x09 => bgra(lin[2 * i], lin[2 * i + 1], 0, 255),
                    0x0a => bgra(lin[2 * i + 1], 0, 0, 255),
                    0x0b => bgra(lin[4 * i], lin[4 * i + 1], lin[4 * i + 2], lin[4 * i + 3]),
                    0x15 => {
                        let c = |k: usize| {
                            let v = u16::from_le_bytes([lin[8 * i + 2 * k], lin[8 * i + 2 * k + 1]]);
                            match ty {
                                5 => (crate::bfres::half(v).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                                _ => (v >> 8) as u8,
                            }
                        };
                        bgra(c(0), c(1), c(2), c(3))
                    }
                    _ => bgra(lin[4 * i + 2], lin[4 * i + 1], lin[4 * i], lin[4 * i + 3]),
                };
            }
            Ok(())
        }
    }))
    .unwrap_or(Err("decoder failed"));
    res.map_err(|e| format!("{}: {e}", t.name))?;
    let mut rgba = Vec::with_capacity(wu * hu * 4);
    for p in px {
        let [b, g, r, a] = p.to_le_bytes();
        let src = [r, g, b, a];
        rgba.extend(t.channels.map(|c| match c {
            0 => 0,
            2..=5 => src[c as usize - 2],
            _ => 255,
        }));
    }
    Ok(Image { width: w, height: h, rgba })
}

/// Path of a texture in the romfs.
pub fn texture_path(romfs: &Path, name: &str) -> std::path::PathBuf {
    romfs.join("Tex").join(format!("{name}.bntx.zs"))
}

/// Read and decode the texture `Tex/<name>.bntx.zs` (its first texture) at most `max_size`
/// texels on a side.
pub fn load(romfs: &Path, name: &str, max_size: u32) -> Result<Image> {
    let path = texture_path(romfs, name);
    let d = crate::io::read_file(&path)?;
    let t = parse(&d).path_context(&path)?;
    let t = t.first().ok_or("BNTX: no texture").path_context(&path)?;
    decode(&d, t, max_size)
}

// ---------------------------------------------------------------------------------------------
// Writing

/// nn::gfx `R8_G8_B8_A8_SRGB`: colour textures.
pub const FORMAT_RGBA8_SRGB: u32 = 0x0b06;
/// nn::gfx `R8_G8_B8_A8_UNORM`: data textures (normals, material masks).
pub const FORMAT_RGBA8_UNORM: u32 = 0x0b01;

/// Row-major bytes to a block-linear surface: the inverse of [`untile`]. The result is padded to
/// whole blocks of `block_height` GOBs.
fn tile(lin: &[u8], width_bytes: usize, rows: usize, block_height: usize, orin: bool) -> Vec<u8> {
    let gobs_wide = width_bytes.div_ceil(64);
    let block_bytes = 512 * block_height;
    let blocks_tall = rows.div_ceil(8 * block_height);
    let mut out = vec![0u8; gobs_wide * blocks_tall * block_bytes];
    for y in 0..rows {
        let row_base = (y / (8 * block_height)) * gobs_wide * block_bytes + ((y % (8 * block_height)) / 8) * 512;
        for x in (0..width_bytes).step_by(16) {
            let dst = row_base + (x / 64) * block_bytes + gob_offset(x % 64, y % 8, orin);
            let n = 16.min(width_bytes - x);
            out[dst..dst + n].copy_from_slice(&lin[y * width_bytes + x..y * width_bytes + x + n]);
        }
    }
    out
}

/// log2 of the block height (in GOBs) the game's files use for a surface `rows` blocks tall:
/// the smallest power of two covering the rows, at most 16 GOBs.
fn block_height_log2(rows: u32) -> u32 {
    let gobs = rows.div_ceil(8).clamp(1, 16);
    gobs.next_power_of_two().trailing_zeros()
}

/// Halve an RGBA8 image with a box filter (odd sizes keep their last row or column). Colour
/// textures are averaged in linear light.
pub(crate) fn half_size(img: &Image, srgb: bool) -> Image {
    let (w, h) = ((img.width / 2).max(1), (img.height / 2).max(1));
    let to_lin = |c: u8| if srgb { (c as f32 / 255.0).powf(2.2) } else { c as f32 / 255.0 };
    let from_lin = |v: f32| ((if srgb { v.powf(1.0 / 2.2) } else { v }) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0f32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let sx = (2 * x + dx).min(img.width - 1);
                let sy = (2 * y + dy).min(img.height - 1);
                let i = ((sy * img.width + sx) * 4) as usize;
                for (c, s) in sum.iter_mut().take(3).enumerate() {
                    *s += to_lin(img.rgba[i + c]);
                }
                sum[3] += img.rgba[i + 3] as f32 / 255.0;
            }
            rgba.extend([from_lin(sum[0] / 4.0), from_lin(sum[1] / 4.0), from_lin(sum[2] / 4.0), (sum[3] / 4.0 * 255.0 + 0.5) as u8]);
        }
    }
    Image { width: w, height: h, rgba }
}

/// The patricia-trie bit a one-entry `_DIC` stores for `name`: the first set bit, counting from
/// the least significant bit of the last byte.
pub(crate) fn dict_bit(name: &str) -> u32 {
    let b = name.as_bytes();
    (0..b.len() as u32 * 8).find(|&i| (b[b.len() - 1 - (i / 8) as usize] >> (i % 8)) & 1 == 1).unwrap_or(0)
}

/// A Switch 2 (`Ounc`) BNTX file holding one uncompressed RGBA8 texture with a full mip chain,
/// laid out the way the game's own files are: the `_STR` block (strings and the one-entry
/// `_DIC`) after the platform header, `BRTI` up to 0x3F0, `BRTD` at 0x3F0 with the image at
/// 0x400, then the relocation table.
///
/// `format` is [`FORMAT_RGBA8_SRGB`] or [`FORMAT_RGBA8_UNORM`]; names are limited to 64 bytes so
/// the texture info fits before 0x3F0.
pub fn write(name: &str, image: &Image, format: u32) -> Result<Vec<u8>> {
    if name.is_empty() || name.len() > 64 {
        return Err(format!("BNTX: texture name {name:?} must be 1 to 64 bytes").into());
    }
    if image.width == 0 || image.height == 0 || image.rgba.len() != (image.width * image.height * 4) as usize {
        return Err("BNTX: bad image size".into());
    }
    let srgb = format & 0xff == 6;
    let mut mips = vec![image.clone()];
    while mips.len() < 16 {
        let last = mips.last().unwrap();
        if last.width == 1 && last.height == 1 {
            break;
        }
        mips.push(half_size(last, srgb));
    }
    let bh_log2 = block_height_log2(image.height);
    let mut surfaces = vec![];
    for m in &mips {
        let mut bh = 1usize << bh_log2;
        while bh > 1 && (m.height as usize) <= (bh / 2) * 8 {
            bh /= 2;
        }
        surfaces.push(tile(&m.rgba, m.width as usize * 4, m.height as usize, bh, true));
    }
    let image_size: usize = surfaces.iter().map(Vec::len).sum();

    // strings: count, the empty string, then the name
    let str_at = 0x60usize;
    let empty_at = str_at + 0x14;
    let name_at = str_at + 0x18;
    let dic_at = (name_at + 2 + name.len() + 1).next_multiple_of(8);
    let brti = dic_at + 0x28;
    let mip_table = brti + 0x2a0;
    let brtd = 0x3f0usize;
    if mip_table + 8 * mips.len() > brtd {
        return Err("BNTX: texture name too long".into());
    }
    let data_at = brtd + 0x10;
    let rlt = data_at + image_size;
    let file_size = rlt + 0x78;

    let mut d = vec![0u8; file_size];
    let put = |d: &mut Vec<u8>, o: usize, b: &[u8]| d[o..o + b.len()].copy_from_slice(b);
    let u16le = |d: &mut Vec<u8>, o: usize, v: u16| d[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let u32le = |d: &mut Vec<u8>, o: usize, v: u32| d[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let u64le = |d: &mut Vec<u8>, o: usize, v: usize| d[o..o + 8].copy_from_slice(&(v as u64).to_le_bytes());

    // file header
    put(&mut d, 0, b"BNTX\0\0\0\0");
    put(&mut d, 8, &[0x01, 0x01, 0x04, 0x00, 0xff, 0xfe, 0x09, 0x40]);
    u32le(&mut d, 0x10, (name_at + 2) as u32);
    u16le(&mut d, 0x16, str_at as u16);
    u32le(&mut d, 0x18, rlt as u32);
    u32le(&mut d, 0x1c, file_size as u32);
    // platform header
    put(&mut d, 0x20, b"Ounc");
    u32le(&mut d, 0x24, 1);
    u64le(&mut d, 0x28, 0x58);
    u64le(&mut d, 0x30, brtd);
    u64le(&mut d, 0x38, dic_at);
    u64le(&mut d, 0x58, brti);
    // _STR (its size runs to the texture info, so it holds the _DIC too)
    put(&mut d, str_at, b"_STR");
    u32le(&mut d, str_at + 4, (brti - str_at) as u32);
    u32le(&mut d, str_at + 8, (brti - str_at) as u32);
    u32le(&mut d, str_at + 0x10, 1);
    u16le(&mut d, name_at, name.len() as u16);
    put(&mut d, name_at + 2, name.as_bytes());
    // _DIC: the root node, then the texture's
    put(&mut d, dic_at, b"_DIC");
    u32le(&mut d, dic_at + 4, 1);
    u32le(&mut d, dic_at + 8, u32::MAX);
    u16le(&mut d, dic_at + 0xc, 1);
    u64le(&mut d, dic_at + 0x10, empty_at);
    u32le(&mut d, dic_at + 0x18, dict_bit(name));
    u16le(&mut d, dic_at + 0x1e, 1);
    u64le(&mut d, dic_at + 0x20, name_at);
    // BRTI
    put(&mut d, brti, b"BRTI");
    u32le(&mut d, brti + 4, (brtd - brti) as u32);
    u32le(&mut d, brti + 8, (brtd - brti) as u32);
    put(&mut d, brti + 0x10, &[0x09, 0x02]);
    u16le(&mut d, brti + 0x16, mips.len() as u16);
    u32le(&mut d, brti + 0x18, 1);
    u32le(&mut d, brti + 0x1c, format);
    u32le(&mut d, brti + 0x20, 0x21);
    u32le(&mut d, brti + 0x24, image.width);
    u32le(&mut d, brti + 0x28, image.height);
    u32le(&mut d, brti + 0x2c, 1);
    u32le(&mut d, brti + 0x30, 1);
    u32le(&mut d, brti + 0x34, bh_log2);
    u32le(&mut d, brti + 0x38, 0x0001_00c9);
    u32le(&mut d, brti + 0x50, image_size as u32);
    u32le(&mut d, brti + 0x54, 0x200);
    put(&mut d, brti + 0x58, &[2, 3, 4, 5]);
    u32le(&mut d, brti + 0x5c, 1);
    u64le(&mut d, brti + 0x60, name_at);
    u64le(&mut d, brti + 0x68, 0x20);
    u64le(&mut d, brti + 0x70, mip_table);
    u64le(&mut d, brti + 0x80, brti + 0xa0);
    u64le(&mut d, brti + 0x88, brti + 0x1a0);
    // BRTD and the image
    put(&mut d, brtd, b"BRTD");
    u64le(&mut d, brtd + 8, image_size + 0x10);
    let mut at = data_at;
    for (i, s) in surfaces.iter().enumerate() {
        u64le(&mut d, mip_table + 8 * i, at);
        put(&mut d, at, s);
        at += s.len();
    }
    // relocation table: section 0 the headers and texture info, section 1 the image (pointers
    // to it are relocated against its base)
    let r = rlt;
    put(&mut d, r, b"_RLT");
    u32le(&mut d, r + 4, rlt as u32);
    u32le(&mut d, r + 8, 2);
    // sections: (pointer, offset, size, first entry, entry count)
    u32le(&mut d, r + 0x1c, (mip_table + 8 * mips.len()) as u32);
    u32le(&mut d, r + 0x24, 5);
    u32le(&mut d, r + 0x30, brtd as u32);
    u32le(&mut d, r + 0x34, (image_size + 0x10) as u32);
    u32le(&mut d, r + 0x38, 5);
    u32le(&mut d, r + 0x3c, 2);
    // entries: (offset, struct count, pointers per struct, u64s skipped after them)
    let entries: [(usize, u16, u8, u8); 7] = [
        (0x28, 2, 1, 1),                                    // texture info array, _DIC
        (0x58, 2, 1, (dic_at + 0x10 - 0x58 - 8) as u8 / 8), // the texture info pointer, the root node's name
        (dic_at + 0x20, 1, 1, 0),                           // the texture node's name
        (brti + 0x60, 1, 3, 0),                             // name, platform header, mip table
        (brti + 0x80, 1, 2, 0),                             // runtime texture and view
        (0x30, 1, 1, 0),                                    // BRTD
        (mip_table, 1, mips.len() as u8, 0),
    ];
    for (i, (o, sc, oc, pc)) in entries.iter().enumerate() {
        let e = r + 0x40 + 8 * i;
        u32le(&mut d, e, *o as u32);
        u16le(&mut d, e + 4, *sc);
        d[e + 6] = *oc;
        d[e + 7] = *pc;
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gob_layouts_are_permutations() {
        for orin in [false, true] {
            let mut seen = [false; 512];
            for y in 0..8 {
                for x in 0..64 {
                    let o = gob_offset(x, y, orin);
                    assert!(!seen[o], "{orin} {x} {y}");
                    seen[o] = true;
                }
            }
        }
        // the Orin GOB keeps four rows of a 16-byte column together
        assert_eq!((gob_offset(0, 1, true), gob_offset(16, 0, true), gob_offset(0, 4, true)), (16, 64, 128));
        assert_eq!((gob_offset(0, 1, false), gob_offset(16, 0, false), gob_offset(0, 2, false)), (16, 32, 64));
    }

    #[test]
    fn untile_one_gob_column() {
        // 16 bytes wide, 16 rows, block height 2: the first 8 rows fill GOB 0, the next GOB 1
        let mut tiled = vec![0u8; 1024];
        for y in 0..16 {
            for x in 0..16 {
                tiled[(y / 8) * 512 + gob_offset(x, y % 8, true)] = (y * 16 + x) as u8;
            }
        }
        let lin = untile(&tiled, 16, 16, 2, true);
        assert!(lin.iter().enumerate().all(|(i, &b)| b == i as u8));
    }
    #[test]
    fn tile_inverts_untile() {
        let (w, rows) = (200usize, 37usize);
        let lin: Vec<u8> = (0..w * rows).map(|i| (i * 7 % 251) as u8).collect();
        for bh in [1, 2, 4, 8, 16] {
            let tiled = tile(&lin, w, rows, bh, true);
            assert_eq!(untile(&tiled, w, rows, bh, true), lin, "block height {bh}");
        }
    }

    #[test]
    fn block_heights_match_shipped_files() {
        // 512 rows of BC1 blocks (128), ASTC 8x8 (64), 256-texel ASTC 8x8 (32), 16-texel BC3 (4)
        assert_eq!([128, 64, 32, 4].map(block_height_log2), [4, 3, 2, 0]);
    }

    #[test]
    fn written_texture_reads_back() {
        let (w, h) = (100u32, 60u32);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i % 251) as u8, (i / 7 % 256) as u8, 200, 255]).collect();
        let img = Image { width: w, height: h, rgba: rgba.clone() };
        let d = write("Imported_Test_Alb", &img, FORMAT_RGBA8_SRGB).unwrap();
        let t = parse(&d).unwrap();
        assert_eq!(t.len(), 1);
        let t = &t[0];
        assert_eq!((t.name.as_str(), t.width, t.height, t.format, t.mip_count), ("Imported_Test_Alb", w, h, FORMAT_RGBA8_SRGB, 7));
        assert!(t.orin);
        let back = decode(&d, t, 4096).unwrap();
        assert_eq!(back.rgba, rgba);
        let small = decode(&d, t, 1).unwrap();
        assert_eq!((small.width, small.height), (1, 1));
        // relocation table and file size agree with the header
        let rlt = u32::from_le_bytes(d[0x18..0x1c].try_into().unwrap()) as usize;
        assert_eq!(&d[rlt..rlt + 4], b"_RLT");
        assert_eq!(u32::from_le_bytes(d[0x1c..0x20].try_into().unwrap()) as usize, d.len());
    }

    #[test]
    fn dict_bits() {
        // matches the shipped CmnTex_Replace_Map_Soil_Replace_Soil.bntx (last byte 'l' = 0x6C)
        assert_eq!(dict_bit("CmnTex_Replace_Map_Soil_Replace_Soil"), 2);
        assert_eq!(dict_bit("a"), 0);
    }
}
