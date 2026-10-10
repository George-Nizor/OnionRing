//! Pictures for things that aren't games in the library: systems, emulators, catalog ports and
//! browser games. Fetched on demand, cached under `Cache/pictures/`, and a miss is remembered
//! for a week so we don't keep asking.
//!
//! - system:   the console's photo from its Wikipedia article
//! - emulator: the project's GitHub owner avatar (usually its logo), else Wikipedia
//! - port:     first real image in the README, else the original game's box art, else the
//!   repository's social preview image
//! - browser:  the page's own preview image (og:image), else the source README

use crate::art::{self, BASE, PATH_ENC, best_match};
use crate::catalog::Catalog;
use crate::paths::DataRoot;
use anyhow::{Result, bail};
use percent_encoding::utf8_percent_encode;
use regex::Regex;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const MISS_TTL: u64 = 7 * 24 * 3600;
const PIC_W: u32 = 560;

fn cache_dir(root: &DataRoot) -> PathBuf {
    root.cache().join("pictures")
}

fn key(kind: &str, id: &str) -> String {
    let safe: String = format!("{kind}-{id}").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    safe
}

/// Already-cached picture, if any.
pub fn cached(root: &DataRoot, kind: &str, id: &str) -> Option<PathBuf> {
    let p = cache_dir(root).join(format!("{}.jpg", key(kind, id)));
    p.is_file().then_some(p)
}

fn get_bytes(url: &str, max: u64) -> Result<(Vec<u8>, String)> {
    let resp = art::agent().get(url).call()?;
    let ctype = resp.header("Content-Type").unwrap_or("").to_lowercase();
    let mut buf = Vec::new();
    resp.into_reader().take(max).read_to_end(&mut buf)?;
    Ok((buf, ctype))
}

fn get_text(url: &str) -> Result<String> {
    let (b, _) = get_bytes(url, 1_500_000)?;
    Ok(String::from_utf8_lossy(&b).to_string())
}

/// Download an image and store it as a resized JPEG. Rejects tiny files and non-images.
fn save_image(url: &str, dest: &Path) -> Result<()> {
    let (bytes, ctype) = get_bytes(url, 12_000_000)?;
    if bytes.len() < 2048 || ctype.contains("svg") || ctype.contains("html") {
        bail!("not a usable image");
    }
    // Decode from the bytes (the format comes from the content, not a file extension).
    let img = image::load_from_memory(&bytes)?;
    let img = if img.width() > PIC_W { img.resize(PIC_W, u32::MAX, image::imageops::FilterType::Lanczos3) } else { img };
    if img.width() < 64 || img.height() < 64 {
        bail!("too small");
    }
    let mut out = std::fs::File::create(dest)?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 86).encode_image(&img.to_rgb8())?;
    Ok(())
}

/// Lead image of a Wikipedia article: a 640 px rendition first (small, always a raster), then
/// the original.
fn wikipedia(title: &str) -> Vec<String> {
    let url = format!("https://en.wikipedia.org/api/rest_v1/page/summary/{}", utf8_percent_encode(&title.replace(' ', "_"), PATH_ENC));
    let Some(v) = get_text(&url).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else { return vec![] };
    let mut out = vec![];
    if let Some(t) = v.pointer("/thumbnail/source").and_then(|s| s.as_str()) {
        // Thumbnail URLs carry their width ("/320px-"); ask for a bigger one.
        out.push(Regex::new(r"/\d+px-").unwrap().replace(t, "/640px-").to_string());
        out.push(t.to_string());
    }
    if let Some(o) = v.pointer("/originalimage/source").and_then(|s| s.as_str()) {
        out.push(o.to_string());
    }
    out
}

/// Wikipedia article for each platform where its plain name is ambiguous.
fn platform_article(id: &str, name: &str) -> String {
    match id {
        "ps1" => "PlayStation (console)",
        "gc" => "GameCube",
        "nds" => "Nintendo DS",
        "3ds" => "Nintendo 3DS",
        "psp" => "PlayStation Portable",
        "genesis" => "Sega Genesis",
        "gb" => "Game Boy",
        "gbc" => "Game Boy Color",
        "ps5" => "PlayStation 5",
        _ => name,
    }
    .to_string()
}

static MD_IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"!\[[^\]]*\]\(\s*<?([^)\s>]+)"#).unwrap());
static HTML_IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<img[^>]+src\s*=\s*["']([^"']+)["']"#).unwrap());
static OG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]+(?:property|name)\s*=\s*["'](?:og:image|twitter:image)["'][^>]*content\s*=\s*["']([^"']+)["']|<meta[^>]+content\s*=\s*["']([^"']+)["'][^>]*(?:property|name)\s*=\s*["'](?:og:image|twitter:image)["']"#).unwrap()
});
static NOT_SCREENSHOT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)shields\.io|badge|badgen|travis|appveyor|/workflows/|codecov|discord|ko-fi|patreon|paypal|liberapay|buymeacoffee|\.svg(\?|$)|logo-small|favicon|emoji").unwrap()
});

