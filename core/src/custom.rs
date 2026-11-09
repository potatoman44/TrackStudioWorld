//! Custom objects: imported models written into a mod as new object types.
//!
//! An import becomes an object type the game loads like its own, built from a shipped one (the
//! template, [`TEMPLATE_ACTOR`], a static block wall):
//!
//! * `Model/<folder>.<model>.bfres.zs`: the template model with the imported meshes (see
//!   [`crate::bfres_write`]);
//! * `Tex/<model>_<n>_Alb.bntx.zs` per material, `Tex/<model>_Nrm` (flat) and `Tex/<model>_Mix`
//!   (the template's material mask averaged to one value);
//! * the object's definitions, copied from the template's and renamed: `ActorGParam`,
//!   `ActorInfoGParam`, `ModelInfoGParam` and `FmdbExSummaryGParam`, added to the
//!   `System/GymlArchive` packs that hold the template's (the mod gets whole copies of those
//!   packs);
//! * a row in `RSDB/Actor` for the new type;
//! * resource size table entries for all of them.
//!
//! The names follow the shipped pattern: folder `CmnObj_Custom<Name>000`, model `<folder>_A`,
//! object type `<model>_00`.
//!
//! Objects have no collision yet: the template's physics component is left out.
//!
//! What a mod holds is listed in `<mod>/custom_objects.json` (next to `romfs/`, so it is never
//! copied to the console), which the editor reads to offer the types in Assets.

use crate::bfres_write::{self, Mesh, ShapeInput};
use crate::bntx::{self, FORMAT_RGBA8_SRGB, FORMAT_RGBA8_UNORM, Image};
use crate::byml::{self, Byml};
use crate::catalog::{ActorInfo, Catalog};
use crate::error::{Context, Result};
use crate::io::{compress, read_file, write_atomic};
use crate::restbl::ResourceSizeTable;
use crate::sarc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The shipped object type every import is copied from.
pub const TEMPLATE_ACTOR: &str = "CmnObj_BattleBlockWallFirstCircuit000_A_00";
const TEMPLATE_FOLDER: &str = "CmnObj_BattleBlockWallFirstCircuit000";
const TEMPLATE_MODEL: &str = "CmnObj_BattleBlockWallFirstCircuit000_A";
/// The manifest of a mod's custom objects, in the mod's folder.
pub const MANIFEST: &str = "custom_objects.json";
/// Albedo textures larger than this are halved until they fit.
const MAX_TEXTURE: u32 = 2048;
const RSDB_ACTOR: &str = "RSDB/Actor.Product.100.rstbl.byml.zs";

/// One material of an import.
#[derive(Debug, Clone)]
pub struct ImportPart {
    pub mesh: Mesh,
    /// the albedo image, rows top to bottom
    pub image: Option<Image>,
    /// multiplied into the image (or the whole colour when there is none)
    pub color: [f32; 4],
    /// how brightly it is lit: the brightness percentile (0 to 1) of the baked-lighting texel
    /// every vertex samples
    pub light: f32,
}

/// How one material of a custom object is drawn (the model editor's settings).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MaterialSettings {
    /// the material's name in the source model
    pub name: String,
    /// its image, a PNG in the object's source folder
    pub image: Option<String>,
    /// multiplied into the image and the material colour
    pub tint: [f32; 4],
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    pub flip_v: bool,
    /// see [`ImportPart::light`]
    pub light: f32,
    pub visible: bool,
}

impl Default for MaterialSettings {
    fn default() -> Self {
        MaterialSettings {
            name: String::new(),
            image: None,
            tint: [1.0; 4],
            uv_scale: [1.0; 2],
            uv_offset: [0.0; 2],
            flip_v: false,
            light: 0.75,
            visible: true,
        }
    }
}

/// How a custom object's source model is turned into the game model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelSettings {
    pub scale: f32,
    /// degrees about X, then Y, then Z
    pub rotate: [f32; 3],
    /// added after scaling and rotating
    pub offset: [f32; 3],
    /// move the model so its origin is at the middle of its bottom
    pub ground: bool,
    pub materials: Vec<MaterialSettings>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        ModelSettings { scale: 1.0, rotate: [0.0; 3], offset: [0.0; 3], ground: false, materials: vec![] }
    }
}

