//! Editor state behind the QML bridge: open course, selection, view options, models,
//! gizmo dragging and the JSON the panels display. Plain Rust; no Qt types here.

use crate::config::{Config, cache_dir, emulator_load_dir};
use glam::{Mat4, Quat, Vec3};
use mkwtrack::banc::{course_base, hash_of};
use mkwtrack::byml::{Byml, vec3};
use mkwtrack::catalog::{ActorInfo, Catalog, Param, StagePlacement, banc_files};
use mkwtrack::check;
use mkwtrack::doc::{Document, Loc};
use mkwtrack::math::quat_zyx;
use mkwtrack::model::Model;
use mkwtrack::raycast::{Bvh, Frustum, Ray, ray_line_param, ray_plane};
use mkwtrack::scene::{self, Scene, ViewOptions, stages_for};
use mkwtrack::tools::Clipboard;
use mkwtrack::typedjson;
use mkwtrack::workspace::Workspace;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct ModelCache {
    pub map: Mutex<HashMap<String, Arc<Model>>>,
}

impl ModelCache {
    pub fn get(&self, rel: &str) -> Option<Arc<Model>> {
        self.map.lock().unwrap().get(rel).cloned()
    }
    pub fn has(&self, rel: &str) -> bool {
        self.map.lock().unwrap().contains_key(rel)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

pub struct Drag {
    mode: GizmoMode,
    axis: usize,
    center: Vec3,
    dir: Vec3,
    start: Vec3,
    start_param: f32,
    /// (hash, position, rotation, scale) of each item when the drag began
    items: Vec<(u64, Vec3, Quat, Vec3)>,
    /// each point item's Bézier handles when the drag began (they turn and scale with it)
    controls: Vec<(Option<Vec3>, Option<Vec3>)>,
    /// dragging one Bézier handle: (which, keep the other one on the same line)
    handle: Option<(usize, bool)>,
    moved: bool,
    /// whether the course had unsaved changes before (a drag that changes nothing keeps that)
    was_dirty: bool,
}

/// The move, rotation and scale steps the gizmo snaps to (0 = free).
#[derive(Clone, Copy)]
pub struct Snap {
    pub step: f32,
    pub angle: f32,
}

fn snap_to(v: f32, step: f32) -> f32 {
    if step > 0.0 { (v / step).round() * step } else { v }
}

pub struct State {
    pub cfg: Config,
    /// the game's catalog plus the open mod's custom objects
    pub cat: Option<Arc<Catalog>>,
    /// the game's catalog as read from the romfs
    pub base_cat: Option<Arc<Catalog>>,
    /// custom objects imported into the open mod
    pub custom: Vec<mkwtrack::custom::CustomObject>,
    /// where the open new course was started (the course whose place it took)
    pub new_location: String,
    pub doc: Option<Document>,
    pub selection: Vec<u64>,
    pub view: ViewOptions,
    pub xray: bool,
    pub snap_ground: bool,
    pub show_stage: bool,
    pub expanded: HashSet<String>,
    pub scene: Option<Scene>,
    pub models: Arc<ModelCache>,
    pub stages: Vec<StagePlacement>,
    pub ground: Option<Arc<Bvh>>,
    pub drag: Option<Drag>,
    pub link_pick: Option<u64>,
    pub link_type: String,
    pub console: VecDeque<String>,
    pub mod_name: String,
    pub gizmo_mode: GizmoMode,
    pub gizmo_local: bool,
    pub load_gen: u64,
    pub clipboard: Option<Clipboard>,
    /// what clicks and box selection pick: "" everything, "objects", "paths", or one path type
    pub pick_filter: String,
    /// the Bézier handle being edited (its point is the selection)
    pub handle: Option<(u64, usize)>,
    pub snap: Snap,
    pub prefs: crate::prefs::Prefs,
}

/// Console lines kept.
const CONSOLE_LINES: usize = 500;

pub fn rgba(c: [f32; 3], a: f32) -> Value {
    json!([c[0], c[1], c[2], a])
}

fn v3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// Qt.quaternion(w, x, y, z) order.
fn q4(q: Quat) -> Value {
    json!([q.w, q.x, q.y, q.z])
}

fn deg(v: [f32; 3]) -> Value {
    json!(v.map(|x| x.to_degrees()))
}

impl State {
    pub fn new(cfg: Config) -> State {
        State {
            mod_name: "MyTrackMod".into(),
            cfg,
            cat: None,
            base_cat: None,
            custom: vec![],
            new_location: String::new(),
            doc: None,
            selection: vec![],
            view: ViewOptions { models: true, ..Default::default() },
            xray: true,
            snap_ground: false,
            show_stage: true,
            expanded: HashSet::new(),
            scene: None,
            models: Arc::new(ModelCache::default()),
            stages: vec![],
            ground: None,
            drag: None,
            link_pick: None,
            link_type: "BasicSignal".into(),
            console: VecDeque::new(),
            gizmo_mode: GizmoMode::Translate,
            gizmo_local: true,
            load_gen: 0,
            clipboard: None,
            pick_filter: String::new(),
            handle: None,
            snap: Snap { step: 0.0, angle: 0.0 },
            prefs: crate::prefs::Prefs::load(),
        }
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        // the console and status bar never show the home folder (or the user name in it)
        let msg = crate::config::tilde(&msg.into());
        eprintln!("{msg}");
        self.console.push_back(msg);
        if self.console.len() > CONSOLE_LINES {
            self.console.pop_front();
        }
    }

    pub fn ws(&self) -> Workspace {
        Workspace { romfs: self.cfg.romfs.clone(), mods_root: self.cfg.mods.clone() }
    }

    /// The open mod's folder (`<mods>/<mod>`; its files are in `romfs/` inside).
    pub fn mod_dir(&self) -> std::path::PathBuf {
        self.cfg.mods.join(&self.mod_name)
    }

    /// Where models and textures are read from: the open mod first, then the game.
    pub fn roots(&self) -> Vec<std::path::PathBuf> {
        vec![self.mod_dir().join("romfs"), self.cfg.romfs.clone()]
    }

    /// Set the game's catalog and add the open mod's custom objects to it.
    pub fn set_catalog(&mut self, cat: Option<Arc<Catalog>>) {
        self.base_cat = cat;
        self.refresh_custom();
    }

    /// Re-read the open mod's custom objects into the catalog (after an import, or when the
    /// mod changes). They are listed under their own "Custom" category.
    pub fn refresh_custom(&mut self) {
        let Some(base) = self.base_cat.clone() else {
            self.cat = None;
            self.custom.clear();
            return;
        };
        self.custom = if mkwtrack::workspace::valid_mod_name(&self.mod_name) {
            mkwtrack::custom::read_manifest(&self.mod_dir()).objects
        } else {
            vec![]
        };
        if self.custom.is_empty() {
            self.cat = Some(base);
            return;
        }
        let mut data = base.data.clone();
        for (name, mut info) in mkwtrack::custom::catalog_entries(&base, &self.custom) {
            info.category = "Custom".into();
            data.actors.insert(name, info);
        }
        self.cat = Some(Arc::new(Catalog { data, romfs: base.romfs.clone() }));
    }

    /// What a model file holds, for the import dialog: parts, triangles, size and textures.
    pub fn inspect_model(&self, path: &std::path::Path) -> Value {
        match mkwtrack::import::load(path) {
            Ok(m) => {
                let (mn, mx) = m.bounds();
                let missing: Vec<String> = m.parts.iter().filter(|p| p.texture.is_none()).map(|p| p.name.clone()).collect();
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let mut name: String = stem.chars().filter(|c| c.is_ascii_alphanumeric()).take(24).collect();
                if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
                    name = format!("M{name}").chars().take(24).collect();
                }
                json!({"ok": true, "parts": m.parts.len(), "triangles": m.triangle_count(),
                       "size": [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]], "untextured": missing, "name": name, "label": stem})
            }
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        }
    }

    /// Import a model file as a custom object into the open mod.
    pub fn import_model(
        &mut self,
        path: &std::path::Path,
        name: &str,
        label: &str,
        scale: f32,
    ) -> mkwtrack::Result<mkwtrack::custom::Imported> {
        if !mkwtrack::workspace::valid_mod_name(&self.mod_name) {
            return Err(format!("\"{}\" is not a usable mod name", self.mod_name).into());
        }
        let model = mkwtrack::import::load(path)?;
        let source = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let label = if label.trim().is_empty() { name } else { label.trim() };
        let res = mkwtrack::custom::import(&self.cfg.romfs, &self.mod_dir(), name, label, &source, &model, scale)?;
        self.after_custom_change(&res.object.model);
        Ok(res)
    }

