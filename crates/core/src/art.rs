//! Artwork, with no account or API key needed.
//!
//! - Box art and screenshots from the libretro thumbnail server, matched by No-Intro/Redump
//!   names (exact first, then fuzzy within the same region).
//! - PS3 folder games ship their own ICON0/PIC1 artwork.
//! - Ports can point at artwork inside their install.
//!
//! Everything is cached under `Media\<hash>\`. Grid thumbnails are resized JPEGs so the
//! library scrolls smoothly with hundreds of covers.

use crate::catalog::Catalog;
use crate::db::Db;
use crate::install;
use crate::model::Progress;
use crate::paths::DataRoot;
use crate::util::{norm_key, now, path_str, region_of, split_name};
use anyhow::{Context, Result};
use image::imageops::FilterType;
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub(crate) const BASE: &str = "https://thumbnails.libretro.com";
const THUMB_W: u32 = 360;
const INDEX_MAX_AGE: i64 = 14 * 24 * 3600;

pub(crate) const PATH_ENC: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'%').add(b'?').add(b'<').add(b'>').add(b'`').add(b'{').add(b'}');

pub(crate) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .user_agent(concat!("OnionRing/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// File names (without .png) available for one system + kind, cached on disk.
pub(crate) fn index(root: &DataRoot, system: &str, kind: &str) -> Vec<String> {
    let dir = root.cache().join("libretro");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("{}__{}.txt", system.replace([' ', '-'], "_"), kind));
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| (age.as_secs() as i64) < INDEX_MAX_AGE);
    if !fresh {
        let url = format!("{BASE}/{}/{kind}/", utf8_percent_encode(system, PATH_ENC));
        if let Ok(resp) = agent().get(&url).call()
            && let Ok(html) = resp.into_string() {
                let names: Vec<String> = html
                    .split("href=\"")
                    .skip(1)
                    .filter_map(|s| s.split('"').next())
                    .filter(|h| h.ends_with(".png") && !h.starts_with('/') && !h.starts_with('?'))
                    .map(|h| percent_decode_str(&h[..h.len() - 4]).decode_utf8_lossy().to_string())
                    .collect();
                if !names.is_empty() {
                    let _ = std::fs::write(&file, names.join("\n"));
                }
            }
    }
    std::fs::read_to_string(&file).map(|s| s.lines().map(String::from).collect()).unwrap_or_default()
}

/// libretro replaces these characters with `_` in thumbnail names.
fn libretro_name(s: &str) -> String {
    s.chars().map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c }).collect()
}

/// Best thumbnail name for a game, by exact file stem or by normalised title + region.
pub fn best_match(names: &[String], stem: &str, title: &str) -> Option<String> {
    let exact = libretro_name(stem);
    if names.contains(&exact) {
        return Some(exact);
    }
    let want = norm_key(title);
    let want_region = region_of(stem);
    let mut best: Option<(f64, &String)> = None;
    for n in names {
        let k = norm_key(n);
        let mut score = if k == want { 1.0 } else { strsim::jaro_winkler(&k, &want) };
        if score < 0.9 {
            continue;
        }
        if region_of(n) == want_region && want_region.is_some() {
            score += 0.05;
        }
        // Prefer the plain release over demos, betas, kiosk discs.
        let ln = n.to_lowercase();
        if ["(demo", "(beta", "(proto", "(kiosk", "(sample"].iter().any(|t| ln.contains(t)) {
            score -= 0.1;
        }
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, n));
        }
    }
    best.filter(|(s, _)| *s >= 0.93).map(|(_, n)| n.clone())
}

fn fetch(url: &str, dest: &Path) -> Result<()> {
    let resp = agent().get(url).call()?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes)?;
    if bytes.len() < 1024 {
        anyhow::bail!("tiny response");
    }
    std::fs::write(dest, bytes)?;
    Ok(())
}

/// Write a grid-sized JPEG next to the full image.
pub fn make_thumb(src: &Path, dest: &Path, width: u32) -> Result<()> {
    let img = image::open(src).with_context(|| format!("decoding {}", src.display()))?;
    let img = if img.width() > width { img.resize(width, u32::MAX, FilterType::Lanczos3) } else { img };
    let rgb = img.to_rgb8();
    let mut out = std::fs::File::create(dest)?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 86).encode_image(&rgb)?;
    Ok(())
}

struct Job {
    id: i64,
    key: String,
    title: String,
    platform: String,
    kind: String,
    launch_path: String,
    app_id: Option<String>,
}

#[derive(Default)]
struct Found {
    cover: Option<PathBuf>,
    thumb: Option<PathBuf>,
    hero: Option<PathBuf>,
    logo: Option<PathBuf>,
}

fn copy_local(src: &Path, dest: &Path) -> Option<PathBuf> {
    (src.is_file() && std::fs::copy(src, dest).is_ok()).then(|| dest.to_path_buf())
}

