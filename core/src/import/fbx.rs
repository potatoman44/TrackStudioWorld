//! FBX 7.x, binary (32-bit node headers before version 7500, 64-bit from it; arrays raw or
//! zlib-deflated) and ASCII.
//!
//! Both are read into the same tree of nodes, each a name, a list of properties and children.
//! Binary object names (`Name\0\x01Class`) are turned round into the ASCII form
//! (`Class::Name`).
//!
//! * Geometry: `Vertices`, `PolygonVertexIndex` (a negative index ends a polygon and is the
//!   index's bitwise not), and the first `LayerElementNormal`, `LayerElementUV` and
//!   `LayerElementMaterial`, mapped `ByPolygonVertex`, `ByVertice`/`ByVertex`/`ByControlPoint`,
//!   `ByPolygon` or `AllSame`, `Direct` or `IndexToDirect`.
//! * `Connections`: geometry to the models that place it (each is a copy), materials to models
//!   (in connection order, which `LayerElementMaterial` indexes), textures to a material's
//!   `DiffuseColor` (or a base colour property), videos to textures.
//! * Model placement: `Lcl Translation`, `Lcl Rotation` (with `RotationOrder`), `Lcl Scaling`,
//!   pivots and offsets, `PreRotation` and `PostRotation`, the `Geometric*` transforms, and
//!   the parent models, all baked into the vertices.
//! * `GlobalSettings`: the axis system (`UpAxis`, `FrontAxis`, `CoordAxis` and their signs) is
//!   turned to Y up, and `UnitScaleFactor` (centimetres per unit) scales to metres.
//! * Materials: `DiffuseColor` (or `Diffuse`) and `Opacity`; texture files from
//!   `RelativeFilename` or `FileName` on the texture or its video.
//!
//! Not read: FBX 6 and older (different object layout), NURBS and patches, skins, blend
//! shapes, embedded texture data (`Content`), and normals or UVs mapped `ByEdge`.

use super::{Builder, Corner, ImportedModel, Material, WithContext, Xform, resolve_texture};
use crate::error::{Context, Error, Result};
use glam::{DVec3, Mat4, Vec3};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

const BINARY_MAGIC: &[u8] = b"Kaydara FBX Binary  \0";
/// Nodes deeper than this are refused rather than risk the stack.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Prop {
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
}

