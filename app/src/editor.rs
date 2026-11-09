//! The `Editor` QObject that QML talks to. Thin wrappers over `state::State`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qvector3d.h");
        type QVector3D = cxx_qt_lib::QVector3D;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, ready)]
        #[qproperty(bool, busy)]
        #[qproperty(QString, status)]
        #[qproperty(QString, course_title)]
        #[qproperty(QString, mod_name, READ, WRITE = set_mod_name_from_qml, NOTIFY)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(QString, gizmo_mode)]
        #[qproperty(bool, gizmo_local)]
        #[qproperty(bool, link_picking)]
        #[qproperty(bool, ground_ready)]
        #[qproperty(i32, revision)]
        #[qproperty(i32, scene_revision)]
        #[qproperty(i32, rails_revision)]
        #[qproperty(i32, stages_revision)]
        #[qproperty(i32, overlay_revision)]
        #[qproperty(i32, models_loaded)]
        #[qproperty(i32, models_wanted)]
        #[qproperty(i32, thumbs_done)]
        #[qproperty(i32, thumbs_total)]
        #[qproperty(QString, console_text)]
        #[qproperty(QString, romfs_path)]
        #[qproperty(QString, mods_path)]
        #[qproperty(QString, ns2jb_path, cxx_name = "ns2jbPath")]
        #[qproperty(bool, ns2jb_on_save, cxx_name = "ns2jbOnSave")]
        #[qproperty(QString, undo_label)]
        #[qproperty(QString, redo_label)]
        #[qproperty(bool, has_clipboard)]
        #[qproperty(QString, pick_filter)]
        #[qproperty(i32, error_count)]
        #[qproperty(i32, warning_count)]
        #[qproperty(i32, selection_count)]
        type Editor = super::EditorRust;

        #[qsignal]
        fn transforms_moved(self: Pin<&mut Editor>, json: QString);
        #[qsignal]
        fn rails_moved(self: Pin<&mut Editor>, json: QString);

        fn set_mod_name_from_qml(self: Pin<&mut Editor>, name: QString);

        #[qinvokable]
        fn start(self: Pin<&mut Editor>);
        #[qinvokable]
        fn course_list(self: &Editor, filter: &QString) -> QString;
        #[qinvokable]
        fn mod_list(self: &Editor) -> QString;
        #[qinvokable]
        fn startup_course(self: &Editor) -> QString;
        #[qinvokable]
        fn open_course(self: Pin<&mut Editor>, file: &QString, mod_name: &QString) -> bool;
        #[qinvokable]
        fn save(self: Pin<&mut Editor>) -> QString;
        #[qinvokable]
        fn save_as(self: Pin<&mut Editor>, target: &QString) -> QString;
        #[qinvokable]
        fn save_as_default(self: &Editor) -> QString;
        #[qinvokable]
        fn new_course(self: Pin<&mut Editor>, location: &QString, copy: bool) -> bool;
        #[qinvokable]
        fn undo(self: Pin<&mut Editor>);
        #[qinvokable]
        fn redo(self: Pin<&mut Editor>);

        #[qinvokable]
        fn stages(self: &Editor) -> QString;
        #[qinvokable]
        fn actor_groups(self: &Editor) -> QString;
        #[qinvokable]
        fn boxes(self: &Editor) -> QString;
        #[qinvokable]
        fn part_count(self: &Editor, rel: &QString) -> i32;
        #[qinvokable]
        fn mesh_part(self: &Editor, rel: &QString, i: i32) -> QVariant;
        #[qinvokable]
        fn primitive(self: &Editor, shape: &QString) -> QVariant;
        #[qinvokable]
        fn rail_count(self: &Editor) -> i32;
        #[qinvokable]
        fn rail_mesh(self: &Editor, i: i32) -> QVariant;
        #[qinvokable]
        fn links_mesh(self: &Editor) -> QVariant;
        #[qinvokable]
        fn selection_mesh(self: &Editor) -> QVariant;
        #[qinvokable]
        fn gizmo(self: &Editor) -> QString;
        #[qinvokable]
        fn focus_target(self: &Editor, all: bool) -> QString;

        #[qinvokable]
        fn pick(self: Pin<&mut Editor>, origin: &QVector3D, dir: &QVector3D, additive: bool) -> bool;
        #[qinvokable]
        fn ground_at(self: &Editor, origin: &QVector3D, dir: &QVector3D) -> QString;
        #[qinvokable]
        fn begin_gizmo(self: Pin<&mut Editor>, axis: i32, origin: &QVector3D, dir: &QVector3D, aligned: bool) -> bool;
        #[qinvokable]
        fn drag_gizmo(self: Pin<&mut Editor>, origin: &QVector3D, dir: &QVector3D, invert_snap: bool);
        #[qinvokable]
        fn end_gizmo(self: Pin<&mut Editor>);

        #[qinvokable]
        fn outliner(self: &Editor, filter: &QString) -> QString;
        #[qinvokable]
        fn toggle_expand(self: Pin<&mut Editor>, key: &QString);
        #[qinvokable]
        fn set_group_visible(self: Pin<&mut Editor>, key: &QString, visible: bool);
        #[qinvokable]
        fn select_hash(self: Pin<&mut Editor>, hash: &QString, additive: bool);
        #[qinvokable]
        fn select_group(self: Pin<&mut Editor>, key: &QString);
        #[qinvokable]
        fn clear_selection(self: Pin<&mut Editor>);
        #[qinvokable]
        fn selection(self: &Editor) -> QString;

        #[qinvokable]
        fn set_vector(self: Pin<&mut Editor>, key: &QString, x: f64, y: f64, z: f64);
        #[qinvokable]
        fn set_param(self: Pin<&mut Editor>, name: &QString, kind: &QString, text: &QString) -> QString;
        #[qinvokable]
        fn remove_param(self: Pin<&mut Editor>, name: &QString);
        #[qinvokable]
        fn set_raw(self: Pin<&mut Editor>, text: &QString) -> QString;
        #[qinvokable]
        fn set_closed(self: Pin<&mut Editor>, closed: bool);
        #[qinvokable]
        fn toggle_curve(self: Pin<&mut Editor>);
        #[qinvokable]
        fn set_pad(self: Pin<&mut Editor>, group: &QString, key: &QString, kind: &QString, text: &QString) -> QString;
        #[qinvokable]
        fn remove_link(self: Pin<&mut Editor>, index: i32);
        #[qinvokable]
        fn start_link(self: Pin<&mut Editor>, kind: &QString);
        #[qinvokable]
        fn cancel_link(self: Pin<&mut Editor>);

        #[qinvokable]
        fn add_actor(self: Pin<&mut Editor>, name: &QString, origin: &QVector3D, dir: &QVector3D, yaw: f64) -> bool;
        #[qinvokable]
        fn duplicate(self: Pin<&mut Editor>, rx: f64, rz: f64);
        #[qinvokable]
        fn delete_selected(self: Pin<&mut Editor>);
        #[qinvokable]
        fn drop_to_ground(self: Pin<&mut Editor>);
        #[qinvokable]
        fn insert_point(self: Pin<&mut Editor>);
        #[qinvokable]
        fn new_rail(self: Pin<&mut Editor>, name: &QString, shape: &QString, origin: &QVector3D, dir: &QVector3D, yaw: f64) -> bool;
        #[qinvokable]
        fn reverse_rail(self: Pin<&mut Editor>);
        #[qinvokable]
        fn change_object(self: Pin<&mut Editor>, name: &QString) -> QString;

        #[qinvokable]
        fn assets(self: &Editor, filter: &QString, category: &QString, placed_only: bool) -> QString;
        #[qinvokable]
        fn categories(self: &Editor) -> QString;
        #[qinvokable]
        fn rail_types(self: &Editor) -> QString;
        #[qinvokable]
        fn view_state(self: &Editor) -> QString;
        #[qinvokable]
        fn set_view_option(self: Pin<&mut Editor>, name: &QString, on: bool);
        #[qinvokable]
        fn set_rail_type_visible(self: Pin<&mut Editor>, name: &QString, on: bool);
        #[qinvokable]
        fn set_category_visible(self: Pin<&mut Editor>, name: &QString, on: bool);
        #[qinvokable]
        fn set_gizmo(self: Pin<&mut Editor>, mode: &QString, local: bool);
        #[qinvokable]
        fn apply_settings(self: Pin<&mut Editor>, romfs: &QString, mods: &QString) -> QString;
        #[qinvokable]
        fn set_export(self: Pin<&mut Editor>, ns2jb: &QString, on_save: bool) -> QString;
        #[qinvokable]
        #[cxx_name = "exportNs2jb"]
        fn export_ns2jb(self: Pin<&mut Editor>) -> QString;
        #[qinvokable]
        fn open_folder(self: &Editor, which: &QString);
        #[qinvokable]
        fn build_thumbnails(self: Pin<&mut Editor>);
        #[qinvokable]
        fn inspect_model(self: &Editor, path: &QString) -> QString;
        #[qinvokable]
        fn import_model(self: Pin<&mut Editor>, path: &QString, name: &QString, label: &QString, scale: f64) -> QString;
    }

    // editing tools (copy and paste, path drawing, selection, modal transforms, problems)
    #[auto_cxx_name]
    extern "RustQt" {
        #[qinvokable]
        fn copy_selection(self: Pin<&mut Editor>, cut: bool) -> bool;
        #[qinvokable]
        fn paste(self: Pin<&mut Editor>, origin: &QVector3D, dir: &QVector3D, in_place: bool);
        #[qinvokable]
        fn add_point_at(self: Pin<&mut Editor>, origin: &QVector3D, dir: &QVector3D) -> bool;
        #[qinvokable]
        fn extrude(self: Pin<&mut Editor>) -> bool;
        #[qinvokable]
        fn connect_points(self: Pin<&mut Editor>);
        #[qinvokable]
        fn path_tool(self: Pin<&mut Editor>, tool: &QString);
        #[qinvokable]
        fn change_path_type(self: Pin<&mut Editor>, name: &QString) -> QString;
        #[qinvokable]
        fn place_along(self: Pin<&mut Editor>, name: &QString, spacing: f64) -> QString;
        #[qinvokable]
        fn set_pick_filter_from_qml(self: Pin<&mut Editor>, name: &QString);
        #[qinvokable]
        fn box_select(self: Pin<&mut Editor>, corners: &QString, mode: i32) -> i32;
        #[qinvokable]
        fn select_all(self: Pin<&mut Editor>);
        #[qinvokable]
        fn select_items(self: Pin<&mut Editor>, json: &QString);
        #[qinvokable]
        fn pick_for_menu(self: Pin<&mut Editor>, origin: &QVector3D, dir: &QVector3D) -> bool;
        #[qinvokable]
        fn hide_selected(self: Pin<&mut Editor>);
        #[qinvokable]
        fn unhide_all(self: Pin<&mut Editor>);
        #[qinvokable]
        fn set_snap(self: Pin<&mut Editor>, step: f64, angle: f64);
        #[qinvokable]
        fn begin_modal(self: Pin<&mut Editor>, mode: &QString, axis: i32, origin: &QVector3D, dir: &QVector3D) -> bool;
        #[qinvokable]
        fn restart_modal(self: Pin<&mut Editor>, axis: i32, origin: &QVector3D, dir: &QVector3D) -> bool;
        #[qinvokable]
        fn cancel_gizmo(self: Pin<&mut Editor>);
        #[qinvokable]
        fn move_selection_to(self: Pin<&mut Editor>, x: f64, y: f64, z: f64);
        #[qinvokable]
        fn problems(self: &Editor) -> QString;
        #[qinvokable]
        fn toggle_favorite(self: Pin<&mut Editor>, name: &QString) -> bool;
        #[qinvokable]
        fn recent_courses(self: &Editor) -> QString;
        #[qinvokable]
        fn pref(self: &Editor, key: &QString) -> QString;
        #[qinvokable]
        fn set_pref(self: Pin<&mut Editor>, key: &QString, json: &QString);
        #[qinvokable]
        fn run_test(self: Pin<&mut Editor>) -> QString;
    }

    impl cxx_qt::Threading for Editor {}
}