    /// Forget the cached model and texture previews of a rebuilt custom object, and show the
    /// mod's objects as they now are.
    fn after_custom_change(&mut self, model_rel: &str) {
        self.models.map.lock().unwrap().remove(model_rel);
        let _ = std::fs::remove_file(crate::thumbs::thumb_path(model_rel));
        self.refresh_custom();
        self.rebuild();
    }

    /// The open mod's custom objects for the model editor: settings, materials (with their
    /// image previews) and whether each can be edited.
    pub fn custom_json(&self) -> Value {
        let dir = self.mod_dir();
        json!(
            self.custom
                .iter()
                .map(|o| {
                    let materials: Vec<Value> = o
                        .settings
                        .materials
                        .iter()
                        .enumerate()
                        .map(|(i, m)| {
                            let img = mkwtrack::custom::material_image(&dir, &o.name, &o.settings, i);
                            json!({"name": m.name, "image": img.map(|p| crate::config::file_url(&p)).unwrap_or_default(), "settings": m})
                        })
                        .collect();
                    json!({"name": o.name, "label": o.label, "actor": o.actor, "model": o.model, "source": o.source, "aabb": o.aabb,
                       "editable": mkwtrack::custom::has_source(&dir, &o.name), "settings": o.settings, "materials": materials})
                })
                .collect::<Vec<_>>()
        )
    }

    /// Rebuild a custom object with new settings (JSON of [`mkwtrack::custom::ModelSettings`]).
    pub fn update_custom(&mut self, name: &str, label: &str, settings: &str) -> mkwtrack::Result<mkwtrack::custom::Imported> {
        let settings: mkwtrack::custom::ModelSettings = serde_json::from_str(settings)?;
        let label = if label.trim().is_empty() { name } else { label.trim() };
        let res = mkwtrack::custom::update(&self.cfg.romfs, &self.mod_dir(), name, label, settings)?;
        self.after_custom_change(&res.object.model);
        Ok(res)
    }

    /// Give a custom object's material a new image and rebuild it.
    pub fn set_custom_texture(
        &mut self,
        name: &str,
        index: usize,
        image: &std::path::Path,
    ) -> mkwtrack::Result<mkwtrack::custom::Imported> {
        let dir = self.mod_dir();
        let settings = mkwtrack::custom::set_texture(&dir, name, index, image)?;
        let label = self.custom.iter().find(|o| o.name == name).map(|o| o.label.clone()).unwrap_or_else(|| name.to_string());
        let res = mkwtrack::custom::update(&self.cfg.romfs, &dir, name, &label, settings)?;
        self.after_custom_change(&res.object.model);
        Ok(res)
    }

    /// Remove a custom object from the open mod.
    pub fn delete_custom(&mut self, name: &str) -> mkwtrack::Result<Vec<String>> {
        let model = self.custom.iter().find(|o| o.name == name).map(|o| o.model.clone());
        let changed = mkwtrack::custom::delete(&self.mod_dir(), name)?;
        if let Some(m) = model {
            self.after_custom_change(&m);
        }
        Ok(changed)
    }

    /// Read a model (from the open mod or the game) and its textures into the caches now, for
    /// the model editor's preview. Returns its part count.
    pub fn preview_model(&self, rel: &str) -> mkwtrack::Result<usize> {
        if let Some(m) = self.models.get(rel) {
            return Ok(m.parts.len());
        }
        let roots = self.roots();
        let root = roots.iter().find(|r| r.join(rel).is_file()).unwrap_or(&roots[roots.len() - 1]);
        let m = mkwtrack::bfres::load(root, rel)?;
        let mut names: Vec<String> = m.parts.iter().filter_map(|p| p.texture.clone()).collect();
        names.sort();
        names.dedup();
        crate::textures::ensure_all(&roots, &names);
        let n = m.parts.len();
        self.models.map.lock().unwrap().insert(rel.to_string(), Arc::new(m));
        Ok(n)
    }

    /// The catalog cache folder, one per romfs path.
    pub fn catalog_cache(&self) -> std::path::PathBuf {
        cache_dir().join(format!("catalog-{:08x}", fnv1a(self.cfg.romfs.to_string_lossy().as_bytes())))
    }

    // ------------------------------------------------------------ courses

    pub fn course_list(&self, filter: &str) -> Value {
        let Some(cat) = &self.cat else { return json!([]) };
        let f = filter.to_lowercase();
        let ws = self.ws();
        let mods = ws.mods();
        let mut out = vec![];
        for rel in banc_files(&self.cfg.romfs) {
            let base = course_base(&rel);
            let info = cat.data.courses.get(&base).and_then(|v| v.first());
            let display = info.map(|i| i.display.clone()).unwrap_or_default();
            if !f.is_empty() && !rel.to_lowercase().contains(&f) && !display.to_lowercase().contains(&f) {
                continue;
            }
            let group =
                if rel.contains("/World_Game03") { "Free-roam grid".to_string() } else { base.split('_').next().unwrap_or("").to_string() };
            let modded: Vec<&String> = mods.iter().filter(|m| self.cfg.mods.join(m).join("romfs").join(&rel).exists()).collect();
            out.push(json!({"file": rel, "name": rel.trim_start_matches("Banc/").trim_end_matches(".bcett.byml.zs"), "display": display,
                            "rule": info.map(|i| i.rule.clone()).unwrap_or_default(), "group": group, "mods": modded}));
        }
        let order =
            ["Race", "LongRally", "InterStageEx1", "InterStageEx2", "BalloonBattle", "CoinBattle", "BombBattle", "Mission", "Parade"];
        out.sort_by_key(|v| {
            let g = v["group"].as_str().unwrap_or("").to_string();
            (order.iter().position(|o| *o == g).unwrap_or(99), g, v["file"].as_str().unwrap_or("").to_string())
        });
        json!(out)
    }

    pub fn open(&mut self, file: &str, m: Option<&str>) -> mkwtrack::Result<()> {
        let cat = self.cat.clone().ok_or("the game catalog is still loading")?;
        let loaded = self.ws().load(file, m)?;
        self.set_doc(Document::new(loaded), &cat);
        if let Some(m) = m.filter(|m| !m.is_empty())
            && self.mod_name != m
        {
            self.mod_name = m.to_string();
            self.refresh_custom();
        }
        self.rebuild();
        let d = self.doc.as_ref().unwrap();
        let pts: usize = d.rails().iter().map(|r| Document::points(r).len()).sum();
        self.log(format!(
            "Opened {file}{}: {} objects, {} paths, {pts} points, {} stage model(s)",
            if d.from_mod { format!(" from mod \"{}\"", self.mod_name) } else { String::new() },
            d.actors().len(),
            d.rails().len(),
            self.stages.len()
        ));
        Ok(())
    }

    /// Start a new course (not yet tied to a file) where the course `location` is: empty, or a
    /// copy of that course's objects and paths. Saving it asks which course it replaces.
    pub fn new_course(&mut self, location: &str, copy: bool) -> mkwtrack::Result<()> {
        let cat = self.cat.clone().ok_or("the game catalog is still loading")?;
        let loaded = self.ws().load(location, None)?;
        let (doc, pad) = if copy {
            (loaded.doc, loaded.pad.or_else(|| Some(Byml::Hash64(Default::default()))))
        } else {
            (mkwtrack::banc::empty_course(&loaded.doc), Some(Byml::Hash64(Default::default())))
        };
        let mut d = Document::new(mkwtrack::workspace::Loaded {
            file: String::new(),
            from_mod: false,
            version: loaded.version,
            doc,
            pad,
            pad_file: None,
        });
        d.dirty = true;
        self.new_location = location.to_string();
        self.set_doc(d, &cat);
        self.rebuild();
        self.log(format!(
            "New course at {}: {} (File → Save As picks the course it replaces)",
            location.trim_start_matches("Banc/"),
            if copy { "a copy of its objects and paths" } else { "empty" }
        ));
        Ok(())
    }

    /// The window title's course part: the file (and mod), or that the course is new.
    pub fn course_title(&self) -> String {
        let Some(d) = &self.doc else { return String::new() };
        if d.file.is_empty() {
            return format!("New course (at {})", mkwtrack::banc::course_base(&self.new_location));
        }
        format!("{}{}", d.file.trim_start_matches("Banc/"), if d.from_mod { format!("  [{}]", self.mod_name) } else { String::new() })
    }

    /// The course Save As suggests replacing: the open one, or where a new one was started.
    pub fn save_as_default(&self) -> String {
        match &self.doc {
            Some(d) if !d.file.is_empty() => d.file.clone(),
            _ => self.new_location.clone(),
        }
    }

