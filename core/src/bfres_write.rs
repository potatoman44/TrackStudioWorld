//! Writing BFRES models for imported geometry, by rebuilding a shipped model (the template).
//!
//! Building a model from nothing would mean choosing shaders, shader options and render state
//! the game accepts, which nothing here can check. Instead a shipped model is copied and only
//! what an import changes is rewritten:
//!
//! * the vertex and index data of the template's shapes, which are given the imported meshes;
//!   template shapes with nothing to draw get one degenerate triangle;
//! * the counts, buffer sizes, bounds and sub-mesh ranges that describe them;
//! * the textures the materials sample for albedo (`_a0`), normals (`_n0`) and material masks
//!   (`mix0`); the baked lighting (`bake_*`) and shared (`_user*`) textures are kept, and the
//!   second UV set, which samples the baked lighting, points every vertex at one texel of it;
//! * the file and model names.
//!
//! Layout notes beyond those in [`crate::bfres`] (checked against the shipped models):
//!
//! * The vertex and index data sit in one region from the buffer info's offset (header 0xB0,
//!   then +8), indices first; shapes address them relative to its start. After it, aligned to
//!   0x1000, comes the 0x120-byte memory pool block the header points at (0xA8), then the
//!   relocation table (header 0x18). The file size is at 0x1C and the region's size at the
//!   buffer info's +4.
//! * The relocation table has six sections: 0 the structures, 1 the index data, 2 the vertex
//!   data, 3 empty at the memory pool, 4 the memory pool, 5 empty at the table. A pointer listed
//!   under section 1 or 4 points into that section.
//! * A mesh's +0x18 points at its index buffer size (u32); its sub-mesh array (+0x00) holds
//!   (byte offset, index count) pairs, their count a u16 at +0x34.
//! * A shape's bounding boxes (+0x38, centre and half extent, one per sub-mesh plus one) and
//!   bounding spheres (+0x40, centre and radius, one per mesh).
//! * Strings are a u16 length, the bytes and a NUL, 2-aligned, in the `_STR` pool (header 0xD0,
//!   its size a u32 at 0xD8). New strings are appended after it, before the buffer region.
//! * The header's name (0x10 as a u32 to the characters, 0x20 as a string) is
//!   `<folder>.<model>`; the model's own name is at FMDL +0x08 and in the one-entry model
//!   dictionary at header 0x30.

use crate::bntx::dict_bit;
use crate::error::Result;

/// One imported mesh: triangles in model space.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// V = 0 is the texture's top row
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// What goes into one usable template shape.
#[derive(Debug, Clone)]
pub struct ShapeInput {
    pub mesh: Mesh,
    /// the albedo texture (`Tex/<name>.bntx.zs`)
    pub albedo: String,
    /// where every vertex samples the material's baked lighting
    pub bake_uv: [f32; 2],
}

/// A template shape imported meshes can replace: one mesh, one vertex buffer, only attributes
/// this module can write, and a material that samples a single albedo texture.
#[derive(Debug, Clone)]
pub struct UsableShape {
    pub index: usize,
    pub material: String,
    /// sampler name and texture name of every texture the material samples
    pub textures: Vec<(String, String)>,
}

const SUPPORTED_SAMPLERS: [&str; 7] = ["_a0", "_n0", "_user0", "bake_shadow0", "bake_light0", "bake_light1", "mix0"];

struct Rd<'a>(&'a [u8]);

impl Rd<'_> {
    fn get(&self, o: usize, n: usize) -> Result<&[u8]> {
        o.checked_add(n).and_then(|e| self.0.get(o..e)).ok_or_else(|| format!("BFRES: read past the end at {o:#x}").into())
    }
    fn u8(&self, o: usize) -> Result<u8> {
        Ok(self.get(o, 1)?[0])
    }
    fn u16(&self, o: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.get(o, 2)?.try_into().unwrap()))
    }
    fn u32(&self, o: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.get(o, 4)?.try_into().unwrap()))
    }
    fn off(&self, o: usize) -> Result<usize> {
        Ok(u64::from_le_bytes(self.get(o, 8)?.try_into().unwrap()) as usize)
    }
    /// The string a pointer at `o` points to.
    fn str(&self, o: usize) -> Result<String> {
        let p = self.off(o)?;
        let n = self.u16(p)? as usize;
        Ok(String::from_utf8_lossy(self.get(p + 2, n)?).into_owned())
    }
    /// Names of a `_DIC` (without the root).
    fn dict(&self, o: usize) -> Result<Vec<String>> {
        let p = self.off(o)?;
        if p == 0 {
            return Ok(vec![]);
        }
        (0..self.u32(p + 4)? as usize).map(|i| self.str(p + 0x18 + 0x10 * i + 8)).collect()
    }
}

