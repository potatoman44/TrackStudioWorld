//! An open course and every editing operation on it, with undo and redo.
//!
//! Objects (actors) are found by their `Hash`. Paths (rails) keep their points in `Points`;
//! each point's `Parent` is the path's hash, and paths join up through `Links` from a path's
//! last point to the next path's first point.

use crate::banc::hash_of;
use crate::byml::{Byml, vec3};
use crate::catalog::Catalog;
use crate::checkpoints;
use crate::error::Result;
use crate::math::{euler_from_quat, quat_zyx};
use crate::typedjson;
use crate::workspace::Loaded;
use glam::{Quat, Vec3};
use std::collections::{HashMap, HashSet};

/// Point parameters that mark one specific point and must not be copied to a new one.
pub(crate) const UNIQUE_POINT_KEYS: [&str; 5] = ["IsStart", "IsGoal", "IsLapCheck", "CheckPoint", "ChangeId"];
/// Link types the editor creates.
pub const LINK_TYPES: [&str; 3] = ["BasicSignal", "MessageLink", "CreateLink"];
/// Undo steps kept.
const UNDO_LIMIT: usize = 50;

/// Where an item lives in the course: an object, a path, or a point of a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Loc {
    Actor(usize),
    Rail(usize),
    Point(usize, usize),
}

#[derive(Clone)]
struct Snapshot {
    root: Byml,
    pad: Option<Byml>,
    deleted: HashSet<u64>,
    /// what the step did ("Move", "Delete"...), for the Undo and Redo menu items
    label: String,
}

pub struct Document {
    pub file: String,
    pub version: u16,
    pub root: Byml,
    /// PathAdditionalData: per-point width and clearance values, keyed by point hash
    pub pad: Option<Byml>,
    pub pad_file: Option<String>,
    pub from_mod: bool,
    /// hashes deleted since loading (their PathAdditionalData entries are dropped on save)
    pub deleted: HashSet<u64>,
    pub dirty: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub index: HashMap<u64, Loc>,
    /// every hash in use (including PathAdditionalData keys), so new ones never collide
    used: HashSet<u64>,
}

