//! Importing 3D models made in other tools: Wavefront OBJ ([`obj`]), COLLADA ([`dae`]) and FBX
//! ([`fbx`]), plus the images their materials use.
//!
//! [`load`] gives triangle meshes in model space, one part per material, with the material's
//! diffuse colour and the diffuse (base colour) texture's file when it can be found.
//!
//! What every format comes out as:
//!
//! * Axes: Y up, counter-clockwise front faces. OBJ is taken to be Y up already; COLLADA follows
//!   `<up_axis>` and FBX the `GlobalSettings` axis system. Node transforms (COLLADA visual scene
//!   nodes, FBX model transforms and their parents) are baked into the vertices.
//! * Units: metres, the game's unit, when the file says what its unit is (COLLADA
//!   `<unit meter>`, FBX `UnitScaleFactor`, which is in centimetres), so a 1 m cube from either
//!   is 1 across. Otherwise, and always for OBJ (which has no units), the file's own numbers are
//!   kept.
//! * UVs: V = 0 is the image's top row. All three formats put V = 0 at the bottom, so V is
//!   flipped.
//! * Normals: unit length. Faces without them in the file get flat face normals.
//! * Vertices are shared within a part where position, normal and UV all match; polygons are
//!   triangulated as fans. Parts that end up with no triangles are left out.
//!
//! Texture files are looked for as written in the model file, then relative to the model file's
//! folder, then by bare file name in that folder.

pub mod dae;
pub mod fbx;
pub mod obj;

use crate::error::{Context, Error, Result};
use glam::{Mat3, Mat4, Vec3};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ImportedPart {
    /// the material's name, or `default` for faces without one
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    /// unit length
    pub normals: Vec<[f32; 3]>,
    /// V = 0 is the texture's first (top) row
    pub uvs: Vec<[f32; 2]>,
    /// triangles, counter-clockwise front faces
    pub indices: Vec<u32>,
    /// the diffuse (base colour) image, when the file was found
    pub texture: Option<PathBuf>,
    /// diffuse colour factor, RGBA
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Default)]
pub struct ImportedModel {
    pub parts: Vec<ImportedPart>,
}

impl ImportedModel {
    pub fn triangle_count(&self) -> usize {
        self.parts.iter().map(|p| p.indices.len() / 3).sum()
    }

    /// The smallest and largest corner of the vertices (zero for an empty model).
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in self.parts.iter().flat_map(|p| &p.positions) {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        if min[0] > max[0] { ([0.0; 3], [0.0; 3]) } else { (min, max) }
    }
}

/// Read a model file, by its extension: `.obj`, `.dae` or `.fbx`.
pub fn load(path: &Path) -> Result<ImportedModel> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let data = std::fs::read(path).path_context(path)?;
    let dir = path.parent().map(|d| std::path::absolute(d).unwrap_or_else(|_| d.to_path_buf())).unwrap_or_default();
    match ext.as_str() {
        "obj" => obj::parse(&String::from_utf8_lossy(&data), &dir),
        "dae" => dae::parse(&String::from_utf8_lossy(&data), &dir),
        "fbx" => fbx::parse(&data, &dir),
        _ => Err(Error::new(format!("unsupported model format .{ext} (expected .obj, .dae or .fbx)"))),
    }
    .path_context(path)
}

/// An image file as RGBA8, rows top to bottom: PNG, JPEG, TGA or BMP.
pub fn load_image(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::ImageReader::open(path)
        .path_context(path)?
        .with_guessed_format()
        .path_context(path)?
        .decode()
        .map_err(|e| e.to_string())
        .path_context(path)?
        .to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// Find a texture a model file names: as written, relative to the model's folder, then by bare
/// file name in that folder. Windows separators and `file://` URLs are understood.
pub(crate) fn resolve_texture(dir: &Path, name: &str) -> Option<PathBuf> {
    let name = name.trim().trim_matches('"');
    if name.is_empty() {
        return None;
    }
    let mut name = name.to_string();
    if let Some(rest) = name.strip_prefix("file://") {
        name = percent_decode(rest);
        // file:///C:/x on Windows
        if name.len() > 3 && name.starts_with('/') && name.as_bytes()[2] == b':' {
            name.remove(0);
        }
    }
    let unix = name.replace('\\', "/");
    let file_name = unix.rsplit('/').next().unwrap_or(&unix).to_string();
    let candidates = [PathBuf::from(&name), PathBuf::from(&unix), dir.join(&unix), dir.join(&file_name)];
    let found = candidates.into_iter().find(|p| p.is_file())?;
    Some(std::fs::canonicalize(&found).or_else(|_| std::path::absolute(&found)).unwrap_or(found))
}

/// Like [`Context::context`], with the message made only on failure.
pub(crate) trait WithContext<T> {
    fn with_context(self, f: impl FnOnce() -> String) -> Result<T>;
}

impl<T> WithContext<T> for Result<T> {
    fn with_context(self, f: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|e| Error::new(format!("{}: {}", f(), e.message())))
    }
}