impl Prop {
    fn f64(&self) -> Option<f64> {
        match *self {
            Prop::Int(i) => Some(i as f64),
            Prop::Float(f) => Some(f),
            _ => None,
        }
    }
    fn i64(&self) -> Option<i64> {
        match *self {
            Prop::Int(i) => Some(i),
            Prop::Float(f) => Some(f as i64),
            _ => None,
        }
    }
    fn str(&self) -> Option<&str> {
        match self {
            Prop::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Node {
    pub name: String,
    pub props: Vec<Prop>,
    pub children: Vec<Node>,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
    fn str(&self, i: usize) -> Option<&str> {
        self.props.get(i).and_then(Prop::str)
    }
    fn id(&self) -> Option<i64> {
        match self.props.first()? {
            Prop::Int(i) => Some(*i),
            _ => None,
        }
    }
    /// The node's values as numbers: its array, or (older ASCII) its list of numbers.
    fn floats(&self) -> Vec<f64> {
        match self.props.first() {
            Some(Prop::Floats(f)) => f.clone(),
            Some(Prop::Ints(i)) => i.iter().map(|&i| i as f64).collect(),
            _ => self.props.iter().filter_map(Prop::f64).collect(),
        }
    }
    fn ints(&self) -> Vec<i64> {
        match self.props.first() {
            Some(Prop::Ints(i)) => i.clone(),
            Some(Prop::Floats(f)) => f.iter().map(|&f| f as i64).collect(),
            _ => self.props.iter().filter_map(Prop::i64).collect(),
        }
    }
    /// The values of a property in `Properties70` (or the older `Properties60`).
    fn property(&self, name: &str) -> Option<&[Prop]> {
        let (list, entry, skip) = match self.child("Properties70") {
            Some(p) => (p, "P", 4),
            None => (self.child("Properties60")?, "Property", 3),
        };
        let p = list.children(entry).find(|p| p.str(0) == Some(name))?;
        Some(p.props.get(skip..).unwrap_or_default())
    }
    fn property_f64(&self, name: &str) -> Option<f64> {
        self.property(name)?.first()?.f64()
    }
    fn property_vec3(&self, name: &str) -> Option<DVec3> {
        let v = self.property(name)?;
        Some(DVec3::new(v.first()?.f64()?, v.get(1)?.f64()?, v.get(2)?.f64()?))
    }
}

/// `Class::Name` -> `Name`.
fn object_name(s: &str) -> &str {
    s.split_once("::").map_or(s, |(_, n)| n)
}

/// Read an FBX file, binary or ASCII. `dir` is the folder texture paths are relative to.
pub fn parse(data: &[u8], dir: &Path) -> Result<ImportedModel> {
    let nodes = parse_nodes(data)?;
    let root = Node { children: nodes, ..Default::default() };
    let version = if data.starts_with(BINARY_MAGIC) {
        u32::from_le_bytes(data[23..27].try_into().unwrap()) as i64
    } else {
        root.child("FBXHeaderExtension").and_then(|h| h.child("FBXVersion")).and_then(|v| v.props.first()?.i64()).unwrap_or(7000)
    };
    if version < 7000 {
        return Err(Error::new(format!("FBX version {version} is not supported (7.0 or newer is)")));
    }
    Scene::new(&root, dir).build()
}

/// The top-level nodes of an FBX file.
pub(crate) fn parse_nodes(data: &[u8]) -> Result<Vec<Node>> {
    if data.starts_with(BINARY_MAGIC) {
        parse_binary(data).context("binary FBX")
    } else {
        let text = String::from_utf8_lossy(data);
        let mut a = Ascii { s: text.trim_start_matches('\u{feff}').as_bytes(), pos: 0 };
        let nodes = a.nodes(false, 0).context("ASCII FBX")?;
        if nodes.is_empty() {
            return Err("not an FBX file".into());
        }
        Ok(nodes)
    }
}

// --- binary ---

struct Bin<'a> {
    d: &'a [u8],
    wide: bool,
}

impl Bin<'_> {
    fn get(&self, o: usize, n: usize) -> Result<&[u8]> {
        o.checked_add(n).and_then(|e| self.d.get(o..e)).ok_or_else(|| Error::new(format!("read past the end at {o:#x}")))
    }
    fn arr<const N: usize>(&self, o: usize) -> Result<[u8; N]> {
        Ok(self.get(o, N)?.try_into().unwrap())
    }
    fn u32(&self, o: usize) -> Result<usize> {
        Ok(u32::from_le_bytes(self.arr(o)?) as usize)
    }
    /// A node header field: u32, or u64 from version 7500.
    fn word(&self, o: usize) -> Result<usize> {
        if self.wide { usize::try_from(u64::from_le_bytes(self.arr(o)?)).map_err(|_| Error::new("offset too large")) } else { self.u32(o) }
    }

    /// A node record at `pos`, or None for the empty record that ends a list.
    fn node(&self, pos: &mut usize, depth: usize) -> Result<Option<Node>> {
        if depth > MAX_DEPTH {
            return Err("nodes nested too deeply".into());
        }
        let start = *pos;
        let w = if self.wide { 8 } else { 4 };
        let end = self.word(start)?;
        let count = self.word(start + w)?;
        let name_len = self.get(start + 3 * w, 1)?[0] as usize;
        *pos = start + 3 * w + 1;
        if end == 0 {
            return Ok(None);
        }
        if end <= start || end > self.d.len() {
            return Err(Error::new(format!("bad node end offset at {start:#x}")));
        }
        let name = String::from_utf8_lossy(self.get(*pos, name_len)?).into_owned();
        *pos += name_len;
        let mut props = vec![];
        for _ in 0..count {
            if *pos >= end {
                return Err(Error::new(format!("properties of {name} run past the node")));
            }
            props.push(self.prop(pos)?);
        }
        let mut children = vec![];
        while *pos < end {
            match self.node(pos, depth + 1)? {
                Some(c) => children.push(c),
                None => break,
            }
        }
        *pos = end;
        Ok(Some(Node { name, props, children }))
    }

    fn prop(&self, pos: &mut usize) -> Result<Prop> {
        let ty = self.get(*pos, 1)?[0];
        let o = *pos + 1;
        let (prop, len) = match ty {
            b'Y' => (Prop::Int(i16::from_le_bytes(self.arr(o)?) as i64), 2),
            b'C' => (Prop::Int(self.get(o, 1)?[0] as i64), 1),
            b'I' => (Prop::Int(i32::from_le_bytes(self.arr(o)?) as i64), 4),
            b'L' => (Prop::Int(i64::from_le_bytes(self.arr(o)?)), 8),
            b'F' => (Prop::Float(f32::from_le_bytes(self.arr(o)?) as f64), 4),
            b'D' => (Prop::Float(f64::from_le_bytes(self.arr(o)?)), 8),
            b'S' | b'R' => {
                let n = self.u32(o)?;
                let bytes = self.get(o + 4, n)?;
                let prop = if ty == b'S' { Prop::Str(binary_string(bytes)) } else { Prop::Bytes(bytes.to_vec()) };
                (prop, 4 + n)
            }
            b'f' | b'd' | b'l' | b'i' | b'b' => {
                let size = match ty {
                    b'd' | b'l' => 8,
                    b'f' | b'i' => 4,
                    _ => 1,
                };
                let (count, encoding, stored) = (self.u32(o)?, self.u32(o + 4)?, self.u32(o + 8)?);
                let raw = self.get(o + 12, stored)?;
                let want = count.checked_mul(size).ok_or("array too large")?;
                let bytes = match encoding {
                    0 => raw.to_vec(),
                    1 => {
                        // deflate cannot expand data more than about 1032 times
                        let limit = want.min(stored.saturating_mul(1100) + 64);
                        let mut out = vec![];
                        flate2::read::ZlibDecoder::new(raw).take(limit as u64).read_to_end(&mut out).context("array")?;
                        out
                    }
                    e => return Err(Error::new(format!("unknown array encoding {e}"))),
                };
                let bytes = bytes.get(..want).ok_or("array shorter than its count")?;
                let prop = match ty {
                    b'f' => Prop::Floats(bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c) as f64).collect()),
                    b'd' => Prop::Floats(bytes.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect()),
                    b'i' => Prop::Ints(bytes.as_chunks::<4>().0.iter().map(|c| i32::from_le_bytes(*c) as i64).collect()),
                    b'l' => Prop::Ints(bytes.as_chunks::<8>().0.iter().map(|c| i64::from_le_bytes(*c)).collect()),
                    _ => Prop::Ints(bytes.iter().map(|&b| b as i64).collect()),
                };
                (prop, 12 + stored)
            }
            t => return Err(Error::new(format!("unknown property type {:?} at {:#x}", t as char, *pos))),
        };
        *pos = o + len;
        Ok(prop)
    }
}

/// `Name\0\x01Class` -> `Class::Name`; other strings as they are.
fn binary_string(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    match s.split_once("\0\u{1}") {
        Some((name, class)) => format!("{class}::{name}"),
        None => s.into_owned(),
    }
}