/// An imported object type, as listed in the mod's manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomObject {
    /// the name given at import (`<Name>` in the file names)
    pub name: String,
    /// shown in Assets
    pub label: String,
    /// the object type, e.g. `CmnObj_CustomRamp000_A_00`
    pub actor: String,
    /// `Model/<folder>.<model>.bfres.zs`
    pub model: String,
    pub aabb: [[f32; 3]; 2],
    /// the file it was imported from
    pub source: String,
    /// every romfs file the import wrote
    pub files: Vec<String>,
    /// how the source is turned into the model (absent for objects imported before the model
    /// editor, which have no source kept)
    #[serde(default)]
    pub settings: ModelSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub objects: Vec<CustomObject>,
}

/// Names are letters and digits, starting with a letter, at most 24 long.
pub fn valid_name(name: &str) -> bool {
    (1..=24).contains(&name.len()) && name.starts_with(|c: char| c.is_ascii_alphabetic()) && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// (folder, model, object type) for an import name.
pub fn names(name: &str) -> (String, String, String) {
    let folder = format!("CmnObj_Custom{name}000");
    let model = format!("{folder}_A");
    let actor = format!("{model}_00");
    (folder, model, actor)
}

/// The manifest of the mod in `mod_dir` (`<mods>/<mod>`); empty when there is none.
pub fn read_manifest(mod_dir: &Path) -> Manifest {
    std::fs::read(mod_dir.join(MANIFEST)).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

fn write_manifest(mod_dir: &Path, m: &Manifest) -> Result<()> {
    write_atomic(&mod_dir.join(MANIFEST), serde_json::to_string_pretty(m)?.as_bytes())
}

/// Catalog entries for custom objects: the template's entry with their name, label, model and
/// bounds.
pub fn catalog_entries(cat: &Catalog, objects: &[CustomObject]) -> Vec<(String, ActorInfo)> {
    let base = cat.data.actors.get(TEMPLATE_ACTOR).cloned().unwrap_or_default();
    objects
        .iter()
        .map(|o| {
            let info = ActorInfo {
                gyaml: format!("Work/Gyml/ActorGParam/{}.gear__ActorGParam.gyml", o.actor),
                label: o.label.clone(),
                category: "StaticObj".into(),
                model: Some(o.model.clone()),
                aabb: Some(o.aabb),
                placed: false,
                ..base.clone()
            };
            (o.actor.clone(), info)
        })
        .collect()
}

/// A romfs file from the mod when it has one, else from the game.
fn read_layered(romfs: &Path, mod_romfs: &Path, rel: &str) -> Result<Vec<u8>> {
    let m = mod_romfs.join(rel);
    read_file(&if m.is_file() { m } else { romfs.join(rel) })
}

/// Every `System/GymlArchive` pack of the game, by romfs path.
fn gyml_packs(romfs: &Path) -> Result<Vec<String>> {
    let dir = romfs.join("System/GymlArchive");
    let mut v: Vec<String> = std::fs::read_dir(&dir)
        .path_context(&dir)?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".sarc.zs"))
        .map(|f| format!("System/GymlArchive/{f}"))
        .collect();
    v.sort();
    Ok(v)
}

fn work(key: &str) -> String {
    format!("Work/{}", key.strip_suffix(".bgyml").map_or_else(|| key.to_string(), |s| format!("{s}.gyml")))
}

fn set_xyz(m: &mut Byml, key: &str, v: [f32; 3]) {
    let mut p = Byml::map();
    for (k, x) in ["X", "Y", "Z"].iter().zip(v) {
        p.insert(k, Byml::F32(x));
    }
    m.insert(key, p);
}