use crate::config::{Config, cache_dir};
use crate::state::{GizmoMode, State};
use crate::thumbs;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QVariant, QVector2D, QVector3D, QVector4D};
use glam::Vec3;
use mkwtrack::banc::hash_of;
use mkwtrack::catalog::Catalog;
use mkwtrack::doc::{Document, LINK_TYPES, Loc};
use mkwtrack::raycast::Ray;
use mkwtrack::scene::{self, Lines, Tris};
use mkwtrack::tools::RailShape;
use mkwtrack::{bfres, byml::Byml, typedjson};
use serde_json::json;
use std::sync::Arc;

pub struct EditorRust {
    ready: bool,
    busy: bool,
    status: QString,
    course_title: QString,
    mod_name: QString,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
    gizmo_mode: QString,
    gizmo_local: bool,
    link_picking: bool,
    ground_ready: bool,
    revision: i32,
    scene_revision: i32,
    rails_revision: i32,
    stages_revision: i32,
    overlay_revision: i32,
    models_loaded: i32,
    models_wanted: i32,
    thumbs_done: i32,
    thumbs_total: i32,
    console_text: QString,
    romfs_path: QString,
    mods_path: QString,
    ns2jb_path: QString,
    ns2jb_on_save: bool,
    undo_label: QString,
    redo_label: QString,
    has_clipboard: bool,
    pick_filter: QString,
    error_count: i32,
    warning_count: i32,
    selection_count: i32,
    st: State,
}

impl Default for EditorRust {
    fn default() -> Self {
        let cfg = Config::load();
        EditorRust {
            ready: false,
            busy: false,
            status: QString::from("Starting…"),
            course_title: QString::default(),
            mod_name: QString::from("MyTrackMod"),
            dirty: false,
            can_undo: false,
            can_redo: false,
            gizmo_mode: QString::from("translate"),
            gizmo_local: true,
            link_picking: false,
            ground_ready: false,
            revision: 0,
            scene_revision: 0,
            rails_revision: 0,
            stages_revision: 0,
            overlay_revision: 0,
            models_loaded: 0,
            models_wanted: 0,
            thumbs_done: 0,
            thumbs_total: 0,
            console_text: QString::default(),
            romfs_path: q(crate::config::tilde(&cfg.romfs.display().to_string())),
            mods_path: q(crate::config::tilde(&cfg.mods.display().to_string())),
            ns2jb_path: q(crate::config::tilde(&cfg.ns2jb.display().to_string())),
            ns2jb_on_save: cfg.ns2jb_on_save,
            undo_label: QString::default(),
            redo_label: QString::default(),
            has_clipboard: false,
            pick_filter: QString::default(),
            error_count: 0,
            warning_count: 0,
            selection_count: 0,
            st: State::new(cfg),
        }
    }
}

/// What QML has to redraw after a change: the panels (outliner, properties), the object
/// instances, the path meshes, and the overlay (gizmo, links, selection outline).
#[derive(Clone, Copy)]
struct Refresh {
    panels: bool,
    objects: bool,
    rails: bool,
    overlay: bool,
}

impl Refresh {
    const ALL: Refresh = Refresh { panels: true, objects: true, rails: true, overlay: true };
    /// selection changes recolour path markers and move the gizmo
    const SELECTION: Refresh = Refresh { panels: true, objects: false, rails: true, overlay: true };
    const GIZMO: Refresh = Refresh { panels: true, objects: false, rails: false, overlay: true };
    const PANELS: Refresh = Refresh { panels: true, objects: false, rails: false, overlay: false };
    const OBJECTS: Refresh = Refresh { panels: true, objects: true, rails: false, overlay: true };
}