fn parse_binary(data: &[u8]) -> Result<Vec<Node>> {
    let version = u32::from_le_bytes(data.get(23..27).ok_or("truncated header")?.try_into().unwrap());
    let b = Bin { d: data, wide: version >= 7500 };
    let mut pos = 27;
    let mut nodes = vec![];
    while pos < data.len() {
        match b.node(&mut pos, 0)? {
            Some(n) => nodes.push(n),
            None => break,
        }
    }
    Ok(nodes)
}

// --- ASCII ---

struct Ascii<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Ascii<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    /// Skip spaces and `;` comments, and line breaks too when `lines`.
    fn skip(&mut self, lines: bool) {
        while let Some(c) = self.peek() {
            match c {
                b';' => {
                    while self.peek().is_some_and(|c| c != b'\n') {
                        self.pos += 1;
                    }
                }
                b'\n' | b'\r' if !lines => return,
                c if c.is_ascii_whitespace() => self.pos += 1,
                _ => return,
            }
        }
    }

    fn err(&self, what: &str) -> Error {
        let line = self.s[..self.pos.min(self.s.len())].iter().filter(|&&c| c == b'\n').count() + 1;
        Error::new(format!("line {line}: {what}"))
    }

    /// Nodes up to the end of the text, or to the `}` closing a block when `block`.
    fn nodes(&mut self, block: bool, depth: usize) -> Result<Vec<Node>> {
        if depth > MAX_DEPTH {
            return Err(self.err("nodes nested too deeply"));
        }
        let mut out = vec![];
        loop {
            self.skip(true);
            match self.peek() {
                None if block => return Err(self.err("missing }")),
                None => return Ok(out),
                Some(b'}') if block => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'}') => return Err(self.err("unexpected }")),
                Some(_) => out.push(self.node(depth)?),
            }
        }
    }

    fn node(&mut self, depth: usize) -> Result<Node> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c != b':' && !c.is_ascii_whitespace() && c != b'{' && c != b'}') {
            self.pos += 1;
        }
        let name = String::from_utf8_lossy(&self.s[start..self.pos]).into_owned();
        if name.is_empty() || self.peek() != Some(b':') {
            return Err(self.err("expected a name and ':'"));
        }
        self.pos += 1;
        let mut node = Node { name, ..Default::default() };
        loop {
            self.skip(false);
            match self.peek() {
                None | Some(b'\n' | b'\r' | b'}') => return Ok(node),
                Some(b'{') => {
                    self.pos += 1;
                    node.children = self.nodes(true, depth + 1)?;
                    return Ok(node);
                }
                Some(_) => {}
            }
            let prop = self.value(depth)?;
            node.props.push(prop);
            self.skip(false);
            if self.peek() == Some(b',') {
                self.pos += 1;
                self.skip(true);
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Prop> {
        match self.peek() {
            Some(b'"') => {
                self.pos += 1;
                let start = self.pos;
                while self.peek().is_some_and(|c| c != b'"') {
                    self.pos += 1;
                }
                if self.peek().is_none() {
                    return Err(self.err("unterminated string"));
                }
                let s = String::from_utf8_lossy(&self.s[start..self.pos]).replace("&quot;", "\"");
                self.pos += 1;
                Ok(Prop::Str(s))
            }
            // `*count { a: values }`
            Some(b'*') => {
                self.pos += 1;
                self.word();
                self.skip(true);
                if self.peek() != Some(b'{') {
                    return Err(self.err("expected { after an array count"));
                }
                self.pos += 1;
                let inner = self.nodes(true, depth + 1)?;
                let values = inner.iter().find(|n| n.name == "a").map(|a| a.props.as_slice()).unwrap_or_default();
                if values.iter().all(|p| matches!(p, Prop::Int(_))) {
                    Ok(Prop::Ints(values.iter().filter_map(Prop::i64).collect()))
                } else {
                    Ok(Prop::Floats(values.iter().filter_map(Prop::f64).collect()))
                }
            }
            _ => {
                let w = self.word();
                if w.is_empty() {
                    return Err(self.err("expected a value"));
                }
                Ok(if let Ok(i) = w.parse::<i64>() {
                    Prop::Int(i)
                } else if let Ok(f) = w.parse::<f64>() {
                    Prop::Float(f)
                } else {
                    Prop::Str(w)
                })
            }
        }
    }

    /// A bare word or number.
    fn word(&mut self) -> String {
        let start = self.pos;
        while self.peek().is_some_and(|c| !c.is_ascii_whitespace() && !matches!(c, b',' | b'{' | b'}' | b';')) {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.pos]).into_owned()
    }
}

// --- the scene ---

/// How a layer element's values are looked up for a polygon corner.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Mapping {
    PolygonVertex,
    Vertex,
    Polygon,
    AllSame,
}

struct Layer {
    data: Vec<f64>,
    index: Option<Vec<i64>>,
    mapping: Mapping,
    stride: usize,
}

impl Layer {
    /// The first `element` of a geometry, its values in `values` and their indices in one of
    /// `index_names`.
    fn read(geom: &Node, element: &str, values: &str, index_names: &[&str], stride: usize) -> Option<Layer> {
        let e = geom.child(element)?;
        let mapping = match e.child("MappingInformationType").and_then(|m| m.str(0)).unwrap_or("AllSame") {
            "ByPolygonVertex" => Mapping::PolygonVertex,
            "ByVertice" | "ByVertex" | "ByControlPoint" => Mapping::Vertex,
            "ByPolygon" => Mapping::Polygon,
            "AllSame" => Mapping::AllSame,
            _ => return None,
        };
        let indexed = matches!(e.child("ReferenceInformationType").and_then(|m| m.str(0)), Some("IndexToDirect" | "Index"));
        let index = if indexed { index_names.iter().find_map(|n| e.child(n)).map(Node::ints) } else { None };
        Some(Layer { data: e.child(values)?.floats(), index, mapping, stride })
    }