/// Image URLs from a GitHub README, most likely screenshots first.
fn readme_images(repo: &str) -> Vec<String> {
    let base = format!("https://raw.githubusercontent.com/{repo}/HEAD/");
    let text = ["README.md", "readme.md", "Readme.md", "README.MD"]
        .iter()
        .find_map(|f| get_text(&format!("{base}{f}")).ok().filter(|t| !t.starts_with("404")));
    let Some(text) = text else { return vec![] };
    MD_IMG
        .captures_iter(&text)
        .chain(HTML_IMG.captures_iter(&text))
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .filter(|u| !NOT_SCREENSHOT.is_match(u))
        .map(|u| {
            if u.starts_with("http") {
                u.replace("github.com/", "raw.githubusercontent.com/").replace("/blob/", "/").replace("raw.githubusercontent.com/user-attachments", "github.com/user-attachments")
            } else {
                format!("{base}{}", u.trim_start_matches("./").trim_start_matches('/'))
            }
        })
        .take(4)
        .collect()
}

fn og_image(page: &str) -> Option<String> {
    let html = get_text(page).ok()?;
    let c = OG.captures(&html)?;
    let u = c.get(1).or_else(|| c.get(2))?.as_str().replace("&amp;", "&");
    Some(if u.starts_with("//") {
        format!("https:{u}")
    } else if u.starts_with('/') {
        let p = url::Url::parse(page).ok()?;
        format!("{}://{}{}", p.scheme(), p.host_str()?, u)
    } else {
        u
    })
}

fn libretro_boxart(root: &DataRoot, cat: &Catalog, consoles: &[String], game: &str) -> Option<String> {
    for o in consoles {
        let Some(system) = cat.platforms.iter().find(|p| p.name.eq_ignore_ascii_case(o)).and_then(|p| p.libretro.clone()) else { continue };
        let names = art::index(root, &system, "Named_Boxarts");
        if let Some(n) = best_match(&names, game, game) {
            return Some(format!("{BASE}/{}/Named_Boxarts/{}.png", utf8_percent_encode(&system, PATH_ENC), utf8_percent_encode(&n, PATH_ENC)));
        }
    }
    None
}

/// Candidate image URLs for one thing, best first.
fn candidates(root: &DataRoot, cat: &Catalog, kind: &str, id: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    match kind {
        "system" => {
            if let Some(p) = cat.platform(id) {
                out.extend(wikipedia(&platform_article(&p.id, &p.name)));
            }
        }
        "emulator" => {
            if let Some(e) = cat.emulator(id) {
                if let Some(owner) = e.repo.as_deref().and_then(|r| r.split('/').next()) {
                    out.push(format!("https://github.com/{owner}.png?size=400"));
                }
                out.extend(wikipedia(&format!("{} (emulator)", e.name)));
                out.extend(wikipedia(&e.name));
                if let Some(h) = &e.homepage {
                    out.extend(og_image(h));
                }
            }
        }
        "port" => {
            if let Some(p) = cat.port(id) {
                out.extend(readme_images(&p.repo));
                out.extend(libretro_boxart(root, cat, &p.original, &p.game));
                out.extend(og_image(&format!("https://github.com/{}", p.repo)));
            }
        }
        "browser" => {
            if let Some(b) = cat.browser.iter().find(|b| b.id == id) {
                out.extend(og_image(&b.url));
                if let Some(repo) = b.source.as_deref().and_then(|s| s.strip_prefix("https://github.com/")) {
                    let repo: String = repo.split('/').take(2).collect::<Vec<_>>().join("/");
                    out.extend(readme_images(&repo));
                    out.extend(og_image(&format!("https://github.com/{repo}")));
                }
            }
        }
        _ => {}
    }
    out
}

/// Find (or fetch and cache) a picture. `None` when nothing usable exists.
pub fn picture(root: &DataRoot, cat: &Catalog, kind: &str, id: &str) -> Result<Option<PathBuf>> {
    let dir = cache_dir(root);
    std::fs::create_dir_all(&dir)?;
    let k = key(kind, id);
    let dest = dir.join(format!("{k}.jpg"));
    if dest.is_file() {
        return Ok(Some(dest));
    }
    let miss = dir.join(format!("{k}.none"));
    let recent_miss = std::fs::metadata(&miss)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < MISS_TTL);
    if recent_miss {
        return Ok(None);
    }
    for url in candidates(root, cat, kind, id) {
        if save_image(&url, &dest).is_ok() {
            return Ok(Some(dest));
        }
    }
    std::fs::write(&miss, "")?;
    Ok(None)
}
