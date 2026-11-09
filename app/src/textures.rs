//! Albedo textures for the viewport: decoded from the romfs (`Tex/<name>.bntx.zs`) and cached as
//! PNG files in `~/.cache/trackstudioworld/textures/`, which Qt Quick 3D loads by URL.

use crate::config::cache_dir;
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use std::path::{Path, PathBuf};

/// Largest side kept for the viewport; bigger textures use a smaller mip.
const MAX_SIZE: u32 = 1024;

pub fn png_path(name: &str) -> PathBuf {
    cache_dir().join("textures").join(format!("{}.png", name.replace(['/', '\\'], "_")))
}

/// The cached PNG of a texture, if it is there.
pub fn cached(name: &str) -> Option<PathBuf> {
    let p = png_path(name);
    p.is_file().then_some(p)
}

/// Make sure the texture's PNG is cached and not older than its file, read from the first of
/// `roots` (the open mod's romfs, then the game's) that has it.
pub fn ensure(roots: &[PathBuf], name: &str) -> Result<PathBuf, String> {
    let png = png_path(name);
    let romfs = roots.iter().find(|r| mkwtrack::bntx::texture_path(r, name).is_file()).or(roots.last()).ok_or("no romfs")?;
    let src = mkwtrack::bntx::texture_path(romfs, name);
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if let (Some(a), Some(b)) = (modified(&png), modified(&src))
        && a >= b
    {
        return Ok(png);
    }
    let img = mkwtrack::bntx::load(romfs, name, MAX_SIZE).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(img.rgba.len() / 2);
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Sub)
        .write_image(&img.rgba, img.width, img.height, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    mkwtrack::io::write_atomic(&png, &out).map_err(|e| e.to_string())?;
    Ok(png)
}

/// [`ensure`] for many textures on worker threads; returns the errors.
pub fn ensure_all(roots: &[PathBuf], names: &[String]) -> Vec<String> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).saturating_sub(1).clamp(1, 8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let errors = std::sync::Mutex::new(vec![]);
    std::thread::scope(|s| {
        for _ in 0..threads.min(names.len()) {
            s.spawn(|| {
                while let Some(name) = names.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
                    if let Err(e) = ensure(roots, name) {
                        errors.lock().unwrap().push(format!("Texture {name}: {e}"));
                    }
                }
            });
        }
    });
    errors.into_inner().unwrap()
}