    /// The value for corner `k` (counting through the whole mesh) of polygon `p`, at control
    /// point `v`.
    fn get(&self, k: usize, v: usize, p: usize) -> Option<&[f64]> {
        let i = match self.mapping {
            Mapping::PolygonVertex => k,
            Mapping::Vertex => v,
            Mapping::Polygon => p,
            Mapping::AllSame => 0,
        };
        let i = match &self.index {
            Some(ix) => usize::try_from(*ix.get(i)?).ok()?,
            None => i,
        };
        self.data.get(i * self.stride..(i + 1) * self.stride)
    }
}

struct Scene<'a> {
    objects: HashMap<i64, &'a Node>,
    /// child -> (parent, property) in file order
    parents: HashMap<i64, Vec<(i64, Option<&'a str>)>>,
    /// parent -> (child, property) in file order
    children: HashMap<i64, Vec<(i64, Option<&'a str>)>>,
    globals: Option<&'a Node>,
    dir: &'a Path,
}

impl<'a> Scene<'a> {
    fn new(root: &'a Node, dir: &'a Path) -> Scene<'a> {
        let objects = root.child("Objects").map(|o| o.children.iter().filter_map(|n| Some((n.id()?, n))).collect()).unwrap_or_default();
        let mut parents: HashMap<i64, Vec<_>> = HashMap::new();
        let mut children: HashMap<i64, Vec<_>> = HashMap::new();
        for c in root.child("Connections").into_iter().flat_map(|c| c.children("C")) {
            let (Some(child), Some(parent)) = (c.props.get(1).and_then(Prop::i64), c.props.get(2).and_then(Prop::i64)) else { continue };
            let prop = if c.str(0) == Some("OP") { c.str(3) } else { None };
            parents.entry(child).or_default().push((parent, prop));
            children.entry(parent).or_default().push((child, prop));
        }
        Scene { objects, parents, children, globals: root.child("GlobalSettings"), dir }
    }

    fn object(&self, id: i64, class: &str) -> Option<&'a Node> {
        self.objects.get(&id).copied().filter(|n| n.name == class)
    }

    fn connected(&self, list: &HashMap<i64, Vec<(i64, Option<&'a str>)>>, id: i64, class: &str) -> Vec<(i64, &'a Node, Option<&'a str>)> {
        list.get(&id).into_iter().flatten().filter_map(|&(o, p)| Some((o, self.object(o, class)?, p))).collect()
    }

    /// File axes and units to Y up and metres.
    fn global_transform(&self) -> Mat4 {
        let Some(g) = self.globals else { return Mat4::IDENTITY };
        let int = |name: &str, d: i64| g.property(name).and_then(|v| v.first()?.i64()).unwrap_or(d);
        let axis = |name: &str, sign: &str, d: i64| {
            let a = int(name, d).clamp(0, 2) as usize;
            let s = if int(sign, 1) < 0 { -1.0 } else { 1.0 };
            let mut row = [0.0f32; 3];
            row[a] = s;
            Vec3::from(row)
        };
        // rows: right -> X, up -> Y, front -> Z
        let (x, y, z) = (axis("CoordAxis", "CoordAxisSign", 0), axis("UpAxis", "UpAxisSign", 1), axis("FrontAxis", "FrontAxisSign", 2));
        let mut m = Mat4::IDENTITY;
        if x.cross(y).dot(z).abs() > 0.5 {
            m = Mat4::from_mat3(glam::Mat3::from_cols(x, y, z).transpose());
        }
        if let Some(s) = g.property_f64("UnitScaleFactor").filter(|s| s.is_finite() && *s > 0.0) {
            m *= Mat4::from_scale(Vec3::splat(s as f32 / 100.0));
        }
        m
    }

    /// A model's transform relative to its parent.
    fn local(model: &Node) -> Mat4 {
        let v = |name: &str, d: f64| model.property_vec3(name).unwrap_or(DVec3::splat(d)).as_vec3();
        let order = model.property("RotationOrder").and_then(|p| p.first()?.i64()).unwrap_or(0);
        let t = Mat4::from_translation;
        let r_off = t(v("RotationOffset", 0.0));
        let r_piv = t(v("RotationPivot", 0.0));
        let s_off = t(v("ScalingOffset", 0.0));
        let s_piv = t(v("ScalingPivot", 0.0));
        let pre = euler(v("PreRotation", 0.0), 0);
        let post = euler(v("PostRotation", 0.0), 0);
        t(v("Lcl Translation", 0.0))
            * r_off
            * r_piv
            * pre
            * euler(v("Lcl Rotation", 0.0), order)
            * post.inverse()
            * r_piv.inverse()
            * s_off
            * s_piv
            * Mat4::from_scale(v("Lcl Scaling", 1.0))
            * s_piv.inverse()
    }

    fn world(&self, id: i64, depth: usize) -> Mat4 {
        let Some(model) = self.object(id, "Model") else { return Mat4::IDENTITY };
        let parent = self.connected(&self.parents, id, "Model").first().map(|p| p.0);
        let up = match parent {
            Some(p) if depth < MAX_DEPTH => self.world(p, depth + 1),
            _ => Mat4::IDENTITY,
        };
        up * Self::local(model)
    }

    fn material(&self, id: i64, node: &Node) -> Material {
        let name = node.str(1).map(object_name).filter(|n| !n.is_empty()).map_or_else(|| format!("material{id}"), str::to_string);
        let mut m = Material::named(&name);
        if let Some(c) = node.property_vec3("DiffuseColor").or_else(|| node.property_vec3("Diffuse")) {
            m.color = [c.x as f32, c.y as f32, c.z as f32, 1.0];
        }
        if let Some(a) = node.property_f64("Opacity") {
            m.color[3] = a as f32;
        }
        let textures = self.connected(&self.children, id, "Texture");
        let diffuse = |p: &str| {
            let p = p.to_ascii_lowercase();
            p == "diffusecolor" || p.contains("basecolor") || p.contains("base_color")
        };
        let texture = textures.iter().find(|t| t.2.is_some_and(diffuse));
        m.texture = texture.and_then(|&(tid, tex, _)| {
            let videos = self.connected(&self.children, tid, "Video");
            std::iter::once(tex).chain(videos.iter().map(|v| v.1)).find_map(|n| {
                ["RelativeFilename", "FileName", "Filename"]
                    .iter()
                    .filter_map(|f| n.child(f)?.str(0))
                    .find_map(|f| resolve_texture(self.dir, f))
            })
        });
        m
    }

    fn build(&self) -> Result<ImportedModel> {
        let global = self.global_transform();
        let mut b = Builder::default();
        let mut geoms: Vec<_> = self.objects.iter().filter(|(_, n)| n.name == "Geometry").map(|(&id, &n)| (id, n)).collect();
        geoms.sort_by_key(|g| g.0);
        for (gid, geom) in geoms {
            let name = geom.str(1).map(object_name).unwrap_or_default().to_string();
            let polys = polygons(geom).with_context(|| format!("geometry {name}"))?;
            let models = self.connected(&self.parents, gid, "Model");
            let instances: Vec<(Mat4, Vec<Material>)> = if models.is_empty() {
                vec![(Mat4::IDENTITY, vec![])]
            } else {
                models
                    .iter()
                    .map(|&(mid, model, _)| {
                        let geometric = Mat4::from_translation(model.property_vec3("GeometricTranslation").unwrap_or_default().as_vec3())
                            * euler(model.property_vec3("GeometricRotation").unwrap_or_default().as_vec3(), 0)
                            * Mat4::from_scale(model.property_vec3("GeometricScaling").unwrap_or(DVec3::ONE).as_vec3());
                        let mats =
                            self.connected(&self.children, mid, "Material").into_iter().map(|(id, n, _)| self.material(id, n)).collect();
                        (self.world(mid, 0) * geometric, mats)
                    })
                    .collect()
            };
            for (m, mats) in instances {
                let xf = Xform::new(global * m);
                let parts: Vec<usize> = mats.iter().map(|m| b.part(m)).collect();
                let mut default = None;
                for (mat, corners) in &polys {
                    let part = match parts.get(*mat) {
                        Some(&p) => p,
                        None => *default.get_or_insert_with(|| b.part(&Material::named("default"))),
                    };
                    b.polygon(part, &xf, corners);
                }
            }
        }
        Ok(b.finish())
    }
}

