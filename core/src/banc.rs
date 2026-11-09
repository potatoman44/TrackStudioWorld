//! Banc course files (`Banc/*.bcett.byml.zs`): names, the summaries the game reads, and the
//! checks run before saving.
//!
//! A course file is a BYML map with `Actors` (objects), `Rails` (paths, each with `Points`) and
//! summaries of them. The game reads the summaries to decide which systems a course needs
//! before it spawns anything (version 1.8.0):
//!
//! * `CountActor[name]` gates water (`SystemObj_WaterBubbleArea_00`, `Whirlpool`...), respawn
//!   generation (`SystemObj_AutoGenerate*SlatePoint_00`), coin bars, landmarks and camera areas,
//!   sound environments and more (44 references to the key, including an accessor at 0x2a0f70
//!   with 22 callers);
//! * `CountRail[name]` gates water cascades, citizens and rail slides (0x2a0ed0, 0x2a1140);
//! * `Collects.MobVehicle` and `Collects.Block` gate traffic and blocks (0x7a5a40, 0x79a0e0);
//! * `AABB` is read while the course loads (0x2a4cc0).
//!
//! So [`recompute`] must run before every save: an object type added without its count can
//! leave the system it needs switched off.

use crate::byml::Byml;
use crate::catalog::Catalog;
use crate::error::Result;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Mario Kart World's title ID (emulator mod folders and ns2jb-modloader use it).
pub const TITLE_ID: &str = "0400C3F00006E000";