fn q(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

fn ray(origin: &QVector3D, dir: &QVector3D) -> Ray {
    let v = |q: &QVector3D| Vec3::new(q.x(), q.y(), q.z());
    Ray { origin: v(origin), dir: v(dir).normalize_or(Vec3::NEG_Z) }
}

fn map(entries: Vec<(&str, QVariant)>) -> QVariant {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in entries {
        m.insert(q(k), v);
    }
    QVariant::from(&m)
}

fn vec3_list(v: &[[f32; 3]]) -> QVariant {
    let mut l = QList::<QVariant>::default();
    l.reserve(v.len() as isize);
    for p in v {
        l.append(QVariant::from(&QVector3D::new(p[0], p[1], p[2])));
    }
    QVariant::from(&l)
}

fn vec4_list(v: &[[f32; 4]]) -> QVariant {
    let mut l = QList::<QVariant>::default();
    l.reserve(v.len() as isize);
    for p in v {
        l.append(QVariant::from(&QVector4D::new(p[0], p[1], p[2], p[3])));
    }
    QVariant::from(&l)
}

fn lines_variant(l: &Lines) -> QVariant {
    map(vec![
        ("positions", vec3_list(&l.positions)),
        ("colors", vec4_list(&l.colors)),
        ("count", QVariant::from(&(l.positions.len() as i32))),
    ])
}

fn tris_variant(t: &Tris) -> QVariant {
    map(vec![
        ("positions", vec3_list(&t.positions)),
        ("colors", vec4_list(&t.colors)),
        ("count", QVariant::from(&(t.positions.len() as i32))),
    ])
}

impl qobject::Editor {
    fn st(&self) -> &State {
        &self.rust().st
    }

    fn stm(self: Pin<&mut Self>) -> &mut State {
        &mut self.rust_mut().get_mut().st
    }

    /// Bump the revision counters QML watches, then sync the flags.
    fn bump(mut self: Pin<&mut Self>, r: Refresh) {
        if r.panels {
            let v = *self.revision() + 1;
            self.as_mut().set_revision(v);
        }
        if r.objects {
            let v = *self.scene_revision() + 1;
            self.as_mut().set_scene_revision(v);
        }
        if r.rails {
            let v = *self.rails_revision() + 1;
            self.as_mut().set_rails_revision(v);
        }
        if r.overlay {
            let v = *self.overlay_revision() + 1;
            self.as_mut().set_overlay_revision(v);
        }
        if r.panels {
            let (mut errors, mut warnings) = (0, 0);
            if let Some(d) = &self.st().doc {
                for p in mkwtrack::check::check(&d.root, &d.file, self.st().cat.as_deref()) {
                    match p.severity {
                        mkwtrack::check::Severity::Error => errors += 1,
                        mkwtrack::check::Severity::Warning => warnings += 1,
                        mkwtrack::check::Severity::Info => {}
                    }
                }
            }
            self.as_mut().set_error_count(errors);
            self.as_mut().set_warning_count(warnings);
        }
        self.as_mut().sync_flags();
    }

    fn sync_flags(mut self: Pin<&mut Self>) {
        let (dirty, cu, cr, link) = {
            let s = self.st();
            match &s.doc {
                Some(d) => (d.dirty, d.can_undo(), d.can_redo(), s.link_pick.is_some()),
                None => (false, false, false, false),
            }
        };
        self.as_mut().set_dirty(dirty);
        self.as_mut().set_can_undo(cu);
        self.as_mut().set_can_redo(cr);
        self.as_mut().set_link_picking(link);
        let (ul, rl) = match &self.st().doc {
            Some(d) => (d.undo_label().unwrap_or("").to_string(), d.redo_label().unwrap_or("").to_string()),
            None => (String::new(), String::new()),
        };
        self.as_mut().set_undo_label(q(ul));
        self.as_mut().set_redo_label(q(rl));
        let clip = self.st().clipboard.as_ref().is_some_and(|c| !c.is_empty());
        self.as_mut().set_has_clipboard(clip);
        let n = self.st().selection.len() as i32;
        self.as_mut().set_selection_count(n);
        let console = self.st().console.iter().rev().take(200).rev().cloned().collect::<Vec<_>>().join("\n");
        self.as_mut().set_console_text(q(console));
        let last = self.st().console.back().cloned().unwrap_or_default();
        self.as_mut().set_status(q(last));
    }

    /// Structural change: everything that depends on the document is rebuilt.
    fn changed(mut self: Pin<&mut Self>) {
        self.as_mut().stm().rebuild();
        self.bump(Refresh::ALL);
    }

    fn log(mut self: Pin<&mut Self>, msg: impl Into<String>) {
        self.as_mut().stm().log(msg);
        self.sync_flags();
    }

    fn set_mod_name_from_qml(mut self: Pin<&mut Self>, name: QString) {
        let changed = self.st().mod_name != name.to_string();
        self.as_mut().stm().mod_name = name.to_string();
        self.as_mut().rust_mut().get_mut().mod_name = name;
        self.as_mut().mod_name_changed();
        if changed {
            // the mod's custom objects come and go with it
            let st = self.as_mut().stm();
            st.refresh_custom();
            st.rebuild();
            self.as_mut().bump(Refresh::ALL);
            self.load_models();
        }
    }

    // ------------------------------------------------------------ startup

    fn start(mut self: Pin<&mut Self>) {
        if let Err(e) = self.st().cfg.valid() {
            self.as_mut().log(e);
            self.as_mut().set_busy(false);
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_status(q("Reading the game catalog…"));
        let qt = self.qt_thread();
        let cfg = self.st().cfg.clone();
        let cache = self.st().catalog_cache();
        std::thread::spawn(move || {
            let qt2 = qt.clone();
            let progress = move |m: &str| {
                let m = m.to_string();
                let _ = qt2.queue(move |ed| ed.set_status(q(m)));
            };
            let t = std::time::Instant::now();
            let res = Catalog::load(&cfg.romfs, &cache, &progress);
            let took = t.elapsed();
            let _ = qt.queue(move |mut ed| {
                ed.as_mut().set_busy(false);
                match res {
                    Ok(cat) => {
                        let n = cat.data.actors.len();
                        ed.as_mut().stm().set_catalog(Some(Arc::new(cat)));
                        ed.as_mut().log(format!("Game catalog ready: {n} object types ({:.1} s)", took.as_secs_f32()));
                        ed.as_mut().set_ready(true);
                    }
                    Err(e) => ed.as_mut().log(format!("Could not read the game files: {e}")),
                }
            });
        });
    }

    fn startup_course(&self) -> QString {
        q(self.st().cfg.open.clone().unwrap_or_default())
    }

    fn course_list(&self, filter: &QString) -> QString {
        q(self.st().course_list(&filter.to_string()).to_string())
    }

    fn mod_list(&self) -> QString {
        q(json!(self.st().ws().mods()).to_string())
    }

    fn open_course(mut self: Pin<&mut Self>, file: &QString, mod_name: &QString) -> bool {
        let m = mod_name.to_string();
        let res = self.as_mut().stm().open(&file.to_string(), Some(&m));
        if let Err(e) = res {
            self.as_mut().log(format!("Could not open {file}: {e}"));
            return false;
        }
        let title = self.st().course_title();
        self.as_mut().set_course_title(q(title));
        let from = if self.st().doc.as_ref().is_some_and(|d| d.from_mod) { self.st().mod_name.clone() } else { String::new() };
        self.as_mut().stm().prefs.opened(&file.to_string(), &from);
        let mn = self.st().mod_name.clone();
        self.as_mut().rust_mut().get_mut().mod_name = q(&mn);
        self.as_mut().mod_name_changed();
        self.as_mut().set_ground_ready(false);
        if let Some(sel) = self.st().cfg.select.clone() {
            let d = self.st().doc.as_ref().unwrap();
            let hs: Vec<u64> = match sel.split_once('#') {
                // "LapPathGraph#3": point 3 of the first path with that name
                Some((rail, i)) => d
                    .rails()
                    .iter()
                    .find(|r| r.str_or("Name", "") == rail)
                    .and_then(|r| Document::points(r).get(i.parse::<usize>().unwrap_or(0)))
                    .map(hash_of)
                    .into_iter()
                    .collect(),
                None => d.actors().iter().filter(|a| a.str_or("Name", "") == sel).map(hash_of).take(1).collect(),
            };
            self.as_mut().stm().selection = hs;
            self.as_mut().stm().reveal_selection();
            self.as_mut().stm().rebuild();
        }
        self.as_mut().changed();
        let v = *self.stages_revision() + 1;
        self.as_mut().set_stages_revision(v);
        self.load_models();
        true
    }

    /// Read the course's models from the romfs on a worker thread, decoding their textures into
    /// the cache: stages first (each shown as it arrives, then the ground BVH), then object
    /// models.
    fn load_models(mut self: Pin<&mut Self>) {
        let wanted = self.st().wanted_models();
        let cache = self.st().models.clone();
        let roots = self.st().roots();
        let stages = self.st().stages.clone();
        let generation = self.st().load_gen;
        self.as_mut().set_models_wanted(wanted.len() as i32);
        let already = wanted.iter().filter(|(m, _)| cache.has(m)).count();
        self.as_mut().set_models_loaded(already as i32);
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let mut loaded = already;
            let mut stage_done = false;
            let finish_stages = |cache: &crate::state::ModelCache| Arc::new(scene::stage_bvh(&stages, |m| cache.get(m)));
            for (i, (rel, is_stage)) in wanted.iter().enumerate() {
                if !*is_stage && !stage_done {
                    stage_done = true;
                    let bvh = finish_stages(&cache);
                    let _ = qt.queue(move |mut ed| {
                        if ed.st().load_gen == generation {
                            let n = bvh.len();
                            ed.as_mut().stm().ground = Some(bvh);
                            ed.as_mut().set_ground_ready(true);
                            ed.as_mut().log(format!("Ready to edit (stage surface: {n} triangles)"));
                        }
                    });
                }
                if !cache.has(rel) {
                    let romfs = roots.iter().find(|r| r.join(rel).is_file()).unwrap_or(&roots[roots.len() - 1]);
                    match bfres::load(romfs, rel) {
                        Ok(m) => {
                            let mut names: Vec<String> = m.parts.iter().filter_map(|p| p.texture.clone()).collect();
                            names.sort();
                            names.dedup();
                            let errors = crate::textures::ensure_all(&roots, &names);
                            if !errors.is_empty() {
                                let msg = format!("Model {rel}: {} texture(s) not decoded, first: {}", errors.len(), errors[0]);
                                let _ = qt.queue(move |ed| ed.log(msg));
                            }
                            cache.map.lock().unwrap().insert(rel.clone(), Arc::new(m));
                        }
                        Err(e) => {
                            let msg = format!("Model {rel}: {e}");
                            let _ = qt.queue(move |ed| ed.log(msg));
                        }
                    }
                    loaded += 1;
                }
                let last = i + 1 == wanted.len();
                let is_stage = *is_stage;
                let n = loaded as i32;
                if is_stage || last || n % 12 == 0 {
                    let _ = qt.queue(move |mut ed| {
                        if ed.st().load_gen != generation {
                            return;
                        }
                        ed.as_mut().set_models_loaded(n);
                        if is_stage {
                            let v = *ed.stages_revision() + 1;
                            ed.as_mut().set_stages_revision(v);
                        } else {
                            ed.as_mut().stm().rebuild();
                            let v = *ed.scene_revision() + 1;
                            ed.as_mut().set_scene_revision(v);
                        }
                    });
                }
            }
            if !stage_done {
                let bvh = finish_stages(&cache);
                let _ = qt.queue(move |mut ed| {
                    if ed.st().load_gen == generation {
                        ed.as_mut().stm().ground = Some(bvh);
                        ed.as_mut().set_ground_ready(true);
                    }
                });
            }
        });
    }

    fn save(mut self: Pin<&mut Self>) -> QString {
        let v = self.as_mut().stm().save();
        self.as_mut().sync_flags();
        let v2 = *self.revision() + 1;
        self.as_mut().set_revision(v2);
        q(crate::config::tilde(&v.to_string()))
    }

    /// Save the open course as `target` (a course file), replacing that course in the mod.
    fn save_as(mut self: Pin<&mut Self>, target: &QString) -> QString {
        let target = target.to_string();
        let v = self.as_mut().stm().save_as(&target);
        // the course is now that file, saved or not
        let title = self.st().course_title();
        self.as_mut().set_course_title(q(title));
        if v["ok"] == true {
            let m = self.st().mod_name.clone();
            let st = self.as_mut().stm();
            if let Some(d) = st.doc.as_mut() {
                d.from_mod = true;
            }
            st.prefs.opened(&target, &m);
            let title = self.st().course_title();
            self.as_mut().set_course_title(q(title));
        }
        self.as_mut().sync_flags();
        let v2 = *self.revision() + 1;
        self.as_mut().set_revision(v2);
        q(crate::config::tilde(&v.to_string()))
    }

    fn save_as_default(&self) -> QString {
        q(self.st().save_as_default())
    }

    /// Start a new course at the place of the course `location`, empty or as a copy of it.
    fn new_course(mut self: Pin<&mut Self>, location: &QString, copy: bool) -> bool {
        if let Err(e) = self.as_mut().stm().new_course(&location.to_string(), copy) {
            self.as_mut().log(format!("Could not start a new course: {e}"));
            return false;
        }
        let title = self.st().course_title();
        self.as_mut().set_course_title(q(title));
        self.as_mut().set_ground_ready(false);
        self.as_mut().changed();
        let v = *self.stages_revision() + 1;
        self.as_mut().set_stages_revision(v);
        self.load_models();
        true
    }

    fn undo(mut self: Pin<&mut Self>) {
        self.as_mut().stm().cancel_drag();
        let ok = self.as_mut().stm().doc.as_mut().map(|d| d.undo()).unwrap_or(false);
        if ok {
            let st = self.as_mut().stm();
            st.handle = None;
            let doc = st.doc.as_ref().unwrap();
            st.selection.retain(|h| doc.index.contains_key(h));
            self.as_mut().log("Undone");
            self.changed();
        }
    }

    fn redo(mut self: Pin<&mut Self>) {
        self.as_mut().stm().cancel_drag();
        let ok = self.as_mut().stm().doc.as_mut().map(|d| d.redo()).unwrap_or(false);
        if ok {
            let st = self.as_mut().stm();
            st.handle = None;
            let doc = st.doc.as_ref().unwrap();
            st.selection.retain(|h| doc.index.contains_key(h));
            self.as_mut().log("Redone");
            self.changed();
        }
    }

    // ------------------------------------------------------------ scene data

    fn stages(&self) -> QString {
        q(self.st().stages_json().to_string())
    }

    fn actor_groups(&self) -> QString {
        q(self.st().actor_groups_json().to_string())
    }

    fn boxes(&self) -> QString {
        q(self.st().boxes_json().to_string())
    }

    fn part_count(&self, rel: &QString) -> i32 {
        self.st().models.get(&rel.to_string()).map(|m| m.parts.len() as i32).unwrap_or(0)
    }

    fn mesh_part(&self, rel: &QString, i: i32) -> QVariant {
        let rel = rel.to_string();
        let Some(m) = self.st().models.get(&rel) else { return QVariant::default() };
        let Some(p) = m.parts.get(i as usize) else { return QVariant::default() };
        let mut uvs = QList::<QVariant>::default();
        uvs.reserve(p.uvs.len() as isize);
        for v in &p.uvs {
            uvs.append(QVariant::from(&QVector2D::new(v[0], v[1])));
        }
        let mut idx = QList::<i32>::default();
        idx.reserve(p.indices.len() as isize);
        for &x in &p.indices {
            idx.append(x as i32);
        }
        let tex = p.texture.as_deref().and_then(crate::textures::cached);
        map(vec![
            ("positions", vec3_list(&p.positions)),
            ("normals", vec3_list(&p.normals)),
            ("uvs", QVariant::from(&uvs)),
            ("indexes", QVariant::from(&idx)),
            ("texture", QVariant::from(&q(tex.map(|t| crate::config::file_url(&t)).unwrap_or_default()))),
            ("cutout", QVariant::from(&p.cutout)),
            ("alphaCutoff", QVariant::from(&p.alpha_cutoff)),
        ])
    }

    fn primitive(&self, shape: &QString) -> QVariant {
        let (p, n) = scene::primitive(&shape.to_string());
        map(vec![("positions", vec3_list(&p)), ("normals", vec3_list(&n))])
    }

    fn rail_count(&self) -> i32 {
        self.st().scene.as_ref().map(|s| s.rails.len() as i32).unwrap_or(0)
    }

    fn rail_mesh(&self, i: i32) -> QVariant {
        let Some(r) = self.st().scene.as_ref().and_then(|s| s.rails.get(i as usize)) else { return QVariant::default() };
        map(vec![
            ("lines", lines_variant(&r.lines)),
            ("markers", tris_variant(&r.markers)),
            ("hash", QVariant::from(&q(r.hash.to_string()))),
            ("name", QVariant::from(&q(&r.name))),
        ])
    }

    fn links_mesh(&self) -> QVariant {
        match &self.st().doc {
            Some(d) => lines_variant(&scene::links(d)),
            None => lines_variant(&Lines::default()),
        }
    }

    fn selection_mesh(&self) -> QVariant {
        let s = self.st();
        match (&s.doc, &s.cat) {
            (Some(d), Some(c)) => lines_variant(&scene::selection_outline(d, c, &s.selection.iter().copied().collect())),
            _ => lines_variant(&Lines::default()),
        }
    }

    fn gizmo(&self) -> QString {
        q(self.st().gizmo_json().to_string())
    }

    fn focus_target(&self, all: bool) -> QString {
        q(self.st().focus_json(all).to_string())
    }

    // ------------------------------------------------------------ picking and gizmo

    fn pick(mut self: Pin<&mut Self>, origin: &QVector3D, dir: &QVector3D, additive: bool) -> bool {
        let was_link = self.st().link_pick.is_some();
        let hit = self.as_mut().stm().pick(&ray(origin, dir), additive);
        if hit.is_some() && !was_link {
            self.as_mut().stm().reveal_selection();
        }
        if was_link {
            self.as_mut().changed();
        } else {
            self.as_mut().bump(Refresh::SELECTION);
        }
        hit.is_some()
    }

    fn ground_at(&self, origin: &QVector3D, dir: &QVector3D) -> QString {
        match self.st().ground_hit(&ray(origin, dir)) {
            Some(p) => q(format!("{:.1}, {:.1}, {:.1}", p.x, p.y, p.z)),
            None => QString::default(),
        }
    }

    fn begin_gizmo(mut self: Pin<&mut Self>, axis: i32, origin: &QVector3D, dir: &QVector3D, aligned: bool) -> bool {
        let ok = self.as_mut().stm().begin_drag(None, axis.clamp(0, 3) as usize, &ray(origin, dir), aligned, true);
        self.sync_flags();
        ok
    }

    fn drag_gizmo(mut self: Pin<&mut Self>, origin: &QVector3D, dir: &QVector3D, invert_snap: bool) {
        let moved = self.as_mut().stm().drag_to(&ray(origin, dir), invert_snap);
        if moved.is_empty() {
            return;
        }
        let actors = self.st().transforms_json(&moved);
        let rails = self.st().rails_touched(&moved);
        if self.st().handle.is_some() {
            // the handle lines and markers are part of the path mesh
            let v = *self.rails_revision() + 1;
            self.as_mut().set_rails_revision(v);
        }
        self.as_mut().transforms_moved(q(actors.to_string()));
        self.as_mut().rails_moved(q(json!(rails).to_string()));
        let v = *self.overlay_revision() + 1;
        self.as_mut().set_overlay_revision(v);
        self.as_mut().set_dirty(true);
    }

    fn end_gizmo(mut self: Pin<&mut Self>) {
        self.as_mut().stm().end_drag();
        self.bump(Refresh::GIZMO);
    }

    // ------------------------------------------------------------ outliner and selection

    fn outliner(&self, filter: &QString) -> QString {
        q(self.st().outliner_json(&filter.to_string()).to_string())
    }

    fn toggle_expand(mut self: Pin<&mut Self>, key: &QString) {
        let k = key.to_string();
        let st = self.as_mut().stm();
        if !st.expanded.remove(&k) {
            st.expanded.insert(k);
        }
        self.bump(Refresh::PANELS);
    }

    fn set_group_visible(mut self: Pin<&mut Self>, key: &QString, visible: bool) {
        self.as_mut().stm().set_group_visible(&key.to_string(), visible);
        self.bump(Refresh::ALL);
    }

    fn select_hash(mut self: Pin<&mut Self>, hash: &QString, additive: bool) {
        let Ok(h) = hash.to_string().parse::<u64>() else { return };
        let st = self.as_mut().stm();
        if additive {
            if let Some(i) = st.selection.iter().position(|x| *x == h) {
                st.selection.remove(i);
            } else {
                st.selection.push(h);
            }
        } else {
            st.selection = vec![h];
        }
        st.rebuild();
        self.bump(Refresh::SELECTION);
    }

    fn select_group(mut self: Pin<&mut Self>, key: &QString) {
        let st = self.as_mut().stm();
        st.selection = st.group_hashes(&key.to_string());
        st.rebuild();
        self.bump(Refresh::SELECTION);
    }

    /// Esc: a picked curve handle goes back to its point; otherwise nothing stays selected.
    fn clear_selection(mut self: Pin<&mut Self>) {
        let st = self.as_mut().stm();
        if st.handle.take().is_none() {
            st.selection.clear();
        }
        st.rebuild();
        self.bump(Refresh::SELECTION);
    }

    fn selection(&self) -> QString {
        q(self.st().selection_json().to_string())
    }

    // ------------------------------------------------------------ property edits

    fn current(&self) -> Option<u64> {
        let s = self.st();
        (s.selection.len() == 1).then(|| s.selection[0])
    }

    fn set_vector(mut self: Pin<&mut Self>, key: &QString, x: f64, y: f64, z: f64) {
        let Some(h) = self.current() else { return };
        let k = key.to_string();
        let st = self.as_mut().stm();
        let Some(doc) = st.doc.as_mut() else { return };
        let is_point = matches!(doc.index.get(&h), Some(Loc::Point(..)));
        let key = match (k.as_str(), is_point) {
            ("Translate", true) => "Translation",
            (other, _) => other,
        };
        let v: Vec<f32> = match key {
            "Rotate" => vec![(x as f32).to_radians(), (y as f32).to_radians(), (z as f32).to_radians()],
            "RectSize" | "RectOffset" => vec![x as f32, y as f32],
            _ => vec![x as f32, y as f32, z as f32],
        };
        doc.set_vector(h, key, &v);
        self.changed();
    }

    /// Set a parameter on the selection (every selected item with the same parameter table).
    fn set_param(mut self: Pin<&mut Self>, name: &QString, kind: &QString, text: &QString) -> QString {
        let targets = self.st().param_targets();
        if targets.is_empty() {
            return q("select something first");
        }
        match typedjson::parse_text(&kind.to_string(), &text.to_string()) {
            Ok(v) => {
                let n = name.to_string();
                if let Some(d) = self.as_mut().stm().doc.as_mut() {
                    d.set_dynamic_many(&targets, &n, Some(v));
                }
                self.changed();
                QString::default()
            }
            Err(e) => q(e.to_string()),
        }
    }

    fn remove_param(mut self: Pin<&mut Self>, name: &QString) {
        let targets = self.st().param_targets();
        let n = name.to_string();
        if let Some(d) = self.as_mut().stm().doc.as_mut() {
            d.set_dynamic_many(&targets, &n, None);
        }
        self.changed();
    }

    fn set_raw(mut self: Pin<&mut Self>, text: &QString) -> QString {
        let Some(h) = self.current() else { return q("select one item") };
        let t = text.to_string();
        let st = self.as_mut().stm();
        let Some(doc) = st.doc.as_mut() else { return q("no course open") };
        let keep: &[&str] = match doc.index.get(&h) {
            Some(Loc::Actor(_)) => &State::ACTOR_KEEP,
            Some(Loc::Point(..)) => &State::POINT_KEEP,
            _ => &State::RAIL_KEEP,
        };
        match doc.replace_fields(h, &t, keep) {
            Ok(()) => {
                self.as_mut().log("Raw fields applied");
                self.changed();
                QString::default()
            }
            Err(e) => q(e.to_string()),
        }
    }

    fn set_closed(mut self: Pin<&mut Self>, closed: bool) {
        let Some(h) = self.current() else { return };
        if let Some(d) = self.as_mut().stm().doc.as_mut() {
            d.set_field(h, "IsClosed", Some(Byml::Bool(closed)));
        }
        self.changed();
    }

    fn toggle_curve(mut self: Pin<&mut Self>) {
        let Some(h) = self.current() else { return };
        if let Some(d) = self.as_mut().stm().doc.as_mut() {
            d.toggle_curve(h);
        }
        self.changed();
    }

    fn set_pad(mut self: Pin<&mut Self>, group: &QString, key: &QString, kind: &QString, text: &QString) -> QString {
        let Some(h) = self.current() else { return q("select one point") };
        let Ok(g) = group.to_string().parse::<u64>() else { return q("bad group") };
        match typedjson::parse_text(&kind.to_string(), &text.to_string()) {
            Ok(v) => {
                let k = key.to_string();
                if let Some(d) = self.as_mut().stm().doc.as_mut() {
                    d.set_pad_value(h, g, &k, v);
                }
                self.changed();
                QString::default()
            }
            Err(e) => q(e.to_string()),
        }
    }

    fn remove_link(mut self: Pin<&mut Self>, index: i32) {
        let Some(h) = self.current() else { return };
        if let Some(d) = self.as_mut().stm().doc.as_mut() {
            d.remove_link(h, index.max(0) as usize);
        }
        self.changed();
    }

    fn start_link(mut self: Pin<&mut Self>, kind: &QString) {
        let Some(h) = self.current() else {
            self.log("Select one object or point to link from");
            return;
        };
        let k = kind.to_string();
        let st = self.as_mut().stm();
        st.link_type = if LINK_TYPES.contains(&k.as_str()) { k } else { "BasicSignal".into() };
        st.link_pick = Some(h);
        let msg = format!("Click the {} target (Esc cancels). Clicking a path picks its first point.", st.link_type);
        self.log(msg);
    }

    fn cancel_link(mut self: Pin<&mut Self>) {
        self.as_mut().stm().link_pick = None;
        self.log("Link cancelled");
    }

    // ------------------------------------------------------------ tools

    fn add_actor(mut self: Pin<&mut Self>, name: &QString, origin: &QVector3D, dir: &QVector3D, yaw: f64) -> bool {
        let n = name.to_string();
        let st = self.as_mut().stm();
        let (Some(cat), true) = (st.cat.clone(), st.doc.is_some()) else { return false };
        let pos = st.placement_at(&ray(origin, dir), st.working_height());
        let h = st.doc.as_mut().unwrap().add_actor(&cat, &n, pos, yaw as f32);
        match h {
            Some(h) => {
                st.selection = vec![h];
                let extra = if cat.data.actors.get(&n).is_some_and(|a| a.template.is_none()) {
                    " (never placed in a shipped course, so no extra fields were copied)"
                } else {
                    ""
                };
                let need_model = cat.actor(&n).and_then(|i| i.model.clone()).is_some_and(|m| !st.models.has(&m));
                self.as_mut().log(format!("Added {n}{extra}"));
                self.as_mut().changed();
                if need_model {
                    self.load_models();
                }
                true
            }
            None => false,
        }
    }

    fn duplicate(mut self: Pin<&mut Self>, rx: f64, rz: f64) {
        let st = self.as_mut().stm();
        let sel = st.selection.clone();
        let Some(doc) = st.doc.as_mut() else { return };
        let out = doc.duplicate(&sel, Vec3::new(rx as f32, 0.0, rz as f32).normalize_or(Vec3::X) * 3.0);
        if !out.is_empty() {
            st.selection = out;
            let n = sel.len();
            self.as_mut().log(format!("Duplicated {n}"));
            self.changed();
        }
    }

    fn delete_selected(mut self: Pin<&mut Self>) {
        let st = self.as_mut().stm();
        let sel = std::mem::take(&mut st.selection);
        let Some(doc) = st.doc.as_mut() else { return };
        if sel.is_empty() {
            return;
        }
        let msg = doc.delete(&sel);
        self.as_mut().log(msg);
        self.changed();
    }

    fn drop_to_ground(mut self: Pin<&mut Self>) {
        let st = self.as_mut().stm();
        let (Some(cat), Some(ground)) = (st.cat.clone(), st.ground.clone()) else {
            self.log("The stage surface is still loading");
            return;
        };
        let sel = st.selection.clone();
        let Some(doc) = st.doc.as_mut() else { return };
        let (n, total) = doc.drop_to_ground(&sel, &cat, &|p| ground.ground_below(p));
        self.as_mut().log(format!("Dropped {n} of {total} onto the stage surface"));
        self.changed();
    }

    fn insert_point(mut self: Pin<&mut Self>) {
        let st = self.as_mut().stm();
        let Some(&h) = st.selection.last() else { return };
        let Some(doc) = st.doc.as_mut() else { return };
        match doc.insert_point_after(h) {
            Some(n) => {
                st.selection = vec![n];
                self.as_mut().log("Inserted a point");
                self.changed();
            }
            None => self.log("Select a path point first"),
        }
    }

    fn new_rail(mut self: Pin<&mut Self>, name: &QString, shape: &QString, origin: &QVector3D, dir: &QVector3D, yaw: f64) -> bool {
        let n = name.to_string();
        let shape = RailShape::from_name(&shape.to_string());
        let st = self.as_mut().stm();
        let Some(cat) = st.cat.clone() else { return false };
        let pos = st.placement_at(&ray(origin, dir), st.working_height());
        let Some(doc) = st.doc.as_mut() else { return false };
        match doc.new_rail_shaped(&cat, &n, pos, yaw as f32, shape) {
            Some(last) => {
                st.selection = vec![last];
                let how = match shape {
                    RailShape::Point => "1 point: Alt+Click where the path goes next (E extends it ahead)",
                    RailShape::Circle => "a closed loop of 4 points",
                    _ => "its last point selected: Alt+Click or E to extend it",
                };
                self.as_mut().log(format!("New {n}, {how}"));
                self.changed();
                true
            }
            None => false,
        }
    }

    fn reverse_rail(mut self: Pin<&mut Self>) {
        let st = self.as_mut().stm();
        let Some(&h) = st.selection.first() else { return };
        let Some(doc) = st.doc.as_mut() else { return };
        let Some(rail) = doc.rail_hash_of(h) else { return };
        doc.reverse_rail(rail);
        self.as_mut()
            .log("Path reversed. Links into its old first point and out of its old last point were left as they were; check them.");
        self.changed();
    }

    fn change_object(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let n = name.to_string();
        let st = self.as_mut().stm();
        let Some(cat) = st.cat.clone() else { return q("catalog not ready") };
        let sel = st.selection.clone();
        let Some(doc) = st.doc.as_mut() else { return q("no course open") };
        for h in &sel {
            if let Err(e) = doc.change_type(&cat, *h, &n) {
                return q(e.to_string());
            }
        }
        self.as_mut().log(format!("Changed {} object(s) to {n}", sel.len()));
        self.changed();
        QString::default()
    }

    // ------------------------------------------------------------ panels

    fn assets(&self, filter: &QString, category: &QString, placed_only: bool) -> QString {
        let s = self.st();
        q(s.asset_json(&filter.to_string(), &category.to_string(), placed_only, &s.prefs.favorites).to_string())
    }

    fn categories(&self) -> QString {
        q(self.st().categories_json().to_string())
    }

    fn rail_types(&self) -> QString {
        q(self.st().rail_types_json().to_string())
    }

    fn view_state(&self) -> QString {
        q(self.st().view_json().to_string())
    }

    fn set_view_option(mut self: Pin<&mut Self>, name: &QString, on: bool) {
        let st = self.as_mut().stm();
        match name.to_string().as_str() {
            "stage" => st.show_stage = on,
            "models" => st.view.models = on,
            "xray" => st.xray = on,
            "widths" => st.view.widths = on,
            "snap" => st.snap_ground = on,
            _ => {}
        }
        st.rebuild();
        let v = *self.stages_revision() + 1;
        self.as_mut().set_stages_revision(v);
        self.bump(Refresh::ALL);
    }

    fn set_rail_type_visible(mut self: Pin<&mut Self>, name: &QString, on: bool) {
        let st = self.as_mut().stm();
        let n = name.to_string();
        if on {
            st.view.hidden_rail_types.remove(&n);
        } else {
            st.view.hidden_rail_types.insert(n);
        }
        st.rebuild();
        self.bump(Refresh::SELECTION);
    }

    fn set_category_visible(mut self: Pin<&mut Self>, name: &QString, on: bool) {
        let st = self.as_mut().stm();
        let n = name.to_string();
        if on {
            st.view.hidden_categories.remove(&n);
        } else {
            st.view.hidden_categories.insert(n);
        }
        st.rebuild();
        self.bump(Refresh::OBJECTS);
    }

    fn set_gizmo(mut self: Pin<&mut Self>, mode: &QString, local: bool) {
        let m = mode.to_string();
        let st = self.as_mut().stm();
        st.gizmo_mode = match m.as_str() {
            "rotate" => GizmoMode::Rotate,
            "scale" => GizmoMode::Scale,
            _ => GizmoMode::Translate,
        };
        st.gizmo_local = local;
        self.as_mut().set_gizmo_mode(q(m));
        self.as_mut().set_gizmo_local(local);
        let v = *self.overlay_revision() + 1;
        self.as_mut().set_overlay_revision(v);
    }

    fn apply_settings(mut self: Pin<&mut Self>, romfs: &QString, mods: &QString) -> QString {
        let mut cfg = self.st().cfg.clone();
        cfg.romfs = crate::config::expand(&romfs.to_string());
        cfg.mods = crate::config::expand(&mods.to_string());
        if let Err(e) = cfg.valid().and_then(|_| cfg.save()) {
            return q(crate::config::tilde(&e));
        }
        self.as_mut().set_romfs_path(romfs.clone());
        self.as_mut().set_mods_path(mods.clone());
        let st = self.as_mut().stm();
        st.cfg = cfg;
        st.set_catalog(None);
        st.doc = None;
        st.scene = None;
        self.as_mut().set_ready(false);
        self.as_mut().set_course_title(QString::default());
        self.as_mut().bump(Refresh::ALL);
        self.start();
        QString::default()
    }

    /// The ns2jb-modloader export target (an SD card or a folder to copy onto one). Saved
    /// without reloading the game files.
    fn set_export(mut self: Pin<&mut Self>, ns2jb: &QString, on_save: bool) -> QString {
        let path = crate::config::expand(&ns2jb.to_string());
        if on_save && !path.is_dir() {
            return q(crate::config::tilde(&format!(
                "{} is not a folder: pick the Switch 2 SD card (or a folder to copy onto it)",
                path.display()
            )));
        }
        let st = self.as_mut().stm();
        st.cfg.ns2jb = path;
        st.cfg.ns2jb_on_save = on_save;
        if let Err(e) = st.cfg.save() {
            return q(crate::config::tilde(&format!("Settings not saved: {e}")));
        }
        self.as_mut().set_ns2jb_path(ns2jb.clone());
        self.as_mut().set_ns2jb_on_save(on_save);
        QString::default()
    }

    /// Export the saved mod for ns2jb-modloader now (File menu).
    fn export_ns2jb(mut self: Pin<&mut Self>) -> QString {
        let v = self.st().export_ns2jb();
        let msg = if v["ok"] == true {
            format!(
                "Exported {} file(s) for ns2jb-modloader to {}",
                v["files"].as_array().map(|a| a.len()).unwrap_or(0),
                v["dir"].as_str().unwrap_or("")
            )
        } else {
            format!(
                "ns2jb-modloader export failed: {}",
                v["errors"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).unwrap_or_default()
            )
        };
        self.as_mut().log(msg);
        q(crate::config::tilde(&v.to_string()))
    }

    fn open_folder(&self, which: &QString) {
        let s = self.st();
        let dir = match which.to_string().as_str() {
            "mods" => s.cfg.mods.join(&s.mod_name),
            "ns2jb" => mkwtrack::workspace::ns2jb_romfs(&s.cfg.ns2jb),
            "emulator" => crate::config::emulator_load_dir(),
            _ => cache_dir(),
        };
        std::fs::create_dir_all(&dir).ok();
        let _ = std::process::Command::new(if cfg!(windows) { "explorer" } else { "xdg-open" }).arg(&dir).spawn();
    }

    /// Render a thumbnail for every object model into the cache (in the background).
    fn build_thumbnails(mut self: Pin<&mut Self>) {
        let Some(cat) = self.st().cat.clone() else { return };
        if *self.thumbs_total() > 0 && *self.thumbs_done() < *self.thumbs_total() {
            return;
        }
        let jobs = thumbs::jobs(&cat);
        self.as_mut().set_thumbs_total(jobs.len() as i32);
        self.as_mut().set_thumbs_done(0);
        let qt = self.qt_thread();
        let roots = self.st().roots();
        std::thread::spawn(move || {
            let qt2 = qt.clone();
            thumbs::render_all(&roots, jobs, &move |done| {
                let _ = qt2.queue(move |mut ed| ed.as_mut().set_thumbs_done(done as i32));
            });
            let _ = qt.queue(|mut ed| {
                let v = *ed.revision() + 1;
                ed.as_mut().set_revision(v);
            });
        });
    }
}