/// Euler angles in degrees, in FBX's `RotationOrder` (0 = XYZ: X applied first).
fn euler(deg: Vec3, order: i64) -> Mat4 {
    let r = deg * (std::f32::consts::PI / 180.0);
    let (x, y, z) = (Mat4::from_rotation_x(r.x), Mat4::from_rotation_y(r.y), Mat4::from_rotation_z(r.z));
    match order {
        1 => y * z * x, // XZY
        2 => x * z * y, // YZX
        3 => z * x * y, // YXZ
        4 => y * x * z, // ZXY
        5 => x * y * z, // ZYX
        _ => z * y * x, // XYZ, spheric XYZ
    }
}

/// A geometry's polygons in its own coordinates, each with its material slot.
fn polygons(geom: &Node) -> Result<Vec<(usize, Vec<Corner>)>> {
    let verts = geom.child("Vertices").map(Node::floats).unwrap_or_default();
    let index = geom.child("PolygonVertexIndex").map(Node::ints).unwrap_or_default();
    let normals = Layer::read(geom, "LayerElementNormal", "Normals", &["NormalsIndex", "NormalIndex"], 3);
    let uvs = Layer::read(geom, "LayerElementUV", "UV", &["UVIndex"], 2);
    let materials = Layer::read(geom, "LayerElementMaterial", "Materials", &[], 1);
    let mut out = vec![];
    let mut corners = vec![];
    for (k, &raw) in index.iter().enumerate() {
        let end = raw < 0;
        let v = if end { !raw } else { raw } as usize;
        let p = verts.get(v * 3..v * 3 + 3).ok_or_else(|| Error::new(format!("vertex index {v} out of range")))?;
        let poly = out.len();
        let n = normals.as_ref().and_then(|l| l.get(k, v, poly)).map(|n| DVec3::new(n[0], n[1], n[2]).as_vec3());
        let uv = uvs.as_ref().and_then(|l| l.get(k, v, poly)).map(|t| [t[0] as f32, t[1] as f32]);
        corners.push(Corner { p: DVec3::new(p[0], p[1], p[2]).as_vec3(), n, uv });
        if end {
            let mat = materials.as_ref().and_then(|l| l.get(k, v, poly)).map_or(0, |m| m[0].max(0.0) as usize);
            out.push((mat, std::mem::take(&mut corners)));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::load;
    use super::super::tests::{assert_ccw, assert_close, temp_dir, write_png};
    use super::*;

    /// A cube from -1 to 1, as six quads: -Z, -X, +X, -Y, +Y, +Z.
    const VERTS: [f64; 24] = [-1., -1., -1., 1., -1., -1., 1., 1., -1., -1., 1., -1., -1., -1., 1., 1., -1., 1., 1., 1., 1., -1., 1., 1.];
    const QUADS: [[i32; 4]; 6] = [[0, 3, 2, 1], [0, 4, 7, 3], [1, 2, 6, 5], [0, 1, 5, 4], [3, 7, 6, 2], [4, 5, 6, 7]];
    const NORMALS: [[f64; 3]; 6] = [[0., 0., -1.], [-1., 0., 0.], [1., 0., 0.], [0., -1., 0.], [0., 1., 0.], [0., 0., 1.]];

    /// A Blender-style ASCII export: Z up, centimetres, the cube moved up 1 along Z and
    /// scaled 2x along X, normals per polygon, indexed UVs per corner, and the +Z face in a
    /// second material.
    const ASCII: &str = r#"; FBX 7.4.0 project file
; ----------------------------------------------------

FBXHeaderExtension:  {
	FBXHeaderVersion: 1003
	FBXVersion: 7400
}
GlobalSettings:  {
	Version: 1000
	Properties70:  {
		P: "UpAxis", "int", "Integer", "",2
		P: "UpAxisSign", "int", "Integer", "",1
		P: "FrontAxis", "int", "Integer", "",1
		P: "FrontAxisSign", "int", "Integer", "",-1
		P: "CoordAxis", "int", "Integer", "",0
		P: "CoordAxisSign", "int", "Integer", "",1
		P: "UnitScaleFactor", "double", "Number", "",1
	}
}
Objects:  {
	Geometry: 1000, "Geometry::Cube", "Mesh" {
		Vertices: *24 {
			a: -1,-1,-1,1,-1,-1,1,1,-1,-1,1,-1,-1,-1,1,1,-1,1,1,1,1,-1,1,1
		}
		PolygonVertexIndex: *24 {
			a: 0,3,2,-2,0,4,7,-4,1,2,6,-6,0,1,5,-5,3,7,6,-3,4,5,6,-8
		}
		GeometryVersion: 124
		LayerElementNormal: 0 {
			Version: 101
			Name: ""
			MappingInformationType: "ByPolygon"
			ReferenceInformationType: "Direct"
			Normals: *18 {
				a: 0,0,-1,-1,0,0,1,0,0,0,-1,0,0,1,0,0,0,1
			}
		}
		LayerElementUV: 0 {
			Version: 101
			Name: "UVMap"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "IndexToDirect"
			UV: *8 {
				a: 0,0,1,0,1,1,0,1
			}
			UVIndex: *24 {
				a: 0,1,2,3,0,1,2,3,0,1,2,3,0,1,2,3,0,1,2,3,0,1,2,3
			}
		}
		LayerElementMaterial: 0 {
			MappingInformationType: "ByPolygon"
			ReferenceInformationType: "IndexToDirect"
			Materials: *6 {
				a: 0,0,0,0,0,1
			}
		}
		Layer: 0 {
			Version: 100
			LayerElement:  {
				Type: "LayerElementNormal"
				TypedIndex: 0
			}
		}
	}
	Model: 2000, "Model::Cube", "Mesh" {
		Version: 232
		Properties70:  {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,0,1
			P: "Lcl Scaling", "Lcl Scaling", "", "A",2,1,1
		}
		Shading: T
		Culling: "CullingOff"
	}
	Material: 3000, "Material::Wood", "" {
		Properties70:  {
			P: "DiffuseColor", "Color", "", "A",0.8,0.8,0.8
		}
	}
	Material: 3001, "Material::Top", "" {
		Properties70:  {
			P: "DiffuseColor", "Color", "", "A",0.2,0.4,0.6
			P: "Opacity", "double", "Number", "",0.5
		}
	}
	Texture: 4000, "Texture::Wood", "" {
		Type: "TextureVideoClip"
		FileName: "D:\Projects\cube\textures\wood.png"
		RelativeFilename: "textures\wood.png"
	}
}
Connections:  {
	;Model::Cube, Model::RootNode
	C: "OO",2000,0
	C: "OO",1000,2000
	C: "OO",3000,2000
	C: "OO",3001,2000
	C: "OP",4000,3000, "DiffuseColor"
}
"#;

    #[test]
    fn reads_an_ascii_z_up_cube() {
        let d = temp_dir("fbx-ascii");
        std::fs::create_dir_all(d.join("textures")).unwrap();
        write_png(&d.join("textures/wood.png"));
        std::fs::write(d.join("cube.Fbx"), ASCII).unwrap();
        let m = load(&d.join("cube.Fbx")).unwrap();
        assert_eq!(m.triangle_count(), 12);
        let names: Vec<_> = m.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Wood", "Top"]);
        let (min, max) = m.bounds();
        // centimetres become metres
        assert_close(min, [-0.02, 0.0, -0.01]);
        assert_close(max, [0.02, 0.02, 0.01]);
        let wood = &m.parts[0];
        assert_eq!(wood.texture, Some(std::fs::canonicalize(d.join("textures/wood.png")).unwrap()));
        assert_close(wood.color[..3].try_into().unwrap(), [0.8, 0.8, 0.8]);
        assert!(wood.uvs.contains(&[0.0, 1.0]) && wood.uvs.contains(&[1.0, 0.0]));
        let top = &m.parts[1];
        assert_eq!(top.texture, None);
        assert_eq!(top.color, [0.2, 0.4, 0.6, 0.5]);
        assert!(top.normals.iter().all(|n| *n == [0.0, 1.0, 0.0]));
        assert_ccw(&m);
    }

    // --- a binary FBX writer, just enough for the tests ---

    enum P {
        I(i32),
        L(i64),
        D(f64),
        S(String),
        /// values, and whether to deflate them
        Ds(Vec<f64>, bool),
        Is(Vec<i32>, bool),
    }

    struct N {
        name: &'static str,
        props: Vec<P>,
        kids: Vec<N>,
    }

    fn n(name: &'static str, props: Vec<P>, kids: Vec<N>) -> N {
        N { name, props, kids }
    }

    fn s(v: &str) -> P {
        P::S(v.to_string())
    }

    /// An object name as binary files store it.
    fn obj(class: &str, name: &str) -> P {
        P::S(format!("{name}\0\u{1}{class}"))
    }

    fn p70(entries: Vec<(&str, Vec<P>)>) -> N {
        let ps = entries
            .into_iter()
            .map(|(name, values)| {
                let mut props = vec![s(name), s(""), s(""), s("A")];
                props.extend(values);
                n("P", props, vec![])
            })
            .collect();
        n("Properties70", vec![], ps)
    }

    fn array(out: &mut Vec<u8>, ty: u8, count: usize, raw: Vec<u8>, deflate: bool) {
        use std::io::Write;
        out.push(ty);
        out.extend_from_slice(&(count as u32).to_le_bytes());
        let data = if deflate {
            let mut e = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
            e.write_all(&raw).unwrap();
            e.finish().unwrap()
        } else {
            raw
        };
        out.extend_from_slice(&(deflate as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
    }

    fn write_node(out: &mut Vec<u8>, node: &N, wide: bool) {
        let start = out.len();
        let w = if wide { 8 } else { 4 };
        out.resize(start + 3 * w, 0);
        out.push(node.name.len() as u8);
        out.extend_from_slice(node.name.as_bytes());
        let props_start = out.len();
        for p in &node.props {
            match p {
                P::I(v) => {
                    out.push(b'I');
                    out.extend_from_slice(&v.to_le_bytes());
                }
                P::L(v) => {
                    out.push(b'L');
                    out.extend_from_slice(&v.to_le_bytes());
                }
                P::D(v) => {
                    out.push(b'D');
                    out.extend_from_slice(&v.to_le_bytes());
                }
                P::S(v) => {
                    out.push(b'S');
                    out.extend_from_slice(&(v.len() as u32).to_le_bytes());
                    out.extend_from_slice(v.as_bytes());
                }
                P::Ds(v, z) => array(out, b'd', v.len(), v.iter().flat_map(|f| f.to_le_bytes()).collect(), *z),
                P::Is(v, z) => array(out, b'i', v.len(), v.iter().flat_map(|f| f.to_le_bytes()).collect(), *z),
            }
        }
        let props_len = out.len() - props_start;
        for k in &node.kids {
            write_node(out, k, wide);
        }
        if !node.kids.is_empty() {
            out.extend(std::iter::repeat_n(0, 3 * w + 1));
        }
        let fields = [out.len(), node.props.len(), props_len];
        for (i, f) in fields.into_iter().enumerate() {
            let at = start + i * w;
            if wide {
                out[at..at + 8].copy_from_slice(&(f as u64).to_le_bytes());
            } else {
                out[at..at + 4].copy_from_slice(&(f as u32).to_le_bytes());
            }
        }
    }

    fn write_binary(nodes: &[N], version: u32) -> Vec<u8> {
        let mut out = BINARY_MAGIC.to_vec();
        out.extend_from_slice(&[0x1a, 0]);
        out.extend_from_slice(&version.to_le_bytes());
        for node in nodes {
            write_node(&mut out, node, version >= 7500);
        }
        let w = if version >= 7500 { 8 } else { 4 };
        out.extend(std::iter::repeat_n(0, 3 * w + 1 + 16));
        out
    }

    /// A Maya-style scene in metres: the cube, scaled 2x along X and raised 1, under a parent
    /// turned 90 degrees about Y and moved 10 along X. Normals per corner (deflated), UVs per
    /// control point, one material for everything, its texture's file named on the video.
    fn binary_cube(version: u32) -> Vec<u8> {
        let pvi: Vec<i32> = QUADS.iter().flat_map(|q| [q[0], q[1], q[2], !q[3]]).collect();
        let normals: Vec<f64> = NORMALS.iter().flat_map(|n| [n; 4]).flatten().copied().collect();
        let uvs: Vec<f64> = VERTS.chunks(3).flat_map(|v| [(v[0] + 1.0) / 2.0, (v[2] + 1.0) / 2.0]).collect();
        let layer = |name, mapping, reference, values: N| {
            n(
                name,
                vec![P::I(0)],
                vec![
                    n("MappingInformationType", vec![s(mapping)], vec![]),
                    n("ReferenceInformationType", vec![s(reference)], vec![]),
                    values,
                ],
            )
        };
        let nodes = vec![
            n("FBXHeaderExtension", vec![], vec![n("FBXVersion", vec![P::I(version as i32)], vec![])]),
            n(
                "GlobalSettings",
                vec![],
                vec![p70(vec![
                    ("UpAxis", vec![P::I(1)]),
                    ("UpAxisSign", vec![P::I(1)]),
                    ("FrontAxis", vec![P::I(2)]),
                    ("FrontAxisSign", vec![P::I(1)]),
                    ("CoordAxis", vec![P::I(0)]),
                    ("CoordAxisSign", vec![P::I(1)]),
                    ("UnitScaleFactor", vec![P::D(100.0)]),
                ])],
            ),
            n(
                "Objects",
                vec![],
                vec![
                    n(
                        "Geometry",
                        vec![P::L(100), obj("Geometry", "Cube"), s("Mesh")],
                        vec![
                            n("Vertices", vec![P::Ds(VERTS.to_vec(), false)], vec![]),
                            n("PolygonVertexIndex", vec![P::Is(pvi, true)], vec![]),
                            layer("LayerElementNormal", "ByPolygonVertex", "Direct", n("Normals", vec![P::Ds(normals, true)], vec![])),
                            layer("LayerElementUV", "ByVertice", "Direct", n("UV", vec![P::Ds(uvs, false)], vec![])),
                            layer("LayerElementMaterial", "AllSame", "IndexToDirect", n("Materials", vec![P::Is(vec![0], false)], vec![])),
                        ],
                    ),
                    n(
                        "Model",
                        vec![P::L(200), obj("Model", "Parent"), s("Null")],
                        vec![p70(vec![
                            ("Lcl Translation", vec![P::D(10.0), P::D(0.0), P::D(0.0)]),
                            ("Lcl Rotation", vec![P::D(0.0), P::D(90.0), P::D(0.0)]),
                        ])],
                    ),
                    n(
                        "Model",
                        vec![P::L(201), obj("Model", "Cube"), s("Mesh")],
                        vec![p70(vec![
                            ("Lcl Translation", vec![P::D(0.0), P::D(1.0), P::D(0.0)]),
                            ("Lcl Scaling", vec![P::D(2.0), P::D(1.0), P::D(1.0)]),
                        ])],
                    ),
                    n(
                        "Material",
                        vec![P::L(300), obj("Material", "Paint"), s("")],
                        vec![p70(vec![("DiffuseColor", vec![P::D(1.0), P::D(0.5), P::D(0.25)])])],
                    ),
                    n("Texture", vec![P::L(400), obj("Texture", "Paint"), s("")], vec![n("RelativeFilename", vec![s("")], vec![])]),
                    n(
                        "Video",
                        vec![P::L(500), obj("Video", "paint"), s("Clip")],
                        vec![n("RelativeFilename", vec![s("maps\\paint.png")], vec![])],
                    ),
                ],
            ),
            n(
                "Connections",
                vec![],
                vec![
                    n("C", vec![s("OO"), P::L(200), P::L(0)], vec![]),
                    n("C", vec![s("OO"), P::L(201), P::L(200)], vec![]),
                    n("C", vec![s("OO"), P::L(100), P::L(201)], vec![]),
                    n("C", vec![s("OO"), P::L(300), P::L(201)], vec![]),
                    n("C", vec![s("OP"), P::L(400), P::L(300), s("DiffuseColor")], vec![]),
                    n("C", vec![s("OO"), P::L(500), P::L(400)], vec![]),
                ],
            ),
        ];
        write_binary(&nodes, version)
    }

    #[test]
    fn reads_binary_cubes_with_parents_and_deflated_arrays() {
        let d = temp_dir("fbx-binary");
        std::fs::create_dir_all(d.join("maps")).unwrap();
        write_png(&d.join("maps/paint.png"));
        for version in [7400, 7500] {
            let path = d.join(format!("cube{version}.fbx"));
            std::fs::write(&path, binary_cube(version)).unwrap();
            let m = load(&path).unwrap();
            assert_eq!(m.triangle_count(), 12, "version {version}");
            assert_eq!(m.parts.len(), 1);
            let p = &m.parts[0];
            assert_eq!(p.name, "Paint");
            assert_eq!(p.color, [1.0, 0.5, 0.25, 1.0]);
            assert_eq!(p.texture, Some(std::fs::canonicalize(d.join("maps/paint.png")).unwrap()));
            // the child's 2x X scale ends up along Z after the parent's turn; the file is in metres
            let (min, max) = m.bounds();
            assert_close(min, [9.0, 0.0, -2.0]);
            assert_close(max, [11.0, 2.0, 2.0]);
            // vertex 0 has UV (0, 0): the image's bottom left
            assert!(p.uvs.contains(&[0.0, 1.0]));
            assert_eq!(p.positions.len(), 24);
            assert_ccw(&m);
        }
    }

    #[test]
    fn binary_and_ascii_trees_match() {
        let bin = parse_nodes(&binary_cube(7500)).unwrap();
        let objects = bin.iter().find(|n| n.name == "Objects").unwrap();
        let geom = objects.child("Geometry").unwrap();
        assert_eq!(geom.str(1), Some("Geometry::Cube"));
        assert_eq!(geom.child("PolygonVertexIndex").unwrap().ints()[3], -2);
        let ascii = parse_nodes(ASCII.as_bytes()).unwrap();
        let geom = ascii.iter().find(|n| n.name == "Objects").unwrap().child("Geometry").unwrap();
        assert_eq!(geom.child("PolygonVertexIndex").unwrap().ints()[3], -2);
        assert_eq!(geom.child("Vertices").unwrap().floats(), VERTS);
    }

    #[test]
    fn axis_systems() {
        let scene = |up: i64, up_sign: i64, front: i64, front_sign: i64| {
            let text = format!(
                "GlobalSettings: {{\n Properties70: {{\n P: \"UpAxis\", \"int\", \"Integer\", \"\",{up}\n P: \"UpAxisSign\", \"int\", \"Integer\", \"\",{up_sign}\n P: \"FrontAxis\", \"int\", \"Integer\", \"\",{front}\n P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",{front_sign}\n }}\n}}\n"
            );
            let root = Node { children: parse_nodes(text.as_bytes()).unwrap(), ..Default::default() };
            Scene::new(&root, Path::new(".")).global_transform()
        };
        let p = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(scene(1, 1, 2, 1).transform_point3(p), p);
        assert_eq!(scene(2, 1, 1, -1).transform_point3(p), Vec3::new(1.0, 3.0, -2.0));
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        let d = Path::new(".");
        assert!(parse(b"", d).is_err());
        assert!(parse(b"Objects: {", d).is_err());
        assert!(parse(b"FBXHeaderExtension: {\n FBXVersion: 6100\n}\n", d).is_err());
        let bin = binary_cube(7400);
        for len in 0..bin.len() {
            let _ = parse(&bin[..len], d);
        }
        for i in 0..bin.len() {
            let mut bad = bin.clone();
            bad[i] ^= 0xFF;
            let _ = parse(&bad, d);
        }
    }
}