/// `%20` and the like in a URL path.
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A material as the readers find it.
#[derive(Debug, Clone)]
pub(crate) struct Material {
    pub name: String,
    pub color: [f32; 4],
    pub texture: Option<PathBuf>,
}

impl Material {
    pub fn named(name: &str) -> Material {
        Material { name: name.to_string(), color: [1.0; 4], texture: None }
    }
}

/// One polygon corner in the file's coordinates. UVs have V = 0 at the bottom, as all three
/// formats store them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Corner {
    pub p: Vec3,
    pub n: Option<Vec3>,
    pub uv: Option<[f32; 2]>,
}

/// Where a mesh's vertices go: positions by `m`, normals by its inverse transpose. A mirroring
/// transform turns the winding round.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Xform {
    m: Mat4,
    n: Mat3,
    flip: bool,
}

impl Xform {
    pub fn new(m: Mat4) -> Xform {
        let m3 = Mat3::from_mat4(m);
        let det = m3.determinant();
        let n = if det.abs() > f32::EPSILON * 1e-6 { m3.inverse().transpose() } else { m3 };
        Xform { m, n, flip: det < 0.0 }
    }
}

/// Matrix taking a Z-up right-handed frame (Blender, 3ds Max) to Y up: (x, y, z) -> (x, z, -y).
pub(crate) fn z_up_to_y_up() -> Mat4 {
    Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

#[derive(Default)]
struct PartBuilder {
    part: ImportedPart,
    seen: HashMap<[u32; 8], u32>,
}

/// Collects polygons into parts, one per material name, sharing identical vertices.
#[derive(Default)]
pub(crate) struct Builder {
    parts: Vec<PartBuilder>,
    by_name: HashMap<String, usize>,
}

impl Builder {
    /// The part for a material, made on first use. The first material of a name decides its
    /// colour and texture.
    pub fn part(&mut self, mat: &Material) -> usize {
        if let Some(&i) = self.by_name.get(&mat.name) {
            return i;
        }
        let part = ImportedPart { name: mat.name.clone(), texture: mat.texture.clone(), color: mat.color, ..Default::default() };
        self.parts.push(PartBuilder { part, seen: HashMap::new() });
        self.by_name.insert(mat.name.clone(), self.parts.len() - 1);
        self.parts.len() - 1
    }

    /// Add a polygon (three or more corners, counter-clockwise), triangulated as a fan.
    pub fn polygon(&mut self, part: usize, xf: &Xform, corners: &[Corner]) {
        if corners.len() < 3 {
            return;
        }
        let pos: Vec<Vec3> = corners.iter().map(|c| xf.m.transform_point3(c.p)).collect();
        let mut face: Option<Vec3> = None;
        let mut face_normal = || {
            *face.get_or_insert_with(|| {
                // Newell's method: robust for non-planar and concave polygons
                let mut n = Vec3::ZERO;
                for (i, a) in pos.iter().enumerate() {
                    let b = pos[(i + 1) % pos.len()];
                    n += Vec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
                }
                n.try_normalize().unwrap_or(Vec3::Y)
            })
        };
        let pb = &mut self.parts[part];
        let mut ids = Vec::with_capacity(corners.len());
        for (c, p) in corners.iter().zip(&pos) {
            let n = c.n.and_then(|n| (xf.n * n).try_normalize()).unwrap_or_else(&mut face_normal);
            let uv = c.uv.map_or([0.0, 0.0], |[u, v]| [u, 1.0 - v]);
            let (p, n) = (p.to_array(), n.to_array());
            let key = [p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]].map(f32::to_bits);
            let part = &mut pb.part;
            let id = *pb.seen.entry(key).or_insert_with(|| {
                part.positions.push(p);
                part.normals.push(n);
                part.uvs.push(uv);
                (part.positions.len() - 1) as u32
            });
            ids.push(id);
        }
        for i in 1..ids.len() - 1 {
            let (a, b, c) = (ids[0], ids[i], ids[i + 1]);
            if a == b || b == c || a == c {
                continue;
            }
            let tri = if xf.flip { [a, c, b] } else { [a, b, c] };
            pb.part.indices.extend_from_slice(&tri);
        }
    }

    pub fn finish(self) -> ImportedModel {
        ImportedModel { parts: self.parts.into_iter().map(|p| p.part).filter(|p| !p.indices.is_empty()).collect() }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh, empty folder for one test's files.
    pub fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mkwtrack-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A 2x1 PNG: red, then green.
    pub fn write_png(path: &Path) {
        let img = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        img.save(path).unwrap();
    }

    pub fn assert_close(a: [f32; 3], b: [f32; 3]) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-4, "{a:?} != {b:?}");
        }
    }

    /// Every triangle's winding agrees with its vertex normals (counter-clockwise front faces).
    pub fn assert_ccw(m: &ImportedModel) {
        for p in &m.parts {
            for t in p.indices.chunks(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(p.positions[i as usize]));
                let n = Vec3::from(p.normals[t[0] as usize]);
                assert!((b - a).cross(c - a).dot(n) > 0.0, "triangle {t:?} of {} winds against its normal", p.name);
                assert!((n.length() - 1.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn shares_vertices_and_flips_v() {
        let mut b = Builder::default();
        let part = b.part(&Material::named("m"));
        let c = |x: f32, y: f32| Corner { p: Vec3::new(x, y, 0.0), n: None, uv: Some([x, y]) };
        let xf = Xform::new(Mat4::IDENTITY);
        b.polygon(part, &xf, &[c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0), c(0.0, 1.0)]);
        b.polygon(part, &xf, &[c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0)]);
        b.part(&Material::named("unused"));
        let m = b.finish();
        assert_eq!(m.parts.len(), 1);
        assert_eq!(m.parts[0].positions.len(), 4);
        assert_eq!(m.triangle_count(), 3);
        assert_eq!(m.parts[0].uvs[0], [0.0, 1.0]);
        assert_eq!(m.parts[0].normals[0], [0.0, 0.0, 1.0]);
        assert_ccw(&m);
    }

    #[test]
    fn mirroring_keeps_faces_front() {
        let mut b = Builder::default();
        let part = b.part(&Material::named("m"));
        let c = |x: f32, y: f32| Corner { p: Vec3::new(x, y, 0.0), n: Some(Vec3::Z), uv: None };
        b.polygon(part, &Xform::new(Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))), &[c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0)]);
        assert_ccw(&b.finish());
    }

    #[test]
    fn unknown_extension_is_an_error() {
        let d = temp_dir("ext");
        let p = d.join("a.stl");
        std::fs::write(&p, "solid").unwrap();
        assert!(load(&p).is_err());
    }

    #[test]
    fn reads_png() {
        let d = temp_dir("png");
        write_png(&d.join("t.png"));
        let (w, h, px) = load_image(&d.join("t.png")).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(px, [255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn finds_textures() {
        let d = temp_dir("resolve");
        write_png(&d.join("wood.png"));
        let want = std::fs::canonicalize(d.join("wood.png")).unwrap();
        assert_eq!(resolve_texture(&d, "wood.png"), Some(want.clone()));
        assert_eq!(resolve_texture(&d, "C:\\Users\\someone\\textures\\wood.png"), Some(want.clone()));
        assert_eq!(resolve_texture(&d, want.to_str().unwrap()), Some(want.clone()));
        assert_eq!(resolve_texture(Path::new("/nonexistent"), &format!("file://{}", want.display())), Some(want));
        assert_eq!(resolve_texture(&d, "missing.png"), None);
    }
}
