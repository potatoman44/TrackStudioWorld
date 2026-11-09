//! COLLADA (`.dae`, versions 1.4 and 1.5).
//!
//! * Geometry: `<mesh>` with `<triangles>`, `<polylist>` and `<polygons>` (holes in `<ph>` are
//!   ignored, the outline is kept), reading `VERTEX`, `NORMAL` and the first `TEXCOORD` set
//!   through `<input offset>`, from `<source>`s that may be shared and strided.
//! * Placement: the scene's `<visual_scene>` (or the first one), each node's `<matrix>`,
//!   `<translate>`, `<rotate>` and `<scale>` in order, `<instance_node>`, and the materials
//!   `<instance_geometry>` binds. Geometry no node uses is not read.
//! * Materials: `<instance_effect>`, the `profile_COMMON` technique's `<diffuse>` colour or
//!   `<texture>`, resolved through a `sampler2D` and `surface` `<newparam>` pair (1.4), a
//!   sampler's `<instance_image>` (1.5), or straight to an image id as some exporters write it.
//! * `<asset>`: `<unit meter>` scales to metres when present; `<up_axis>` `Z_UP` and
//!   `X_UP` are turned to Y up.
//!
//! Not read: `<trifans>`, `<tristrips>`, `<lines>`, skins and morphs (`<instance_controller>`),
//! `<lookat>` and `<skew>`, and external references to other files.

use super::{Builder, Corner, ImportedModel, Material, WithContext, Xform, resolve_texture};
use crate::error::{Context, Error, Result};
use glam::{Mat4, Vec3};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::path::Path;

/// Read COLLADA text. `dir` is the folder image paths are relative to.
pub fn parse(text: &str, dir: &Path) -> Result<ImportedModel> {
    let doc = Document::parse(text).map_err(|e| e.to_string()).context("COLLADA")?;
    let root = doc.root_element();
    if root.tag_name().name() != "COLLADA" {
        return Err("not a COLLADA document".into());
    }
    let ids: HashMap<&str, Node> = root.descendants().filter_map(|n| Some((n.attribute("id")?, n))).collect();
    let mut r = Reader { ids, dir, materials: HashMap::new() };

    let asset = child(root, "asset");
    let mut base = Mat4::IDENTITY;
    if let Some(m) = asset.and_then(|a| child(a, "unit")).and_then(|u| u.attribute("meter")).and_then(|m| m.trim().parse::<f32>().ok()) {
        base = Mat4::from_scale(Vec3::splat(m));
    }
    match asset.and_then(|a| child(a, "up_axis")).and_then(|u| u.text()).map(str::trim) {
        Some("Z_UP") => base = super::z_up_to_y_up() * base,
        // X up, -Y right: (x, y, z) -> (-y, x, z)
        Some("X_UP") => base = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * base,
        _ => {}
    }

    let mut b = Builder::default();
    let scene = child(root, "scene")
        .and_then(|s| child(s, "instance_visual_scene"))
        .and_then(|i| r.lookup(i.attribute("url")?))
        .or_else(|| root.descendants().find(|n| n.has_tag_name_local("visual_scene")));
    match scene {
        Some(scene) => {
            for node in scene.children().filter(|n| n.has_tag_name_local("node")) {
                r.node(node, base, &mut b, 0)?;
            }
        }
        None => {
            for g in root.descendants().filter(|n| n.has_tag_name_local("geometry")) {
                r.geometry(g, &HashMap::new(), base, &mut b)?;
            }
        }
    }
    Ok(b.finish())
}

trait LocalName {
    fn has_tag_name_local(&self, name: &str) -> bool;
}

impl LocalName for Node<'_, '_> {
    fn has_tag_name_local(&self, name: &str) -> bool {
        self.is_element() && self.tag_name().name() == name
    }
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.has_tag_name_local(name))
}

fn children<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| c.has_tag_name_local(name))
}

fn floats(n: Node) -> Vec<f32> {
    n.text().unwrap_or_default().split_ascii_whitespace().map(|t| t.parse().unwrap_or(0.0)).collect()
}

fn ints(n: Node) -> Vec<i64> {
    n.text().unwrap_or_default().split_ascii_whitespace().map(|t| t.parse().unwrap_or(-1)).collect()
}