fn process(root: &DataRoot, cat: &Catalog, job: &Job, indexes: &Mutex<HashMap<(String, String), Vec<String>>>) -> Found {
    let dir = root.media_dir(&job.key);
    let _ = std::fs::create_dir_all(&dir);
    let mut f = Found::default();

    // Ports: their own icon as the logo; box art comes from the original console's set.
    let mut system = cat.platform(&job.platform).and_then(|p| p.libretro.clone());
    if job.kind == "port"
        && let (Some(app_id), Some(def)) = (&job.app_id, job.app_id.as_deref().and_then(|a| cat.port(a))) {
            if let (Some(app), Some(rel)) = (install::installed(root, app_id), &def.art) {
                let src = app.dir(root).join(rel);
                let ext = src.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "png".into());
                f.logo = copy_local(&src, &dir.join(format!("logo.{ext}")));
            }
            system = def
                .original
                .iter()
                .find_map(|o| cat.platforms.iter().find(|p| p.name.eq_ignore_ascii_case(o)).and_then(|p| p.libretro.clone()));
        }

    // Mashups: the mod's own screenshot (or its game's art), from the Get more picture.
    if job.kind == "mashup"
        && let Some(id) = &job.app_id
        && let Ok(Some(p)) = crate::pictures::picture(root, cat, "mashup", id)
    {
        f.hero = copy_local(&p, &dir.join("hero.jpg"));
        f.cover = copy_local(&p, &dir.join("cover.jpg"));
    }

    // PS3 folder games: <game>\PS3_GAME\USRDIR\EBOOT.BIN -> PS3_GAME\PIC1.PNG / ICON0.PNG
    if job.platform == "ps3" {
        let eboot = Path::new(&job.launch_path);
        if let Some(ps3_game) = eboot.parent().and_then(|p| p.parent()) {
            f.hero = copy_local(&ps3_game.join("PIC1.PNG"), &dir.join("hero.png")).or(f.hero);
            f.logo = copy_local(&ps3_game.join("ICON0.PNG"), &dir.join("icon.png")).or(f.logo);
        }
    }

    if let Some(system) = system {
        let (stem, _) = split_name(Path::new(&job.launch_path));
        let generic = stem.eq_ignore_ascii_case("eboot") || job.kind == "port";
        let stem = if generic { job.title.clone() } else { stem };
        let get_index = |kind: &str| -> Vec<String> {
            let k = (system.clone(), kind.to_string());
            if let Some(v) = indexes.lock().unwrap().get(&k) {
                return v.clone();
            }
            let v = index(root, &system, kind);
            indexes.lock().unwrap().insert(k, v.clone());
            v
        };
        let names = get_index("Named_Boxarts");
        if let Some(name) = best_match(&names, &stem, &job.title) {
            let url = format!("{BASE}/{}/Named_Boxarts/{}.png", utf8_percent_encode(&system, PATH_ENC), utf8_percent_encode(&name, PATH_ENC));
            let dest = dir.join("cover.png");
            if fetch(&url, &dest).is_ok() {
                f.cover = Some(dest);
            }
            if f.hero.is_none() {
                let snaps = get_index("Named_Snaps");
                if let Some(sname) = best_match(&snaps, &name, &job.title) {
                    let url = format!("{BASE}/{}/Named_Snaps/{}.png", utf8_percent_encode(&system, PATH_ENC), utf8_percent_encode(&sname, PATH_ENC));
                    let dest = dir.join("hero.png");
                    if fetch(&url, &dest).is_ok() {
                        f.hero = Some(dest);
                    }
                }
            }
        }
    }

    if let Some(c) = &f.cover {
        let t = dir.join("cover_thumb.jpg");
        if make_thumb(c, &t, THUMB_W).is_ok() {
            f.thumb = Some(t);
        }
    }
    f
}

/// Fetch art for games that have not been checked yet (or all, when `force`).
pub fn refresh(root: &DataRoot, cat: &Catalog, db: &Db, force: bool, progress: &(dyn Fn(Progress) + Sync)) -> Result<usize> {
    if force {
        db.reset_art()?;
    }
    let jobs: Vec<Job> = db
        .art_pending(force)?
        .into_iter()
        .map(|(id, key, title, platform, kind, launch_path, app_id)| Job { id, key, title, platform, kind, launch_path, app_id })
        .collect();
    let total = jobs.len();
    if total == 0 {
        return Ok(0);
    }
    let indexes = Mutex::new(HashMap::new());
    let done = std::sync::atomic::AtomicUsize::new(0);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(6).build()?;
    let results: Vec<(i64, Found)> = pool.install(|| {
        jobs.par_iter()
            .map(|j| {
                let r = process(root, cat, j, &indexes);
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                progress(Progress::new("art", j.title.clone(), n, total));
                (j.id, r)
            })
            .collect()
    });
    let s = |p: &Option<PathBuf>| p.as_deref().map(path_str);
    let mut with_art = 0;
    for (id, f) in &results {
        if f.cover.is_some() || f.hero.is_some() {
            with_art += 1;
        }
        db.set_art(*id, s(&f.cover).as_deref(), s(&f.thumb).as_deref(), s(&f.hero).as_deref(), s(&f.logo).as_deref())?;
    }
    db.kv_set("art_refreshed", &now().to_string())?;
    Ok(with_art)
}

/// Use a user-picked image as a game's cover.
pub fn set_custom_cover(root: &DataRoot, db: &Db, id: i64, image: &Path) -> Result<()> {
    let g = db.game(id)?;
    let dir = root.media_dir(&g.key);
    std::fs::create_dir_all(&dir)?;
    let ext = image.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
    let cover = dir.join(format!("cover_custom.{ext}"));
    std::fs::copy(image, &cover)?;
    let thumb = dir.join("cover_thumb.jpg");
    make_thumb(&cover, &thumb, THUMB_W)?;
    db.set_art(id, Some(&path_str(&cover)), Some(&path_str(&thumb)), None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_prefers_region() {
        let names: Vec<String> = [
            "Legend of Zelda, The - The Wind Waker (Europe) (En,Fr,De,Es,It)",
            "Legend of Zelda, The - The Wind Waker (USA)",
            "Legend of Zelda, The - The Wind Waker (USA) (Kiosk Demo)",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            best_match(&names, "Legend of Zelda, The - The Wind Waker (USA)", "x").as_deref(),
            Some("Legend of Zelda, The - The Wind Waker (USA)")
        );
        let fuzzy = best_match(&names, "Zelda Wind Waker", "The Legend of Zelda: The Wind Waker").unwrap();
        assert!(!fuzzy.contains("Kiosk"));
    }
}