/// Importing models as custom objects.
impl qobject::Editor {
    /// What a model file holds (parts, triangles, size, untextured materials), as JSON.
    fn inspect_model(&self, path: &QString) -> QString {
        q(self.st().inspect_model(&crate::config::expand(&path.to_string())).to_string())
    }

    /// Import a model file into the open mod as a new object type. Returns JSON: `ok`, and the
    /// new type (`actor`) or an `error`.
    fn import_model(mut self: Pin<&mut Self>, path: &QString, name: &QString, label: &QString, scale: f64) -> QString {
        let path = crate::config::expand(&path.to_string());
        self.as_mut().set_busy(true);
        let res = self.as_mut().stm().import_model(&path, &name.to_string(), &label.to_string(), scale as f32);
        self.as_mut().set_busy(false);
        let out = match res {
            Ok(r) => {
                let _ = std::fs::remove_file(thumbs::thumb_path(&r.object.model));
                let st = self.as_mut().stm();
                st.log(format!(
                    "Imported {} as {} into mod \"{}\": {} file(s) written{}",
                    r.object.source,
                    r.object.actor,
                    st.mod_name,
                    r.written.len(),
                    r.notes.iter().map(|n| format!("; {n}")).collect::<String>()
                ));
                json!({"ok": true, "actor": r.object.actor, "label": r.object.label})
            }
            Err(e) => {
                self.as_mut().log(format!("Import failed: {e}"));
                json!({"ok": false, "error": crate::config::tilde(&e.to_string())})
            }
        };
        self.as_mut().sync_flags();
        self.as_mut().bump(Refresh::ALL);
        self.as_mut().load_models();
        if out["ok"] == true {
            self.build_thumbnails();
        }
        q(out.to_string())
    }
}

