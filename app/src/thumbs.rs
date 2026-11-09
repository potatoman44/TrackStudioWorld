//! Asset-browser thumbnails: a small CPU rasterizer (textured, lit, alpha-tested,
//! 2x supersampled) run over every object model on worker threads, cached as PNG.

use crate::config::cache_dir;
use glam::{Vec2, Vec3};
use mkwtrack::catalog::Catalog;
use mkwtrack::{bfres, bntx};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const SIZE: u32 = 128;
const SS: u32 = 2;

/// `Model/CmnObj_ItemBox000.CmnObj_ItemBox000_A.bfres.zs` -> `thumbs/CmnObj_ItemBox000.CmnObj_ItemBox000_A.png`
pub fn thumb_path(model_rel: &str) -> PathBuf {
    let file = model_rel.rsplit('/').next().unwrap_or(model_rel);
    cache_dir().join("thumbs").join(format!("{}.png", file.trim_end_matches(".zs").trim_end_matches(".bfres")))
}

/// Model files that still need a thumbnail.
pub fn jobs(cat: &Catalog) -> Vec<String> {
    let set: BTreeSet<String> = cat.data.actors.values().filter_map(|a| a.model.clone()).collect();
    set.into_iter().filter(|m| !thumb_path(m).exists()).collect()
}

struct Tex {
    w: u32,
    h: u32,
    px: Vec<[u8; 4]>,
}

fn load_tex(romfs: &Path, name: &str) -> Option<Tex> {
    let img = bntx::load(romfs, name, 256).ok()?;
    Some(Tex { w: img.width, h: img.height, px: img.rgba.as_chunks::<4>().0.to_vec() })
}

/// Render one model's thumbnail, reading it (and its textures) from the first of `roots` that
/// has it.
pub fn render(roots: &[PathBuf], rel: &str) -> Result<(), String> {
    let romfs = roots.iter().find(|r| r.join(rel).is_file()).or(roots.last()).ok_or("no romfs")?.as_path();
    let model = bfres::load(romfs, rel).map_err(|e| e.to_string())?;
    let n = SIZE * SS;
    let mut color = vec![[0u8; 4]; (n * n) as usize];
    let mut depth = vec![f32::MAX; (n * n) as usize];
    let (mn, mx) = (Vec3::from(model.min), Vec3::from(model.max));
    let center = (mn + mx) / 2.0;
    let radius = ((mx - mn).length() / 2.0).max(0.01);
    // camera from the front-right, a little above; orthographic
    let fwd = -Vec3::new(0.75, 0.55, 1.0).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let light = Vec3::new(0.4, 0.9, 0.6).normalize();
    let scale = n as f32 / (2.0 * radius * 1.05);
    let project = |p: Vec3| {
        let d = p - center;
        Vec3::new(n as f32 / 2.0 + d.dot(right) * scale, n as f32 / 2.0 - d.dot(up) * scale, d.dot(fwd))
    };
    let mut textures: HashMap<String, Option<Tex>> = HashMap::new();
    for part in &model.parts {
        let tex = part.texture.as_ref().and_then(|t| textures.entry(t.clone()).or_insert_with(|| load_tex(romfs, t)).as_ref());
        let sp: Vec<Vec3> = part.positions.iter().map(|p| project(Vec3::from(*p))).collect();
        for t in part.indices.as_chunks::<3>().0 {
            let (i0, i1, i2) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let (a, b, c) = (sp[i0], sp[i1], sp[i2]);
            let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
            let x1 = a.x.max(b.x).max(c.x).ceil().min(n as f32 - 1.0) as i32;
            let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
            let y1 = a.y.max(b.y).max(c.y).ceil().min(n as f32 - 1.0) as i32;
            let nrm = |i: usize| Vec3::from(part.normals[i]);
            let uv = |i: usize| Vec2::from(part.uvs[i]);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) / area;
                    let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = a.z * w0 + b.z * w1 + c.z * w2;
                    let k = (y as u32 * n + x as u32) as usize;
                    if z >= depth[k] {
                        continue;
                    }
                    let mut base = [205u8, 205, 205, 255];
                    if let Some(tx) = tex {
                        let u = uv(i0) * w0 + uv(i1) * w1 + uv(i2) * w2;
                        let tu = ((u.x.rem_euclid(1.0)) * tx.w as f32) as u32 % tx.w;
                        let tv = ((u.y.rem_euclid(1.0)) * tx.h as f32) as u32 % tx.h;
                        base = tx.px[(tv * tx.w + tu) as usize];
                        if part.cutout && (base[3] as f32) < part.alpha_cutoff * 255.0 {
                            continue;
                        }
                    }
                    let nn = (nrm(i0) * w0 + nrm(i1) * w1 + nrm(i2) * w2).normalize_or(Vec3::Y);
                    let lit = 0.45 + 0.65 * nn.dot(light).abs();
                    depth[k] = z;
                    color[k] = [
                        (base[0] as f32 * lit).min(255.0) as u8,
                        (base[1] as f32 * lit).min(255.0) as u8,
                        (base[2] as f32 * lit).min(255.0) as u8,
                        255,
                    ];
                }
            }
        }
    }
    // 2x2 box filter down to SIZE
    let mut out = image::RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut acc = [0u32; 4];
            for dy in 0..SS {
                for dx in 0..SS {
                    let c = color[((y * SS + dy) * n + x * SS + dx) as usize];
                    for i in 0..3 {
                        acc[i] += c[i] as u32 * c[3] as u32 / 255;
                    }
                    acc[3] += c[3] as u32;
                }
            }
            let a = acc[3] / (SS * SS);
            let px = if acc[3] > 0 { [0, 1, 2].map(|i| (acc[i] * 255 / acc[3]) as u8) } else { [0, 0, 0] };
            out.put_pixel(x, y, image::Rgba([px[0], px[1], px[2], a as u8]));
        }
    }
    let path = thumb_path(rel);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    out.save(&path).map_err(|e| e.to_string())
}

pub fn render_all(roots: &[PathBuf], jobs: Vec<String>, progress: &(dyn Fn(usize) + Sync)) {
    let queue = Arc::new(Mutex::new(jobs));
    let done = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(2).clamp(1, 8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            let queue = queue.clone();
            let done = &done;
            s.spawn(move || {
                loop {
                    let Some(job) = queue.lock().unwrap().pop() else { break };
                    if let Err(e) = render(roots, &job) {
                        eprintln!("thumbnail {job}: {e}");
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d.is_multiple_of(10) {
                        progress(d);
                    }
                }
            });
        }
    });
    progress(done.load(Ordering::Relaxed));
}