/// An element's local transform: its transform children, applied in document order.
fn local_transform(node: Node) -> Mat4 {
    let mut m = Mat4::IDENTITY;
    for c in node.children().filter(Node::is_element) {
        let f = floats(c);
        let get = |i: usize| f.get(i).copied().unwrap_or(0.0);
        let t = match c.tag_name().name() {
            // row major
            "matrix" if f.len() >= 16 => Mat4::from_cols_slice(&f[..16]).transpose(),
            "translate" => Mat4::from_translation(Vec3::new(get(0), get(1), get(2))),
            "scale" => Mat4::from_scale(Vec3::new(get(0), get(1), get(2))),
            "rotate" => match Vec3::new(get(0), get(1), get(2)).try_normalize() {
                Some(axis) => Mat4::from_axis_angle(axis, get(3).to_radians()),
                None => continue,
            },
            _ => continue,
        };
        m *= t;
    }
    m
}

struct Reader<'a, 'i, 'p> {
    ids: HashMap<&'a str, Node<'a, 'i>>,
    dir: &'p Path,
    materials: HashMap<String, Material>,
}

impl<'a, 'i> Reader<'a, 'i, '_> {
    /// An element by `#id` (or bare id).
    fn lookup(&self, url: &str) -> Option<Node<'a, 'i>> {
        self.ids.get(url.trim().trim_start_matches('#')).copied()
    }

    fn node(&mut self, node: Node<'a, 'i>, parent: Mat4, b: &mut Builder, depth: usize) -> Result<()> {
        if depth > 256 {
            return Err("COLLADA: nodes nested too deeply (or instancing themselves)".into());
        }
        let m = parent * local_transform(node);
        for c in node.children().filter(Node::is_element) {
            match c.tag_name().name() {
                "node" => self.node(c, m, b, depth + 1)?,
                "instance_node" => {
                    if let Some(n) = c.attribute("url").and_then(|u| self.lookup(u)) {
                        self.node(n, m, b, depth + 1)?;
                    }
                }
                "instance_geometry" => {
                    let Some(g) = c.attribute("url").and_then(|u| self.lookup(u)) else { continue };
                    // material symbol -> material id
                    let bind: HashMap<String, String> = c
                        .descendants()
                        .filter(|n| n.has_tag_name_local("instance_material"))
                        .filter_map(|n| Some((n.attribute("symbol")?.to_string(), n.attribute("target")?.to_string())))
                        .collect();
                    self.geometry(g, &bind, m, b)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn material(&mut self, symbol: Option<&str>, bind: &HashMap<String, String>) -> Material {
        let Some(symbol) = symbol else { return Material::named("default") };
        let target = bind.get(symbol).map(String::as_str).unwrap_or(symbol);
        if let Some(m) = self.materials.get(target) {
            return m.clone();
        }
        let m = match self.lookup(target).filter(|n| n.has_tag_name_local("material")) {
            Some(node) => self.read_material(node),
            None => Material::named(symbol),
        };
        self.materials.insert(target.to_string(), m.clone());
        m
    }

    fn read_material(&self, node: Node) -> Material {
        let name = node.attribute("name").or(node.attribute("id")).unwrap_or("default");
        let mut mat = Material::named(name);
        let Some(effect) = child(node, "instance_effect").and_then(|i| self.lookup(i.attribute("url")?)) else { return mat };
        let Some(diffuse) = effect
            .descendants()
            .find(|n| n.has_tag_name_local("technique"))
            .and_then(|t| t.children().filter(Node::is_element).find_map(|shading| child(shading, "diffuse")))
        else {
            return mat;
        };
        if let Some(c) = child(diffuse, "color") {
            let f = floats(c);
            if f.len() >= 3 {
                mat.color = [f[0], f[1], f[2], f.get(3).copied().unwrap_or(1.0)];
            }
        }
        if let Some(tex) = child(diffuse, "texture").and_then(|t| t.attribute("texture")) {
            mat.texture = self.image_file(effect, tex).and_then(|f| resolve_texture(self.dir, &f));
        }
        mat
    }

    /// The image file a `<texture texture="...">` means within an effect.
    fn image_file(&self, effect: Node, sampler: &str) -> Option<String> {
        let param = |sid: &str| effect.descendants().find(|n| n.has_tag_name_local("newparam") && n.attribute("sid") == Some(sid));
        let image_id = match param(sampler).and_then(|p| child(p, "sampler2D")) {
            Some(s) => match child(s, "source").and_then(|s| s.text()) {
                // 1.4: sampler2D -> surface -> image
                Some(surface) => {
                    let surface = param(surface.trim()).and_then(|p| child(p, "surface"))?;
                    child(surface, "init_from")?.text()?.trim().to_string()
                }
                // 1.5: sampler2D -> instance_image
                None => child(s, "instance_image")?.attribute("url")?.trim_start_matches('#').to_string(),
            },
            // the texture names the image itself
            None => sampler.to_string(),
        };
        let image = self.lookup(&image_id).filter(|n| n.has_tag_name_local("image"))?;
        let init = child(image, "init_from")?;
        let file = child(init, "ref").unwrap_or(init).text()?;
        Some(super::percent_decode(file.trim()))
    }

    fn geometry(&mut self, g: Node<'a, 'i>, bind: &HashMap<String, String>, m: Mat4, b: &mut Builder) -> Result<()> {
        let Some(mesh) = child(g, "mesh") else { return Ok(()) };
        let gname = g.attribute("id").unwrap_or("geometry").to_string();
        let xf = Xform::new(m);
        for prim in mesh.children().filter(Node::is_element) {
            let kind = prim.tag_name().name();
            if !matches!(kind, "triangles" | "polylist" | "polygons") {
                continue;
            }
            let mat = self.material(prim.attribute("material"), bind);
            let part = b.part(&mat);
            let inputs = Inputs::read(self, prim).with_context(|| format!("COLLADA geometry {gname}"))?;
            let stride = inputs.stride;
            let mut corners = vec![];
            let mut emit = |idx: &[i64], b: &mut Builder| -> Result<()> {
                corners.clear();
                for c in idx.chunks_exact(stride) {
                    corners.push(inputs.corner(c).with_context(|| format!("COLLADA geometry {gname}"))?);
                }
                b.polygon(part, &xf, &corners);
                Ok(())
            };
            match kind {
                "triangles" => {
                    let p = child(prim, "p").map(ints).unwrap_or_default();
                    for tri in p.chunks_exact(3 * stride) {
                        emit(tri, b)?;
                    }
                }
                "polylist" => {
                    let p = child(prim, "p").map(ints).unwrap_or_default();
                    let counts = child(prim, "vcount").map(ints).unwrap_or_default();
                    let mut at = 0;
                    for n in counts {
                        let len = n.max(0) as usize * stride;
                        let poly =
                            p.get(at..at + len).ok_or_else(|| Error::new(format!("COLLADA geometry {gname}: vcount runs past <p>")))?;
                        emit(poly, b)?;
                        at += len;
                    }
                }
                _ => {
                    let outlines = children(prim, "p").chain(children(prim, "ph").filter_map(|ph| child(ph, "p")));
                    for p in outlines {
                        emit(&ints(p), b)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// A `<source>`'s values, `stride` per element.
struct Source {
    data: Vec<f32>,
    stride: usize,
    offset: usize,
}

impl Source {
    fn read(node: Node) -> Result<Source> {
        let data = child(node, "float_array").map(floats).unwrap_or_default();
        let acc = child(node, "technique_common").and_then(|t| child(t, "accessor"));
        let attr = |name: &str, d: usize| acc.and_then(|a| a.attribute(name)).and_then(|s| s.parse().ok()).unwrap_or(d);
        Ok(Source { data, stride: attr("stride", 1).max(1), offset: attr("offset", 0) })
    }

    fn get<const N: usize>(&self, i: i64) -> Result<[f32; N]> {
        let start = usize::try_from(i).ok().map(|i| self.offset + i * self.stride);
        let mut out = [0.0; N];
        for (k, o) in out.iter_mut().enumerate().take(self.stride.min(N)) {
            *o = start.and_then(|s| self.data.get(s + k)).copied().ok_or_else(|| Error::new(format!("index {i} out of range")))?;
        }
        Ok(out)
    }
}

/// Which source each slot of a primitive's index tuples reads.
struct Inputs {
    stride: usize,
    position: (usize, Source),
    normal: Option<(usize, Source)>,
    uv: Option<(usize, Source)>,
}

impl Inputs {
    fn read(r: &Reader, prim: Node) -> Result<Inputs> {
        let source = |url: &str| r.lookup(url).ok_or_else(|| Error::new(format!("missing source {url}"))).and_then(Source::read);
        let (mut position, mut normal, mut uv) = (None, None, None);
        let mut uv_set = i64::MAX;
        let mut stride = 1;
        for input in children(prim, "input") {
            let (Some(sem), Some(url)) = (input.attribute("semantic"), input.attribute("source")) else { continue };
            let offset: usize = input.attribute("offset").and_then(|o| o.parse().ok()).unwrap_or(0);
            stride = stride.max(offset + 1);
            match sem {
                "VERTEX" => {
                    let vertices = r.lookup(url).ok_or_else(|| Error::new(format!("missing vertices {url}")))?;
                    for vi in children(vertices, "input") {
                        let Some(src) = vi.attribute("source") else { continue };
                        match vi.attribute("semantic") {
                            Some("POSITION") => position = Some((offset, source(src)?)),
                            Some("NORMAL") if normal.is_none() => normal = Some((offset, source(src)?)),
                            Some("TEXCOORD") if uv.is_none() => uv = Some((offset, source(src)?)),
                            _ => {}
                        }
                    }
                }
                "NORMAL" => normal = Some((offset, source(url)?)),
                "TEXCOORD" => {
                    let set = input.attribute("set").and_then(|s| s.parse().ok()).unwrap_or(0);
                    if set < uv_set {
                        uv_set = set;
                        uv = Some((offset, source(url)?));
                    }
                }
                _ => {}
            }
        }
        let position = position.ok_or("no POSITION input")?;
        Ok(Inputs { stride, position, normal, uv })
    }

    fn corner(&self, idx: &[i64]) -> Result<Corner> {
        let p = self.position.1.get::<3>(idx[self.position.0])?;
        let n = self.normal.as_ref().map(|(o, s)| s.get::<3>(idx[*o])).transpose()?;
        let uv = self.uv.as_ref().map(|(o, s)| s.get::<2>(idx[*o])).transpose()?;
        Ok(Corner { p: Vec3::from(p), n: n.map(Vec3::from), uv })
    }
}

#[cfg(test)]
mod tests {
    use super::super::load;
    use super::super::tests::{assert_ccw, assert_close, temp_dir, write_png};
    use super::*;

    /// A Blender-style export: Z up, centimetre units, a cube 2 across (in file units) centred
    /// on the origin with its top as a separate material, moved up 1 along Z and scaled 2x
    /// along X by its node. The top is a `<polylist>` quad with no normals, the rest are
    /// `<triangles>` with normals and UVs in one shared index stream.
    fn cube_dae(image_from: &str) -> String {
        format!(
            r##"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <asset><unit name="centimeter" meter="0.01"/><up_axis>Z_UP</up_axis></asset>
  <library_images>
    <image id="wood_png" name="wood_png"><init_from>{image_from}</init_from></image>
  </library_images>
  <library_effects>
    <effect id="Wood-effect"><profile_COMMON>
      <newparam sid="wood_png-surface"><surface type="2D"><init_from>wood_png</init_from></surface></newparam>
      <newparam sid="wood_png-sampler"><sampler2D><source>wood_png-surface</source></sampler2D></newparam>
      <technique sid="common"><lambert>
        <diffuse><texture texture="wood_png-sampler" texcoord="UVMap"/></diffuse>
      </lambert></technique>
    </profile_COMMON></effect>
    <effect id="Top-effect"><profile_COMMON><technique sid="common"><phong>
      <diffuse><color sid="diffuse">0.2 0.4 0.6 1</color></diffuse>
    </phong></technique></profile_COMMON></effect>
    <effect id="Direct-effect"><profile_COMMON><technique sid="common"><blinn>
      <diffuse><texture texture="wood_png" texcoord="UVMap"/></diffuse>
    </blinn></technique></profile_COMMON></effect>
  </library_effects>
  <library_materials>
    <material id="Wood-material" name="Wood"><instance_effect url="#Wood-effect"/></material>
    <material id="Top-material" name="Top"><instance_effect url="#Top-effect"/></material>
  </library_materials>
  <library_geometries>
    <geometry id="Cube-mesh" name="Cube"><mesh>
      <source id="Cube-pos">
        <float_array id="Cube-pos-array" count="24">-1 -1 -1  1 -1 -1  1 1 -1  -1 1 -1  -1 -1 1  1 -1 1  1 1 1  -1 1 1</float_array>
        <technique_common><accessor source="#Cube-pos-array" count="8" stride="3">
          <param name="X" type="float"/><param name="Y" type="float"/><param name="Z" type="float"/>
        </accessor></technique_common>
      </source>
      <source id="Cube-nrm">
        <float_array id="Cube-nrm-array" count="15">0 0 -1  -1 0 0  1 0 0  0 -1 0  0 1 0</float_array>
        <technique_common><accessor source="#Cube-nrm-array" count="5" stride="3"/></technique_common>
      </source>
      <source id="Cube-uv">
        <float_array id="Cube-uv-array" count="8">0 0 1 0 1 1 0 1</float_array>
        <technique_common><accessor source="#Cube-uv-array" count="4" stride="2"/></technique_common>
      </source>
      <vertices id="Cube-vtx"><input semantic="POSITION" source="#Cube-pos"/></vertices>
      <triangles material="Wood-material" count="10">
        <input semantic="VERTEX" source="#Cube-vtx" offset="0"/>
        <input semantic="NORMAL" source="#Cube-nrm" offset="1"/>
        <input semantic="TEXCOORD" source="#Cube-uv" offset="2" set="0"/>
        <p>0 0 0 3 0 3 2 0 2  0 0 0 2 0 2 1 0 1
           0 1 0 4 1 1 7 1 2  0 1 0 7 1 2 3 1 3
           1 2 0 2 2 1 6 2 2  1 2 0 6 2 2 5 2 3
           0 3 0 1 3 1 5 3 2  0 3 0 5 3 2 4 3 3
           3 4 0 7 4 1 6 4 2  3 4 0 6 4 2 2 4 3</p>
      </triangles>
      <polylist material="Top-material" count="1">
        <input semantic="VERTEX" source="#Cube-vtx" offset="0"/>
        <vcount>4</vcount>
        <p>4 5 6 7</p>
      </polylist>
    </mesh></geometry>
  </library_geometries>
  <library_visual_scenes>
    <visual_scene id="Scene" name="Scene">
      <node id="Parent" name="Parent">
        <translate>0 0 1</translate>
        <node id="Cube" name="Cube" type="NODE">
          <matrix sid="transform">2 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1</matrix>
          <instance_geometry url="#Cube-mesh" name="Cube">
            <bind_material><technique_common>
              <instance_material symbol="Wood-material" target="#Wood-material"/>
              <instance_material symbol="Top-material" target="#Top-material"/>
            </technique_common></bind_material>
          </instance_geometry>
        </node>
      </node>
    </visual_scene>
  </library_visual_scenes>
  <scene><instance_visual_scene url="#Scene"/></scene>
</COLLADA>
"##
        )
    }

    #[test]
    fn reads_a_z_up_cube_with_a_node_transform() {
        let d = temp_dir("dae-cube");
        std::fs::create_dir_all(d.join("textures")).unwrap();
        write_png(&d.join("textures/wood.png"));
        std::fs::write(d.join("cube.DAE"), cube_dae("textures/wood.png")).unwrap();
        let m = load(&d.join("cube.DAE")).unwrap();
        assert_eq!(m.triangle_count(), 12);
        let names: Vec<_> = m.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Wood", "Top"]);
        // file X in -2..2 after the node, Z in 0..2 -> Y in 0..2, file Y -> -Z; cm stay cm
        let (min, max) = m.bounds();
        // centimetres become metres
        assert_close(min, [-0.02, 0.0, -0.01]);
        assert_close(max, [0.02, 0.02, 0.01]);
        let wood = &m.parts[0];
        assert_eq!(wood.texture, Some(std::fs::canonicalize(d.join("textures/wood.png")).unwrap()));
        assert_eq!(wood.color, [1.0; 4]);
        assert!(wood.uvs.contains(&[0.0, 1.0]) && wood.uvs.contains(&[1.0, 0.0]));
        let top = &m.parts[1];
        assert_eq!(top.color, [0.2, 0.4, 0.6, 1.0]);
        assert!(top.normals.iter().all(|n| *n == [0.0, 1.0, 0.0]));
        assert_ccw(&m);
    }

    #[test]
    fn metre_units_and_direct_image_ids_work() {
        let d = temp_dir("dae-units");
        write_png(&d.join("wood.png"));
        let text = cube_dae("C:\\exports\\wood.png").replace(r#"meter="0.01""#, r#"meter="1""#).replace("#Wood-effect", "#Direct-effect");
        std::fs::write(d.join("cube.dae"), text).unwrap();
        let m = load(&d.join("cube.dae")).unwrap();
        let (min, max) = m.bounds();
        assert_close(min, [-2.0, 0.0, -1.0]);
        assert_close(max, [2.0, 2.0, 1.0]);
        assert_eq!(m.parts[0].texture, Some(std::fs::canonicalize(d.join("wood.png")).unwrap()));
    }

    #[test]
    fn broken_documents_are_errors() {
        let d = Path::new(".");
        assert!(parse("<COLLADA>", d).is_err());
        assert!(parse("<other/>", d).is_err());
        let bad = cube_dae("x.png").replace("0 0 0 3 0 3 2 0 2", "0 0 0 3 0 3 99 0 2");
        assert!(parse(&bad, d).is_err());
    }
}