    /// Save the open course into the mod as the course `target`, replacing that one there.
    pub fn save_as(&mut self, target: &str) -> Value {
        let Some(doc) = self.doc.as_ref() else { return json!({"ok": false, "errors": ["no course open"]}) };
        if !banc_files(&self.cfg.romfs).iter().any(|f| f == target) {
            return json!({"ok": false, "errors": [format!("{target} is not one of the game's course files")]});
        }
        let far = self.distance_to(doc, target).filter(|d| *d > 1000.0);
        self.doc.as_mut().unwrap().retarget(target);
        let mut v = self.save();
        if let Some(d) = far
            && let Some(w) = v["warnings"].as_array_mut()
        {
            w.push(json!(format!(
                "This course is {:.1} km from where {} is. The game loads the scenery around the course it replaces, so build near it.",
                d / 1000.0,
                mkwtrack::banc::course_base(target)
            )));
        }
        v
    }

    /// How far the open course's objects and paths are from the middle of `target`'s bounds.
    fn distance_to(&self, doc: &Document, target: &str) -> Option<f32> {
        let mut pts: Vec<Vec3> = doc.actors().iter().map(|a| Vec3::from(a.vec3("Translate", [0.0; 3]))).collect();
        for r in doc.rails() {
            pts.extend(Document::points(r).iter().map(|p| Vec3::from(p.vec3("Translation", [0.0; 3]))));
        }
        if pts.is_empty() {
            return None;
        }
        let ours = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
        let other = self.ws().load(target, None).ok()?;
        let aabb = other.doc.get("AABB")?;
        let centre = (Vec3::from(aabb.vec3("Min", [0.0; 3])) + Vec3::from(aabb.vec3("Max", [0.0; 3]))) / 2.0;
        Some(ours.distance(centre))
    }

    /// Show `doc`, clearing what belonged to the previous course.
    fn set_doc(&mut self, doc: Document, cat: &Catalog) {
        self.stages = stages_for(cat, &doc.root, 12).into_iter().cloned().collect();
        self.doc = Some(doc);
        self.selection.clear();
        self.expanded.clear();
        self.drag = None;
        self.link_pick = None;
        self.handle = None;
        self.view.hidden_items.clear();
        self.view.hidden_rails.clear();
        self.ground = None;
        self.load_gen += 1;
    }

    /// Models the open course needs: stages first, then object models.
    pub fn wanted_models(&self) -> Vec<(String, bool)> {
        let (Some(cat), Some(doc)) = (&self.cat, &self.doc) else { return vec![] };
        let mut out: Vec<(String, bool)> = self.stages.iter().filter_map(|s| s.model.clone()).map(|m| (m, true)).collect();
        let mut seen: HashSet<String> = out.iter().map(|x| x.0.clone()).collect();
        for a in doc.actors() {
            if let Some(m) = cat.actor(a.str_or("Gyaml", "")).and_then(|i| i.model.clone())
                && seen.insert(m.clone())
            {
                out.push((m, false));
            }
        }
        out
    }

    pub fn rebuild(&mut self) {
        let (Some(cat), Some(doc)) = (&self.cat, &self.doc) else {
            self.scene = None;
            return;
        };
        let sel: HashSet<u64> = self.selection.iter().copied().collect();
        if self.handle.is_some_and(|(h, _)| self.selection != [h]) {
            self.handle = None;
        }
        self.view.active_handle = self.handle;
        let models = self.models.clone();
        self.scene = Some(scene::build(doc, cat, &self.view, &|m| models.has(m), &sel));
    }

    // ------------------------------------------------------------ scene JSON

    pub fn stages_json(&self) -> Value {
        if !self.show_stage {
            return json!([]);
        }
        json!(self.stages.iter().filter_map(|s| {
            let m = s.model.as_ref()?;
            let model = self.models.get(m)?;
            Some(json!({"rel": m, "name": s.name, "parts": model.parts.len(), "p": s.translate, "q": q4(quat_zyx(s.rotate)), "s": s.scale}))
        }).collect::<Vec<_>>())
    }

    pub fn actor_groups_json(&self) -> Value {
        let Some(sc) = &self.scene else { return json!([]) };
        json!(sc.groups.iter().filter_map(|(rel, inst)| {
            let model = self.models.get(rel)?;
            Some(json!({"rel": rel, "parts": model.parts.len(),
                "inst": inst.iter().map(|i| json!({"h": i.hash.to_string(), "p": v3(i.pos), "q": q4(i.rot), "s": v3(i.scale)})).collect::<Vec<_>>()}))
        }).collect::<Vec<_>>())
    }

    pub fn boxes_json(&self) -> Value {
        let Some(sc) = &self.scene else { return json!([]) };
        json!(
            sc.boxes
                .iter()
                .map(|b| {
                    let (s, q, p) = b.world.to_scale_rotation_translation();
                    json!({"h": b.hash.to_string(), "shape": b.shape, "c": rgba(b.color, 0.45), "p": v3(p), "q": q4(q), "s": v3(s)})
                })
                .collect::<Vec<_>>()
        )
    }

    /// Current transforms of the given objects, for live updates while dragging.
    pub fn transforms_json(&self, hashes: &[u64]) -> Value {
        let Some(doc) = &self.doc else { return json!([]) };
        let Some(cat) = &self.cat else { return json!([]) };
        json!(
            hashes
                .iter()
                .filter_map(|h| {
                    let (loc, a) = doc.find(*h)?;
                    if !matches!(loc, Loc::Actor(_)) {
                        return None;
                    }
                    let (p, q, s) = scene::actor_world(a);
                    let (mn, mx, _) = scene::actor_box(cat, a);
                    let local = Mat4::from_scale_rotation_translation(mx - mn, Quat::IDENTITY, (mn + mx) / 2.0);
                    let (bs, bq, bp) = (Mat4::from_scale_rotation_translation(s, q, p) * local).to_scale_rotation_translation();
                    Some(json!({"h": h.to_string(), "p": v3(p), "q": q4(q), "s": v3(s), "bp": v3(bp), "bq": q4(bq), "bs": v3(bs)}))
                })
                .collect::<Vec<_>>()
        )
    }

    pub fn rail_index_of(&self, rail_hash: u64) -> Option<usize> {
        self.scene.as_ref()?.rails.iter().position(|r| r.hash == rail_hash)
    }

    /// Rails touched by moving these items (indices into the scene rails).
    pub fn rails_touched(&self, hashes: &[u64]) -> Vec<usize> {
        let Some(sc) = &self.scene else { return vec![] };
        let mut out = vec![];
        for h in hashes {
            let rh = scene::rail_of(sc, *h).unwrap_or(*h);
            if let Some(i) = self.rail_index_of(rh)
                && !out.contains(&i)
            {
                out.push(i);
            }
        }
        out
    }

    pub fn gizmo_json(&self) -> Value {
        let Some(doc) = &self.doc else { return json!({"visible": false}) };
        if let Some((h, which)) = self.handle
            && let Some(c) = self.handle_pos(h, which)
        {
            return json!({"visible": true, "p": v3(c), "q": q4(Quat::IDENTITY), "count": 1, "handle": true});
        }
        let items: Vec<(Vec3, Quat)> = doc.movable(&self.selection).into_iter().map(|(_, p, q, _)| (p, q)).collect();
        // count: how much is selected
        let count = self.selection.len();
        if items.is_empty() {
            return json!({"visible": false, "count": count});
        }
        let c = items.iter().map(|x| x.0).sum::<Vec3>() / items.len() as f32;
        let q = if self.gizmo_local && items.len() == 1 { items[0].1 } else { Quat::IDENTITY };
        json!({"visible": true, "p": v3(c), "q": q4(q), "count": count})
    }

    pub fn focus_json(&self, all: bool) -> Value {
        let Some(doc) = &self.doc else { return json!(null) };
        let mut pts: Vec<Vec3> = vec![];
        if all {
            pts.extend(doc.actors().iter().map(|a| Vec3::from(a.vec3("Translate", [0.0; 3]))));
            pts.extend(doc.rails().iter().flat_map(Document::points).map(|p| Vec3::from(p.vec3("Translation", [0.0; 3]))));
        } else {
            for h in &self.selection {
                match doc.find(*h) {
                    Some((Loc::Rail(_), r)) => pts.extend(Document::points(r).iter().map(|p| Vec3::from(p.vec3("Translation", [0.0; 3])))),
                    Some(_) => pts.extend(doc.transform(*h).map(|t| t.0)),
                    None => {}
                }
            }
        }
        if pts.is_empty() {
            return json!(null);
        }
        let mn = pts.iter().fold(Vec3::splat(f32::MAX), |a, b| a.min(*b));
        let mx = pts.iter().fold(Vec3::splat(f32::MIN), |a, b| a.max(*b));
        json!({"c": v3((mn + mx) / 2.0), "r": ((mx - mn).length() / 2.0).max(6.0)})
    }

    // ------------------------------------------------------------ picking and gizmo