/// Editing tools: copy and paste, drawing and reshaping paths, selection, modal transforms,
/// problems and preferences.
impl qobject::Editor {
    fn copy_selection(mut self: Pin<&mut Self>, cut: bool) -> bool {
        let st = self.as_mut().stm();
        let Some(doc) = st.doc.as_ref() else { return false };
        let clip = doc.copy(&st.selection);
        if clip.is_empty() {
            self.log("Select objects, paths or points to copy");
            return false;
        }
        let n = clip.len();
        st.clipboard = Some(clip);
        if cut {
            let sel = std::mem::take(&mut st.selection);
            if let Some(d) = st.doc.as_mut() {
                d.delete(&sel);
                d.relabel_undo("Cut");
            }
            self.as_mut().log(format!("Cut {n} item(s)"));
            self.changed();
        } else {
            self.as_mut().log(format!("Copied {n} item(s): Ctrl+V pastes them at the centre of the view, Ctrl+Shift+V where they were"));
        }
        true
    }

    fn paste(mut self: Pin<&mut Self>, origin: &QVector3D, dir: &QVector3D, in_place: bool) {
        let st = self.as_mut().stm();
        let Some(clip) = st.clipboard.clone() else { return };
        let offset = if in_place { Vec3::ZERO } else { st.placement_at(&ray(origin, dir), Some(clip.centre.y)) - clip.centre };
        let Some(doc) = st.doc.as_mut() else { return };
        let out = doc.paste(&clip, offset);
        if !out.is_empty() {
            st.selection = out;
            st.handle = None;
            self.as_mut().log(format!("Pasted {} item(s)", clip.len()));
            self.changed();
        }
    }

