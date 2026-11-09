//! SARC archives (little endian), such as the game's `System/GymlArchive/*.sarc.zs` packs.

use crate::error::{Error, Result};
use std::collections::BTreeMap;

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn get(&self, o: usize, n: usize) -> Result<&[u8]> {
        o.checked_add(n).and_then(|e| self.0.get(o..e)).ok_or_else(|| Error::new(format!("SARC: read past the end at {o:#x}")))
    }
    fn u16(&self, o: usize) -> Result<usize> {
        Ok(u16::from_le_bytes(self.get(o, 2)?.try_into().unwrap()) as usize)
    }
    fn u32(&self, o: usize) -> Result<usize> {
        Ok(u32::from_le_bytes(self.get(o, 4)?.try_into().unwrap()) as usize)
    }
}

/// Every file in the archive, by path. Files stored without a name are keyed by their hash in
/// hex.
pub fn read(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let b = Bytes(data);
    if b.get(0, 4)? != b"SARC" {
        return Err("not a SARC archive".into());
    }
    if b.u16(6)? != 0xFEFF {
        return Err("big-endian SARC archives are not supported".into());
    }
    let sfat = b.u16(4)?;
    let data_off = b.u32(0x0C)?;
    if b.get(sfat, 4)? != b"SFAT" {
        return Err("SARC: missing SFAT".into());
    }
    let count = b.u16(sfat + 6)?;
    let nodes = sfat + b.u16(sfat + 4)?;
    let sfnt = nodes + 16 * count;
    if b.get(sfnt, 4)? != b"SFNT" {
        return Err("SARC: missing SFNT".into());
    }
    let names = sfnt + b.u16(sfnt + 4)?;
    let mut out = BTreeMap::new();
    for i in 0..count {
        let e = nodes + 16 * i;
        let (hash, attr) = (b.u32(e)?, b.u32(e + 4)?);
        let (start, end) = (b.u32(e + 8)?, b.u32(e + 12)?);
        let name = if attr >> 24 != 0 {
            let o = names + (attr & 0xFF_FFFF) * 4;
            let rest = data.get(o..).ok_or_else(|| Error::new("SARC: bad name offset"))?;
            let z = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
            String::from_utf8_lossy(&rest[..z]).into_owned()
        } else {
            format!("{hash:08x}")
        };
        let len = end.checked_sub(start).ok_or_else(|| Error::new(format!("SARC: {name} ends before it starts")))?;
        out.insert(name, b.get(data_off + start, len)?.to_vec());
    }
    Ok(out)
}

/// The SARC name hash (multiplier 0x65, bytes as signed chars).
pub fn name_hash(name: &str) -> u32 {
    name.bytes().fold(0u32, |h, c| h.wrapping_mul(0x65).wrapping_add(c as i8 as i32 as u32))
}