    /// Whether the pick filter lets clicks and box selection pick this item.
    pub fn pickable(&self, h: u64) -> bool {
        let f = self.pick_filter.as_str();
        if f.is_empty() {
            return true;
        }
        let Some(doc) = &self.doc else { return false };
        match doc.find(h) {
            Some((Loc::Actor(_), _)) => f == "objects",
            Some((Loc::Point(r, _), _)) | Some((Loc::Rail(r), _)) => f == "paths" || doc.rails()[r].str_or("Name", "") == f,
            None => false,
        }
    }

    fn handle_pos(&self, h: u64, which: usize) -> Option<Vec3> {
        let (_, e) = self.doc.as_ref()?.find(h)?;
        e.get(["Control0", "Control1"][which]).map(|c| Vec3::from(vec3(Some(c), [0.0; 3])))
    }

    pub fn pick(&mut self, r: &Ray, additive: bool) -> Option<u64> {
        // a Bézier handle of the selected point, when it is in front of everything else
        if !additive
            && self.link_pick.is_none()
            && let Some((h, which, t)) = self.scene.as_ref().and_then(|sc| scene::pick_handle(sc, r))
        {
            let other = self.scene.as_ref().and_then(|sc| scene::pick_where(sc, r, &|x| x != h && self.pickable(x)));
            let nearer = other.and_then(|o| self.doc.as_ref()?.transform(o)).is_none_or(|(p, ..)| p.distance(r.origin) + 1.0 > t);
            if nearer {
                self.selection = vec![h];
                self.handle = Some((h, which));
                self.rebuild();
                return Some(h);
            }
        }
        self.handle = None;
        let hit = self.scene.as_ref().and_then(|sc| scene::pick_where(sc, r, &|x| self.pickable(x)));
        if let Some(src) = self.link_pick.take() {
            match hit {
                Some(dst) if dst != src => {
                    let kind = self.link_type.clone();
                    let res = self.doc.as_mut().map(|d| d.add_link(src, dst, &kind)).unwrap_or(Ok(()));
                    match res {
                        Ok(()) => self.log(format!("{kind} link added")),
                        Err(e) => self.log(format!("Link not added: {e}")),
                    }
                }
                _ => self.log("Link cancelled"),
            }
            self.rebuild();
            return hit;
        }
        match hit {
            Some(h) if additive => {
                if let Some(i) = self.selection.iter().position(|x| *x == h) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(h);
                }
            }
            Some(h) => self.selection = vec![h],
            None if !additive => self.selection.clear(),
            None => {}
        }
        self.rebuild();
        hit
    }

    /// What is under the mouse, without selecting it.
    pub fn hit_at(&self, r: &Ray) -> Option<u64> {
        self.scene.as_ref().and_then(|sc| scene::pick_where(sc, r, &|x| self.pickable(x)))
    }

    /// Select everything inside a box on screen. `mode` 0 replaces the selection, 1 adds to it,
    /// 2 removes from it. Returns how many items the box held.
    pub fn box_select(&mut self, corners: &[Ray; 4], mode: i32) -> usize {
        let f = Frustum::from_rays(corners);
        let hits = self.scene.as_ref().map(|sc| scene::in_frustum(sc, &f, &|x| self.pickable(x))).unwrap_or_default();
        let n = hits.len();
        self.handle = None;
        match mode {
            1 => {
                let have: HashSet<u64> = self.selection.iter().copied().collect();
                self.selection.extend(hits.into_iter().filter(|h| !have.contains(h)));
            }
            2 => {
                let drop: HashSet<u64> = hits.into_iter().collect();
                self.selection.retain(|h| !drop.contains(h));
            }
            _ => self.selection = hits,
        }
        self.rebuild();
        n
    }

    /// Every visible item the pick filter allows.
    pub fn select_all(&mut self) {
        let all: Vec<u64> = match &self.scene {
            Some(sc) => sc.points.iter().map(|p| p.0).chain(sc.proxies.iter().map(|p| p.hash)).filter(|h| self.pickable(*h)).collect(),
            None => vec![],
        };
        self.selection = all;
        self.handle = None;
        self.rebuild();
    }

    /// Hide the selected objects and the paths of the selected points (Unhide All shows them).
    pub fn hide_selection(&mut self) -> usize {
        let Some(doc) = &self.doc else { return 0 };
        let mut n = 0;
        for h in std::mem::take(&mut self.selection) {
            match doc.find(h) {
                Some((Loc::Actor(_), _)) => n += self.view.hidden_items.insert(h) as usize,
                Some((Loc::Point(r, _) | Loc::Rail(r), _)) => n += self.view.hidden_rails.insert(hash_of(&doc.rails()[r])) as usize,
                None => {}
            }
        }
        self.handle = None;
        self.rebuild();
        n
    }

    /// Show everything again: hidden items, paths, groups, path types and categories.
    pub fn unhide_all(&mut self) {
        let v = &mut self.view;
        v.hidden_items.clear();
        v.hidden_rails.clear();
        v.hidden_actor_names.clear();
        v.hidden_rail_types.clear();
        v.hidden_categories.clear();
        self.rebuild();
    }

    pub fn ground_hit(&self, r: &Ray) -> Option<Vec3> {
        let g = self.ground.as_ref()?;
        // far enough for the top view's camera, 30 km up
        g.hit(r, 100_000.0).map(|(d, _)| r.origin + r.dir * d)
    }

    /// A height to place things at when the stage surface isn't under the mouse: the selection's,
    /// else the middle of the course's objects.
    pub fn working_height(&self) -> Option<f32> {
        let doc = self.doc.as_ref()?;
        if let Some(&h) = self.selection.first() {
            let h = match doc.find(h) {
                Some((Loc::Rail(_), r)) => Document::points(r).first().map(hash_of).unwrap_or(h),
                _ => h,
            };
            if let Some((p, ..)) = doc.transform(h) {
                return Some(p.y);
            }
        }
        let mut ys: Vec<f32> = doc.actors().iter().map(|a| a.vec3("Translate", [0.0; 3])[1]).collect();
        ys.sort_by(|a, b| a.total_cmp(b));
        ys.get(ys.len() / 2).copied()
    }

    /// Where a click puts something: the stage surface under it; without one, the level `y`
    /// (the height of the point a path continues from); else 30 m in front of the camera.
    pub fn placement_at(&self, r: &Ray, y: Option<f32>) -> Vec3 {
        self.ground_hit(r)
            .or_else(|| y.and_then(|y| ray_plane(r, Vec3::new(0.0, y, 0.0), Vec3::Y)).filter(|p| p.distance(r.origin) < 100_000.0))
            .unwrap_or(r.origin + r.dir * 30.0)
    }

    /// Start dragging the selection with the gizmo (or a Bézier handle, when one is picked).
    /// `mode` overrides the gizmo's mode (G, R and T); `aligned` keeps a dragged handle's
    /// partner on its line; `record` is false when a modal drag restarts on another axis.
    pub fn begin_drag(&mut self, mode: Option<GizmoMode>, axis: usize, r: &Ray, aligned: bool, record: bool) -> bool {
        let mode = mode.unwrap_or(self.gizmo_mode);
        let handle = self.handle.filter(|_| mode == GizmoMode::Translate).and_then(|(h, w)| Some((h, w, self.handle_pos(h, w)?)));
        let Some(doc) = &mut self.doc else { return false };
        let items: Vec<(u64, Vec3, Quat, Vec3)> = match handle {
            Some((h, _, c)) => vec![(h, c, Quat::IDENTITY, Vec3::ONE)],
            None => doc.movable(&self.selection),
        };
        if items.is_empty() {
            return false;
        }
        let ctl = |doc: &Document, h: u64| -> (Option<Vec3>, Option<Vec3>) {
            match doc.find(h) {
                Some((Loc::Point(..), e)) => {
                    let c = |k: &str| e.get(k).map(|c| Vec3::from(vec3(Some(c), [0.0; 3])));
                    (c("Control0"), c("Control1"))
                }
                _ => (None, None),
            }
        };
        let controls = items.iter().map(|it| if handle.is_some() { (None, None) } else { ctl(doc, it.0) }).collect();
        let center = items.iter().map(|x| x.1).sum::<Vec3>() / items.len() as f32;
        let frame = if self.gizmo_local && items.len() == 1 && handle.is_none() { items[0].2 } else { Quat::IDENTITY };
        let dir = match axis {
            0 => frame * Vec3::X,
            1 => frame * Vec3::Y,
            2 => frame * Vec3::Z,
            _ => -r.dir,
        };
        let (start, start_param) = match (mode, axis) {
            (GizmoMode::Rotate, _) | (_, 3) => (ray_plane(r, center, dir).unwrap_or(center), 0.0),
            _ => (center, ray_line_param(r, center, dir).unwrap_or(0.0)),
        };
        let was_dirty = doc.dirty;
        if record {
            doc.push_undo_as(match (handle, mode) {
                (Some(_), _) => "Move Curve Handle",
                (None, GizmoMode::Translate) => "Move",
                (None, GizmoMode::Rotate) => "Rotate",
                (None, GizmoMode::Scale) => "Scale",
            });
        }
        let handle = handle.map(|(_, w, _)| (w, aligned));
        self.drag = Some(Drag { mode, axis, center, dir, start, start_param, items, controls, handle, moved: false, was_dirty });
        true
    }

    /// Restart a modal drag on another axis (3 = free), from where it began.
    pub fn restart_drag(&mut self, axis: usize, r: &Ray) -> bool {
        let Some(d) = self.drag.take() else { return false };
        if let Some(doc) = &mut self.doc {
            doc.revert_step();
        }
        self.rebuild();
        let ok = self.begin_drag(Some(d.mode), axis, r, d.handle.is_some_and(|h| h.1), false);
        if let Some(nd) = &mut self.drag {
            nd.was_dirty = d.was_dirty;
        }
        ok
    }

    /// Apply a drag; returns the hashes that moved. `invert_snap` (Ctrl held) snaps when
    /// snapping is off and moves freely when it is on.
    pub fn drag_to(&mut self, r: &Ray, invert_snap: bool) -> Vec<u64> {
        let snap_on = (self.snap.step > 0.0 || self.snap.angle > 0.0) != invert_snap;
        let step = if !snap_on {
            0.0
        } else if self.snap.step > 0.0 {
            self.snap.step
        } else {
            1.0
        };
        let angle = if !snap_on {
            0.0
        } else if self.snap.angle > 0.0 {
            self.snap.angle
        } else {
            15.0
        };
        let Some(d) = &mut self.drag else { return vec![] };
        let Some(doc) = &mut self.doc else { return vec![] };
        let cat = self.cat.clone();
        let ground = self.ground.clone();
        let mut moved = vec![];
        let snap_ground = self.snap_ground && d.mode == GizmoMode::Translate && d.handle.is_none();
        for (k, &(h, p0, q0, s0)) in d.items.iter().enumerate() {
            let (p, q, s) = match (d.mode, d.axis) {
                (GizmoMode::Translate, 3) => {
                    let Some(hit) = ray_plane(r, d.center, d.dir) else { continue };
                    let delta = hit - d.start;
                    (p0 + Vec3::new(snap_to(delta.x, step), snap_to(delta.y, step), snap_to(delta.z, step)), q0, s0)
                }
                (GizmoMode::Translate, _) => {
                    let Some(t) = ray_line_param(r, d.center, d.dir) else { continue };
                    (p0 + d.dir * snap_to(t - d.start_param, step), q0, s0)
                }
                (GizmoMode::Rotate, _) => {
                    let Some(hit) = ray_plane(r, d.center, d.dir) else { continue };
                    let (a, b) = ((d.start - d.center).normalize_or_zero(), (hit - d.center).normalize_or_zero());
                    let angle = snap_to(a.cross(b).dot(d.dir).atan2(a.dot(b)), angle.to_radians());
                    let rq = Quat::from_axis_angle(d.dir.normalize(), angle);
                    (d.center + rq * (p0 - d.center), rq * q0, s0)
                }
                (GizmoMode::Scale, 3) => {
                    let Some(hit) = ray_plane(r, d.center, d.dir) else { continue };
                    let base = d.start.distance(d.center);
                    let ratio = if base < 1e-3 { 1.0 } else { hit.distance(d.center) / base };
                    let ratio = if snap_on { snap_to(ratio, 0.1).max(0.1) } else { ratio.clamp(0.01, 100.0) };
                    (d.center + (p0 - d.center) * ratio, q0, s0 * ratio)
                }
                (GizmoMode::Scale, _) => {
                    let Some(t) = ray_line_param(r, d.center, d.dir) else { continue };
                    let ratio = if d.start_param.abs() < 1e-3 { 1.0 } else { (t / d.start_param).clamp(0.01, 100.0) };
                    let ratio = if snap_on { snap_to(ratio, 0.1).max(0.1) } else { ratio };
                    let mut s = s0;
                    s[d.axis] = s0[d.axis] * ratio;
                    (p0, q0, s)
                }
            };
            let mut p = p;
            if snap_ground && let (Some(g), Some((loc, e))) = (&ground, doc.find(h)) {
                let lift = if matches!(loc, Loc::Actor(_)) {
                    cat.as_ref().and_then(|c| c.actor(e.str_or("Gyaml", ""))).and_then(|a| a.banc_offset).map(|o| o[1]).unwrap_or(0.0)
                } else {
                    0.0
                };
                if let Some(y) = g.ground_below(p) {
                    p.y = y + lift;
                }
            }
            if let Some((which, aligned)) = d.handle {
                doc.set_handle(h, which, p, aligned);
            } else {
                doc.set_transform(h, p, q, s);
                // a point's handles turn, scale and move with it
                if let Some(&(c0, c1)) = d.controls.get(k)
                    && (c0.is_some() || c1.is_some())
                {
                    let turn = q * q0.inverse();
                    let grow = if d.mode == GizmoMode::Scale { (s / s0).to_array().iter().product::<f32>() } else { 1.0 };
                    let place = |c: Option<Vec3>| c.map(|c| p + turn * ((c - p0) * grow));
                    doc.set_controls(h, place(c0), place(c1));
                }
            }
            moved.push(h);
        }
        d.moved |= !moved.is_empty();
        self.rebuild();
        moved
    }

    /// Finish a drag; a drag that never moved leaves no undo step.
    pub fn end_drag(&mut self) {
        if let Some(d) = self.drag.take()
            && !d.moved
            && let Some(doc) = &mut self.doc
        {
            doc.cancel_step();
            doc.dirty = d.was_dirty;
        }
    }

    /// Cancel a drag (Esc or right click during G, R or T): everything goes back.
    pub fn cancel_drag(&mut self) {
        if let Some(d) = self.drag.take()
            && let Some(doc) = &mut self.doc
        {
            doc.cancel_step();
            doc.dirty = d.was_dirty;
        }
        self.rebuild();
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Everything the Problems panel lists.
    pub fn problems_json(&self) -> Value {
        let Some(doc) = &self.doc else { return json!([]) };
        json!(
            check::check(&doc.root, &doc.file, self.cat.as_deref())
                .into_iter()
                .map(|p| json!({"severity": p.severity.name(), "message": p.message, "items": p.items.iter().map(|h| h.to_string()).collect::<Vec<_>>(), "fix": p.fix}))
                .collect::<Vec<_>>()
        )
    }

    // ------------------------------------------------------------ outliner

    pub fn outliner_json(&self, filter: &str) -> Value {
        let (Some(doc), Some(cat)) = (&self.doc, &self.cat) else { return json!([]) };
        let f = filter.to_lowercase();
        let sel: HashSet<u64> = self.selection.iter().copied().collect();
        let mut rows = vec![];
        let mut groups: BTreeMap<String, Vec<&Byml>> = BTreeMap::new();
        for a in doc.actors() {
            groups.entry(a.str_or("Name", "").to_string()).or_default().push(a);
        }
        rows.push(json!({"kind": "section", "label": "Objects", "count": doc.actors().len(), "key": "s:objects"}));
        for (name, list) in &groups {
            let info = cat.data.actors.get(name);
            let label = info.map(|i| i.label.as_str()).unwrap_or("");
            if !f.is_empty() && !name.to_lowercase().contains(&f) && !label.to_lowercase().contains(&f) {
                continue;
            }
            let key = format!("a:{name}");
            let open = self.expanded.contains(&key);
            let category = info.map(|i| i.category.as_str()).unwrap_or("Other");
            rows.push(json!({"kind": "group", "key": key, "label": short_name(name), "name": name, "sub": label, "count": list.len(), "expanded": open, "thumb": thumb_url(info), "category": category,
                "color": scene::hex(scene::category_color(category)), "icon": if info.and_then(|i| i.model.as_ref()).is_some() { "cube" } else { "draw-cuboid" },
                "selected": list.iter().any(|a| sel.contains(&hash_of(a))), "visible": !self.view.hidden_actor_names.contains(name) && !self.view.hidden_categories.contains(category)}));
            if open {
                for (i, a) in list.iter().enumerate() {
                    let h = hash_of(a);
                    let t = a.vec3("Translate", [0.0; 3]);
                    let dynk: Vec<String> =
                        a.get("Dynamic").and_then(|d| d.as_map()).map(|m| m.keys().cloned().collect()).unwrap_or_default();
                    let at = format!("{:.1}, {:.1}, {:.1}", t[0], t[1], t[2]);
                    // the parameter names would be cut to a few letters after the position: tooltip only
                    rows.push(json!({"kind": "item", "key": format!("h:{h}"), "hash": h.to_string(), "label": format!("#{i}"),
                        "sub": at, "tip": if dynk.is_empty() { String::new() } else { format!("Parameters set: {}", dynk.join(", ")) }, "selected": sel.contains(&h)}));
                }
            }
        }
        rows.push(json!({"kind": "section", "label": "Paths", "count": doc.rails().len(), "key": "s:paths"}));
        let mut counters: HashMap<String, usize> = HashMap::new();
        for r in doc.rails() {
            let name = r.str_or("Name", "").to_string();
            let n = {
                let c = counters.entry(name.clone()).or_default();
                *c += 1;
                *c
            };
            if !f.is_empty() && !name.to_lowercase().contains(&f) {
                continue;
            }
            let rh = hash_of(r);
            let key = format!("r:{rh}");
            let open = self.expanded.contains(&key);
            let pts = Document::points(r);
            rows.push(json!({"kind": "group", "key": key, "hash": rh.to_string(), "label": format!("{name} {n}"), "sub": if r.get("IsClosed").and_then(|b| b.as_bool()) == Some(true) { "closed loop" } else { "" },
                "count": pts.len(), "expanded": open, "color": scene::hex(scene::rail_color(&name)), "icon": "draw-path",
                "selected": sel.contains(&rh) || pts.iter().any(|p| sel.contains(&hash_of(p))),
                "visible": !self.view.hidden_rails.contains(&rh) && !self.view.hidden_rail_types.contains(&name)}));
            if open {
                for (i, p) in pts.iter().enumerate() {
                    let h = hash_of(p);
                    let dynv: Vec<String> = p
                        .get("Dynamic")
                        .and_then(|d| d.as_map())
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| match v {
                                    Byml::Bool(b) => format!("{k}={b}"),
                                    x => format!("{k}={}", x.as_f64().map(|f| format!("{f}")).unwrap_or_default()),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let links = p.get("Links").and_then(|l| l.as_array()).map(|l| l.len()).unwrap_or(0);
                    rows.push(json!({"kind": "item", "key": format!("h:{h}"), "hash": h.to_string(),
                        "label": if links > 0 { format!("#{i}  → {links}") } else { format!("#{i}") }, "sub": dynv.join(" "), "selected": sel.contains(&h)}));
                }
            }
        }
        json!(rows)
    }

    /// Expand the outliner group holding the selection (one object or path point), so the
    /// outliner can scroll to it.
    pub fn reveal_selection(&mut self) {
        let (Some(doc), [h]) = (&self.doc, self.selection.as_slice()) else { return };
        let key = match doc.find(*h) {
            Some((Loc::Actor(_), a)) => format!("a:{}", a.str_or("Name", "")),
            Some((Loc::Point(r, _), _)) => format!("r:{}", hash_of(&doc.rails()[r])),
            _ => return,
        };
        self.expanded.insert(key);
    }

    pub fn group_hashes(&self, key: &str) -> Vec<u64> {
        let Some(doc) = &self.doc else { return vec![] };
        if let Some(name) = key.strip_prefix("a:") {
            return doc.actors().iter().filter(|a| a.str_or("Name", "") == name).map(hash_of).collect();
        }
        if let Some(h) = key.strip_prefix("r:").and_then(|h| h.parse().ok()) {
            return vec![h];
        }
        vec![]
    }

    pub fn set_group_visible(&mut self, key: &str, visible: bool) {
        if let Some(name) = key.strip_prefix("a:") {
            if visible {
                self.view.hidden_actor_names.remove(name);
            } else {
                self.view.hidden_actor_names.insert(name.into());
            }
        } else if let Some(h) = key.strip_prefix("r:").and_then(|h| h.parse::<u64>().ok()) {
            if visible {
                self.view.hidden_rails.remove(&h);
            } else {
                self.view.hidden_rails.insert(h);
            }
        }
        self.rebuild();
    }

    // ------------------------------------------------------------ properties

    /// The parameter table of an object, path or point.
    pub fn param_table(&self, h: u64) -> Option<String> {
        let (doc, cat) = (self.doc.as_ref()?, self.cat.as_ref()?);
        match doc.find(h)? {
            (Loc::Actor(_), a) => cat.actor(a.str_or("Gyaml", "")).map(|i| i.banc_param.clone()),
            (Loc::Point(r, _), _) => cat.data.rails.get(doc.rails()[r].str_or("Name", "")).map(|d| d.point_param.clone()),
            (Loc::Rail(_), r) => cat.data.rails.get(r.str_or("Name", "")).map(|d| d.rail_param.clone()),
        }
        .filter(|t| !t.is_empty())
    }

    /// The selected items that share the first one's parameter table (all of them when they
    /// all do), for editing a parameter on many at once.
    pub fn param_targets(&self) -> Vec<u64> {
        let Some(first) = self.selection.first().and_then(|h| self.param_table(*h)) else { return self.selection.clone() };
        self.selection.iter().copied().filter(|h| self.param_table(*h).as_deref() == Some(first.as_str())).collect()
    }

    /// Parameter rows for several items of one table: a value they all share shows as set,
    /// differing values as "mixed".
    fn shared_params_json(&self, owners: &[&Byml], table: &str) -> Value {
        let Some(cat) = &self.cat else { return json!([]) };
        let mut schema: Vec<&Param> = cat.data.params.get(table).map(|v| v.iter().collect()).unwrap_or_default();
        schema.sort_by(|a, b| a.name.cmp(&b.name));
        json!(schema.iter().map(|p| {
            let vals: Vec<Option<&Byml>> = owners.iter().map(|o| o.get("Dynamic").and_then(|d| d.get(&p.name))).collect();
            let all_same = vals.windows(2).all(|w| w[0] == w[1]);
            let cur = if all_same { vals.first().copied().flatten() } else { None };
            json!({"name": p.name, "label": p.label, "type": p.kind, "enumName": p.enum_name, "init": p.init, "initName": p.init_name,
                "set": cur.is_some() || !all_same, "mixed": !all_same, "value": cur.map(display).unwrap_or(Value::Null),
                "suggest": cat.data.seen_values.get(&p.name).cloned().unwrap_or_default()})
        }).collect::<Vec<_>>())
    }

    fn params_json(&self, owner: &Byml, table: &str) -> Value {
        let Some(cat) = &self.cat else { return json!([]) };
        let empty = BTreeMap::new();
        let dynm = owner.get("Dynamic").and_then(|d| d.as_map()).unwrap_or(&empty);
        let mut schema: Vec<&Param> = cat.data.params.get(table).map(|v| v.iter().collect()).unwrap_or_default();
        schema.sort_by(|a, b| a.name.cmp(&b.name));
        let mut rows = vec![];
        let known: HashSet<&str> = schema.iter().map(|p| p.name.as_str()).collect();
        for p in schema {
            let cur = dynm.get(&p.name);
            rows.push(
                json!({"name": p.name, "label": p.label, "type": p.kind, "enumName": p.enum_name, "init": p.init, "initName": p.init_name,
                "set": cur.is_some(), "value": cur.map(display).unwrap_or(Value::Null),
                "suggest": cat.data.seen_values.get(&p.name).cloned().unwrap_or_default()}),
            );
        }
        for (k, v) in dynm {
            if known.contains(k.as_str()) {
                continue;
            }
            rows.push(json!({"name": k, "label": "not in the parameter table", "type": byml_kind(v), "set": true, "value": display(v), "extra": true,
                "suggest": cat.data.seen_values.get(k).cloned().unwrap_or_default()}));
        }
        json!(rows)
    }

    fn links_json(&self, owner: &Byml) -> Value {
        let Some(doc) = &self.doc else { return json!([]) };
        json!(
            owner
                .get("Links")
                .and_then(|l| l.as_array())
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, l)| {
                    let dst = l.get("Dst").and_then(|d| d.as_u64()).unwrap_or(0);
                    let desc = match doc.find(dst) {
                        Some((Loc::Point(r, p), _)) => format!("{} point #{p}", doc.rails()[r].str_or("Name", "")),
                        Some((_, e)) => e.str_or("Name", "").to_string(),
                        None => format!("{dst} (not in this file)"),
                    };
                    json!({"i": i, "type": l.str_or("Name", ""), "target": desc, "dst": dst.to_string(), "found": doc.find(dst).is_some()})
                })
                .collect::<Vec<_>>()
        )
    }

    fn raw_json(owner: &Byml, skip: &[&str]) -> String {
        let mut m = serde_json::Map::new();
        for (k, v) in owner.as_map().into_iter().flatten() {
            if !skip.contains(&k.as_str()) {
                m.insert(k.clone(), typedjson::to_json(v));
            }
        }
        serde_json::to_string_pretty(&Value::Object(m)).unwrap()
    }

    pub const ACTOR_KEEP: [&'static str; 6] = ["Translate", "Rotate", "Scale", "Dynamic", "Links", "Hash"];
    pub const POINT_KEEP: [&'static str; 10] =
        ["Translation", "Rotate", "RectSize", "RectOffset", "Control0", "Control1", "Dynamic", "Links", "Hash", "Parent"];
    pub const RAIL_KEEP: [&'static str; 4] = ["Points", "Dynamic", "Hash", "IsClosed"];

    pub fn selection_json(&self) -> Value {
        let (Some(doc), Some(cat)) = (&self.doc, &self.cat) else { return json!({"kind": "none"}) };
        if self.selection.is_empty() {
            let d = doc;
            let pts: usize = d.rails().iter().map(|r| Document::points(r).len()).sum();
            let collects = d
                .root
                .get("Collects")
                .and_then(|c| c.as_map())
                .map(|m| m.iter().map(|(k, v)| format!("{k} {}", v.as_f64().unwrap_or(0.0))).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            return json!({"kind": "course", "file": d.file, "objects": d.actors().len(), "paths": d.rails().len(), "points": pts, "collects": collects,
                "padEntries": d.pad.as_ref().map(|p| if let Byml::Hash64(m) = p { m.len() } else { 0 }).unwrap_or(0), "padFile": d.pad_file, "fromMod": d.from_mod});
        }
        if self.selection.len() > 1 {
            let (mut objects, mut points, mut paths) = (0, 0, 0);
            let mut names: BTreeMap<String, usize> = BTreeMap::new();
            for h in &self.selection {
                match doc.find(*h) {
                    Some((Loc::Actor(_), a)) => {
                        objects += 1;
                        *names.entry(a.str_or("Name", "").to_string()).or_default() += 1;
                    }
                    Some((Loc::Point(r, _), _)) => {
                        points += 1;
                        *names.entry(doc.rails()[r].str_or("Name", "").to_string() + " points").or_default() += 1;
                    }
                    Some((Loc::Rail(_), r)) => {
                        paths += 1;
                        *names.entry(r.str_or("Name", "").to_string() + " paths").or_default() += 1;
                    }
                    None => {}
                }
            }
            let pos: Vec<Vec3> = doc.movable(&self.selection).into_iter().map(|x| x.1).collect();
            let mut types: Vec<(String, usize)> = names.into_iter().collect();
            types.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let (centre, size) = if pos.is_empty() {
                (Vec3::ZERO, Vec3::ZERO)
            } else {
                let mn = pos.iter().fold(Vec3::splat(f32::MAX), |a, b| a.min(*b));
                let mx = pos.iter().fold(Vec3::splat(f32::MIN), |a, b| a.max(*b));
                (pos.iter().sum::<Vec3>() / pos.len() as f32, mx - mn)
            };
            let distance = (pos.len() == 2).then(|| pos[0].distance(pos[1]));
            // parameters, when every selected item has the same table
            let table = self.param_table(self.selection[0]);
            let shared = table.as_ref().is_some_and(|t| self.selection.iter().all(|h| self.param_table(*h).as_ref() == Some(t)));
            let params = match (&table, shared) {
                (Some(t), true) => {
                    let owners: Vec<&Byml> = self.selection.iter().filter_map(|h| doc.find(*h).map(|x| x.1)).collect();
                    self.shared_params_json(&owners, t)
                }
                _ => json!([]),
            };
            let rails = doc.rail_targets(&self.selection).len();
            // selected lap points, and how many of them are checkpoints
            let (mut lap_points, mut checkpoints) = (0, 0);
            for h in &self.selection {
                if let Some((Loc::Point(r, _), p)) = doc.find(*h)
                    && doc.rails()[r].str_or("Name", "") == mkwtrack::checkpoints::LAP
                {
                    lap_points += 1;
                    checkpoints += mkwtrack::checkpoints::is_checkpoint(p) as usize;
                }
            }
            return json!({"kind": "multi", "lapPoints": lap_points, "checkpoints": checkpoints, "count": self.selection.len(), "objects": objects, "points": points, "paths": paths, "rails": rails,
                "types": types.iter().take(6).map(|(n, c)| json!({"name": n, "count": c})).collect::<Vec<_>>(), "moreTypes": types.len().saturating_sub(6),
                "centre": v3(centre), "size": v3(size), "distance": distance, "positioned": pos.len(),
                "paramTable": if shared { table } else { None }, "params": params});
        }
        let h = self.selection[0];
        let Some((loc, e)) = doc.find(h) else { return json!({"kind": "none"}) };
        match loc {
            Loc::Actor(_) => {
                let info = cat.actor(e.str_or("Gyaml", ""));
                json!({"kind": "actor", "hash": h.to_string(), "name": e.str_or("Name", ""), "label": info.map(|i| i.label.clone()), "thumb": thumb_url(info),
                    "category": info.map(|i| i.category.clone()), "collect": info.and_then(|i| i.collect.clone()), "gyaml": e.str_or("Gyaml", ""),
                    "hasModel": info.and_then(|i| i.model.as_ref()).is_some(), "instanceId": e.str_or("InstanceId", ""),
                    "translate": e.vec3("Translate", [0.0; 3]), "rotate": deg(e.vec3("Rotate", [0.0; 3])), "scale": e.vec3("Scale", [1.0; 3]),
                    "paramTable": info.map(|i| i.banc_param.clone()), "params": self.params_json(e, info.map(|i| i.banc_param.as_str()).unwrap_or("")),
                    "links": self.links_json(e), "raw": Self::raw_json(e, &Self::ACTOR_KEEP)})
            }
            Loc::Point(r, i) => {
                let rail = &doc.rails()[r];
                let name = rail.str_or("Name", "");
                let def = cat.data.rails.get(name);
                let pad = match &doc.pad {
                    Some(Byml::Hash64(m)) => match m.get(&h) {
                        Some(Byml::Hash64(groups)) => groups
                            .iter()
                            .flat_map(|(g, vals)| {
                                vals.as_map()
                                    .into_iter()
                                    .flatten()
                                    .map(move |(k, v)| json!({"group": g.to_string(), "key": k, "value": display(v), "type": byml_kind(v)}))
                            })
                            .collect::<Vec<_>>(),
                        _ => vec![],
                    },
                    _ => vec![],
                };
                let opt = |k: &str| e.get(k).map(|v| json!(vec3(Some(v), [0.0; 3])));
                let two = |k: &str| {
                    e.get(k).and_then(|v| v.as_array()).map(|a| json!(a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect::<Vec<_>>()))
                };
                let lap = name == mkwtrack::checkpoints::LAP;
                let checkpoint = (lap && mkwtrack::checkpoints::is_checkpoint(e))
                    .then(|| e.get("Dynamic").and_then(|d| d.get(mkwtrack::checkpoints::KEY)).and_then(|v| v.as_f64()).unwrap_or(0.0));
                let marker = mkwtrack::checkpoints::MARKERS
                    .iter()
                    .find(|k| lap && e.get("Dynamic").and_then(|d| d.get(k)).and_then(|b| b.as_bool()) == Some(true))
                    .map(|k| match *k {
                        "IsStart" => "start",
                        "IsLapCheck" => "lap check",
                        _ => "goal",
                    });
                json!({"kind": "point", "hash": h.to_string(), "rail": name, "lap": lap, "checkpoint": checkpoint, "lapMarker": marker, "railHash": hash_of(rail).to_string(), "index": i, "count": Document::points(rail).len(),
                    "closed": rail.get("IsClosed").and_then(|b| b.as_bool()).unwrap_or(false), "handle": self.handle.map(|x| x.1),
                    "translate": e.vec3("Translation", [0.0; 3]), "rotate": deg(e.vec3("Rotate", [0.0; 3])),
                    "rectSize": two("RectSize"), "rectOffset": two("RectOffset"), "control0": opt("Control0"), "control1": opt("Control1"),
                    "params": self.params_json(e, def.map(|d| d.point_param.as_str()).unwrap_or("")), "paramTable": def.map(|d| d.point_param.clone()),
                    "pad": pad, "links": self.links_json(e), "raw": Self::raw_json(e, &Self::POINT_KEEP)})
            }
            Loc::Rail(_) => {
                let name = e.str_or("Name", "");
                let def = cat.data.rails.get(name);
                json!({"kind": "rail", "hash": h.to_string(), "name": name, "count": Document::points(e).len(),
                    "closed": e.get("IsClosed").and_then(|b| b.as_bool()).unwrap_or(false), "railType": def.map(|d| d.rail_type.clone()),
                    "params": self.params_json(e, def.map(|d| d.rail_param.as_str()).unwrap_or("")), "paramTable": def.map(|d| d.rail_param.clone()),
                    "raw": Self::raw_json(e, &Self::RAIL_KEEP)})
            }
        }
    }

    // ------------------------------------------------------------ assets

    /// The asset browser's list. `category` "favorites" lists the starred ones.
    pub fn asset_json(&self, filter: &str, category: &str, placed_only: bool, favorites: &std::collections::BTreeSet<String>) -> Value {
        let Some(cat) = &self.cat else { return json!([]) };
        let f = filter.to_lowercase();
        let (category, favs_only) = if category == "favorites" { ("", true) } else { (category, false) };
        let in_doc: HashSet<&str> =
            self.doc.as_ref().map(|d| d.actors().iter().map(|a| a.str_or("Name", "")).collect()).unwrap_or_default();
        let mut out: Vec<(bool, &String, &ActorInfo)> = cat
            .data
            .actors
            .iter()
            .filter(|(n, a)| {
                (!placed_only || a.placed || favs_only)
                    && (!favs_only || favorites.contains(n.as_str()))
                    && (category.is_empty() || a.category == category)
                    && (f.is_empty() || n.to_lowercase().contains(&f) || a.label.to_lowercase().contains(&f))
            })
            .map(|(n, a)| (in_doc.contains(n.as_str()), n, a))
            .collect();
        out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        json!(out.iter().take(600).map(|(here, n, a)| {
            json!({"name": n, "short": short_name(n), "label": a.label, "category": a.category, "model": a.model.is_some(), "inCourse": here, "fav": favorites.contains(n.as_str()),
                   "color": scene::hex(scene::category_color(&a.category)), "thumb": thumb_url(Some(a))})
        }).collect::<Vec<_>>())
    }

    /// Export the saved mod for ns2jb-modloader (see `Workspace::export_ns2jb`).
    pub fn export_ns2jb(&self) -> Value {
        let res = self.ws().export_ns2jb(&self.mod_name, &self.cfg.ns2jb);
        json!({"ok": res.ok, "errors": res.errors, "notes": res.notes, "dir": res.mod_romfs.display().to_string(),
            "files": res.written.iter().map(|w| format!("romfs/{w}")).collect::<Vec<_>>()})
    }

    pub fn categories_json(&self) -> Value {
        let Some(cat) = &self.cat else { return json!([]) };
        let mut c: Vec<&String> =
            cat.data.actors.values().map(|a| &a.category).filter(|c| !c.is_empty()).collect::<HashSet<_>>().into_iter().collect();
        c.sort();
        json!(c)
    }

    pub fn rail_types_json(&self) -> Value {
        let Some(cat) = &self.cat else { return json!([]) };
        let mut counts: HashMap<&str, usize> = HashMap::new();
        if let Some(d) = &self.doc {
            for r in d.rails() {
                *counts.entry(r.str_or("Name", "")).or_default() += 1;
            }
        }
        let mut v: Vec<Value> = cat
            .data
            .rails
            .iter()
            .map(|(n, d)| {
                json!({"name": n, "type": d.rail_type, "count": counts.get(n.as_str()).copied().unwrap_or(0),
            "color": scene::hex(scene::rail_color(n))})
            })
            .collect();
        v.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()).then(a["name"].as_str().cmp(&b["name"].as_str())));
        json!(v)
    }

    pub fn view_json(&self) -> Value {
        let (Some(doc), Some(cat)) = (&self.doc, &self.cat) else { return json!({"rails": [], "categories": []}) };
        let mut rails: Vec<&str> = doc.rails().iter().map(|r| r.str_or("Name", "")).collect::<HashSet<_>>().into_iter().collect();
        rails.sort();
        let mut cats: Vec<String> = doc
            .actors()
            .iter()
            .map(|a| cat.actor(a.str_or("Gyaml", "")).map(|i| i.category.clone()).unwrap_or("Other".into()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        json!({"rails": rails.iter().map(|r| json!({"name": r, "visible": !self.view.hidden_rail_types.contains(*r), "color": scene::hex(scene::rail_color(r))})).collect::<Vec<_>>(),
               "categories": cats.iter().map(|c| json!({"name": c, "visible": !self.view.hidden_categories.contains(c), "color": scene::hex(scene::category_color(c))})).collect::<Vec<_>>(),
               "stage": self.show_stage, "models": self.view.models, "xray": self.xray, "widths": self.view.widths, "snap": self.snap_ground,
               "pickFilter": self.pick_filter, "snapStep": self.snap.step, "snapAngle": self.snap.angle,
               "hidden": self.view.hidden_items.len() + self.view.hidden_rails.len() + self.view.hidden_actor_names.len() + self.view.hidden_rail_types.len() + self.view.hidden_categories.len()})
    }

    // ------------------------------------------------------------ save

    pub fn save(&mut self) -> Value {
        let (Some(doc), Some(cat)) = (&self.doc, &self.cat) else { return json!({"ok": false, "errors": ["no course open"]}) };
        if doc.file.is_empty() {
            return json!({"ok": false, "needTarget": true, "errors": ["Choose which course this one replaces (File → Save As)"]});
        }
        let res = self.ws().save(&doc.file, &doc.root, doc.pad.as_ref(), &self.mod_name, &doc.deleted, Some(cat));
        let emu = emulator_load_dir();
        let mod_dir = res.mod_romfs.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let install = if mod_dir.starts_with(&emu) {
            format!("Already in the emulator's mod folder: {}", crate::config::tilde(&mod_dir.display().to_string()))
        } else {
            let (emu, md) = (crate::config::shell_path(&emu), crate::config::shell_path(&mod_dir));
            if cfg!(windows) {
                // PowerShell
                format!("New-Item -ItemType Directory -Force \"{emu}\" | Out-Null; Copy-Item -Recurse -Force \"{md}\" \"{emu}\"")
            } else {
                format!("mkdir -p \"{emu}\" && cp -r \"{md}\" \"{emu}/\"")
            }
        };
        // Switch 2: copy the saved mod into ns2jb-modloader's SD layout too
        let ns2jb = (res.ok && self.cfg.ns2jb_on_save && !self.cfg.ns2jb.as_os_str().is_empty()).then(|| self.export_ns2jb());
        let v = json!({"ok": res.ok, "errors": res.errors, "warnings": res.warnings, "notes": res.notes, "mod": self.mod_name, "ns2jb": ns2jb,
            "written": res.written.iter().map(|w| res.mod_romfs.join(w).display().to_string()).collect::<Vec<_>>(), "install": install,
            "files": res.written.iter().map(|w| format!("romfs/{w}")).collect::<Vec<_>>(), "modDir": mod_dir.display().to_string(),
            "installed": mod_dir.starts_with(&emu)});
        if res.ok {
            if let Some(d) = &mut self.doc {
                d.dirty = false;
            }
            // the folder first, so the status bar (the last line) stays short
            self.log(format!("Mod folder: {}", mod_dir.display()));
            self.log(format!("Saved {} file(s) to mod \"{}\"", res.written.len(), self.mod_name));
            for n in &res.notes {
                self.log(format!("  {n}"));
            }
        } else {
            self.log(format!("Not saved: {}", res.errors.join("; ")));
        }
        v
    }
}

/// file:// URL of the object's cached thumbnail, or "" when it has no model or none is built yet.
fn thumb_url(info: Option<&ActorInfo>) -> String {
    let Some(p) = info.and_then(|i| i.model.as_ref()).map(|m| crate::thumbs::thumb_path(m)) else { return String::new() };
    if p.exists() { crate::config::file_url(&p) } else { String::new() }
}

pub fn short_name(n: &str) -> String {
    let s = n.strip_prefix("CmnObj_").or_else(|| n.strip_prefix("SystemObj_")).unwrap_or(n);
    let s = s.strip_suffix("_00").unwrap_or(s);
    s.replace("000_A", "").replace('_', " ")
}

pub fn display(v: &Byml) -> Value {
    match v {
        Byml::Array(a) => json!(a.iter().map(|x| x.as_f64().map(|f| json!(f)).unwrap_or(json!(null))).collect::<Vec<_>>()),
        other => typedjson::to_plain(other),
    }
}

pub fn byml_kind(v: &Byml) -> &'static str {
    match v {
        Byml::Bool(_) => "bool",
        Byml::I32(_) => "s32",
        Byml::U32(_) => "u32",
        Byml::F32(_) => "f32",
        Byml::F64(_) => "f64",
        Byml::I64(_) => "s64",
        Byml::U64(_) => "u64",
        Byml::Str(_) => "str",
        Byml::Array(a) if a.len() == 3 => "vec3",
        _ => "other",
    }
}

/// 32-bit FNV-1a, to name a cache folder after a path.
fn fnv1a(b: &[u8]) -> u32 {
    b.iter().fold(0x811c9dc5u32, |h, &x| (h ^ x as u32).wrapping_mul(0x01000193))
}