    /// Alt+Click with a path point selected: a new point there, joined to it.
    fn add_point_at(mut self: Pin<&mut Self>, origin: &QVector3D, dir: &QVector3D) -> bool {
        let st = self.as_mut().stm();
        let ([from], Some(doc)) = (st.selection.as_slice(), st.doc.as_ref()) else { return false };
        let from = *from;
        if !matches!(doc.index.get(&from), Some(Loc::Point(..))) {
            return false;
        }
        let y = doc.transform(from).map(|t| t.0.y);
        let pos = st.placement_at(&ray(origin, dir), y);
        let doc = st.doc.as_mut().unwrap();
        let branch = doc.find(from).is_some_and(|(loc, _)| match loc {
            Loc::Point(r, i) => {
                let n = Document::points(&doc.rails()[r]).len();
                i + 1 < n && i > 0
            }
            _ => false,
        });
        match doc.add_point_at(from, pos) {
            Some(n) => {
                st.selection = vec![n];
                if branch {
                    self.as_mut().log("Branched: a new path starts here, linked from the selected point");
                }
                self.changed();
                true
            }
            None => false,
        }
    }

    /// E: a new point one segment on from the selected end point (QML then moves it with the mouse).
    fn extrude(mut self: Pin<&mut Self>) -> bool {
        let st = self.as_mut().stm();
        let [from] = st.selection[..] else {
            self.log("Select one path point to extrude from");
            return false;
        };
        let Some(doc) = st.doc.as_mut() else { return false };
        let Some(pos) = doc.extrude_target(from) else {
            self.log("Select one path point to extrude from");
            return false;
        };
        match doc.add_point_at(from, pos) {
            Some(n) => {
                st.selection = vec![n];
                doc.relabel_undo("Extrude");
                self.changed();
                true
            }
            None => false,
        }
    }