/// Write an archive the way the game's `System/GymlArchive` packs are laid out: entries sorted
/// by name hash, names 4-aligned, the data starting on a 0x100 boundary with every file
/// 0x100-aligned (the alignment of every file in the shipped packs).
pub fn write(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    const ALIGN: usize = 0x100;
    let mut order: Vec<(&String, &Vec<u8>)> = files.iter().collect();
    order.sort_by_key(|(n, _)| name_hash(n));
    if order.windows(2).any(|w| name_hash(w[0].0) == name_hash(w[1].0)) {
        return Err("SARC: two file names have the same hash".into());
    }
    if order.len() > u16::MAX as usize {
        return Err("SARC: too many files".into());
    }
    let mut names = vec![];
    let mut name_offs = vec![];
    for (n, _) in &order {
        name_offs.push(names.len() / 4);
        names.extend_from_slice(n.as_bytes());
        names.push(0);
        names.resize(names.len().next_multiple_of(4), 0);
    }
    let sfat = 0x14;
    let sfnt = sfat + 0x0C + 16 * order.len();
    let data_off = (sfnt + 8 + names.len()).next_multiple_of(ALIGN);
    let mut blob = vec![];
    let mut ranges = vec![];
    for (_, d) in &order {
        blob.resize(blob.len().next_multiple_of(ALIGN), 0);
        ranges.push((blob.len(), blob.len() + d.len()));
        blob.extend_from_slice(d);
    }
    let mut out = Vec::with_capacity(data_off + blob.len());
    out.extend_from_slice(b"SARC");
    out.extend_from_slice(&0x14u16.to_le_bytes());
    out.extend_from_slice(&0xFEFFu16.to_le_bytes());
    out.extend_from_slice(&((data_off + blob.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(data_off as u32).to_le_bytes());
    out.extend_from_slice(&0x0100u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(b"SFAT");
    out.extend_from_slice(&0x0Cu16.to_le_bytes());
    out.extend_from_slice(&(order.len() as u16).to_le_bytes());
    out.extend_from_slice(&0x65u32.to_le_bytes());
    for (i, (n, _)) in order.iter().enumerate() {
        out.extend_from_slice(&name_hash(n).to_le_bytes());
        out.extend_from_slice(&(0x0100_0000u32 | name_offs[i] as u32).to_le_bytes());
        out.extend_from_slice(&(ranges[i].0 as u32).to_le_bytes());
        out.extend_from_slice(&(ranges[i].1 as u32).to_le_bytes());
    }
    out.extend_from_slice(b"SFNT");
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&names);
    out.resize(data_off, 0);
    out.extend_from_slice(&blob);
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal archive in the layout the game's packs use.
    pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut names = vec![];
        let mut name_offs = vec![];
        for (n, _) in files {
            name_offs.push(names.len() / 4);
            names.extend_from_slice(n.as_bytes());
            names.push(0);
            while names.len() % 4 != 0 {
                names.push(0);
            }
        }
        let mut blob = vec![];
        let mut ranges = vec![];
        for (_, d) in files {
            let s = blob.len();
            blob.extend_from_slice(d);
            ranges.push((s, blob.len()));
            while blob.len() % 8 != 0 {
                blob.push(0);
            }
        }
        let sfat = 0x14;
        let sfnt = sfat + 0x0C + 16 * files.len();
        let data_off = sfnt + 8 + names.len();
        let mut out = vec![];
        out.extend_from_slice(b"SARC");
        out.extend_from_slice(&0x14u16.to_le_bytes());
        out.extend_from_slice(&0xFEFFu16.to_le_bytes());
        out.extend_from_slice(&((data_off + blob.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(data_off as u32).to_le_bytes());
        out.extend_from_slice(&0x0100u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(b"SFAT");
        out.extend_from_slice(&0x0Cu16.to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&0x65u32.to_le_bytes());
        for (i, _) in files.iter().enumerate() {
            out.extend_from_slice(&(i as u32).to_le_bytes());
            out.extend_from_slice(&(0x0100_0000u32 | name_offs[i] as u32).to_le_bytes());
            out.extend_from_slice(&(ranges[i].0 as u32).to_le_bytes());
            out.extend_from_slice(&(ranges[i].1 as u32).to_le_bytes());
        }
        out.extend_from_slice(b"SFNT");
        out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&names);
        out.extend_from_slice(&blob);
        out
    }

    #[test]
    fn reads_names_and_data() {
        let sarc = build(&[("Gyml/A.bgyml", b"hello"), ("Gyml/B.bgyml", b"world!!")]);
        let files = read(&sarc).unwrap();
        assert_eq!(files["Gyml/A.bgyml"], b"hello");
        assert_eq!(files["Gyml/B.bgyml"], b"world!!");
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        let sarc = build(&[("a", b"1234"), ("b", b"5678")]);
        assert!(read(b"SARC").is_err());
        for n in 0..sarc.len() {
            let _ = read(&sarc[..n]);
        }
        for i in 0..sarc.len() {
            let mut bad = sarc.clone();
            bad[i] ^= 0xFF;
            let _ = read(&bad);
        }
    }
    #[test]
    fn written_archives_read_back_sorted_by_hash() {
        let mut files = BTreeMap::new();
        files.insert("Gyml/B.bgyml".to_string(), b"world!!".to_vec());
        files.insert("Gyml/A.bgyml".to_string(), b"hello".to_vec());
        files.insert("Gyml/Long/Name.bgyml".to_string(), vec![7; 300]);
        let sarc = write(&files).unwrap();
        assert_eq!(read(&sarc).unwrap(), files);
        let hashes: Vec<u32> = (0..3).map(|i| u32::from_le_bytes(sarc[0x20 + 16 * i..0x24 + 16 * i].try_into().unwrap())).collect();
        assert!(hashes.windows(2).all(|w| w[0] < w[1]));
        // names hash the way the shipped packs do (checked against actor_general_format)
        assert_eq!(name_hash("a"), 0x61);
        assert_eq!(name_hash("ab"), 0x61 * 0x65 + 0x62);
    }
}