/// The texel of a baked-lighting texture to light every imported vertex with: the one at the
/// `light` percentile of brightness (0.75 by default: lit, but not a highlight), as a UV at its
/// centre.
fn bake_texel(img: &Image, light: f32) -> [f32; 2] {
    let mut lum: Vec<(u32, usize)> = img
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .map(|(i, p)| (p[0] as u32 * 2126 + p[1] as u32 * 7152 + p[2] as u32 * 722, i))
        .collect();
    lum.sort_unstable();
    let at = (lum.len().saturating_sub(1) as f32 * light.clamp(0.0, 1.0)).round() as usize;
    let i = lum.get(at).map_or(0, |x| x.1);
    let (w, h) = (img.width.max(1) as usize, img.height.max(1) as usize);
    [((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32]
}

fn solid(w: u32, h: u32, c: [u8; 4]) -> Image {
    Image { width: w, height: h, rgba: c.iter().copied().cycle().take((w * h * 4) as usize).collect() }
}

fn albedo(part: &ImportPart) -> Image {
    let to8 = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let Some(img) = &part.image else {
        return solid(4, 4, part.color.map(to8));
    };
    let mut img = img.clone();
    while img.width > MAX_TEXTURE || img.height > MAX_TEXTURE {
        img = bntx::half_size(&img, true);
    }
    if part.color != [1.0; 4] {
        for p in img.rgba.as_chunks_mut::<4>().0 {
            for (x, f) in p.iter_mut().zip(part.color) {
                *x = (*x as f32 * f.clamp(0.0, 1.0) + 0.5) as u8;
            }
        }
    }
    img
}

/// Parts that end up looking the same (same image and colour) share a material.
fn merge_parts(parts: Vec<ImportPart>) -> Vec<ImportPart> {
    let mut out: Vec<ImportPart> = vec![];
    for p in parts {
        let same = out.iter_mut().find(|o| {
            o.color == p.color
                && o.light == p.light
                && o.image.as_ref().map(|i| (i.width, i.height, &i.rgba)) == p.image.as_ref().map(|i| (i.width, i.height, &i.rgba))
        });
        match same {
            Some(o) => {
                let base = o.mesh.positions.len() as u32;
                o.mesh.positions.extend(&p.mesh.positions);
                o.mesh.normals.extend(&p.mesh.normals);
                o.mesh.uvs.extend(&p.mesh.uvs);
                o.mesh.indices.extend(p.mesh.indices.iter().map(|i| i + base));
            }
            None => out.push(p),
        }
    }
    out
}

/// The size table entry for a file the import writes (from the patterns in the shipped table):
/// textures their size + 8200; packs their size rounded up to 32, + 4096; BYML tables their size
/// rounded up to 32, + 288. Models get the template's entry less its size (it depends on the
/// model's structures, which the import keeps), plus a page to spare.
fn size_entry(rel: &str, size: usize, model_overhead: u32) -> u32 {
    let up32 = |n: usize| ((n + 31) & !31) as u32;
    if rel.ends_with(".bntx.zs") {
        size as u32 + 8200
    } else if rel.ends_with(".sarc.zs") {
        up32(size) + 4096
    } else if rel.ends_with(".bfres.zs") {
        size as u32 + model_overhead + 0x1000
    } else {
        up32(size) + 288
    }
}

/// Every resource size table of the game (`System/Resource/ResourceSizeTable.*`).
pub fn restbl_rels(romfs: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(romfs.join("System/Resource"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.starts_with("ResourceSizeTable.") && f.ends_with(".rsizetable.zs"))
        .map(|f| format!("System/Resource/{f}"))
        .collect();
    v.sort();
    v
}

/// What an import wrote.
#[derive(Debug, Clone)]
pub struct Imported {
    pub object: CustomObject,
    /// romfs-relative paths written, including packs and tables that were rewritten
    pub written: Vec<String>,
    pub notes: Vec<String>,
}

/// Write `parts` as the custom object `name` into the mod in `mod_dir` (`<mods>/<mod>`; its
/// files go to `mod_dir/romfs`), and list it in the manifest with `settings`. Writing a name
/// again replaces it.
fn write_object(
    romfs: &Path,
    mod_dir: &Path,
    name: &str,
    label: &str,
    source: &str,
    parts: Vec<ImportPart>,
    settings: ModelSettings,
) -> Result<Imported> {
    if !valid_name(name) {
        return Err(format!("\"{name}\" can't be used as a name: use 1 to 24 letters and digits, starting with a letter").into());
    }
    let parts = merge_parts(parts.into_iter().filter(|p| !p.mesh.indices.is_empty()).collect());
    if parts.is_empty() {
        return Err("the model has no triangles".into());
    }
    let mod_romfs = mod_dir.join("romfs");
    let (folder, model, actor) = names(name);
    let mut notes = vec![];

    // the template model and the textures its materials sample
    let template_rel = format!("Model/{TEMPLATE_FOLDER}.{TEMPLATE_MODEL}.bfres.zs");
    let template = read_file(&romfs.join(&template_rel)).context("reading the template model")?;
    let usable = bfres_write::usable_shapes(&template)?;
    if parts.len() > usable.len() {
        return Err(format!(
            "the model has {} materials; at most {} can be imported. Merge materials, or bake their textures into one.",
            parts.len(),
            usable.len()
        )
        .into());
    }
    let tex = |u: &bfres_write::UsableShape, sampler: &str| u.textures.iter().find(|(s, _)| s == sampler).map(|(_, t)| t.clone());

    let mut files: Vec<(String, Vec<u8>)> = vec![];
    let mut inputs = vec![];
    let mut bounds = ([f32::MAX; 3], [f32::MIN; 3]);
    for (i, part) in parts.iter().enumerate() {
        let alb = format!("{model}_{i}_Alb");
        files.push((format!("Tex/{alb}.bntx.zs"), bntx::write(&alb, &albedo(part), FORMAT_RGBA8_SRGB)?));
        let bake_uv = tex(&usable[i], "bake_shadow0")
            .and_then(|t| bntx::load(romfs, &t, 128).ok())
            .map_or([0.5, 0.5], |img| bake_texel(&img, part.light));
        for p in &part.mesh.positions {
            for (k, v) in p.iter().enumerate() {
                bounds.0[k] = bounds.0[k].min(*v);
                bounds.1[k] = bounds.1[k].max(*v);
            }
        }
        inputs.push(ShapeInput { mesh: part.mesh.clone(), albedo: alb, bake_uv });
    }
    let nrm = format!("{model}_Nrm");
    files.push((format!("Tex/{nrm}.bntx.zs"), bntx::write(&nrm, &solid(4, 4, [128, 128, 255, 255]), FORMAT_RGBA8_UNORM)?));
    let mix_value = tex(&usable[0], "mix0").and_then(|t| bntx::load(romfs, &t, 64).ok()).map_or([128, 0, 255, 255], |img| {
        let n = (img.rgba.len() / 4).max(1) as u64;
        let mut s = [0u64; 4];
        for p in img.rgba.as_chunks::<4>().0 {
            for c in 0..4 {
                s[c] += p[c] as u64;
            }
        }
        s.map(|x| (x / n) as u8)
    });
    let mix = format!("{model}_Mix");
    files.push((format!("Tex/{mix}.bntx.zs"), bntx::write(&mix, &solid(4, 4, mix_value), FORMAT_RGBA8_UNORM)?));
    let model_rel = format!("Model/{folder}.{model}.bfres.zs");
    let bfres = bfres_write::build(&template, &format!("{folder}.{model}"), &model, &inputs, &nrm, &mix)?;
    crate::bfres::load_model(&bfres).context("the written model does not read back")?;

    // bounds for the definitions
    let (mn, mx) = bounds;
    let center = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
    let radius = ((mx[0] - mn[0]).powi(2) + (mx[1] - mn[1]).powi(2) + (mx[2] - mn[2]).powi(2)).sqrt() / 2.0;

    // definitions, added to the packs that hold the template's
    let t_actor = format!("Gyml/ActorGParam/{TEMPLATE_ACTOR}.gear__ActorGParam.bgyml");
    let t_info = format!("Gyml/ActorInfoGParam/{TEMPLATE_ACTOR}.game__ActorInfoGParam.bgyml");
    let t_minfo = format!("Gyml/Component/ModelInfoGParam/{TEMPLATE_ACTOR}.gear__ModelInfoGParam.bgyml");
    let t_summary = format!("Gyml/FmdbExSummaryGParam/{TEMPLATE_FOLDER}.{TEMPLATE_MODEL}.gear__FmdbExSummaryGParam.bgyml");
    let n_actor = format!("Gyml/ActorGParam/{actor}.gear__ActorGParam.bgyml");
    let n_info = format!("Gyml/ActorInfoGParam/{actor}.game__ActorInfoGParam.bgyml");
    let n_minfo = format!("Gyml/Component/ModelInfoGParam/{actor}.gear__ModelInfoGParam.bgyml");
    let n_summary = format!("Gyml/FmdbExSummaryGParam/{folder}.{model}.gear__FmdbExSummaryGParam.bgyml");
    let mut packs: BTreeMap<String, BTreeMap<String, Vec<u8>>> = BTreeMap::new();
    let mut home: BTreeMap<String, String> = BTreeMap::new(); // template key -> pack
    for p in gyml_packs(romfs)? {
        let files = sarc::read(&read_layered(romfs, &mod_romfs, &p)?).context(p.clone())?;
        let mut wanted = false;
        for k in [&t_actor, &t_info, &t_minfo, &t_summary] {
            if files.contains_key(k.as_str()) {
                home.insert(k.clone(), p.clone());
                wanted = true;
            }
        }
        if wanted {
            packs.insert(p, files);
        }
    }
    let gyml = |key: &str| -> Result<(Byml, u16)> {
        let pack = home.get(key).ok_or_else(|| format!("the template's {key} is not in any GymlArchive pack"))?;
        byml::from_bytes(&packs[pack][key]).context(key.to_string())
    };
    let (mut a, va) = gyml(&t_actor)?;
    if let Some(c) = a.get_mut("Components") {
        c.insert("ModelInfoRef", Byml::Str(work(&n_minfo)));
        c.remove("PhysicsRef");
    }
    a.insert("Label", Byml::Str(label.to_string()));
    let (mut info, vi) = gyml(&t_info)?;
    let template_radius = info.get("ModelRadius").and_then(Byml::as_f64).unwrap_or(1.0) as f32;
    let template_actor_radius = info.get("ActorRadius").and_then(Byml::as_f64).unwrap_or(1.0) as f32;
    info.insert("ActorPath", Byml::Str(work(&n_actor)));
    info.insert("ModelInfoPath", Byml::Str(work(&n_minfo)));
    info.insert("IsUseStaticCompound", Byml::Bool(false));
    set_xyz(&mut info, "ModelAABBMin", mn);
    set_xyz(&mut info, "ModelAABBMax", mx);
    set_xyz(&mut info, "ModelCenter", center);
    info.insert("ModelRadius", Byml::F32(radius));
    info.insert("ActorRadius", Byml::F32(radius * (template_actor_radius / template_radius.max(1e-3)).max(1.0)));
    let (mut minfo, vm) = gyml(&t_minfo)?;
    for r in minfo.array_mut("ModelResource").iter_mut() {
        if r.get("Fmdb").is_some() {
            r.insert("Fmdb", Byml::Str(format!("Work/Model/CmnObj/{folder}/output/{model}.fmdb")));
        }
    }
    let (mut summary, vs) = gyml(&t_summary)?;
    set_xyz(&mut summary, "BoundingMin", mn);
    set_xyz(&mut summary, "BoundingMax", mx);
    set_xyz(&mut summary, "SphereBBCenter", center);
    summary.insert("SphereBBRadius", Byml::F32(radius));
    for (tk, nk, doc, v) in [
        (&t_actor, &n_actor, &a, va),
        (&t_info, &n_info, &info, vi),
        (&t_minfo, &n_minfo, &minfo, vm),
        (&t_summary, &n_summary, &summary, vs),
    ] {
        let pack = home[tk].clone();
        packs.get_mut(&pack).unwrap().insert(nk.clone(), byml::to_bytes(doc, v, false)?);
    }
    for (p, f) in &packs {
        files.push((p.clone(), sarc::write(f)?));
    }

    // the RSDB row
    let (mut rsdb, rv) = byml::from_bytes(&read_layered(romfs, &mod_romfs, RSDB_ACTOR)?).context(RSDB_ACTOR)?;
    let row_id = work(&n_actor);
    if let Some(rows) = rsdb.as_array_mut() {
        rows.retain(|r| r.get("__RowId").and_then(Byml::as_str) != Some(row_id.as_str()));
        let mut row = Byml::map();
        row.insert("Category", Byml::Str("StaticObj".into()));
        row.insert("Label", Byml::Str(label.to_string()));
        row.insert("__RowId", Byml::Str(row_id.clone()));
        let at = rows.partition_point(|r| r.get("__RowId").and_then(Byml::as_str).unwrap_or("") < row_id.as_str());
        rows.insert(at, row);
    } else {
        return Err(format!("{RSDB_ACTOR}: expected a list of rows").into());
    }
    files.push((RSDB_ACTOR.to_string(), byml::to_bytes(&rsdb, rv, false)?));

    // write everything, then the size tables
    let template_entry = restbl_rels(romfs)
        .iter()
        .filter_map(|t| read_file(&romfs.join(t)).ok().and_then(|d| ResourceSizeTable::parse(&d).ok()))
        .filter_map(|t| t.get(&template_rel))
        .max()
        .unwrap_or(template.len() as u32 * 2);
    let overhead = template_entry.saturating_sub(template.len() as u32);
    let mut sizes = vec![];
    let mut written = vec![];
    files.push((model_rel.clone(), bfres));
    for (rel, data) in &files {
        write_atomic(&mod_romfs.join(rel), &compress(data)?)?;
        sizes.push((rel.clone(), size_entry(rel, data.len(), overhead)));
        written.push(rel.clone());
    }
    for t in restbl_rels(romfs) {
        let target = mod_romfs.join(&t);
        let src = if target.exists() { target.clone() } else { romfs.join(&t) };
        let mut table = ResourceSizeTable::parse(&read_file(&src)?).path_context(&src)?;
        for (rel, need) in &sizes {
            if table.get(rel).is_none_or(|h| h < *need) {
                table.set(rel, *need);
            }
        }
        write_atomic(&target, &compress(&table.to_bytes())?)?;
        written.push(t);
    }
    if parts.len() < inputs.len() {
        notes.push("materials with the same texture and colour were merged".into());
    }

    let object = CustomObject {
        name: name.to_string(),
        label: label.to_string(),
        actor,
        model: model_rel,
        aabb: [mn, mx],
        source: source.to_string(),
        files: files.iter().map(|(r, _)| r.clone()).filter(|r| !r.starts_with("System/") && !r.starts_with("RSDB/")).collect(),
        settings,
    };
    let mut manifest = read_manifest(mod_dir);
    match manifest.objects.iter_mut().find(|o| o.name == name) {
        Some(o) => *o = object.clone(),
        None => manifest.objects.push(object.clone()),
    }
    write_manifest(mod_dir, &manifest)?;
    Ok(Imported { object, written, notes })
}

// ---------------------------------------------------------------------------------------------
// Sources and the model editor
//
// An import keeps what it was made from in `<mod>/custom_src/<name>/`: the meshes as read
// (`source.json`) and each material's image (`<n>.png`). Every edit rebuilds the object from
// those and the object's [`ModelSettings`], so edits never pile up on each other.

/// Where an object's source is kept.
pub fn source_dir(mod_dir: &Path, name: &str) -> PathBuf {
    mod_dir.join("custom_src").join(name)
}

/// Whether the object was imported with its source kept (so the model editor can change it).
pub fn has_source(mod_dir: &Path, name: &str) -> bool {
    source_dir(mod_dir, name).join("source.json").is_file()
}

fn save_png(path: &Path, img: &Image) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).path_context(dir)?;
    }
    image::save_buffer(path, &img.rgba, img.width, img.height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

fn load_png(path: &Path) -> Result<Image> {
    let (width, height, rgba) = crate::import::load_image(path)?;
    Ok(Image { width, height, rgba })
}

/// Import `model` as the custom object `name` into the mod in `mod_dir` (`<mods>/<mod>`; its
/// files go to `mod_dir/romfs`), keeping its source for the model editor. `scale` is the
/// starting [`ModelSettings::scale`]. Importing a name again replaces it.
pub fn import(
    romfs: &Path,
    mod_dir: &Path,
    name: &str,
    label: &str,
    source: &str,
    model: &crate::import::ImportedModel,
    scale: f32,
) -> Result<Imported> {
    if !valid_name(name) {
        return Err(format!("\"{name}\" can't be used as a name: use 1 to 24 letters and digits, starting with a letter").into());
    }
    let dir = source_dir(mod_dir, name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).path_context(&dir)?;
    }
    let mut kept = model.clone();
    let mut materials = vec![];
    for (i, p) in kept.parts.iter_mut().enumerate() {
        let mut m = MaterialSettings { name: p.name.clone(), ..Default::default() };
        if let Some(t) = p.texture.take() {
            match load_png(&t) {
                Ok(img) => {
                    let file = format!("{i}.png");
                    save_png(&dir.join(&file), &img)?;
                    m.image = Some(file);
                }
                Err(e) => eprintln!("texture {}: {e}", t.display()),
            }
        }
        materials.push(m);
    }
    std::fs::create_dir_all(&dir).path_context(&dir)?;
    write_atomic(&dir.join("source.json"), &serde_json::to_vec(&kept)?)?;
    let settings = ModelSettings { scale, materials, ..Default::default() };
    build(romfs, mod_dir, name, label, source, settings)
}

/// Rebuild the custom object `name` from its kept source with `settings`, and relabel it.
pub fn update(romfs: &Path, mod_dir: &Path, name: &str, label: &str, settings: ModelSettings) -> Result<Imported> {
    let obj = read_manifest(mod_dir)
        .objects
        .into_iter()
        .find(|o| o.name == name)
        .ok_or_else(|| format!("no custom object \"{name}\" in this mod"))?;
    build(romfs, mod_dir, name, label, &obj.source, settings)
}

/// Give material `index` of the object `name` a new image (copied into its source folder as a
/// PNG). The object is not rebuilt: call [`update`] with its settings, which now name the image.
pub fn set_texture(mod_dir: &Path, name: &str, index: usize, image: &Path) -> Result<ModelSettings> {
    if !has_source(mod_dir, name) {
        return Err("this object was imported before the model editor: import it again to edit it".into());
    }
    let img = load_png(image)?;
    let file = format!("{index}.png");
    save_png(&source_dir(mod_dir, name).join(&file), &img)?;
    let mut manifest = read_manifest(mod_dir);
    let obj = manifest.objects.iter_mut().find(|o| o.name == name).ok_or_else(|| format!("no custom object \"{name}\" in this mod"))?;
    let m = obj.settings.materials.get_mut(index).ok_or("no such material")?;
    m.image = Some(file);
    let settings = obj.settings.clone();
    write_manifest(mod_dir, &manifest)?;
    Ok(settings)
}

/// The PNG of material `index`'s image in the object's source folder, if it has one.
pub fn material_image(mod_dir: &Path, name: &str, settings: &ModelSettings, index: usize) -> Option<PathBuf> {
    let file = settings.materials.get(index)?.image.as_ref()?;
    Some(source_dir(mod_dir, name).join(file)).filter(|p| p.is_file())
}

/// Turn the kept source into the parts to write: transform, UVs, tint, lighting, visibility.
fn build(romfs: &Path, mod_dir: &Path, name: &str, label: &str, source: &str, settings: ModelSettings) -> Result<Imported> {
    use glam::{EulerRot, Mat3, Vec3};
    if !has_source(mod_dir, name) {
        return Err("this object was imported before the model editor: import it again to edit it".into());
    }
    let dir = source_dir(mod_dir, name);
    let model: crate::import::ImportedModel = serde_json::from_slice(&std::fs::read(dir.join("source.json")).path_context(&dir)?)?;
    let r = settings.rotate.map(f32::to_radians);
    // X first, then Y, then Z
    let rot = Mat3::from_euler(EulerRot::ZYX, r[2], r[1], r[0]);
    let scale = if settings.scale.is_finite() && settings.scale > 0.0 { settings.scale } else { 1.0 };
    let mut parts = vec![];
    for (i, p) in model.parts.iter().enumerate() {
        let m = settings.materials.get(i).cloned().unwrap_or_default();
        if !m.visible {
            continue;
        }
        let image = match &m.image {
            Some(f) => Some(load_png(&dir.join(f))?),
            None => None,
        };
        let positions = p.positions.iter().map(|v| (rot * (Vec3::from(*v) * scale) + Vec3::from(settings.offset)).to_array()).collect();
        let normals = p.normals.iter().map(|n| (rot * Vec3::from(*n)).normalize_or(Vec3::Y).to_array()).collect();
        let uvs = p
            .uvs
            .iter()
            .map(|uv| {
                let v = if m.flip_v { 1.0 - uv[1] } else { uv[1] };
                [uv[0] * m.uv_scale[0] + m.uv_offset[0], v * m.uv_scale[1] + m.uv_offset[1]]
            })
            .collect();
        let color = [0, 1, 2, 3].map(|c| p.color[c] * m.tint[c]);
        let mesh = Mesh { positions, normals, uvs, indices: p.indices.clone() };
        parts.push(ImportPart { mesh, image, color, light: m.light });
    }
    if settings.ground {
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for v in parts.iter().flat_map(|p| &p.mesh.positions) {
            mn = mn.min(Vec3::from(*v));
            mx = mx.max(Vec3::from(*v));
        }
        if mn.x <= mx.x {
            // the bottom's middle goes to the origin, then the offset applies (already in mn, mx)
            let off = Vec3::from(settings.offset);
            let shift = off - Vec3::new((mn.x + mx.x) / 2.0, mn.y, (mn.z + mx.z) / 2.0);
            for p in parts.iter_mut() {
                for v in p.mesh.positions.iter_mut() {
                    *v = (Vec3::from(*v) + shift).to_array();
                }
            }
        }
    }
    write_object(romfs, mod_dir, name, label, source, parts, settings)
}

/// Remove the custom object `name` from the mod: its model, textures and source, its
/// definitions in the mod's GymlArchive packs and its `RSDB/Actor` row. Returns the romfs files
/// changed or removed. (Its size table entries are left: unused entries are harmless.)
pub fn delete(mod_dir: &Path, name: &str) -> Result<Vec<String>> {
    let mut manifest = read_manifest(mod_dir);
    let obj =
        manifest.objects.iter().find(|o| o.name == name).cloned().ok_or_else(|| format!("no custom object \"{name}\" in this mod"))?;
    let mod_romfs = mod_dir.join("romfs");
    let mut changed = vec![];
    for f in &obj.files {
        let p = mod_romfs.join(f);
        if p.is_file() {
            std::fs::remove_file(&p).path_context(&p)?;
            changed.push(f.clone());
        }
    }
    let (folder, model, actor) = names(name);
    let keys = [
        format!("Gyml/ActorGParam/{actor}.gear__ActorGParam.bgyml"),
        format!("Gyml/ActorInfoGParam/{actor}.game__ActorInfoGParam.bgyml"),
        format!("Gyml/Component/ModelInfoGParam/{actor}.gear__ModelInfoGParam.bgyml"),
        format!("Gyml/FmdbExSummaryGParam/{folder}.{model}.gear__FmdbExSummaryGParam.bgyml"),
    ];
    if let Ok(packs) = gyml_packs(&mod_romfs) {
        for p in packs {
            let mut files = sarc::read(&read_file(&mod_romfs.join(&p))?).context(p.clone())?;
            let before = files.len();
            files.retain(|k, _| !keys.contains(k));
            if files.len() != before {
                write_atomic(&mod_romfs.join(&p), &compress(&sarc::write(&files)?)?)?;
                changed.push(p);
            }
        }
    }
    let rsdb = mod_romfs.join(RSDB_ACTOR);
    if rsdb.is_file() {
        let (mut root, v) = byml::from_bytes(&read_file(&rsdb)?).context(RSDB_ACTOR)?;
        let row_id = work(&keys[0]);
        if let Some(rows) = root.as_array_mut() {
            rows.retain(|r| r.get("__RowId").and_then(Byml::as_str) != Some(row_id.as_str()));
        }
        write_atomic(&rsdb, &compress(&byml::to_bytes(&root, v, false)?)?)?;
        changed.push(RSDB_ACTOR.to_string());
    }
    let dir = source_dir(mod_dir, name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).path_context(&dir)?;
    }
    manifest.objects.retain(|o| o.name != name);
    write_manifest(mod_dir, &manifest)?;
    Ok(changed)
}

