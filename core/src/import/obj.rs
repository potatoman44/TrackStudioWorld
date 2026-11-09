//! Wavefront OBJ: `v`, `vt`, `vn` and `f` (with negative, relative indices), `usemtl`, and the
//! `.mtl` files `mtllib` names (`Kd`, `d` or `Tr`, and `map_Kd`, options and all).
//!
//! OBJ has no units or up axis; positions are kept as they are and taken to be Y up. Groups,
//! objects, smoothing groups, lines and points are ignored.

use super::{Builder, Corner, Material, Xform, resolve_texture};
use crate::error::{Error, Result};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::path::Path;

/// Read OBJ text. `dir` is the folder `mtllib` and texture names are relative to.
pub fn parse(text: &str, dir: &Path) -> Result<super::ImportedModel> {
    let mut v: Vec<Vec3> = vec![];
    let mut vt: Vec<[f32; 2]> = vec![];
    let mut vn: Vec<Vec3> = vec![];
    let mut materials: HashMap<String, Material> = HashMap::new();
    let mut b = Builder::default();
    let xf = Xform::new(Mat4::IDENTITY);
    let mut part = None;
    let mut corners = vec![];
    for (n, line) in logical_lines(text).into_iter().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let (key, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        let err = |what: &str| Error::new(format!("line {}: {what}", n + 1));
        match key {
            "v" => {
                let f = floats(rest);
                v.push(Vec3::new(f.first().copied().ok_or_else(|| err("bad vertex"))?, at(&f, 1), at(&f, 2)));
            }
            "vt" => {
                let f = floats(rest);
                vt.push([f.first().copied().ok_or_else(|| err("bad texture coordinate"))?, at(&f, 1)]);
            }
            "vn" => {
                let f = floats(rest);
                vn.push(Vec3::new(f.first().copied().ok_or_else(|| err("bad normal"))?, at(&f, 1), at(&f, 2)));
            }
            "f" => {
                corners.clear();
                for c in rest.split_whitespace() {
                    let mut it = c.split('/');
                    let p = index(it.next(), v.len()).ok_or_else(|| err(&format!("bad face corner {c}")))?;
                    let t = index(it.next(), vt.len());
                    let nn = index(it.next(), vn.len());
                    corners.push(Corner { p: v[p], n: nn.map(|i| vn[i]), uv: t.map(|i| vt[i]) });
                }
                let p = *part.get_or_insert_with(|| b.part(&Material::named("default")));
                b.polygon(p, &xf, &corners);
            }
            "usemtl" => {
                let name = if rest.is_empty() { "default" } else { rest };
                let mat = materials.get(name).cloned().unwrap_or_else(|| Material::named(name));
                part = Some(b.part(&mat));
            }
            "mtllib" => {
                // usually one file name, which may contain spaces; else several
                let files = if dir.join(rest).is_file() { vec![rest] } else { rest.split_whitespace().collect() };
                for f in files {
                    if let Ok(data) = std::fs::read(dir.join(f)) {
                        parse_mtl(&String::from_utf8_lossy(&data), dir, &mut materials);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(b.finish())
}

/// Lines with `\` continuations joined.
fn logical_lines(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for line in text.lines() {
        match line.strip_suffix('\\') {
            Some(head) => {
                cur.push_str(head);
                cur.push(' ');
            }
            None => {
                cur.push_str(line);
                out.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn floats(s: &str) -> Vec<f32> {
    s.split_whitespace().map_while(|t| t.parse().ok()).collect()
}

fn at(f: &[f32], i: usize) -> f32 {
    f.get(i).copied().unwrap_or(0.0)
}

/// A 1-based (or negative, counted back from the end) OBJ index into a list of `len`.
fn index(s: Option<&str>, len: usize) -> Option<usize> {
    let i: i64 = s.filter(|s| !s.is_empty())?.parse().ok()?;
    let i = if i < 0 { len as i64 + i } else { i - 1 };
    (0..len as i64).contains(&i).then_some(i as usize)
}

/// Add the materials of an MTL file.
fn parse_mtl(text: &str, dir: &Path, out: &mut HashMap<String, Material>) {
    let mut cur: Option<Material> = None;
    for line in logical_lines(text) {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let (key, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        if key == "newmtl" {
            if let Some(m) = cur.take() {
                out.insert(m.name.clone(), m);
            }
            cur = Some(Material::named(rest));
            continue;
        }
        let Some(m) = cur.as_mut() else { continue };
        let f = floats(rest);
        match key {
            "Kd" if !f.is_empty() => {
                let g = f.get(1).copied().unwrap_or(f[0]);
                let b = f.get(2).copied().unwrap_or(f[0]);
                m.color = [f[0], g, b, m.color[3]];
            }
            "d" if !f.is_empty() => m.color[3] = f[0],
            "Tr" if !f.is_empty() => m.color[3] = 1.0 - f[0],
            "map_Kd" => m.texture = resolve_texture(dir, map_file(rest)),
            _ => {}
        }
    }
    if let Some(m) = cur {
        out.insert(m.name.clone(), m);
    }
}

/// The file name of a `map_*` statement, after its options (`-o 0.5 0.5`, `-clamp on`, ...).
fn map_file(rest: &str) -> &str {
    let mut s = rest.trim_start();
    while s.starts_with('-') {
        let (opt, tail) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
        s = tail.trim_start();
        // how many values the option takes at most: numbers for these, one word for the rest
        let max_numbers = match opt {
            "-o" | "-s" | "-t" => 3,
            "-mm" => 2,
            "-bm" | "-boost" | "-texres" => 1,
            _ => 0,
        };
        if max_numbers == 0 {
            let (_, tail) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
            s = tail.trim_start();
            continue;
        }
        for _ in 0..max_numbers {
            let (word, tail) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
            if word.parse::<f32>().is_err() || tail.trim().is_empty() {
                break;
            }
            s = tail.trim_start();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::tests::{assert_ccw, assert_close, temp_dir, write_png};
    use super::super::{ImportedModel, load};
    use super::*;

    /// A unit cube from (0,0,0) to (1,2,3), quads, six faces: the top in `top`, the rest in
    /// `sides`.
    const CUBE: &str = "\
mtllib cube.mtl
v 0 0 0
v 1 0 0
v 1 2 0
v 0 2 0
v 0 0 3
v 1 0 3
v 1 2 3
v 0 2 3
vt 0 0
vt 1 0
vt 1 1
vt 0 1
vn 0 0 -1
vn 0 0 1
vn -1 0 0
vn 1 0 0
vn 0 -1 0
vn 0 1 0
usemtl sides
f 1/1/1 4/4/1 3/3/1 2/2/1
f 5/1/2 6/2/2 7/3/2 8/4/2
f -8/1/3 -4/2/3 -1/3/3 -5/4/3
f 2/1/4 3/2/4 7/3/4 6/4/4
# no normals here: they are worked out from the winding
f 1/1 2/2 6/3 5/4
usemtl top
f 4//6 8//6 7//6 3//6
";

    const MTL: &str = "\
newmtl sides
Kd 0.5 0.25 1.0
d 0.5
map_Kd -o 0.1 0.2 -clamp on -bm 1 tex/wood.png

newmtl top
Kd 1 0 0
Tr 0.25
";

    fn cube(tag: &str) -> (ImportedModel, std::path::PathBuf) {
        let d = temp_dir(tag);
        std::fs::create_dir_all(d.join("tex")).unwrap();
        write_png(&d.join("tex/wood.png"));
        std::fs::write(d.join("cube.obj"), CUBE).unwrap();
        std::fs::write(d.join("cube.mtl"), MTL).unwrap();
        (load(&d.join("cube.obj")).unwrap(), d)
    }

    #[test]
    fn reads_a_cube_with_materials() {
        let (m, d) = cube("obj-cube");
        assert_eq!(m.triangle_count(), 12);
        let names: Vec<_> = m.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["sides", "top"]);
        let (min, max) = m.bounds();
        assert_close(min, [0.0, 0.0, 0.0]);
        assert_close(max, [1.0, 2.0, 3.0]);
        let sides = &m.parts[0];
        assert_eq!(sides.color, [0.5, 0.25, 1.0, 0.5]);
        assert_eq!(sides.texture, Some(std::fs::canonicalize(d.join("tex/wood.png")).unwrap()));
        // 5 quads, 4 corners each, none shared (each face has its own normal)
        assert_eq!(sides.positions.len(), 20);
        // vt 0 0 is the image's bottom left
        assert!(sides.uvs.contains(&[0.0, 1.0]));
        let top = &m.parts[1];
        assert_eq!(top.color, [1.0, 0.0, 0.0, 0.75]);
        assert_eq!(top.texture, None);
        assert!(top.normals.iter().all(|n| *n == [0.0, 1.0, 0.0]));
        assert_ccw(&m);
    }

    #[test]
    fn faces_without_materials_and_bad_indices() {
        let d = temp_dir("obj-plain");
        std::fs::write(d.join("a.obj"), "v 0 0 0\nv 1 0 0\nv 0 0 -1\nf 1 2 3\n").unwrap();
        let m = load(&d.join("a.obj")).unwrap();
        assert_eq!(m.parts.len(), 1);
        assert_eq!(m.parts[0].name, "default");
        assert_eq!(m.parts[0].normals[0], [0.0, 1.0, 0.0]);
        std::fs::write(d.join("b.obj"), "v 0 0 0\nf 1 2 3\n").unwrap();
        assert!(load(&d.join("b.obj")).is_err());
    }

    #[test]
    fn map_options_are_skipped() {
        assert_eq!(map_file("wood.png"), "wood.png");
        assert_eq!(map_file("-s 2 2 -o 1 my wood.png"), "my wood.png");
        assert_eq!(map_file("-clamp on -imfchan r a.tga"), "a.tga");
        assert_eq!(map_file("-bm 0.5 b.png"), "b.png");
    }
}