    /// Connect two selected points with a link, or disconnect them.
    fn connect_points(mut self: Pin<&mut Self>) {
        let st = self.as_mut().stm();
        let [a, b] = st.selection[..] else {
            self.log("Select two path points (the first one links to the second)");
            return;
        };
        let Some(doc) = st.doc.as_mut() else { return };
        let msg = match doc.toggle_connection(a, b) {
            Ok(true) => "Connected: the first point now links to the second".to_string(),
            Ok(false) => "Disconnected the two points".to_string(),
            Err(e) => format!("Not connected: {e}"),
        };
        self.as_mut().log(msg);
        self.changed();
    }

    /// Path tools that work on the selection: split, join, orient, smooth, straighten,
    /// subdivide, reverse (for every selected path).
    fn path_tool(mut self: Pin<&mut Self>, tool: &QString) {
        let tool = tool.to_string();
        let st = self.as_mut().stm();
        let sel = st.selection.clone();
        let ground = st.ground.clone();
        let Some(doc) = st.doc.as_mut() else { return };
        let msg = match tool.as_str() {
            "split" => match sel[..] {
                [h] => doc.split_rail(h).unwrap_or_else(|e| format!("Not split: {e}")),
                _ => "Select the point to split the path after".into(),
            },
            "join" => match doc.rail_targets(&sel)[..] {
                [a, b] => {
                    let (ha, hb) = (hash_of(&doc.rails()[a]), hash_of(&doc.rails()[b]));
                    match doc.join_rails(ha, hb) {
                        Ok(m) => {
                            st.selection = vec![ha.min(hb)].into_iter().filter(|h| doc.index.contains_key(h)).collect();
                            if st.selection.is_empty() {
                                st.selection = vec![if doc.index.contains_key(&ha) { ha } else { hb }];
                            }
                            m
                        }
                        Err(e) => format!("Not joined: {e}"),
                    }
                }
                _ => "Select a point (or the path) of each of two paths to join".into(),
            },
            "orient" => {
                let n = doc.orient_points(&sel, &|p| ground.as_ref().and_then(|g| g.normal_below(p)));
                format!(
                    "Turned {n} point(s) to face along their path{}",
                    if ground.is_some() { ", upright on the stage surface" } else { "" }
                )
            }
            "smooth" => format!("Gave {} point(s) smooth curve handles", doc.smooth_handles(&sel)),
            "straighten" => format!("Removed the curve handles of {} point(s)", doc.clear_handles(&sel)),
            "subdivide" => {
                let new = doc.subdivide(&sel);
                let n = new.len();
                if n > 0 && sel.iter().all(|h| matches!(doc.index.get(h), Some(Loc::Point(..)))) {
                    st.selection.extend(new);
                }
                format!("Inserted {n} point(s)")
            }
            "reverse" => {
                let rails: Vec<u64> = doc.rail_targets(&sel).into_iter().map(|r| hash_of(&doc.rails()[r])).collect();
                for r in &rails {
                    doc.reverse_rail(*r);
                }
                format!(
                    "Reversed {} path(s). Links into their old first points and out of their old last points were left as they were; check them.",
                    rails.len()
                )
            }
            "renumber" => {
                if mkwtrack::checkpoints::misnumbered(doc.rails()).is_empty() {
                    "The checkpoints already count up along the lap".into()
                } else {
                    doc.push_undo_as("Renumber Checkpoints");
                    format!("Renumbered {} checkpoint(s) so they count up along the lap", doc.renumber_checkpoints())
                }
            }
            "checkpoint-on" | "checkpoint-off" => {
                let on = tool == "checkpoint-on";
                let lap: Vec<u64> = doc
                    .point_targets(&sel)
                    .into_iter()
                    .filter(|&(r, _)| doc.rails()[r].str_or("Name", "") == mkwtrack::checkpoints::LAP)
                    .map(|(r, i)| &Document::points(&doc.rails()[r])[i])
                    .filter(|p| mkwtrack::checkpoints::is_checkpoint(p) != on)
                    .map(hash_of)
                    .collect();
                if lap.is_empty() {
                    String::new()
                } else {
                    // the number is filled in by the renumbering every edit runs
                    doc.set_dynamic_many(&lap, mkwtrack::checkpoints::KEY, on.then_some(Byml::F32(0.0)));
                    doc.relabel_undo(if on { "Make Checkpoint" } else { "Remove Checkpoint" });
                    format!("{} {} checkpoint(s); the lap's checkpoints were renumbered", if on { "Added" } else { "Removed" }, lap.len())
                }
            }
            "select-path" => {
                let rails: Vec<u64> = doc.rail_targets(&sel).into_iter().map(|r| hash_of(&doc.rails()[r])).collect();
                st.selection = rails;
                st.handle = None;
                String::new()
            }
            "select-points" => {
                st.selection = doc.point_targets(&sel).into_iter().map(|(r, i)| hash_of(&Document::points(&doc.rails()[r])[i])).collect();
                st.handle = None;
                String::new()
            }
            other => format!("Unknown path tool {other}"),
        };
        if !msg.is_empty() {
            self.as_mut().log(msg);
        }
        self.changed();
    }