/// Look for a romfs file in the given roots, first match wins.
pub fn layered_path(roots: &[PathBuf], rel: &str) -> Option<PathBuf> {
    roots.iter().map(|r| r.join(rel)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_validation() {
        assert!(valid_name("Ramp2") && valid_name("A"));
        assert!(!valid_name("") && !valid_name("2Ramp") && !valid_name("my ramp") && !valid_name(&"a".repeat(25)));
        assert_eq!(names("Ramp").2, "CmnObj_CustomRamp000_A_00");
        assert_eq!(work("Gyml/ActorGParam/X.gear__ActorGParam.bgyml"), "Work/Gyml/ActorGParam/X.gear__ActorGParam.gyml");
    }

    #[test]
    fn size_entries_follow_the_shipped_table() {
        // values from ResourceSizeTable.Product.100 for shipped files of each kind
        assert_eq!(size_entry("Tex/X.bntx.zs", 91256, 0), 99456);
        assert_eq!(size_entry("System/GymlArchive/actor_general_format.Product_100.sarc.zs", 3742512, 0), 3746624);
        assert_eq!(size_entry("RSDB/Actor.Product.100.rstbl.byml.zs", 708440, 0), 708736);
    }

    #[test]
    fn same_looking_parts_merge() {
        let mesh = Mesh { positions: vec![[0.0; 3]; 3], normals: vec![[0.0, 1.0, 0.0]; 3], uvs: vec![[0.0; 2]; 3], indices: vec![0, 1, 2] };
        let p = |c: f32| ImportPart { mesh: mesh.clone(), image: None, color: [c, c, c, 1.0], light: 0.75 };
        let merged = merge_parts(vec![p(1.0), p(0.5), p(1.0)]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].mesh.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn bake_texel_picks_a_bright_one() {
        let mut img = solid(4, 1, [0, 0, 0, 255]);
        img.rgba[12..16].copy_from_slice(&[255, 255, 255, 255]);
        img.rgba[8..12].copy_from_slice(&[200, 200, 200, 255]);
        // sorted brightness: 0, 0, 200, 255 -> the 75th percentile is the 200 texel (x = 2)
        assert_eq!(bake_texel(&img, 0.75), [2.5 / 4.0, 0.5]);
        assert_eq!(bake_texel(&img, 1.0), [3.5 / 4.0, 0.5]);
    }
}