fn w32(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn w64(d: &mut [u8], o: usize, v: usize) {
    d[o..o + 8].copy_from_slice(&(v as u64).to_le_bytes());
}
fn wf32(d: &mut [u8], o: usize, v: f32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// IEEE half-precision bits for `v` (round to nearest, overflow to infinity).
pub fn to_half(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let frac = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if frac != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = frac | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let round = (m >> (shift - 1)) & 1;
        return sign | (half + round) as u16;
    }
    let half = ((e as u32) << 10) | (frac >> 13);
    let round = (frac >> 12) & 1;
    sign | (half + round) as u16
}

/// Encode one attribute value in an nn::gfx attribute format (the ones the reader in
/// [`crate::bfres`] decodes).
fn encode(out: &mut [u8], format: u32, v: [f32; 4]) -> Result<()> {
    let ty = format & 0xff;
    if format >> 8 == 0x0e {
        let c = |x: f32| -> u32 {
            if ty == 2 { ((x.clamp(-1.0, 1.0) * 511.0).round() as i32 as u32) & 0x3ff } else { (x.clamp(0.0, 1.0) * 1023.0).round() as u32 }
        };
        let w = if ty == 2 { ((v[3].clamp(-1.0, 1.0)).round() as i32 as u32) & 3 } else { (v[3].clamp(0.0, 1.0) * 3.0).round() as u32 };
        out[..4].copy_from_slice(&(c(v[0]) | c(v[1]) << 10 | c(v[2]) << 20 | w << 30).to_le_bytes());
        return Ok(());
    }
    let (count, bits) = match format >> 8 {
        0x02 => (1, 8),
        0x09 => (2, 8),
        0x0b => (4, 8),
        0x0a => (1, 16),
        0x12 => (2, 16),
        0x15 => (4, 16),
        0x14 => (1, 32),
        0x17 => (2, 32),
        0x18 => (3, 32),
        0x19 => (4, 32),
        _ => return Err(format!("BFRES: can't write attribute format {format:#06x}").into()),
    };
    for (i, &x) in v.iter().enumerate().take(count) {
        let p = i * bits / 8;
        match (bits, ty) {
            (8, 1) => out[p] = (x.clamp(0.0, 1.0) * 255.0).round() as u8,
            (8, 2) => out[p] = (x.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8,
            (8, _) => out[p] = x.round().clamp(-128.0, 255.0) as i32 as u8,
            (16, 1) => out[p..p + 2].copy_from_slice(&((x.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes()),
            (16, 2) => out[p..p + 2].copy_from_slice(&((x.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes()),
            (16, 5) => out[p..p + 2].copy_from_slice(&to_half(x).to_le_bytes()),
            (16, _) => out[p..p + 2].copy_from_slice(&(x.round() as i32 as u16).to_le_bytes()),
            (32, 5) => out[p..p + 4].copy_from_slice(&x.to_le_bytes()),
            (32, _) => out[p..p + 4].copy_from_slice(&(x.round() as i32 as u32).to_le_bytes()),
            _ => unreachable!(),
        }
    }
    Ok(())
}

/// Per-vertex tangents from the UVs (Lengyel's method), made perpendicular to the normals.
pub fn tangents(m: &Mesh) -> Vec<[f32; 3]> {
    use glam::Vec3;
    let mut acc = vec![Vec3::ZERO; m.positions.len()];
    for t in m.indices.as_chunks::<3>().0 {
        let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
        if a.max(b).max(c) >= m.positions.len() || a.max(b).max(c) >= m.uvs.len() {
            continue;
        }
        let (p0, p1, p2) = (Vec3::from(m.positions[a]), Vec3::from(m.positions[b]), Vec3::from(m.positions[c]));
        let (u0, u1, u2) = (m.uvs[a], m.uvs[b], m.uvs[c]);
        let (e1, e2) = (p1 - p0, p2 - p0);
        let (du1, dv1, du2, dv2) = (u1[0] - u0[0], u1[1] - u0[1], u2[0] - u0[0], u2[1] - u0[1]);
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-12 {
            continue;
        }
        let t = (e1 * dv2 - e2 * dv1) / det;
        for i in [a, b, c] {
            acc[i] += t;
        }
    }
    acc.iter()
        .enumerate()
        .map(|(i, t)| {
            let n = m.normals.get(i).map_or(Vec3::Y, |n| Vec3::from(*n));
            let t = *t - n * n.dot(*t);
            let t = if t.length_squared() > 1e-12 { t.normalize() } else { n.any_orthonormal_vector() };
            t.to_array()
        })
        .collect()
}

struct Shape {
    fshp: usize,
    fvtx: usize,
    mesh: usize,
    material: usize,
}

fn shapes(r: &Rd) -> Result<(usize, Vec<Shape>)> {
    if !r.0.starts_with(b"FRES") || r.u16(0xdc)? != 1 {
        return Err("BFRES template: expected one model".into());
    }
    let m = r.off(0x28)?;
    if r.get(m, 4)? != b"FMDL" {
        return Err("BFRES template: no model".into());
    }
    let (sh, n) = (r.off(m + 0x28)?, r.u16(m + 0x6a)? as usize);
    let mats = r.off(m + 0x38)?;
    let mut out = vec![];
    for i in 0..n {
        let fshp = sh + 0x60 * i;
        if r.get(fshp, 4)? != b"FSHP" {
            return Err("BFRES template: bad shape".into());
        }
        out.push(Shape {
            fshp,
            fvtx: r.off(fshp + 0x10)?,
            mesh: r.off(fshp + 0x18)?,
            material: mats + 0xb0 * r.u16(fshp + 0x52)? as usize,
        });
    }
    Ok((m, out))
}

/// Attributes of a vertex buffer: (name, format, byte offset); `None` when it has more than one
/// buffer or an attribute this module doesn't fill.
/// A vertex buffer's attributes, (name, format, byte offset), and its stride.
type Layout = (Vec<(String, u32, usize)>, usize);

fn layout(r: &Rd, fvtx: usize) -> Result<Option<Layout>> {
    if r.u8(fvtx + 0x4d)? != 1 {
        return Ok(None);
    }
    let mut attrs = vec![];
    for a in 0..r.u8(fvtx + 0x4c)? as usize {
        let o = r.off(fvtx + 8)? + 0x10 * a;
        let name = r.str(o)?;
        if !(name == "_p0" || name == "_n0" || name == "_t0" || name.starts_with("_u") || name.starts_with("_c")) {
            return Ok(None);
        }
        attrs.push((name, r.u32(o + 8)?, r.u16(o + 0xc)? as usize));
    }
    let stride = r.u32(r.off(fvtx + 0x38)?)? as usize;
    Ok(Some((attrs, stride)))
}

/// Texture (sampler, name) pairs of a material.
fn material_textures(r: &Rd, mat: usize) -> Result<Vec<(String, String)>> {
    let samplers = r.dict(mat + 0x38)?;
    let names = r.off(mat + 0x20)?;
    let n = r.u8(mat + 0xa3)? as usize;
    (0..n.min(samplers.len())).map(|i| Ok((samplers[i].clone(), r.str(names + 8 * i)?))).collect()
}

/// The template shapes imported meshes can go into, in file order.
pub fn usable_shapes(template: &[u8]) -> Result<Vec<UsableShape>> {
    let r = Rd(template);
    let (_, shapes) = shapes(&r)?;
    let mut out = vec![];
    for (i, s) in shapes.iter().enumerate() {
        if r.u8(s.fshp + 0x5b)? != 1 || r.u8(s.fshp + 0x5a)? != 0 || layout(&r, s.fvtx)?.is_none() {
            continue;
        }
        let textures = material_textures(&r, s.material)?;
        if !textures.iter().any(|(smp, _)| smp == "_a0") || textures.iter().any(|(smp, _)| !SUPPORTED_SAMPLERS.contains(&smp.as_str())) {
            continue;
        }
        out.push(UsableShape { index: i, material: r.str(s.material + 8)?, textures });
    }
    Ok(out)
}

/// Appends strings after the `_STR` pool and remembers where each went.
struct Strings {
    at: usize,
    added: Vec<(String, usize)>,
    bytes: Vec<u8>,
}

impl Strings {
    /// File offset of the new string `s` (its length field).
    fn add(&mut self, s: &str) -> usize {
        if let Some((_, o)) = self.added.iter().find(|(x, _)| x == s) {
            return *o;
        }
        let o = self.at + self.bytes.len();
        self.bytes.extend((s.len() as u16).to_le_bytes());
        self.bytes.extend(s.as_bytes());
        self.bytes.push(0);
        if self.bytes.len() % 2 == 1 {
            self.bytes.push(0);
        }
        self.added.push((s.to_string(), o));
        o
    }
}

/// Every pointer the relocation table lists: (section, file offset of the pointer).
fn relocations(r: &Rd) -> Result<Vec<(usize, usize)>> {
    let rlt = r.u32(0x18)? as usize;
    if r.get(rlt, 4)? != b"_RLT" {
        return Err("BFRES template: no relocation table".into());
    }
    let n = r.u32(rlt + 8)? as usize;
    let entries = rlt + 0x10 + 0x18 * n;
    let mut out = vec![];
    for s in 0..n {
        let sec = rlt + 0x10 + 0x18 * s;
        let (first, count) = (r.u32(sec + 0x10)? as usize, r.u32(sec + 0x14)? as usize);
        for e in first..first + count {
            let at = entries + 8 * e;
            let (mut loc, structs, ptrs, skip) = (r.u32(at)? as usize, r.u16(at + 4)?, r.u8(at + 6)? as usize, r.u8(at + 7)? as usize);
            for _ in 0..structs {
                for p in 0..ptrs {
                    out.push((s, loc + 8 * p));
                }
                loc += 8 * (ptrs + skip);
            }
        }
    }
    Ok(out)
}

/// Rebuild `template` with the given meshes in its usable shapes (in [`usable_shapes`] order).
///
/// `file_name` is `<folder>.<model>` (the file is `Model/<file_name>.bfres.zs`), `normal` and
/// `mix` the textures every used material gets for `_n0` and `mix0`.
pub fn build(template: &[u8], file_name: &str, model_name: &str, inputs: &[ShapeInput], normal: &str, mix: &str) -> Result<Vec<u8>> {
    let r = Rd(template);
    let usable = usable_shapes(template)?;
    if inputs.len() > usable.len() {
        return Err(format!("the model template has room for {} materials, the import has {}", usable.len(), inputs.len()).into());
    }
    let (fmdl, shapes) = shapes(&r)?;
    let relocs = relocations(&r)?;
    let buf_info = r.off(0xb0)?;
    let old_buf = r.off(buf_info + 8)?;
    let old_pool = r.off(0xa8)?;
    let old_rlt = r.u32(0x18)? as usize;
    let rlt_len = template.len() - old_rlt;
    let str_pool = r.off(0xd0)?;
    let str_size = r.u32(0xd8)? as usize;
    let rlt_sections = r.u32(old_rlt + 8)? as usize;
    if rlt_sections != 6 {
        return Err(format!("BFRES template: expected 6 relocation sections, found {rlt_sections}").into());
    }

    // the vertex and index data of every shape
    struct Out {
        indices: Vec<u8>,
        format: u32,
        count: usize,
        vertices: Vec<u8>,
        nverts: usize,
        min: [f32; 3],
        max: [f32; 3],
    }
    let mut outs = vec![];
    for (i, s) in shapes.iter().enumerate() {
        let slot = usable.iter().position(|u| u.index == i);
        let input = slot.and_then(|k| inputs.get(k));
        let degenerate = ShapeInput {
            mesh: Mesh { positions: vec![[0.0; 3]], normals: vec![[0.0, 1.0, 0.0]], uvs: vec![[0.0; 2]], indices: vec![0, 0, 0] },
            albedo: String::new(),
            bake_uv: [0.0; 2],
        };
        let input = input.unwrap_or(&degenerate);
        let m = &input.mesh;
        let n = m.positions.len();
        if n == 0 || m.indices.is_empty() || m.indices.len() % 3 != 0 || m.indices.iter().any(|&x| x as usize >= n) {
            return Err(format!("imported mesh {i} has no triangles or bad indices").into());
        }
        let Some((attrs, stride)) = layout(&r, s.fvtx)? else {
            // shapes this module can't fill keep their layout but draw one degenerate triangle
            let stride = r.u32(r.off(s.fvtx + 0x38)?)? as usize;
            outs.push(Out { indices: vec![0; 6], format: 1, count: 3, vertices: vec![0; stride], nverts: 1, min: [0.0; 3], max: [0.0; 3] });
            continue;
        };
        let tang = tangents(m);
        let mut vertices = vec![0u8; n * stride];
        for v in 0..n {
            let out = &mut vertices[v * stride..(v + 1) * stride];
            for (name, format, off) in &attrs {
                let p = m.positions[v];
                let nrm = m.normals.get(v).copied().unwrap_or([0.0, 1.0, 0.0]);
                let uv = m.uvs.get(v).copied().unwrap_or([0.0; 2]);
                let val = match name.as_str() {
                    "_p0" => [p[0], p[1], p[2], 1.0],
                    "_n0" => [nrm[0], nrm[1], nrm[2], 1.0],
                    "_t0" => [tang[v][0], tang[v][1], tang[v][2], 1.0],
                    "_u0" => [uv[0], uv[1], 0.0, 0.0],
                    x if x.starts_with("_u") => [input.bake_uv[0], input.bake_uv[1], 0.0, 0.0],
                    _ => [0.0; 4], // _c*: no blend mask
                };
                encode(&mut out[*off..], *format, val)?;
            }
        }
        let wide = n > 0xffff;
        let indices: Vec<u8> = if wide {
            m.indices.iter().flat_map(|x| x.to_le_bytes()).collect()
        } else {
            m.indices.iter().flat_map(|&x| (x as u16).to_le_bytes()).collect()
        };
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &m.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        outs.push(Out { indices, format: if wide { 2 } else { 1 }, count: m.indices.len(), vertices, nverts: n, min, max });
    }

    // new strings go after the pool; the buffer region moves down by whole pages if they don't
    // fit before it
    let mut strings = Strings { at: (str_pool + str_size).next_multiple_of(2), added: vec![], bytes: vec![] };
    let file_str = strings.add(file_name);
    let model_str = strings.add(model_name);
    let normal_str = strings.add(normal);
    let mix_str = strings.add(mix);
    let albedo_strs: Vec<usize> = inputs.iter().map(|x| strings.add(&x.albedo)).collect();
    let strings_end = strings.at + strings.bytes.len();
    let new_buf = if strings_end <= old_buf { old_buf } else { strings_end.next_multiple_of(0x1000) };

    // the new buffer region: indices, then vertices
    let align = 0x40;
    let mut region = vec![];
    let mut index_at = vec![];
    for o in &outs {
        region.resize(region.len().next_multiple_of(align), 0);
        index_at.push(region.len());
        region.extend(&o.indices);
    }
    region.resize(region.len().next_multiple_of(align), 0);
    let index_size = region.len();
    let mut vertex_at = vec![];
    for o in &outs {
        region.resize(region.len().next_multiple_of(align), 0);
        vertex_at.push(region.len());
        region.extend(&o.vertices);
    }
    let region_size = region.len().next_multiple_of(0x1000);
    let new_pool = new_buf + region_size;
    let new_rlt = new_pool + (old_rlt - old_pool);
    if template[strings.at.min(old_buf)..strings_end.min(old_buf)].iter().any(|&b| b != 0) {
        return Err("BFRES template: something follows the string pool".into());
    }
    let mut d = Vec::with_capacity(new_rlt + rlt_len);
    d.extend_from_slice(&template[..old_buf]);
    d.resize(new_buf, 0);
    d[strings.at..strings_end].copy_from_slice(&strings.bytes);
    d.extend(&region);
    d.resize(new_pool, 0);
    d.extend_from_slice(&template[old_pool..old_rlt]);
    d.extend_from_slice(&template[old_rlt..]);

    // pointers into the moved region and pool
    for &(sec, loc) in &relocs {
        if loc >= old_buf {
            continue; // pointers in the pool block or the table itself: none in shipped files
        }
        let v = r.off(loc)?;
        match sec {
            1 => w64(&mut d, loc, v - old_buf + new_buf),
            4 => w64(&mut d, loc, v - old_pool + new_pool),
            _ => {}
        }
    }
    // header
    w32(&mut d, 0x18, new_rlt as u32);
    w32(&mut d, 0x1c, (new_rlt + rlt_len) as u32);
    w32(&mut d, buf_info + 4, region_size as u32);
    w32(&mut d, 0xd8, (strings_end - str_pool) as u32);
    let str_count_at = str_pool + 0x10;
    let str_count = r.u32(str_count_at)?;
    w32(&mut d, str_count_at, str_count + strings.added.len() as u32);
    w32(&mut d, 0x10, (file_str + 2) as u32);
    w64(&mut d, 0x20, file_str);
    w64(&mut d, fmdl + 8, model_str);
    let dict = r.off(0x30)?;
    w32(&mut d, dict + 0x18, dict_bit(model_name));
    w64(&mut d, dict + 0x20, model_str);
    // relocation table: its own offset and the sections after the structures
    let t = new_rlt;
    w32(&mut d, t + 4, new_rlt as u32);
    let sec = |k: usize| t + 0x10 + 0x18 * k + 8;
    w32(&mut d, sec(1), new_buf as u32);
    w32(&mut d, sec(1) + 4, index_size as u32);
    w32(&mut d, sec(2), (new_buf + index_size) as u32);
    w32(&mut d, sec(2) + 4, (region.len() - index_size) as u32);
    w32(&mut d, sec(3), new_pool as u32);
    w32(&mut d, sec(4), new_pool as u32);
    w32(&mut d, sec(5), new_rlt as u32);

    // shapes
    for (i, (s, o)) in shapes.iter().zip(&outs).enumerate() {
        // vertex buffer
        w32(&mut d, s.fvtx + 0x48, vertex_at[i] as u32);
        w32(&mut d, s.fvtx + 0x50, o.nverts as u32);
        let sizes = r.off(s.fvtx + 0x30)?;
        w32(&mut d, sizes, o.vertices.len() as u32);
        // mesh and its sub-meshes
        w32(&mut d, s.mesh + 0x20, index_at[i] as u32);
        w32(&mut d, s.mesh + 0x28, o.format);
        w32(&mut d, s.mesh + 0x2c, o.count as u32);
        w32(&mut d, s.mesh + 0x30, 0);
        let size_at = r.off(s.mesh + 0x18)?;
        w32(&mut d, size_at, o.indices.len() as u32);
        let subs = r.off(s.mesh)?;
        let nsub = r.u16(s.mesh + 0x34)? as usize;
        for k in 0..nsub {
            w32(&mut d, subs + 8 * k, 0);
            w32(&mut d, subs + 8 * k + 4, if k == 0 { o.count as u32 } else { 0 });
        }
        // bounds
        let c = [(o.min[0] + o.max[0]) / 2.0, (o.min[1] + o.max[1]) / 2.0, (o.min[2] + o.max[2]) / 2.0];
        let e = [(o.max[0] - o.min[0]) / 2.0, (o.max[1] - o.min[1]) / 2.0, (o.max[2] - o.min[2]) / 2.0];
        let boxes = r.off(s.fshp + 0x38)?;
        if boxes != 0 {
            for k in 0..nsub + 1 {
                for a in 0..3 {
                    wf32(&mut d, boxes + 0x18 * k + 4 * a, c[a]);
                    wf32(&mut d, boxes + 0x18 * k + 12 + 4 * a, e[a]);
                }
            }
        }
        let spheres = r.off(s.fshp + 0x40)?;
        if spheres != 0 {
            for (a, v) in c.iter().enumerate() {
                wf32(&mut d, spheres + 4 * a, *v);
            }
            wf32(&mut d, spheres + 12, (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt());
        }
    }

    // materials of the shapes that got a mesh
    for k in 0..inputs.len() {
        let s = &shapes[usable[k].index];
        let names = r.off(s.material + 0x20)?;
        for (j, (sampler, _)) in material_textures(&r, s.material)?.iter().enumerate() {
            let to = match sampler.as_str() {
                "_a0" => albedo_strs[k],
                "_n0" => normal_str,
                "mix0" => mix_str,
                _ => continue,
            };
            w64(&mut d, names + 8 * j, to);
        }
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves() {
        for v in [0.0f32, 1.0, -2.5, 0.333, 65504.0, 1e-5, 0.5] {
            let back = crate::bfres::half(to_half(v));
            assert!((back - v).abs() <= v.abs() * 1e-3 + 1e-6, "{v} -> {back}");
        }
        assert_eq!(to_half(1.0), 0x3c00);
        assert_eq!(to_half(-2.0), 0xc000);
    }

    #[test]
    fn packed_normals_round_trip() {
        let mut b = [0u8; 4];
        encode(&mut b, 0x0e02, [0.6, -0.8, 0.0, 1.0]).unwrap();
        let v = u32::from_le_bytes(b);
        let c = |i: u32| ((((v >> (10 * i)) & 0x3ff) << 22) as i32 >> 22) as f32 / 511.0;
        assert!((c(0) - 0.6).abs() < 0.002 && (c(1) + 0.8).abs() < 0.002 && c(2).abs() < 0.002);
        assert_eq!(v >> 30, 1);
    }

    #[test]
    fn tangents_follow_u() {
        let m = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            indices: vec![0, 2, 1],
        };
        for t in tangents(&m) {
            assert!((t[0] - 1.0).abs() < 1e-5 && t[1].abs() < 1e-5 && t[2].abs() < 1e-5, "{t:?}");
        }
    }
}