    fn change_path_type(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let n = name.to_string();
        let st = self.as_mut().stm();
        let Some(cat) = st.cat.clone() else { return q("catalog not ready") };
        let sel = st.selection.clone();
        let Some(doc) = st.doc.as_mut() else { return q("no course open") };
        match doc.change_rail_type(&cat, &sel, &n) {
            Ok(k) => {
                self.as_mut().log(format!("Changed {k} path(s) to {n}"));
                self.changed();
                QString::default()
            }
            Err(e) => q(e.to_string()),
        }
    }

    fn place_along(mut self: Pin<&mut Self>, name: &QString, spacing: f64) -> QString {
        let n = name.to_string();
        let st = self.as_mut().stm();
        let Some(cat) = st.cat.clone() else { return q("catalog not ready") };
        let Some(&h) = st.selection.first() else { return q("select a path or one of its points") };
        let ground = st.ground.clone();
        let Some(doc) = st.doc.as_mut() else { return q("no course open") };
        let out = doc.place_along(&cat, h, &n, spacing as f32, &|p| ground.as_ref().and_then(|g| g.ground_below(p)));
        if out.is_empty() {
            return q("nothing placed: select a path with at least two points");
        }
        let k = out.len();
        st.selection = out;
        self.as_mut().log(format!("Placed {k} × {n} along the path, {spacing} m apart"));
        self.changed();
        QString::default()
    }

    fn set_pick_filter_from_qml(mut self: Pin<&mut Self>, name: &QString) {
        let n = name.to_string();
        self.as_mut().stm().pick_filter = n.clone();
        self.as_mut().set_pick_filter(q(&n));
        let msg = match n.as_str() {
            "" => "Clicks pick anything".to_string(),
            "objects" => "Clicks pick objects only".to_string(),
            "paths" => "Clicks pick path points only".to_string(),
            t => format!("Clicks pick {t} points only; Alt+Click starts a new {t} or extends the selected one"),
        };
        self.log(msg);
    }

    /// Select inside a box on screen. `corners` is JSON: the rays through its four corners in
    /// order around it, each `[ox, oy, oz, dx, dy, dz]`.
    fn box_select(mut self: Pin<&mut Self>, corners: &QString, mode: i32) -> i32 {
        let v: Vec<[f32; 6]> = serde_json::from_str(&corners.to_string()).unwrap_or_default();
        let [a, b, c, d] = v[..] else { return 0 };
        let r = |x: [f32; 6]| Ray { origin: Vec3::new(x[0], x[1], x[2]), dir: Vec3::new(x[3], x[4], x[5]).normalize_or(Vec3::NEG_Z) };
        let corners = [r(a), r(b), r(c), r(d)];
        let n = self.as_mut().stm().box_select(&corners, mode);
        self.as_mut().bump(Refresh::SELECTION);
        n as i32
    }

    fn select_all(mut self: Pin<&mut Self>) {
        self.as_mut().stm().select_all();
        let n = self.st().selection.len();
        self.as_mut().log(format!("Selected {n} item(s)"));
        self.bump(Refresh::SELECTION);
    }

    fn select_items(mut self: Pin<&mut Self>, json: &QString) {
        let v: Vec<String> = serde_json::from_str(&json.to_string()).unwrap_or_default();
        let st = self.as_mut().stm();
        let Some(doc) = st.doc.as_ref() else { return };
        st.selection = v.iter().filter_map(|h| h.parse::<u64>().ok()).filter(|h| doc.index.contains_key(h)).collect();
        st.handle = None;
        st.reveal_selection();
        st.rebuild();
        self.bump(Refresh::SELECTION);
    }

    /// Right click: select what is under the mouse unless it is already selected.
    fn pick_for_menu(mut self: Pin<&mut Self>, origin: &QVector3D, dir: &QVector3D) -> bool {
        let r = ray(origin, dir);
        let st = self.as_mut().stm();
        let hit = st.hit_at(&r);
        if let Some(h) = hit
            && !st.selection.contains(&h)
        {
            st.selection = vec![h];
            st.handle = None;
            st.rebuild();
            self.as_mut().bump(Refresh::SELECTION);
        }
        hit.is_some()
    }

    fn hide_selected(mut self: Pin<&mut Self>) {
        let n = self.as_mut().stm().hide_selection();
        self.as_mut().log(format!("Hid {n} item(s) (Alt+H shows everything again)"));
        self.bump(Refresh::ALL);
    }

    fn unhide_all(mut self: Pin<&mut Self>) {
        self.as_mut().stm().unhide_all();
        self.as_mut().log("Everything is visible again");
        self.bump(Refresh::ALL);
    }

    fn set_snap(mut self: Pin<&mut Self>, step: f64, angle: f64) {
        let st = self.as_mut().stm();
        st.snap.step = step.max(0.0) as f32;
        st.snap.angle = angle.max(0.0) as f32;
        st.prefs.set("snap", json!({"step": step, "angle": angle}));
        self.bump(Refresh::PANELS);
    }

    /// G, R or T: move, rotate or scale the selection with the mouse until a click (Esc or a
    /// right click cancels). `axis` 0-2 locks to X, Y or Z; 3 is free.
    fn begin_modal(mut self: Pin<&mut Self>, mode: &QString, axis: i32, origin: &QVector3D, dir: &QVector3D) -> bool {
        let mode = match mode.to_string().as_str() {
            "rotate" => GizmoMode::Rotate,
            "scale" => GizmoMode::Scale,
            _ => GizmoMode::Translate,
        };
        let st = self.as_mut().stm();
        // world axes for modal moves (local ones when the gizmo is local and one item is picked)
        let ok = st.begin_drag(Some(mode), axis.clamp(0, 3) as usize, &ray(origin, dir), false, true);
        self.sync_flags();
        ok
    }

    fn restart_modal(mut self: Pin<&mut Self>, axis: i32, origin: &QVector3D, dir: &QVector3D) -> bool {
        let ok = self.as_mut().stm().restart_drag(axis.clamp(0, 3) as usize, &ray(origin, dir));
        self.as_mut().bump(Refresh::ALL);
        ok
    }

    fn cancel_gizmo(mut self: Pin<&mut Self>) {
        if !self.st().dragging() {
            return;
        }
        self.as_mut().stm().cancel_drag();
        self.as_mut().log("Cancelled");
        self.bump(Refresh::ALL);
    }

    /// Move the whole selection so its centre lands on (x, y, z).
    fn move_selection_to(mut self: Pin<&mut Self>, x: f64, y: f64, z: f64) {
        let st = self.as_mut().stm();
        let sel = st.selection.clone();
        let Some(doc) = st.doc.as_mut() else { return };
        let pos: Vec<Vec3> = doc.movable(&sel).into_iter().map(|t| t.1).collect();
        if pos.is_empty() {
            return;
        }
        let centre = pos.iter().sum::<Vec3>() / pos.len() as f32;
        doc.move_by(&sel, Vec3::new(x as f32, y as f32, z as f32) - centre);
        self.changed();
    }

    fn problems(&self) -> QString {
        q(self.st().problems_json().to_string())
    }

    fn toggle_favorite(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let on = self.as_mut().stm().prefs.toggle_favorite(&name.to_string());
        let v = *self.revision() + 1;
        self.as_mut().set_revision(v);
        on
    }

    fn recent_courses(&self) -> QString {
        let s = self.st();
        q(json!(s
            .prefs
            .recent
            .iter()
            .filter(|r| s.cfg.romfs.join(&r.file).exists())
            .map(|r| json!({"file": r.file, "mod": r.mod_name, "name": r.file.trim_start_matches("Banc/").trim_end_matches(".bcett.byml.zs")}))
            .collect::<Vec<_>>())
        .to_string())
    }

    fn pref(&self, key: &QString) -> QString {
        q(self.st().prefs.values.get(&key.to_string()).map(|v| v.to_string()).unwrap_or_else(|| "null".into()))
    }

    fn set_pref(mut self: Pin<&mut Self>, key: &QString, json: &QString) {
        if let Ok(v) = serde_json::from_str(&json.to_string()) {
            self.as_mut().stm().prefs.set(&key.to_string(), v);
        }
    }

    /// Save, then run the test command from Settings (an emulator with the game, say).
    fn run_test(mut self: Pin<&mut Self>) -> QString {
        let res = self.as_mut().save();
        let v: serde_json::Value = serde_json::from_str(&res.to_string()).unwrap_or_default();
        if v["ok"] != true {
            return res;
        }
        let cmd = self.st().prefs.values.get("testCommand").and_then(|c| c.as_str()).unwrap_or("").trim().to_string();
        if cmd.is_empty() {
            self.log("Saved. Set a test command in Settings → Testing to start the game from here (F5)");
        } else {
            let (shell, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };
            match std::process::Command::new(shell).arg(flag).arg(&cmd).spawn() {
                Ok(_) => self.log(format!("Saved, and started: {cmd}")),
                Err(e) => self.log(format!("Saved, but the test command did not start: {e}")),
            }
        }
        res
    }
}