/// A random version-4-style UUID string, the form of `InstanceId`.
pub fn new_uuid() -> String {
    let h = format!("{:032x}", fastrand::u128(..));
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub(crate) fn link(src: u64, dst: u64, kind: &str) -> Byml {
    let mut l = Byml::map();
    l.insert("Dst", Byml::U64(dst));
    l.insert("Gyaml", Byml::Str(format!("Work/Gyml/LinkDefGParam/{kind}.gear__LinkDefGParam.gyml")));
    l.insert("Name", Byml::Str(kind.into()));
    l.insert("Src", Byml::U64(src));
    l
}

pub(crate) fn point_pos(p: &Byml) -> Vec3 {
    Vec3::from(p.vec3("Translation", [0.0; 3]))
}

/// A point's Bézier handle (`Control0` or `Control1`), if it has one.
pub(crate) fn control(p: &Byml, key: &str) -> Option<Vec3> {
    p.get(key).map(|c| Vec3::from(vec3(Some(c), [0.0; 3])))
}

/// Move a point's Bézier handles along with it.
pub(crate) fn shift_controls(p: &mut Byml, by: Vec3) {
    for k in ["Control0", "Control1"] {
        if let Some(c) = p.get(k) {
            let c = Vec3::from(vec3(Some(c), [0.0; 3])) + by;
            p.insert(k, Byml::floats(&c.to_array()));
        }
    }
}

/// A curved segment split in half (de Casteljau at t = 0.5).
#[derive(Clone, Copy)]
struct CurveSplit {
    /// the point after the split segment
    next: usize,
    /// whether the segment's start had an outgoing handle, and its end an incoming one
    had_out: bool,
    had_in: bool,
    /// the halves' outer handles: the start's new outgoing one and the end's new incoming one
    out_half: Vec3,
    in_half: Vec3,
    /// the new middle point's handles
    new_in: Vec3,
    new_out: Vec3,
}

/// Point every link of `item` from `src`.
pub(crate) fn set_link_sources(item: &mut Byml, src: u64) {
    if let Some(Byml::Array(ls)) = item.get_mut("Links") {
        for l in ls.iter_mut() {
            l.insert("Src", Byml::U64(src));
        }
    }
}

impl Document {
    pub fn new(l: Loaded) -> Self {
        let mut d = Document {
            file: l.file,
            version: l.version,
            root: l.doc,
            pad: l.pad,
            pad_file: l.pad_file,
            from_mod: l.from_mod,
            deleted: HashSet::new(),
            dirty: false,
            undo: vec![],
            redo: vec![],
            index: HashMap::new(),
            used: HashSet::new(),
        };
        d.reindex();
        d
    }

    /// Make this the course that replaces `rel` when it is saved: its file, its
    /// PathAdditionalData file and the `FilePath` it names itself by. A course without
    /// PathAdditionalData gets an empty one, so the replaced course's entries are not left
    /// behind.
    pub fn retarget(&mut self, rel: &str) {
        self.file = rel.to_string();
        self.pad_file = Some(crate::banc::pad_rel(rel));
        self.root.insert("FilePath", Byml::Str(crate::banc::file_path_for(rel)));
        if self.pad.is_none() {
            self.pad = Some(Byml::Hash64(Default::default()));
        }
        self.from_mod = false;
        self.dirty = true;
    }

    // ------------------------------------------------------------ access

    pub fn actors(&self) -> &[Byml] {
        self.root.array("Actors")
    }

    pub fn rails(&self) -> &[Byml] {
        self.root.array("Rails")
    }

    pub fn actors_mut(&mut self) -> &mut Vec<Byml> {
        self.root.array_mut("Actors")
    }

    pub fn rails_mut(&mut self) -> &mut Vec<Byml> {
        self.root.array_mut("Rails")
    }

    pub fn points(rail: &Byml) -> &[Byml] {
        rail.array("Points")
    }

    pub(crate) fn points_mut(rail: &mut Byml) -> &mut Vec<Byml> {
        rail.array_mut("Points")
    }

    pub fn get(&self, loc: Loc) -> Option<&Byml> {
        match loc {
            Loc::Actor(i) => self.actors().get(i),
            Loc::Rail(r) => self.rails().get(r),
            Loc::Point(r, p) => self.rails().get(r).and_then(|x| Self::points(x).get(p)),
        }
    }

    pub fn get_mut(&mut self, loc: Loc) -> Option<&mut Byml> {
        match loc {
            Loc::Actor(i) => self.actors_mut().get_mut(i),
            Loc::Rail(r) => self.rails_mut().get_mut(r),
            Loc::Point(r, p) => self.rails_mut().get_mut(r).and_then(|x| Self::points_mut(x).get_mut(p)),
        }
    }

    pub fn find(&self, hash: u64) -> Option<(Loc, &Byml)> {
        let loc = *self.index.get(&hash)?;
        Some((loc, self.get(loc)?))
    }

    /// The path a point or path hash belongs to.
    pub fn rail_hash_of(&self, hash: u64) -> Option<u64> {
        match self.index.get(&hash)? {
            Loc::Point(r, _) | Loc::Rail(r) => self.rails().get(*r).map(hash_of),
            Loc::Actor(_) => None,
        }
    }

    pub fn reindex(&mut self) {
        let mut index = HashMap::new();
        for (i, a) in self.actors().iter().enumerate() {
            index.insert(hash_of(a), Loc::Actor(i));
        }
        for (r, rail) in self.rails().iter().enumerate() {
            index.insert(hash_of(rail), Loc::Rail(r));
            for (p, pt) in Self::points(rail).iter().enumerate() {
                index.insert(hash_of(pt), Loc::Point(r, p));
            }
        }
        self.used = index.keys().copied().collect();
        if let Some(Byml::Hash64(m)) = &self.pad {
            self.used.extend(m.keys().copied());
        }
        self.index = index;
    }

    /// A hash nothing in the course uses yet.
    pub fn new_hash(&mut self) -> u64 {
        loop {
            let h = fastrand::u64(1..);
            if self.used.insert(h) {
                return h;
            }
        }
    }

    /// Give the lap checkpoints their numbers (see [`checkpoints`]); every edit that can change
    /// the lap graph or which points are checkpoints calls this. Returns how many changed.
    pub fn renumber_checkpoints(&mut self) -> usize {
        checkpoints::renumber(&mut self.root)
    }

    // ------------------------------------------------------------ undo

    /// Record the current state as an undo step; call before every change.
    pub fn push_undo(&mut self) {
        self.push_undo_as("Edit");
    }

    /// [`push_undo`](Self::push_undo) with a name for the Undo and Redo menu items.
    pub fn push_undo_as(&mut self, label: &str) {
        self.undo.push(Snapshot { root: self.root.clone(), pad: self.pad.clone(), deleted: self.deleted.clone(), label: label.into() });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// What Undo would undo, e.g. "Move".
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    /// What Redo would redo.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    /// Rename the newest undo step (a drag records "Move" when it starts and learns later
    /// whether it moved, rotated or scaled).
    pub fn relabel_undo(&mut self, label: &str) {
        if let Some(s) = self.undo.last_mut() {
            s.label = label.into();
        }
    }

    fn restore(&mut self, s: Snapshot) -> Snapshot {
        let cur = Snapshot {
            root: std::mem::replace(&mut self.root, s.root),
            pad: std::mem::replace(&mut self.pad, s.pad),
            deleted: std::mem::replace(&mut self.deleted, s.deleted),
            label: s.label,
        };
        self.reindex();
        self.dirty = true;
        cur
    }

    /// Go back to the state the newest undo step recorded, keeping the step (a modal move
    /// that switches axis starts again from where it began).
    pub fn revert_step(&mut self) {
        let Some(s) = self.undo.last().cloned() else { return };
        self.root = s.root;
        self.pad = s.pad;
        self.deleted = s.deleted;
        self.reindex();
    }

    /// Drop the newest step and its changes, as if it never happened (a cancelled drag).
    pub fn cancel_step(&mut self) {
        self.revert_step();
        self.undo.pop();
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        let cur = self.restore(s);
        self.redo.push(cur);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        let cur = self.restore(s);
        self.undo.push(cur);
        true
    }

    // ------------------------------------------------------------ transforms

    /// (position, rotation, scale) of an object or point (a path has none of its own).
    pub fn transform(&self, hash: u64) -> Option<(Vec3, Quat, Vec3)> {
        let (loc, e) = self.find(hash)?;
        let rot = quat_zyx(e.vec3("Rotate", [0.0; 3]));
        match loc {
            Loc::Actor(_) => Some((Vec3::from(e.vec3("Translate", [0.0; 3])), rot, Vec3::from(e.vec3("Scale", [1.0; 3])))),
            Loc::Point(..) => Some((point_pos(e), rot, Vec3::ONE)),
            Loc::Rail(_) => None,
        }
    }

    /// The objects and points a selection moves (a path means its points), each once, with
    /// their transforms.
    pub fn movable(&self, sel: &[u64]) -> Vec<(u64, Vec3, Quat, Vec3)> {
        let mut seen = HashSet::new();
        let mut out = vec![];
        for h in sel {
            let hs: Vec<u64> = match self.find(*h) {
                Some((Loc::Rail(_), r)) => Self::points(r).iter().map(hash_of).collect(),
                Some(_) => vec![*h],
                None => vec![],
            };
            for h in hs {
                if seen.insert(h)
                    && let Some((p, q, s)) = self.transform(h)
                {
                    out.push((h, p, q, s));
                }
            }
        }
        out
    }

    /// Set a transform without recording undo (a drag records one step when it starts).
    /// Moving a point moves its curve handles with it.
    pub fn set_transform(&mut self, hash: u64, pos: Vec3, rot: Quat, scale: Vec3) {
        let Some(loc) = self.index.get(&hash).copied() else { return };
        let euler = euler_from_quat(rot);
        let Some(e) = self.get_mut(loc) else { return };
        match loc {
            Loc::Actor(_) => {
                e.insert("Translate", Byml::floats(&pos.to_array()));
                // optional keys stay absent while they hold their defaults
                if e.get("Rotate").is_some() || euler.iter().any(|x| x.abs() > 1e-7) {
                    e.insert("Rotate", Byml::floats(&euler));
                }
                if e.get("Scale").is_some() || (scale - Vec3::ONE).abs().max_element() > 1e-6 {
                    e.insert("Scale", Byml::floats(&scale.to_array()));
                }
            }
            Loc::Point(..) => {
                let moved = pos - point_pos(e);
                e.insert("Translation", Byml::floats(&pos.to_array()));
                e.insert("Rotate", Byml::floats(&euler));
                shift_controls(e, moved);
            }
            Loc::Rail(_) => {}
        }
        self.dirty = true;
    }

    /// Set a point's Bézier handles without recording undo (drags move them with the point:
    /// `None` leaves a handle as it is).
    pub fn set_controls(&mut self, hash: u64, c0: Option<Vec3>, c1: Option<Vec3>) {
        let Some(loc @ Loc::Point(..)) = self.index.get(&hash).copied() else { return };
        let Some(e) = self.get_mut(loc) else { return };
        for (k, c) in [("Control0", c0), ("Control1", c1)] {
            if let Some(c) = c {
                e.insert(k, Byml::floats(&c.to_array()));
            }
        }
        self.dirty = true;
    }

    /// Move one Bézier handle of a point (`which` 0 is `Control0`, 1 is `Control1`) without
    /// recording undo. With `aligned`, the other handle turns to stay on the same line
    /// through the point, keeping its length.
    pub fn set_handle(&mut self, hash: u64, which: usize, pos: Vec3, aligned: bool) {
        let Some(loc @ Loc::Point(..)) = self.index.get(&hash).copied() else { return };
        let Some(e) = self.get_mut(loc) else { return };
        let (key, other) = if which == 0 { ("Control0", "Control1") } else { ("Control1", "Control0") };
        e.insert(key, Byml::floats(&pos.to_array()));
        let p = point_pos(e);
        if aligned && let Some(o) = control(e, other) {
            let dir = (p - pos).normalize_or_zero();
            if dir != Vec3::ZERO {
                e.insert(other, Byml::floats(&(p + dir * o.distance(p)).to_array()));
            }
        }
        self.dirty = true;
    }

    /// Set one vector field (Translate, Rotate, Scale, Translation, RectSize, Control0...).
    pub fn set_vector(&mut self, hash: u64, key: &str, v: &[f32]) {
        let Some(loc) = self.index.get(&hash).copied() else { return };
        self.push_undo_as(match key {
            "Rotate" => "Rotate",
            "Scale" => "Scale",
            "Control0" | "Control1" => "Move Curve Handle",
            "RectSize" | "RectOffset" => "Resize Gate",
            _ => "Move",
        });
        let Some(e) = self.get_mut(loc) else { return };
        if key == "Translation" && v.len() == 3 {
            let moved = Vec3::new(v[0], v[1], v[2]) - point_pos(e);
            shift_controls(e, moved);
        }
        e.insert(key, Byml::floats(v));
    }

    // ------------------------------------------------------------ parameters and fields

    /// Set (or with None, remove) one parameter under `Dynamic`.
    pub fn set_dynamic(&mut self, hash: u64, key: &str, value: Option<Byml>) {
        self.set_dynamic_many(&[hash], key, value);
    }

    /// Set (or remove) one parameter on several objects or points, as one undo step.
    pub fn set_dynamic_many(&mut self, hashes: &[u64], key: &str, value: Option<Byml>) {
        let locs: Vec<Loc> = hashes.iter().filter_map(|h| self.index.get(h).copied()).collect();
        if locs.is_empty() {
            return;
        }
        self.push_undo_as(if value.is_some() { "Set Parameter" } else { "Remove Parameter" });
        for loc in locs {
            let Some(e) = self.get_mut(loc) else { continue };
            if e.get("Dynamic").and_then(|d| d.as_map()).is_none() {
                e.insert("Dynamic", Byml::map());
            }
            let dynamic = e.get_mut("Dynamic").expect("just inserted");
            match &value {
                Some(v) => dynamic.insert(key, v.clone()),
                None => {
                    dynamic.remove(key);
                }
            }
            if dynamic.as_map().is_some_and(|m| m.is_empty()) {
                e.remove("Dynamic");
            }
        }
        self.renumber_checkpoints();
    }

    pub fn set_field(&mut self, hash: u64, key: &str, value: Option<Byml>) {
        let Some(loc) = self.index.get(&hash).copied() else { return };
        self.push_undo_as(&format!("Set {key}"));
        if let Some(e) = self.get_mut(loc) {
            match value {
                Some(v) => e.insert(key, v),
                None => {
                    e.remove(key);
                }
            }
        }
        self.renumber_checkpoints();
    }

    /// Replace every key except `keep` with the fields from typed JSON text.
    pub fn replace_fields(&mut self, hash: u64, json: &str, keep: &[&str]) -> Result<()> {
        let v: serde_json::Value = serde_json::from_str(json)?;
        let Byml::Map(new) = typedjson::from_json(&v)? else { return Err("expected a JSON object".into()) };
        let loc = *self.index.get(&hash).ok_or("nothing selected")?;
        self.push_undo_as("Edit Fields");
        let m = self.get_mut(loc).and_then(|e| e.as_map_mut()).ok_or("not an object")?;
        m.retain(|k, _| keep.contains(&k.as_str()));
        m.extend(new.into_iter().filter(|(k, _)| !keep.contains(&k.as_str())));
        self.reindex();
        self.renumber_checkpoints();
        Ok(())
    }

    /// Give point `to` a copy of point `from`'s PathAdditionalData entry, if it has one. Shipped
    /// courses give 99.6% of RoutePathGraph points an entry and no other point any, so new route
    /// points copy their neighbour's rather than leave the game a state it rarely ships with.
    pub(crate) fn copy_pad(&mut self, from: u64, to: u64) {
        if let Some(Byml::Hash64(m)) = &mut self.pad
            && let Some(e) = m.get(&from).cloned()
        {
            m.insert(to, e);
        }
    }

    /// Set a PathAdditionalData value of a point.
    pub fn set_pad_value(&mut self, hash: u64, group: u64, key: &str, value: Byml) {
        self.push_undo_as("Set Path Data");
        if let Some(Byml::Hash64(m)) = &mut self.pad
            && let Some(Byml::Hash64(g)) = m.get_mut(&hash)
            && let Some(e) = g.get_mut(&group)
        {
            e.insert(key, value);
        }
    }

    /// Add Bézier handles 4 m either side of a point along its heading, or remove them.
    pub fn toggle_curve(&mut self, hash: u64) {
        let Some(loc @ Loc::Point(..)) = self.index.get(&hash).copied() else { return };
        self.push_undo_as("Curve Handles");
        let e = self.get_mut(loc).expect("indexed point");
        if e.get("Control0").is_some() || e.get("Control1").is_some() {
            e.remove("Control0");
            e.remove("Control1");
        } else {
            let t = point_pos(e);
            let fwd = quat_zyx(e.vec3("Rotate", [0.0; 3])) * Vec3::Z * 4.0;
            e.insert("Control0", Byml::floats(&(t - fwd).to_array()));
            e.insert("Control1", Byml::floats(&(t + fwd).to_array()));
        }
    }

    // ------------------------------------------------------------ objects

    /// Add an object of type `name` standing at `pos` (the ground under it), turned `yaw`.
    ///
    /// Shipped placements already include the type's `BancTransOffset` (every shipped item box
    /// sits 1.5 m above the road, its offset), and the game only registers the offset as an
    /// editor-display field (next to `BancDummyShape` and `BancColor`), so it is added here.
    pub fn add_actor(&mut self, cat: &Catalog, name: &str, pos: Vec3, yaw: f32) -> Option<u64> {
        cat.data.actors.get(name)?;
        self.push_undo_as("Add Object");
        self.add_actor_raw(cat, name, pos, yaw)
    }

    /// [`add_actor`](Self::add_actor) without recording an undo step.
    pub(crate) fn add_actor_raw(&mut self, cat: &Catalog, name: &str, pos: Vec3, yaw: f32) -> Option<u64> {
        let info = cat.data.actors.get(name)?;
        let mut a = match info.template.as_ref().and_then(|t| typedjson::from_json(t).ok()) {
            Some(m @ Byml::Map(_)) => m,
            _ => {
                let mut m = Byml::map();
                m.insert("IsGridify", Byml::Bool(false));
                m
            }
        };
        let h = self.new_hash();
        let lift = info.banc_offset.map_or(0.0, |o| o[1]);
        a.insert("Gyaml", Byml::Str(info.gyaml.clone()));
        a.insert("Hash", Byml::U64(h));
        a.insert("InstanceId", Byml::Str(new_uuid()));
        a.insert("Name", Byml::Str(name.into()));
        a.insert("Rotate", Byml::floats(&[0.0, yaw, 0.0]));
        a.insert("Translate", Byml::floats(&[pos.x, pos.y + lift, pos.z]));
        self.actors_mut().push(a);
        self.reindex();
        Some(h)
    }

    /// Change an object's type in place: new Gyaml, Name and template fields; parameters are
    /// kept only where the new type's parameter table has them.
    pub fn change_type(&mut self, cat: &Catalog, hash: u64, name: &str) -> Result<()> {
        let info = cat.data.actors.get(name).ok_or("unknown object type")?.clone();
        let Some(loc @ Loc::Actor(_)) = self.index.get(&hash).copied() else { return Err("select an object".into()) };
        self.push_undo_as("Change Object");
        let keep: HashSet<&str> =
            cat.data.params.get(&info.banc_param).map(|v| v.iter().map(|p| p.name.as_str()).collect()).unwrap_or_default();
        let a = self.get_mut(loc).expect("indexed actor");
        for k in ["Bakeable", "PrimitiveType", "ScreenSetting", "ParentPrefab", "SRTHash"] {
            a.remove(k);
        }
        if let Some(Byml::Map(t)) = info.template.as_ref().and_then(|t| typedjson::from_json(t).ok()) {
            for (k, v) in t {
                if k != "Scale" || a.get("Scale").is_none() {
                    a.insert(&k, v);
                }
            }
        }
        a.insert("Gyaml", Byml::Str(info.gyaml.clone()));
        a.insert("Name", Byml::Str(name.into()));
        if let Some(Byml::Map(m)) = a.get_mut("Dynamic") {
            m.retain(|k, _| keep.contains(k.as_str()));
            if m.is_empty() {
                a.remove("Dynamic");
            }
        }
        Ok(())
    }

    /// Duplicate objects, whole paths, or points (a duplicated point is inserted after the
    /// original). Returns the new hashes.
    pub fn duplicate(&mut self, sel: &[u64], offset: Vec3) -> Vec<u64> {
        let hashes: Vec<u64> = sel.iter().copied().filter(|h| self.index.contains_key(h)).collect();
        if hashes.is_empty() {
            return vec![];
        }
        self.push_undo_as("Duplicate");
        let mut out = vec![];
        for hash in hashes {
            // look up again: inserted points shift the ones after them
            let Some(loc) = self.index.get(&hash).copied() else { continue };
            match loc {
                Loc::Actor(i) => out.push(self.duplicate_actor(i, offset)),
                Loc::Rail(r) => out.push(self.duplicate_rail(r, offset * 2.0)),
                Loc::Point(..) => out.extend(self.insert_point_raw(hash)),
            }
            self.reindex();
        }
        self.renumber_checkpoints();
        out
    }

    fn duplicate_actor(&mut self, i: usize, offset: Vec3) -> u64 {
        let mut a = self.actors()[i].clone();
        let h = self.new_hash();
        a.insert("Hash", Byml::U64(h));
        a.insert("InstanceId", Byml::Str(new_uuid()));
        a.remove("SRTHash");
        set_link_sources(&mut a, h);
        let t = Vec3::from(a.vec3("Translate", [0.0; 3])) + offset;
        a.insert("Translate", Byml::floats(&t.to_array()));
        self.actors_mut().push(a);
        h
    }

    /// A copy of a path with new hashes; links between its own points follow the copy.
    fn duplicate_rail(&mut self, r: usize, offset: Vec3) -> u64 {
        let mut rail = self.rails()[r].clone();
        let rh = self.new_hash();
        rail.insert("Hash", Byml::U64(rh));
        rail.insert("InstanceId", Byml::Str(new_uuid()));
        let new_hashes: Vec<u64> = (0..Self::points(&rail).len()).map(|_| self.new_hash()).collect();
        let mut remap = HashMap::new();
        for (p, &nh) in Self::points_mut(&mut rail).iter_mut().zip(&new_hashes) {
            remap.insert(hash_of(p), nh);
            p.insert("Hash", Byml::U64(nh));
            p.insert("InstanceId", Byml::Str(new_uuid()));
            p.insert("Parent", Byml::U64(rh));
            let t = point_pos(p) + offset;
            p.insert("Translation", Byml::floats(&t.to_array()));
            shift_controls(p, offset);
        }
        for p in Self::points_mut(&mut rail).iter_mut() {
            let ph = hash_of(p);
            for l in p.array_mut("Links").iter_mut() {
                l.insert("Src", Byml::U64(ph));
                if let Some(nd) = l.u64_of("Dst").and_then(|d| remap.get(&d)) {
                    l.insert("Dst", Byml::U64(*nd));
                }
            }
            if p.array("Links").is_empty() {
                p.remove("Links");
            }
        }
        for (old, new) in remap {
            self.copy_pad(old, new);
        }
        self.rails_mut().push(rail);
        rh
    }

    // ------------------------------------------------------------ deleting

    /// Drop links into `dead` (counting them), and point links into `retarget` keys at the
    /// mapped hash instead.
    fn remove_links_to(&mut self, dead: &HashSet<u64>, retarget: &HashMap<u64, u64>) -> usize {
        let mut lost = 0;
        let mut fix = |e: &mut Byml| {
            let Some(Byml::Array(ls)) = e.get_mut("Links") else { return };
            ls.retain_mut(|l| {
                let d = l.u64_of("Dst").unwrap_or(0);
                if let Some(n) = retarget.get(&d) {
                    l.insert("Dst", Byml::U64(*n));
                    true
                } else if dead.contains(&d) {
                    lost += 1;
                    false
                } else {
                    true
                }
            });
            if ls.is_empty() {
                e.remove("Links");
            }
        };
        self.actors_mut().iter_mut().for_each(&mut fix);
        for r in self.rails_mut().iter_mut() {
            Self::points_mut(r).iter_mut().for_each(&mut fix);
        }
        lost
    }

    /// Delete objects, points and paths. Outgoing links of a removed last point move to the
    /// new last point, and links into a removed first point move to the new first point, so
    /// the path graph stays connected. Returns a summary for the user.
    pub fn delete(&mut self, sel: &[u64]) -> String {
        let locs: Vec<Loc> = sel.iter().filter_map(|h| self.index.get(h).copied()).collect();
        if locs.is_empty() {
            return "Nothing selected".into();
        }
        self.push_undo_as("Delete");
        let mut dead: HashSet<u64> = HashSet::new();
        let mut retarget: HashMap<u64, u64> = HashMap::new();
        let mut rails_del: HashSet<usize> = locs.iter().filter_map(|l| if let Loc::Rail(r) = l { Some(*r) } else { None }).collect();
        let mut actors_del: Vec<usize> = locs.iter().filter_map(|l| if let Loc::Actor(i) = l { Some(*i) } else { None }).collect();
        let mut pts_by_rail: HashMap<usize, HashSet<usize>> = HashMap::new();
        for l in &locs {
            if let Loc::Point(r, p) = l
                && !rails_del.contains(r)
            {
                pts_by_rail.entry(*r).or_default().insert(*p);
            }
        }
        for (r, pts) in pts_by_rail {
            let all = Self::points(&self.rails()[r]).to_vec();
            dead.extend(pts.iter().map(|&i| hash_of(&all[i])));
            let mut remaining: Vec<Byml> = all.iter().enumerate().filter(|(i, _)| !pts.contains(i)).map(|(_, p)| p.clone()).collect();
            let Some(new_last) = remaining.last_mut() else {
                rails_del.insert(r);
                continue;
            };
            let last = all.len() - 1;
            if pts.contains(&last) {
                let nh = hash_of(new_last);
                let moved: Vec<Byml> = all[last]
                    .array("Links")
                    .iter()
                    .cloned()
                    .map(|mut l| {
                        l.insert("Src", Byml::U64(nh));
                        l
                    })
                    .collect();
                new_last.array_mut("Links").extend(moved);
                if new_last.array("Links").is_empty() {
                    new_last.remove("Links");
                }
            }
            if pts.contains(&0) {
                retarget.insert(hash_of(&all[0]), hash_of(&remaining[0]));
            }
            self.rails_mut()[r].insert("Points", Byml::Array(remaining));
        }
        let mut rails_del: Vec<usize> = rails_del.into_iter().collect();
        rails_del.sort_unstable_by(|a, b| b.cmp(a));
        for r in rails_del {
            let rail = self.rails_mut().remove(r);
            dead.insert(hash_of(&rail));
            dead.extend(Self::points(&rail).iter().map(hash_of));
        }
        actors_del.sort_unstable_by(|a, b| b.cmp(a));
        for i in actors_del {
            let a = self.actors_mut().remove(i);
            dead.insert(hash_of(&a));
        }
        self.deleted.extend(dead.iter().copied());
        let lost = self.remove_links_to(&dead, &retarget);
        self.reindex();
        self.renumber_checkpoints();
        let mut msg = format!("Deleted {} item(s)", sel.len());
        if lost > 0 {
            msg += &format!("; removed {lost} link(s) that pointed at them");
        }
        if !retarget.is_empty() {
            msg += "; links into removed first points now go to the new first point";
        }
        msg
    }

    // ------------------------------------------------------------ paths

    /// Insert a copy of a point after it: halfway to the next point (on the curve, splitting
    /// its Bézier handles so the path keeps its shape), or one step past the end.
    pub(crate) fn insert_point_raw(&mut self, hash: u64) -> Option<u64> {
        let Some(Loc::Point(r, i)) = self.index.get(&hash).copied() else { return None };
        let nh = self.new_hash();
        let rail = &self.rails()[r];
        let pts = Self::points(rail);
        let closed = rail.get("IsClosed").and_then(|b| b.as_bool()).unwrap_or(false);
        let a = point_pos(&pts[i]);
        let next_i = if i + 1 < pts.len() {
            Some(i + 1)
        } else if closed && pts.len() > 1 {
            Some(0)
        } else {
            None
        };
        // a curved segment is split with de Casteljau at t = 0.5
        let mut split: Option<CurveSplit> = None;
        let pos = match (next_i, i.checked_sub(1).and_then(|j| pts.get(j))) {
            (Some(j), _) => {
                let b = point_pos(&pts[j]);
                let (c1, c0) = (control(&pts[i], "Control1"), control(&pts[j], "Control0"));
                if c1.is_some() || c0.is_some() {
                    let (q1, q2) = (c1.unwrap_or(a), c0.unwrap_or(b));
                    let (m0, m1, m2) = (a.lerp(q1, 0.5), q1.lerp(q2, 0.5), q2.lerp(b, 0.5));
                    let (n0, n1) = (m0.lerp(m1, 0.5), m1.lerp(m2, 0.5));
                    split = Some(CurveSplit {
                        next: j,
                        had_out: c1.is_some(),
                        had_in: c0.is_some(),
                        out_half: m0,
                        in_half: m2,
                        new_in: n0,
                        new_out: n1,
                    });
                    n0.lerp(n1, 0.5)
                } else {
                    (a + b) / 2.0
                }
            }
            (None, Some(prev)) => a + (a - point_pos(prev)),
            (None, None) => a + quat_zyx(pts[i].vec3("Rotate", [0.0; 3])) * Vec3::Z * 10.0,
        };
        let last = i == pts.len() - 1;
        let mut np = pts[i].clone();
        np.insert("Hash", Byml::U64(nh));
        np.insert("InstanceId", Byml::Str(new_uuid()));
        np.remove("ParentPrefab");
        if let Some(d) = np.get_mut("Dynamic") {
            for k in UNIQUE_POINT_KEYS {
                d.remove(k);
            }
            if d.as_map().is_some_and(|m| m.is_empty()) {
                np.remove("Dynamic");
            }
        }
        np.insert("Translation", Byml::floats(&pos.to_array()));
        match (split, next_i) {
            (Some(CurveSplit { new_in: n0, new_out: n1, .. }), _) => {
                np.insert("Control0", Byml::floats(&n0.to_array()));
                np.insert("Control1", Byml::floats(&n1.to_array()));
            }
            // a straight segment stays straight
            (None, Some(_)) => {
                np.remove("Control0");
                np.remove("Control1");
            }
            (None, None) => shift_controls(&mut np, pos - a),
        }
        if let Some(CurveSplit { next: j, had_out, had_in, out_half: m0, in_half: m2, .. }) = split {
            let pts = Self::points_mut(&mut self.rails_mut()[r]);
            if had_out {
                pts[i].insert("Control1", Byml::floats(&m0.to_array()));
            }
            if had_in {
                pts[j].insert("Control0", Byml::floats(&m2.to_array()));
            }
        }
        let pts = Self::points_mut(&mut self.rails_mut()[r]);
        np.remove("Links");
        if last {
            // outgoing links stay on the path's last point
            if let Some(ls) = pts[i].remove("Links") {
                np.insert("Links", ls);
                set_link_sources(&mut np, nh);
            }
        }
        pts.insert(i + 1, np);
        self.copy_pad(hash, nh);
        self.reindex();
        self.renumber_checkpoints();
        Some(nh)
    }

    pub fn insert_point_after(&mut self, hash: u64) -> Option<u64> {
        if !matches!(self.index.get(&hash), Some(Loc::Point(..))) {
            return None;
        }
        self.push_undo_as("Insert Point");
        self.insert_point_raw(hash)
    }

    /// Checkpoint rectangle (`RectSize`, `RectOffset`) for a new point of path type `name`:
    /// copied from a path of that type in this course, else a default by type.
    pub(crate) fn rail_defaults(&self, name: &str) -> (Byml, Byml) {
        let existing = self.rails().iter().filter(|r| r.str_or("Name", "") == name).flat_map(Self::points).next();
        if let Some((s, o)) = existing.and_then(|p| p.get("RectSize").zip(p.get("RectOffset"))) {
            return (s.clone(), o.clone());
        }
        let (s, o) = match name {
            "LapPathGraph" => ([30.0, 20.0], [0.0, 10.0]),
            "RoutePathGraph" => ([1.5, 4.0], [0.0, 3.0]),
            "GlidePath" => ([30.0, 20.0], [0.0, 0.0]),
            "GuideGraphRail" => ([0.1, 1.0], [0.0, 0.0]),
            _ => ([0.5, 0.5], [0.0, 0.0]),
        };
        (Byml::floats(&s), Byml::floats(&o))
    }

    /// A new two-point path of type `name`, 20 m long, heading `yaw`. Returns its last point.
    /// See [`new_rail_shaped`](Self::new_rail_shaped) for other shapes.
    pub fn new_rail(&mut self, cat: &Catalog, name: &str, start: Vec3, yaw: f32) -> Option<u64> {
        self.new_rail_shaped(cat, name, start, yaw, crate::tools::RailShape::Line)
    }

    /// Link from an object or point to another (a path as the target means its first point).
    pub fn add_link(&mut self, src: u64, dst: u64, kind: &str) -> Result<()> {
        let sloc = *self.index.get(&src).ok_or("source not found")?;
        if matches!(sloc, Loc::Rail(_)) {
            return Err("links go from an object or a path point".into());
        }
        let dst = match self.find(dst) {
            Some((Loc::Rail(_), r)) => Self::points(r).first().map(hash_of).ok_or("empty path")?,
            Some(_) => dst,
            None => return Err("target not found".into()),
        };
        self.push_undo_as("Link");
        self.get_mut(sloc).expect("indexed source").array_mut("Links").push(link(src, dst, kind));
        self.renumber_checkpoints();
        Ok(())
    }

    pub fn remove_link(&mut self, owner: u64, index: usize) {
        let Some(loc) = self.index.get(&owner).copied() else { return };
        self.push_undo_as("Remove Link");
        let e = self.get_mut(loc).expect("indexed owner");
        if let Some(Byml::Array(ls)) = e.get_mut("Links") {
            if index < ls.len() {
                ls.remove(index);
            }
            if ls.is_empty() {
                e.remove("Links");
            }
        }
        self.renumber_checkpoints();
    }

    /// Reverse a path: point order, curve handles, and each point's heading. Links at its
    /// ends are left as they were.
    pub fn reverse_rail(&mut self, hash: u64) {
        let Some(Loc::Rail(r)) = self.index.get(&hash).copied() else { return };
        self.push_undo_as("Reverse Path");
        let pts = Self::points_mut(&mut self.rails_mut()[r]);
        pts.reverse();
        for p in pts.iter_mut() {
            let (c0, c1) = (p.remove("Control0"), p.remove("Control1"));
            if let Some(c) = c1 {
                p.insert("Control0", c);
            }
            if let Some(c) = c0 {
                p.insert("Control1", c);
            }
            let q = quat_zyx(p.vec3("Rotate", [0.0; 3])) * Quat::from_rotation_y(std::f32::consts::PI);
            p.insert("Rotate", Byml::floats(&euler_from_quat(q)));
        }
        self.reindex();
        self.renumber_checkpoints();
    }

    /// Drop objects and points (a selected path means all its points) onto the surface
    /// `ground` reports below them. Objects keep their `BancTransOffset` height. Returns
    /// (dropped, tried).
    pub fn drop_to_ground(&mut self, sel: &[u64], cat: &Catalog, ground: &dyn Fn(Vec3) -> Option<f32>) -> (usize, usize) {
        let mut targets: Vec<Loc> = vec![];
        for h in sel {
            match self.index.get(h) {
                Some(Loc::Rail(r)) => targets.extend((0..Self::points(&self.rails()[*r]).len()).map(|p| Loc::Point(*r, p))),
                Some(l) => targets.push(*l),
                None => {}
            }
        }
        if targets.is_empty() {
            return (0, 0);
        }
        self.push_undo_as("Drop to Ground");
        let mut n = 0;
        for &loc in &targets {
            let is_actor = matches!(loc, Loc::Actor(_));
            let key = if is_actor { "Translate" } else { "Translation" };
            let Some(e) = self.get(loc) else { continue };
            let t = Vec3::from(e.vec3(key, [0.0; 3]));
            let lift = if is_actor { cat.actor(e.str_or("Gyaml", "")).and_then(|a| a.banc_offset).map_or(0.0, |o| o[1]) } else { 0.0 };
            let Some(y) = ground(t) else { continue };
            let e = self.get_mut(loc).expect("indexed");
            e.insert(key, Byml::floats(&[t.x, y + lift, t.z]));
            if !is_actor {
                shift_controls(e, Vec3::Y * (y + lift - t.y));
            }
            n += 1;
        }
        (n, targets.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::banc::tests::course;

    fn doc() -> Document {
        Document::new(Loaded {
            file: "Banc/Test.bcett.byml.zs".into(),
            from_mod: false,
            version: 7,
            doc: course(),
            pad: None,
            pad_file: None,
        })
    }

    #[test]
    fn retargeting_names_the_replaced_course() {
        let mut d = doc();
        d.retarget("Banc/Race_DesertHills_A_00[Game03].bcett.byml.zs");
        assert_eq!(d.file, "Banc/Race_DesertHills_A_00[Game03].bcett.byml.zs");
        assert_eq!(d.pad_file.as_deref(), Some("Banc/PathAdditionalData/Race_DesertHills_A_00.byml"));
        assert_eq!(d.root.str_or("FilePath", ""), "Course/Race_DesertHills_A_00.bcett.json");
        assert!(d.pad.is_some() && d.dirty && !d.from_mod);
    }

    #[test]
    fn move_undo_redo() {
        let mut d = doc();
        let before = d.root.clone();
        let (p, r, s) = d.transform(1).unwrap();
        d.push_undo();
        d.set_transform(1, p + Vec3::X, r, s);
        assert_eq!(d.find(1).unwrap().1.vec3("Translate", [9.0; 3]), [1.0, 0.0, 0.0]);
        assert!(d.find(1).unwrap().1.get("Rotate").is_none() && d.find(1).unwrap().1.get("Scale").is_none(), "defaults stay absent");
        assert!(d.undo());
        assert_eq!(d.root, before);
        assert!(d.redo() && !d.redo());
        assert_eq!(d.find(1).unwrap().1.vec3("Translate", [9.0; 3]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn moving_a_point_moves_its_handles() {
        let mut d = doc();
        d.toggle_curve(12);
        let c0 = d.find(12).unwrap().1.vec3("Control0", [0.0; 3]);
        d.set_vector(12, "Translation", &[10.0, 5.0, 0.0]);
        assert_eq!(d.find(12).unwrap().1.vec3("Control0", [0.0; 3]), [c0[0], c0[1] + 5.0, c0[2]]);
    }

    #[test]
    fn duplicate_gives_new_hashes() {
        let mut d = doc();
        let new = d.duplicate(&[1, 10], Vec3::X);
        assert_eq!(new.len(), 2);
        assert_eq!(d.actors().len(), 4);
        assert_eq!(d.rails().len(), 2);
        let copy = &d.rails()[1];
        assert!(Document::points(copy).iter().all(|p| p.u64_of("Parent") == Some(new[1]) && ![11, 12, 13].contains(&hash_of(p))));
        assert_eq!(d.index.len(), 4 + 2 + 6, "4 objects, 2 paths and 6 points, each indexed once");
    }

    #[test]
    fn deleting_a_first_point_retargets_links() {
        let mut d = doc();
        d.add_link(1, 11, "BasicSignal").unwrap();
        let msg = d.delete(&[11]);
        assert!(msg.contains("new first point"), "{msg}");
        let links = d.find(1).unwrap().1.array("Links");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].u64_of("Dst"), Some(12));
        assert!(d.deleted.contains(&11));
        // deleting the target outright drops the link
        d.delete(&[10]);
        assert!(d.find(1).unwrap().1.get("Links").is_none());
    }

    #[test]
    fn deleting_a_last_point_keeps_outgoing_links() {
        let mut d = doc();
        d.add_link(13, 1, "BasicSignal").unwrap();
        d.delete(&[13]);
        let l = d.find(12).unwrap().1.array("Links");
        assert_eq!((l.len(), l[0].u64_of("Src"), l[0].u64_of("Dst")), (1, Some(12), Some(1)));
    }

    #[test]
    fn insert_reverse_and_drop() {
        let mut d = doc();
        let n = d.insert_point_after(11).unwrap();
        assert_eq!(Document::points(&d.rails()[0]).len(), 4);
        assert_eq!(d.transform(n).unwrap().0, Vec3::new(5.0, 0.0, 0.0));
        let end = d.insert_point_after(13).unwrap();
        assert_eq!(d.transform(end).unwrap().0, Vec3::new(30.0, 0.0, 0.0));
        d.reverse_rail(10);
        assert_eq!(hash_of(&Document::points(&d.rails()[0])[0]), end);
        assert!(d.add_link(10, 1, "BasicSignal").is_err(), "paths themselves don't link");
    }
}