/// `Banc/Race_FirstCircuit_A_00[Game03].bcett.byml.zs` -> `Race_FirstCircuit_A_00`
pub fn course_base(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let name = name.strip_suffix(".zs").unwrap_or(name);
    let name = name.strip_suffix(".bcett.byml").unwrap_or(name);
    match (name.rfind('['), name.ends_with(']')) {
        (Some(i), true) => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// The `FilePath` a course file names itself by: `Course/<course>.bcett.json` (data: every
/// shipped course file, e.g. `Course/Race_FirstCircuit_A_00.bcett.json`).
pub fn file_path_for(rel: &str) -> String {
    format!("Course/{}.bcett.json", course_base(rel))
}

/// A course with no objects or paths, at the place of `location` (a course file's root): its
/// `AABB` and `FilePath` are kept, so the stage around it is drawn, and the summaries are
/// emptied.
pub fn empty_course(location: &Byml) -> Byml {
    let mut doc = Byml::map();
    for key in ["AABB", "FilePath"] {
        if let Some(v) = location.get(key) {
            doc.insert(key, v.clone());
        }
    }
    doc.insert("Actors", Byml::Array(vec![]));
    doc.insert("Rails", Byml::Array(vec![]));
    for key in ["CountActor", "CountRail", "Collects"] {
        doc.insert(key, Byml::map());
    }
    doc
}

/// The PathAdditionalData file that goes with a course file.
pub fn pad_rel(rel: &str) -> String {
    format!("Banc/PathAdditionalData/{}.byml", course_base(rel))
}

/// A romfs-relative path inside `Banc/`, with forward slashes; anything else is refused.
pub fn safe_rel(rel: &str) -> Result<String> {
    let r = rel.replace('\\', "/").trim_start_matches('/').to_string();
    if r.split('/').any(|p| p == "..") || !r.starts_with("Banc/") {
        return Err(format!("not a Banc path: {rel}").into());
    }
    Ok(r)
}

/// The `Hash` of an actor, rail or point (0 when missing).
pub fn hash_of(v: &Byml) -> u64 {
    v.u64_of("Hash").unwrap_or(0)
}

/// Placeholder `SRTHash` for new objects. The executable never reads this key (neither the
/// string nor a hash of it appears in any module), so any value works; this one is a CRC32
/// of the transform, so equal transforms get equal values.
pub fn srt_hash(a: &Byml) -> u32 {
    let mut b = Vec::with_capacity(36);
    for v in [a.vec3("Scale", [1.0; 3]), a.vec3("Rotate", [0.0; 3]), a.vec3("Translate", [0.0; 3])] {
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
    }
    crc32fast::hash(&b)
}

fn count_by_name(items: &[Byml]) -> Byml {
    let mut m: BTreeMap<String, Byml> = BTreeMap::new();
    for it in items {
        if let Byml::I32(n) = m.entry(it.str_or("Name", "").to_string()).or_insert(Byml::I32(0)) {
            *n += 1;
        }
    }
    Byml::Map(m)
}

/// Rebuild `CountActor`, `CountRail` and `Collects`, grow `AABB` over new or moved actors
/// (`original` is the shipped file, to tell which actors moved) and renumber the lap
/// checkpoints. Returns notes for the save report. On an unedited shipped file this changes
/// nothing.
pub fn recompute(doc: &mut Byml, original: Option<&Byml>, cat: Option<&Catalog>) -> Vec<String> {
    let mut notes = vec![];
    let actors = doc.array("Actors").to_vec();
    let rails = doc.array("Rails").to_vec();
    if doc.get("CountActor").is_some() || !actors.is_empty() {
        doc.insert("CountActor", count_by_name(&actors));
    }
    if doc.get("CountRail").is_some() || !rails.is_empty() {
        doc.insert("CountRail", count_by_name(&rails));
    }
    if doc.get("Collects").is_some() || !actors.is_empty() {
        let mut counts: BTreeMap<String, Byml> = BTreeMap::new();
        let mut guessed = BTreeSet::new();
        for a in &actors {
            let name = a.str_or("Name", "");
            let (class, verified) = cat.map(|c| c.collect_class(name)).unwrap_or((None, true));
            let Some(class) = class else { continue };
            if let Byml::I32(n) = counts.entry(class.clone()).or_insert(Byml::I32(0)) {
                *n += 1;
            }
            if !verified {
                guessed.insert((name.to_string(), class));
            }
        }
        doc.insert("Collects", Byml::Map(counts));
        for (n, c) in guessed {
            notes.push(format!("{n} counted as Collects.{c} by name pattern (not seen in shipped files)"));
        }
    }
    if let Some(note) = grow_aabb(doc, original, &actors) {
        notes.push(note);
    }
    match crate::checkpoints::renumber(doc) {
        0 => {}
        n => notes.push(format!("Renumbered {n} lap checkpoint(s) so they count up along the lap")),
    }
    notes
}

/// Grow `AABB` to cover actors that are new or moved. Shipped AABBs don't always contain every
/// actor, so unchanged actors never grow it, and it never shrinks.
fn grow_aabb(doc: &mut Byml, original: Option<&Byml>, actors: &[Byml]) -> Option<String> {
    let aabb = doc.get("AABB")?;
    let before: BTreeMap<u64, [f32; 3]> =
        original.map(|o| o.array("Actors").iter().map(|a| (hash_of(a), a.vec3("Translate", [0.0; 3]))).collect()).unwrap_or_default();
    let mut mn = aabb.vec3("Min", [f32::MAX; 3]);
    let mut mx = aabb.vec3("Max", [f32::MIN; 3]);
    let mut grew = false;
    for a in actors {
        let t = a.vec3("Translate", [0.0; 3]);
        if before.get(&hash_of(a)) == Some(&t) {
            continue;
        }
        for i in 0..3 {
            if t[i] - 1.0 < mn[i] {
                mn[i] = t[i] - 1.0;
                grew = true;
            }
            if t[i] + 1.0 > mx[i] {
                mx[i] = t[i] + 1.0;
                grew = true;
            }
        }
    }
    if !grew {
        return None;
    }
    let a = doc.get_mut("AABB")?;
    a.insert("Min", Byml::floats(&mn));
    a.insert("Max", Byml::floats(&mx));
    Some("AABB grown to cover moved or added objects".into())
}

/// Problems found before saving: errors (which block the save) and warnings.
#[derive(Default, Debug)]
pub struct Validation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Every actor, rail and point needs a unique `Hash`; actors need a `Gyaml`; rails need points
/// whose `Parent` is the rail. Links to hashes that aren't in the file are only warnings: some
/// shipped files have them.
pub fn validate(doc: &Byml) -> Validation {
    let mut v = Validation::default();
    let mut seen: BTreeMap<u64, String> = BTreeMap::new();
    let mut claim = |h: Option<u64>, what: String, errors: &mut Vec<String>| match h {
        None => errors.push(format!("{what} has no Hash")),
        Some(h) => {
            if let Some(prev) = seen.get(&h) {
                errors.push(format!("duplicate Hash {h}: {what} and {prev}"));
            }
            seen.insert(h, what);
        }
    };
    for (i, a) in doc.array("Actors").iter().enumerate() {
        claim(a.u64_of("Hash"), format!("object #{i} {}", a.str_or("Name", "")), &mut v.errors);
        if a.get("Gyaml").is_none() {
            v.errors.push(format!("object #{i} has no Gyaml"));
        }
    }
    for (i, r) in doc.array("Rails").iter().enumerate() {
        claim(r.u64_of("Hash"), format!("path #{i} {}", r.str_or("Name", "")), &mut v.errors);
        let pts = r.array("Points");
        if pts.is_empty() {
            v.errors.push(format!("path #{i} {} has no points", r.str_or("Name", "")));
        }
        for (j, p) in pts.iter().enumerate() {
            claim(p.u64_of("Hash"), format!("path #{i} point {j}"), &mut v.errors);
            if p.u64_of("Parent") != r.u64_of("Hash") {
                v.warnings.push(format!("path #{i} point {j}: Parent is not the path's Hash"));
            }
        }
    }
    let known: HashSet<u64> = seen.keys().copied().collect();
    let mut check = |links: &[Byml], who: String| {
        for l in links {
            let d = l.u64_of("Dst").unwrap_or(0);
            if !known.contains(&d) {
                v.warnings.push(format!("{who} links to {d}, which is not in this file"));
            }
        }
    };
    for (i, a) in doc.array("Actors").iter().enumerate() {
        check(a.array("Links"), format!("object #{i} {}", a.str_or("Name", "")));
    }
    for (i, r) in doc.array("Rails").iter().enumerate() {
        for (j, p) in r.array("Points").iter().enumerate() {
            check(p.array("Links"), format!("path #{i} point {j}"));
        }
    }
    v
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn actor(name: &str, hash: u64, t: [f32; 3]) -> Byml {
        let mut a = Byml::map();
        a.insert("Name", Byml::Str(name.into()));
        a.insert("Gyaml", Byml::Str(format!("Work/Gyml/ActorGParam/{name}.gear__ActorGParam.gyml")));
        a.insert("Hash", Byml::U64(hash));
        a.insert("Translate", Byml::floats(&t));
        a
    }

    pub fn rail(name: &str, hash: u64, points: &[(u64, [f32; 3])]) -> Byml {
        let mut r = Byml::map();
        r.insert("Name", Byml::Str(name.into()));
        r.insert("Gyaml", Byml::Str(format!("Work/Gyml/GraphRail/{name}.gyml")));
        r.insert("Hash", Byml::U64(hash));
        r.insert("IsClosed", Byml::Bool(false));
        let pts = points
            .iter()
            .map(|(h, t)| {
                let mut p = Byml::map();
                p.insert("Hash", Byml::U64(*h));
                p.insert("Parent", Byml::U64(hash));
                p.insert("Translation", Byml::floats(t));
                p
            })
            .collect();
        r.insert("Points", Byml::Array(pts));
        r
    }

    pub fn course() -> Byml {
        let mut d = Byml::map();
        d.insert(
            "Actors",
            Byml::Array(vec![
                actor("CmnObj_Coin000_A_00", 1, [0.0; 3]),
                actor("CmnObj_Coin000_A_00", 2, [5.0, 0.0, 0.0]),
                actor("StartPosition", 3, [0.0; 3]),
            ]),
        );
        d.insert("Rails", Byml::Array(vec![rail("RoutePathGraph", 10, &[(11, [0.0; 3]), (12, [10.0, 0.0, 0.0]), (13, [20.0, 0.0, 0.0])])]));
        let mut aabb = Byml::map();
        aabb.insert("Min", Byml::floats(&[-1.0, -1.0, -1.0]));
        aabb.insert("Max", Byml::floats(&[6.0, 1.0, 1.0]));
        d.insert("AABB", aabb);
        d
    }

    #[test]
    fn empty_courses_keep_their_place() {
        let mut base = course();
        base.insert("FilePath", Byml::Str("Course/Test.bcett.json".into()));
        let e = empty_course(&base);
        assert!(e.array("Actors").is_empty() && e.array("Rails").is_empty());
        assert_eq!(e.get("AABB"), base.get("AABB"));
        assert_eq!(e.get("FilePath"), base.get("FilePath"));
        assert_eq!(file_path_for("Banc/Race_FirstCircuit_A_00[Game03].bcett.byml.zs"), "Course/Race_FirstCircuit_A_00.bcett.json");
        // an empty course saves: nothing to fail validation
        assert!(validate(&e).errors.is_empty());
    }

    #[test]
    fn names() {
        let f = "Banc/Race_FirstCircuit_A_00[Game03].bcett.byml.zs";
        assert_eq!(course_base(f), "Race_FirstCircuit_A_00");
        assert_eq!(course_base("Banc/World_Game03/0309.bcett.byml.zs"), "0309");
        assert_eq!(pad_rel(f), "Banc/PathAdditionalData/Race_FirstCircuit_A_00.byml");
        assert_eq!(safe_rel("\\Banc\\x.bcett.byml.zs").unwrap(), "Banc/x.bcett.byml.zs");
        assert!(safe_rel("Banc/../etc/passwd").is_err() && safe_rel("System/x").is_err());
    }

    #[test]
    fn recompute_counts_and_grows_aabb() {
        let original = course();
        let mut d = original.clone();
        let notes = recompute(&mut d, Some(&original), None);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(d.get("CountActor").unwrap().get("CmnObj_Coin000_A_00"), Some(&Byml::I32(2)));
        assert_eq!(d.get("CountRail").unwrap().get("RoutePathGraph"), Some(&Byml::I32(1)));
        assert_eq!(d.get("AABB"), original.get("AABB"), "unmoved actors never grow the AABB");
        d.array_mut("Actors").push(actor("CmnObj_Coin000_A_00", 4, [50.0, 0.0, 0.0]));
        let notes = recompute(&mut d, Some(&original), None);
        assert_eq!(notes.len(), 1);
        assert_eq!(d.get("AABB").unwrap().vec3("Max", [0.0; 3]), [51.0, 1.0, 1.0]);
        assert_eq!(d.get("CountActor").unwrap().get("CmnObj_Coin000_A_00"), Some(&Byml::I32(3)));
    }

    #[test]
    fn validate_finds_duplicates_and_dangling_links() {
        let d = course();
        assert!(validate(&d).errors.is_empty());
        let mut bad = d.clone();
        bad.array_mut("Actors").push(actor("X", 12, [0.0; 3]));
        let mut link = Byml::map();
        link.insert("Dst", Byml::U64(999));
        bad.array_mut("Actors")[0].array_mut("Links").push(link);
        let v = validate(&bad);
        assert!(v.errors.iter().any(|e| e.contains("duplicate Hash 12")), "{v:?}");
        assert!(v.warnings.iter().any(|w| w.contains("999")), "{v:?}");
    }

    #[test]
    fn srt_hash_depends_on_transform() {
        let a = actor("A", 1, [1.0, 2.0, 3.0]);
        assert_eq!(srt_hash(&a), srt_hash(&actor("B", 2, [1.0, 2.0, 3.0])));
        assert_ne!(srt_hash(&a), srt_hash(&actor("A", 1, [1.0, 2.0, 3.5])));
    }
}
