//! Pictures for things that aren't games in the library: systems, emulators, catalog ports and
//! browser games. Fetched on demand, cached under `Cache/pictures/`, and a miss is remembered
//! for a week so we don't keep asking.
//!
//! - system:   the console's photo from its Wikipedia article
//! - emulator: the project's GitHub owner avatar (usually its logo), else Wikipedia
//! - port:     the original game's box art (libretro), else its cover from Wikipedia, else a
//!   screenshot from the README, else the repository's own preview image, else an in-game
//!   screenshot from libretro, else the README's other images (shots of settings or controls,
//!   art kept in another repository)
//! - browser:  the page's own preview image (og:image), else the original game's box art or
//!   Wikipedia cover, else the source README, else the repository's own preview image, else an
//!   in-game screenshot from libretro
//!
//! README images skip badges, buttons and logos for other services, stats widgets, avatars,
//! vector art and GIFs; anything that won't decode (animations, AVIF) is skipped too.
//!
//! GitHub's generated repository card (the repo name as text) is never used: it says nothing
//! a tile's title doesn't.
//!
//! Ports and browser games must each look different, so that a tile's picture says which game
//! it is. Every image downloaded for one is hashed (sha256 of its bytes, plus a small pixel
//! fingerprint that survives re-encoding and resizing) into `Cache/pictures/hashes.json`. An
//! image that turns up on two different games is generic (a banner shared by a family of
//! ports, a site logo): it is banned for good and taken off the games that already had it,
//! which then look again. Two ports of the same game don't make its box art generic; the
//! second one just picks something else.

use crate::art::{self, BASE, PATH_ENC, best_match};
use crate::catalog::Catalog;
use crate::paths::DataRoot;
use crate::util::norm_key;
use anyhow::{Result, bail};
use image::DynamicImage;
use image::imageops::FilterType;
use percent_encoding::utf8_percent_encode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex};

const MISS_TTL: u64 = 7 * 24 * 3600;
const PIC_W: u32 = 560;
/// Kinds whose pictures must not repeat: each one has to identify its game.
const UNIQUE: [&str; 2] = ["port", "browser"];
/// Bump to throw away port and browser pictures chosen by older rules (bans are kept).
const INDEX_VERSION: u32 = 1;
const INDEX_FILE: &str = "hashes.json";
/// Wider (or taller) than this is a banner or a strip, not a picture of the game.
const MAX_ASPECT: f32 = 3.2;
/// Smallest usable picture: the long side and the short side, in pixels.
const MIN_LONG: u32 = 200;
const MIN_SHORT: u32 = 112;
/// Fingerprints this close (differing bits out of 256) are the same image.
const SAME_LOOK: u32 = 12;
/// Two versions of one game whose pictures are this close show the same art (one game's box
/// art from two sources measures 13-40 apart); different games' pictures measure 49 and up.
const SAME_GAME_LOOK: u32 = 44;

fn cache_dir(root: &DataRoot) -> PathBuf {
    root.cache().join("pictures")
}

fn key(kind: &str, id: &str) -> String {
    format!("{kind}-{id}").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// Already-cached picture, if any.
pub fn cached(root: &DataRoot, kind: &str, id: &str) -> Option<PathBuf> {
    let p = cache_dir(root).join(format!("{}.jpg", key(kind, id)));
    p.is_file().then_some(p)
}

// ---------------------------------------------------------------------------------------------
// Downloading

/// Wikimedia (and others) ask automated clients to say who they are and where to find out
/// more; anonymous-looking clients get rate limited first.
static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .user_agent(concat!("OnionRing/", env!("CARGO_PKG_VERSION"), " (game launcher; +https://github.com/George-Nizor/OnionRing)"))
        .build()
});

/// GET, waiting and trying once more when the server says we're asking too often.
fn call(url: &str) -> Result<ureq::Response> {
    match AGENT.get(url).call() {
        Err(ureq::Error::Status(429, _)) => {
            std::thread::sleep(std::time::Duration::from_millis(2500));
            Ok(AGENT.get(url).call()?)
        }
        r => Ok(r?),
    }
}

fn get_bytes(url: &str, max: u64) -> Result<Vec<u8>> {
    let resp = call(url)?;
    let mut buf = Vec::new();
    resp.into_reader().take(max).read_to_end(&mut buf)?;
    Ok(buf)
}

fn get_text(url: &str) -> Result<String> {
    Ok(String::from_utf8_lossy(&get_bytes(url, 1_500_000)?).to_string())
}

/// An image download. Pages, vector art and animations are refused before the body is read.
fn get_image(url: &str) -> Result<Vec<u8>> {
    let resp = call(url)?;
    let ctype = resp.header("Content-Type").unwrap_or("").to_lowercase();
    if ["html", "svg", "gif", "json", "xml", "text/"].iter().any(|t| ctype.contains(t)) {
        bail!("not a picture ({ctype})");
    }
    let mut buf = Vec::new();
    resp.into_reader().take(12_000_000).read_to_end(&mut buf)?;
    if buf.len() < 2048 {
        bail!("too small to be a picture");
    }
    Ok(buf)
}

/// A downloaded image, checked and ready to store.
struct Fetched {
    /// sha256 of the bytes as downloaded.
    sha: String,
    /// Pixel fingerprint (hex), see [`look`].
    look: String,
    jpeg: Vec<u8>,
    url: String,
}

/// Download one candidate and turn it into a resized JPEG. Refuses tiny images, banners, blank
/// images and anything that won't decode (animated GIFs, AVIF, broken files).
fn fetch(url: &str) -> Result<Fetched> {
    let bytes = get_image(url)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    // The format comes from the content, not a file extension.
    let img = image::load_from_memory(&bytes)?;
    let (w, h) = (img.width(), img.height());
    if w.max(h) < MIN_LONG || w.min(h) < MIN_SHORT {
        bail!("too small ({w}x{h})");
    }
    if w.max(h) as f32 / w.min(h) as f32 > MAX_ASPECT {
        bail!("a banner, not a picture ({w}x{h})");
    }
    let fp = look(&img);
    if fp.iter().map(|b| b.count_ones()).sum::<u32>() < 6 {
        bail!("blank");
    }
    let img = if w > PIC_W { img.resize(PIC_W, u32::MAX, FilterType::Lanczos3) } else { img };
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 86).encode_image(&on_white(&img))?;
    Ok(Fetched { sha, look: hex::encode(fp), jpeg, url: url.to_string() })
}

/// Flatten transparency onto white (JPEG has no alpha; dropping it can leave black or noise
/// where a logo was see-through).
fn on_white(img: &DynamicImage) -> image::RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
        image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])])
    })
}

/// 256-bit difference hash: on a 17x16 grey thumbnail, does each pixel get brighter than the
/// one to its left. The same picture at another size or in another format gives (nearly) the
/// same bits; different pictures differ in about half of them.
fn look(img: &DynamicImage) -> [u8; 32] {
    let g = img.resize_exact(17, 16, FilterType::Triangle).to_luma8();
    let mut out = [0u8; 32];
    for y in 0..16 {
        for x in 0..16 {
            if g.get_pixel(x + 1, y)[0] > g.get_pixel(x, y)[0] {
                let i = (y * 16 + x) as usize;
                out[i / 8] |= 1 << (i % 8);
            }
        }
    }
    out
}

/// How many of the 256 fingerprint bits differ (`None` for a malformed fingerprint).
fn look_distance(a: &str, b: &str) -> Option<u32> {
    match (hex::decode(a), hex::decode(b)) {
        (Ok(a), Ok(b)) if a.len() == b.len() => Some(a.iter().zip(&b).map(|(x, y)| (x ^ y).count_ones()).sum()),
        _ => None,
    }
}

fn same_look(a: &str, b: &str) -> bool {
    look_distance(a, b).is_some_and(|d| d <= SAME_LOOK)
}

// ---------------------------------------------------------------------------------------------
// The hash index: which picture came from which image, and which images are generic

#[derive(Serialize, Deserialize, Default)]
struct Index {
    #[serde(default)]
    version: u32,
    /// sha256 of the downloaded bytes -> the pictures (cache keys) made from them.
    #[serde(default)]
    hashes: BTreeMap<String, Vec<String>>,
    /// Cache key -> pixel fingerprint.
    #[serde(default)]
    looks: BTreeMap<String, String>,
    /// Cache key -> the game it shows (normalised name).
    #[serde(default)]
    subjects: BTreeMap<String, String>,
    /// Cache key -> where the picture was downloaded from.
    #[serde(default)]
    sources: BTreeMap<String, String>,
    /// Generic images, never used again: by sha256 and by fingerprint.
    #[serde(default)]
    banned: BTreeSet<String>,
    #[serde(default)]
    banned_looks: BTreeSet<String>,
}

impl Index {
    fn sha_of(&self, k: &str) -> Option<String> {
        self.hashes.iter().find(|(_, keys)| keys.iter().any(|x| x == k)).map(|(s, _)| s.clone())
    }

    fn remove(&mut self, k: &str) {
        self.hashes.values_mut().for_each(|keys| keys.retain(|x| x != k));
        self.hashes.retain(|_, keys| !keys.is_empty());
        self.looks.remove(k);
        self.subjects.remove(k);
        self.sources.remove(k);
    }

    fn is_banned(&self, f: &Fetched) -> bool {
        self.banned.contains(&f.sha) || self.banned_looks.iter().any(|l| same_look(l, &f.look))
    }

    /// Other pictures made from this image (same bytes or same look).
    fn users(&self, k: &str, f: &Fetched) -> Vec<String> {
        let mut out: Vec<String> = self.hashes.get(&f.sha).map(|keys| keys.iter().filter(|x| *x != k).cloned().collect()).unwrap_or_default();
        for (other, l) in &self.looks {
            if other != k && !out.contains(other) && same_look(l, &f.look) {
                out.push(other.clone());
            }
        }
        out
    }

    /// What to do with a downloaded image for `k`.
    fn judge(&self, k: &str, subject: &str, f: &Fetched) -> Verdict {
        if self.is_banned(f) {
            return Verdict::Banned;
        }
        let users = self.users(k, f);
        if !users.is_empty() {
            let same_game = users.iter().all(|u| self.subjects.get(u).is_some_and(|s| s == subject));
            return if same_game { Verdict::Taken(users) } else { Verdict::Generic(users) };
        }
        // Another version of this game showing the same art from another source (its box art,
        // and this, its Wikipedia cover): the two tiles would still look alike.
        let twins: Vec<String> = self
            .subjects
            .iter()
            .filter(|(other, s)| *other != k && *s == subject)
            .filter(|(other, _)| self.looks.get(*other).and_then(|l| look_distance(l, &f.look)).is_some_and(|d| d <= SAME_GAME_LOOK))
            .map(|(other, _)| other.clone())
            .collect();
        if twins.is_empty() { Verdict::Keep } else { Verdict::Taken(twins) }
    }
}

enum Verdict {
    /// Use it.
    Keep,
    /// Already found to be generic.
    Banned,
    /// Another version of the same game shows it (or a copy of it): fine for that one, but this
    /// tile needs something else so the two can be told apart.
    Taken(Vec<String>),
    /// A different game already shows it: it's generic.
    Generic(Vec<String>),
}

/// Guards `hashes.json` and the pictures it describes across threads (the app asks for many
/// pictures at once). Holds the cache folders already checked against [`INDEX_VERSION`].
static INDEX_LOCK: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn lock_index() -> std::sync::MutexGuard<'static, HashSet<PathBuf>> {
    INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Read the index. Pictures chosen before it existed (or under older rules) may be generic, so
/// they are dropped and looked up again. A file that can't be read right now (locked by a virus
/// scanner, say) is an error, never a reason to start over.
fn load_index(dir: &Path) -> Result<Index> {
    let ix: Index = match std::fs::read(dir.join(INDEX_FILE)) {
        // Unreadable JSON is a lost index: start again.
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Index::default(),
        Err(e) => return Err(e.into()),
    };
    if ix.version == INDEX_VERSION {
        return Ok(ix);
    }
    purge(dir, &UNIQUE);
    let fresh = Index { version: INDEX_VERSION, banned: ix.banned, banned_looks: ix.banned_looks, ..Default::default() };
    save_index(dir, &fresh)?;
    Ok(fresh)
}

fn save_index(dir: &Path, ix: &Index) -> Result<()> {
    let tmp = dir.join(format!("{INDEX_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(ix)?)?;
    std::fs::rename(&tmp, dir.join(INDEX_FILE))?;
    Ok(())
}

/// Delete cached pictures and remembered misses of these kinds. Returns pictures deleted.
fn purge(dir: &Path, kinds: &[&str]) -> usize {
    let mut n = 0;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let ours = kinds.iter().any(|k| name.starts_with(&format!("{k}-")));
        if ours && (name.ends_with(".jpg") || name.ends_with(".none")) && std::fs::remove_file(e.path()).is_ok() && name.ends_with(".jpg") {
            n += 1;
        }
    }
    n
}

/// Make sure this cache folder's pictures follow the current rules (once per process).
fn ensure_current(dir: &Path) {
    let mut checked = lock_index();
    if !checked.contains(dir) && load_index(dir).is_ok() {
        checked.insert(dir.to_path_buf());
    }
}

/// Keep a downloaded image for `k` if no other game uses it. Writes the picture and returns
/// true when it was kept. An image already shown by a different game is generic: it is banned,
/// and the games that had it lose it so they look again.
fn claim(dir: &Path, k: &str, subject: &str, f: &Fetched, dest: &Path) -> Result<bool> {
    let _guard = lock_index();
    let mut ix = load_index(dir)?;
    match ix.judge(k, subject, f) {
        Verdict::Banned | Verdict::Taken(_) => Ok(false),
        Verdict::Keep => {
            ix.remove(k);
            ix.hashes.entry(f.sha.clone()).or_default().push(k.to_string());
            ix.looks.insert(k.to_string(), f.look.clone());
            ix.subjects.insert(k.to_string(), subject.to_string());
            ix.sources.insert(k.to_string(), f.url.clone());
            // Index first: a picture on disk must always be in the index, or a later copy of
            // it couldn't be recognised.
            save_index(dir, &ix)?;
            std::fs::write(dest, &f.jpeg)?;
            Ok(true)
        }
        Verdict::Generic(users) => {
            ix.banned.insert(f.sha.clone());
            ix.banned_looks.insert(f.look.clone());
            for u in users {
                if let Some(s) = ix.sha_of(&u) {
                    ix.banned.insert(s);
                }
                if let Some(l) = ix.looks.get(&u).cloned() {
                    ix.banned_looks.insert(l);
                }
                let _ = std::fs::remove_file(dir.join(format!("{u}.jpg")));
                ix.remove(&u);
            }
            save_index(dir, &ix)?;
            Ok(false)
        }
    }
}

/// Forget cached pictures of these kinds, and remembered misses, so they are looked up again.
/// Images already found to be generic stay banned. Returns how many pictures were removed.
pub fn forget(root: &DataRoot, kinds: &[&str]) -> Result<usize> {
    let dir = cache_dir(root);
    if !dir.is_dir() {
        return Ok(0);
    }
    let _guard = lock_index();
    let n = purge(&dir, kinds);
    let mut ix = load_index(&dir)?;
    let gone: Vec<String> = ix.looks.keys().chain(ix.subjects.keys()).filter(|k| kinds.iter().any(|kind| k.starts_with(&format!("{kind}-")))).cloned().collect();
    gone.iter().for_each(|k| ix.remove(k));
    save_index(&dir, &ix)?;
    Ok(n)
}

/// How many images have been found to be generic (shared by unrelated games) and banned.
pub fn generic_images(root: &DataRoot) -> usize {
    peek_index(&cache_dir(root)).banned.len()
}

/// The index as it is on disk, without bringing it up to date (for reports).
fn peek_index(dir: &Path) -> Index {
    std::fs::read(dir.join(INDEX_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// Where pictures come from

fn wiki_summary(title: &str) -> Option<serde_json::Value> {
    let url = format!("https://en.wikipedia.org/api/rest_v1/page/summary/{}", utf8_percent_encode(&title.replace(' ', "_"), PATH_ENC));
    get_text(&url).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
}

/// Lead image of a Wikipedia article: a 640 px rendition first (small, always a raster), then
/// the original.
fn wikipedia(title: &str) -> Vec<String> {
    wiki_summary(title).map(|v| lead_images(&v)).unwrap_or_default()
}

/// A game's cover from its Wikipedia article, only when the article really is about this one
/// video game: not a franchise, a character or a disambiguation page, and not a redirect to
/// another game ("Dead Rising 2: Case Zero" lands on "Dead Rising 2").
fn wikipedia_cover(game: &str, year: Option<&str>) -> Vec<String> {
    // "Quake (QuakeWorld)", "Street Fighter III: 3rd Strike [3SX]": the article is the game's.
    let title = game.split(" (").next().and_then(|t| t.split(" [").next()).unwrap_or(game).trim();
    let want = norm_key(&fold(title));
    // Article names are case-sensitive: "DOOM" is filed as "Doom".
    let mut titles = vec![title.to_string()];
    if title.chars().any(|c| c.is_alphabetic()) && title == title.to_uppercase() {
        let mut cs = title.to_lowercase().chars().collect::<Vec<_>>();
        cs[0] = cs[0].to_ascii_uppercase();
        titles.insert(0, cs.into_iter().collect());
    }
    let mut articles: Vec<String> = Vec::new();
    for t in &titles {
        articles.extend(year.map(|y| format!("{t} ({y} video game)")));
        articles.extend([format!("{t} (video game)"), t.clone()]);
    }
    for article in articles {
        let Some(v) = wiki_summary(&article) else { continue };
        let standard = v.get("type").and_then(|t| t.as_str()) == Some("standard");
        let about = v.get("description").and_then(|d| d.as_str()).unwrap_or("");
        let landed = v.get("title").and_then(|t| t.as_str()).map(|t| norm_key(&fold(t))).unwrap_or_default();
        if standard && ONE_GAME.is_match(about) && !NOT_ONE_GAME.is_match(about) && landed.contains(&want) {
            return lead_images(&v);
        }
    }
    vec![]
}

static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(19[5-9]\d|20\d\d)\b").unwrap());

/// "2001 video game", "First-person shooter game", "1997 role-playing game".
static ONE_GAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bgame\b").unwrap());
static NOT_ONE_GAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)franchise|series|character|engine|company|developer|publisher|console|list of|mod\b|board game|card game|tabletop|game show").unwrap()
});

fn lead_images(v: &serde_json::Value) -> Vec<String> {
    let mut out = vec![];
    if let Some(t) = v.pointer("/thumbnail/source").and_then(|s| s.as_str()) {
        // Thumbnail URLs carry their width ("/320px-"); ask for a bigger one when the original
        // is big enough (Wikimedia refuses to scale up).
        if v.pointer("/originalimage/width").and_then(|w| w.as_u64()).is_some_and(|w| w >= 640) {
            out.push(THUMB_WIDTH.replace(t, "/640px-").to_string());
        }
        out.push(t.to_string());
    }
    if let Some(o) = v.pointer("/originalimage/source").and_then(|s| s.as_str()) {
        out.push(o.to_string());
    }
    // Covers that aren't free images come in one size only: the three are then the same.
    out.dedup();
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

static THUMB_WIDTH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/\d+px-").unwrap());
static MD_IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"!\[[^\]]*\]\(\s*<?([^)\s>]+)"#).unwrap());
static HTML_IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<img\b[^>]*>"#).unwrap());
static ATTR_SRC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)\ssrc\s*=\s*["']([^"']+)["']"#).unwrap());
static ATTR_WIDTH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)\swidth\s*=\s*["']?(\d+)(?:px)?["'\s/>]"#).unwrap());
static OG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]+(?:property|name)\s*=\s*["'](?:og:image|twitter:image)["'][^>]*content\s*=\s*["']([^"']+)["']|<meta[^>]+content\s*=\s*["']([^"']+)["'][^>]*(?:property|name)\s*=\s*["'](?:og:image|twitter:image)["']"#).unwrap()
});
/// README images that are never a picture of the game: badges, buttons and logos for other
/// services, stats widgets, avatars, vector art and animations.
static NOT_PICTURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)shields\.io|badgen|badge|/workflows/|/actions/|travis-ci|appveyor|codecov|coveralls|codacy|codefactor|",
        r"sonarcloud|deepscan|circleci|discord|ko-fi|kofi|patreon|paypal|liberapay|buymeacoffee|buy-me-a-coffee|",
        r"opencollective|sponsor|donate|gitter|matrix\.org|reddit|twitter|bsky|mastodon|star-history|starchart|",
        r"repobeats|contrib\.rocks|readme-stats|readme-typing|komarev|visitor|hits\.|profile-counter|wakatime|",
        r"forthebadge|flathub|get-it-on|google-play|play\.google|app-?store|microsoft-store|download-?button|",
        r"weblate|crowdin|transifex|/widgets?/|open-?graph\.png|",
        r"avatars\.githubusercontent|gravatar|githubassets|favicon|emoji|logo-small|/icons?/|",
        r"(?:windows|linux|macos|apple|android)[-_]?(?:logo|icon)|",
        r"\.svg(?:[?#]|$)|\.gif(?:[?#]|$)",
    ))
    .unwrap()
});
/// File names that suggest a screenshot (tried first) or a logo/banner (tried last: often the
/// same art across a whole family of ports).
static SHOTISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)screen|shot|snap|gameplay|preview|capture|ingame|in-game|compar").unwrap());
static LOGOISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)logo|banner|icon|header|splash|wordmark|brand|button|cover-?photo").unwrap());
/// Pictures of the port's own settings, controls or installer rather than the game.
static TOOLISH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)config|setting|option|install|setup|launcher|menu|wizard|diagram|gyro|deck|controller|control|input|keybind|binding|mapping|dialog|window|terminal|console-?output|editor").unwrap()
});

/// Turn an image reference from a README into a fetchable URL.
fn absolute(u: &str, base: &str) -> Option<String> {
    let u = u.trim().trim_matches(|c| c == '"' || c == '\'').replace("&amp;", "&");
    if u.is_empty() || u.starts_with("data:") || u.starts_with('#') {
        return None;
    }
    if let Some(rest) = u.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    if u.starts_with("http://") || u.starts_with("https://") {
        // A file page on github.com: fetch the raw file instead.
        if let Some(rest) = u.strip_prefix("https://github.com/").filter(|r| r.contains("/blob/")) {
            return Some(format!("https://raw.githubusercontent.com/{}", rest.replacen("/blob/", "/", 1)));
        }
        return Some(u);
    }
    Some(format!("{base}{}", u.trim_start_matches("./").trim_start_matches('/').replace(' ', "%20")))
}

/// Image URLs from a GitHub README, best first (see [`readme_rank`]), split in two: pictures
/// of the game, and the rest (shots of settings or controls, art kept in another repository
/// and shared by a family of projects), which is tried after every other source.
#[derive(Default)]
struct Readme {
    own: Vec<String>,
    late: Vec<String>,
}

fn readme_images(repo: &str) -> Readme {
    let base = format!("https://raw.githubusercontent.com/{repo}/HEAD/");
    let text = ["README.md", "readme.md", "Readme.md", "README.MD", "README"].iter().find_map(|f| get_text(&format!("{base}{f}")).ok());
    let Some(text) = text else { return Readme::default() };
    let mut found: Vec<(usize, String)> = MD_IMG.captures_iter(&text).filter_map(|c| c.get(1).map(|m| (m.start(), m.as_str().to_string()))).collect();
    for tag in HTML_IMG.find_iter(&text) {
        // A small width means an icon or a button.
        if ATTR_WIDTH.captures(tag.as_str()).and_then(|c| c[1].parse::<u32>().ok()).is_some_and(|w| w < 120) {
            continue;
        }
        if let Some(c) = ATTR_SRC.captures(tag.as_str()) {
            found.push((tag.start(), c[1].to_string()));
        }
    }
    found.sort_by_key(|(at, _)| *at);
    let mut urls: Vec<String> = Vec::new();
    for (_, u) in found {
        if NOT_PICTURE.is_match(&u) {
            continue;
        }
        if let Some(u) = absolute(&u, &base)
            && !urls.contains(&u)
        {
            urls.push(u);
        }
    }
    urls.sort_by_key(|u| readme_rank(u, repo));
    let (mut own, mut late): (Vec<String>, Vec<String>) = urls.into_iter().partition(|u| readme_rank(u, repo) < LATE);
    own.truncate(6);
    late.truncate(3);
    Readme { own, late }
}

/// [`readme_rank`] from here on: not a picture of the game itself.
const LATE: u8 = 3;

/// Order of README images, lowest first: screenshots, other pictures, logos and banners; then,
/// as a last resort, shots of settings or controls, and anything kept in another repository.
fn readme_rank(url: &str, repo: &str) -> u8 {
    // Judge by the file name and its folder, not the repository's own name.
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    let tail: String = path.rsplit('/').take(2).collect::<Vec<_>>().join("/");
    let gameplay = name.to_lowercase().contains("gameplay") || name.to_lowercase().contains("ingame");
    let kind = if TOOLISH.is_match(name) && !gameplay {
        LATE
    } else if SHOTISH.is_match(&tail) {
        0
    } else if LOGOISH.is_match(&tail) {
        2
    } else {
        1
    };
    // Art kept in another repository is shared by a family of projects (a group banner,
    // another tool's screenshots).
    let own = format!("{}/", repo.to_lowercase());
    let lower = path.to_lowercase();
    let elsewhere = ["raw.githubusercontent.com/", "://github.com/"]
        .iter()
        .any(|h| lower.split(h).nth(1).is_some_and(|rest| !rest.starts_with("user-attachments/") && !rest.starts_with(&own)));
    kind + if elsewhere { LATE + 1 } else { 0 }
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

/// The repository's own social preview image. GitHub's generated card (the repo's name as text)
/// isn't a picture of anything, so it doesn't count.
fn repo_preview(repo: &str) -> Vec<String> {
    og_image(&format!("https://github.com/{repo}")).filter(|u| !u.contains("opengraph.githubassets.com")).into_iter().collect()
}

/// libretro thumbnail folder for a console as the catalog names it ("SNES", "Xbox 360",
/// "PC (original release)", "Xbox (2001)").
fn libretro_system(cat: &Catalog, console: &str) -> Option<String> {
    let console = console.split(" (").next().unwrap_or(console).trim();
    if let Some(p) = cat.platforms.iter().find(|p| p.name.eq_ignore_ascii_case(console) || p.id.eq_ignore_ascii_case(console))
        && let Some(l) = &p.libretro
    {
        return Some(l.clone());
    }
    let s = match console.to_lowercase().as_str() {
        "snes" | "super nes" | "super nintendo" | "super famicom" => "Nintendo - Super Nintendo Entertainment System",
        "nes" | "famicom" => "Nintendo - Nintendo Entertainment System",
        "n64" => "Nintendo - Nintendo 64",
        "psp" => "Sony - PlayStation Portable",
        "ps1" | "psx" => "Sony - PlayStation",
        "ps vita" | "playstation vita" => "Sony - PlayStation Vita",
        "xbox 360" => "Microsoft - Xbox 360",
        "xbox" => "Microsoft - Xbox",
        "wii u" => "Nintendo - Wii U",
        "virtual boy" => "Nintendo - Virtual Boy",
        "sega 32x" | "32x" => "Sega - 32X",
        "sega genesis" | "genesis" | "mega drive" | "sega mega drive" => "Sega - Mega Drive - Genesis",
        "sega game gear" | "game gear" => "Sega - Game Gear",
        "sega saturn" | "saturn" => "Sega - Saturn",
        "sega cd" | "mega-cd" | "sega mega-cd" => "Sega - Mega-CD - Sega CD",
        "sega master system" | "master system" => "Sega - Master System - Mark III",
        "dreamcast" | "sega dreamcast" => "Sega - Dreamcast",
        "arcade" => "MAME",
        "pc" | "pc (original release)" | "dos" | "ms-dos" => "DOS",
        _ => return None,
    };
    Some(s.to_string())
}

/// Thumbnail name lists, fetched once per process (and cached on disk by `art::index`).
static NAMES: LazyLock<Mutex<HashMap<String, Arc<Vec<String>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn libretro_names(root: &DataRoot, system: &str, kind: &str) -> Arc<Vec<String>> {
    let mut memo = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    let k = format!("{system}/{kind}");
    if let Some(v) = memo.get(&k) {
        return v.clone();
    }
    let v = Arc::new(art::index(root, system, kind));
    if !v.is_empty() {
        memo.insert(k, v.clone());
    }
    v
}

/// "Einhänder" -> "Einhander": libretro names are plain ASCII.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            'É' => 'E',
            _ => c,
        })
        .collect()
}

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

/// Sequel numbers in a normalised title: digits and roman numerals ("x4" -> 4, "iii" -> 3).
fn numbers(norm: &str) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    for tok in norm.split(' ') {
        let roman = match tok {
            "ii" => Some(2),
            "iii" => Some(3),
            "iv" => Some(4),
            "vi" => Some(6),
            "vii" => Some(7),
            "viii" => Some(8),
            "ix" => Some(9),
            _ => None,
        };
        match roman {
            Some(r) => v.push(r),
            None => v.extend(DIGITS.find_iter(tok).filter_map(|m| m.as_str().parse::<u32>().ok())),
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

fn alnum(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Best libretro thumbnail name for a game, never another entry in its series ("Dino Crisis"
/// for "Dino Crisis 2", "Caesar II" for "Caesar III"). US releases are preferred.
fn match_game(names: &[String], game: &str) -> Option<String> {
    let game = fold(game);
    let want = norm_key(&game);
    let anchor = alnum(want.split(' ').max_by_key(|t| t.len())?);
    if anchor.is_empty() {
        return None;
    }
    let nums = numbers(&want);
    // libretro adds "Version" to some titles ("Pokemon - Red Version"); compare without it.
    // Sorted, so that ties ("Doom" and "DOOM (1993)") always resolve the same way.
    let mut pool: BTreeMap<String, &String> = BTreeMap::new();
    for n in names {
        if !alnum(n).contains(&anchor) {
            continue;
        }
        let key = norm_key(n);
        let have = numbers(&key);
        // A title without a number can still be the same game ("Crash Bandicoot - Warped" for
        // "Crash Bandicoot 3: Warped"), but not when it is just the series' first game.
        let same_entry = have == nums || (have.is_empty() && !nums.is_empty() && !want.starts_with(&format!("{key} ")));
        if same_entry {
            pool.entry(n.replace(" Version", "")).or_insert(n);
        }
    }
    let plain: Vec<String> = pool.keys().cloned().collect();
    if let Some(hit) = best_match(&plain, &format!("{game} (USA)"), &game) {
        return pool.get(&hit).map(|n| (*n).clone());
    }
    // The full release name can carry a subtitle the catalog leaves off ("Street Fighter III -
    // 3rd Strike - Fight for the Future"). Shortest such name wins, US release first.
    let prefix = format!("{want} ");
    pool.iter()
        .filter(|(p, _)| norm_key(p).starts_with(&prefix))
        .min_by_key(|(p, _)| (crate::util::region_of(p).as_deref() != Some("USA"), p.len()))
        .map(|(_, n)| (*n).clone())
}

/// libretro box art (`Named_Boxarts`) or in-game screenshot (`Named_Snaps`) of a game, one per
/// console it came out on.
fn libretro(root: &DataRoot, cat: &Catalog, consoles: &[String], game: &str, kind: &str) -> Vec<String> {
    let mut systems: Vec<String> = Vec::new();
    for c in consoles {
        if let Some(s) = libretro_system(cat, c)
            && !systems.contains(&s)
        {
            systems.push(s);
        }
    }
    let mut out = Vec::new();
    for system in systems {
        let names = libretro_names(root, &system, kind);
        if let Some(n) = match_game(&names, game) {
            out.push(format!("{BASE}/{}/{kind}/{}.png", utf8_percent_encode(&system, PATH_ENC), utf8_percent_encode(&n, PATH_ENC)));
        }
    }
    out
}

/// One place to look for pictures. Sources are only asked when the ones before them gave
/// nothing usable, so a port with good box art never fetches its README.
type Source<'a> = Box<dyn FnOnce() -> Vec<String> + 'a>;

fn sources<'a>(root: &'a DataRoot, cat: &'a Catalog, kind: &str, id: &str) -> Vec<Source<'a>> {
    let mut out: Vec<Source<'a>> = Vec::new();
    match kind {
        "system" => {
            if let Some(p) = cat.platform(id) {
                let article = platform_article(&p.id, &p.name);
                out.push(Box::new(move || wikipedia(&article)));
            }
        }
        "emulator" => {
            if let Some(e) = cat.emulator(id) {
                if let Some(owner) = e.repo.as_deref().and_then(|r| r.split('/').next()) {
                    let avatar = format!("https://github.com/{owner}.png?size=400");
                    out.push(Box::new(move || vec![avatar]));
                }
                let name = e.name.clone();
                out.push(Box::new(move || {
                    let mut v = wikipedia(&format!("{name} (emulator)"));
                    v.extend(wikipedia(&name));
                    v
                }));
                if let Some(h) = e.homepage.clone() {
                    out.push(Box::new(move || og_image(&h).into_iter().collect()));
                }
            }
        }
        "port" => {
            if let Some(p) = cat.port(id) {
                let (consoles, game, repo) = (p.original.clone(), p.game.clone(), p.repo.clone());
                let readme: Rc<OnceCell<Readme>> = Rc::default();
                {
                    let (consoles, game) = (consoles.clone(), game.clone());
                    out.push(Box::new(move || libretro(root, cat, &consoles, &game, "Named_Boxarts")));
                }
                {
                    let game = game.clone();
                    out.push(Box::new(move || wikipedia_cover(&game, None)));
                }
                {
                    let (readme, repo) = (readme.clone(), repo.clone());
                    out.push(Box::new(move || readme.get_or_init(|| readme_images(&repo)).own.clone()));
                }
                {
                    let repo = repo.clone();
                    out.push(Box::new(move || repo_preview(&repo)));
                }
                out.push(Box::new(move || libretro(root, cat, &consoles, &game, "Named_Snaps")));
                out.push(Box::new(move || readme.get_or_init(|| readme_images(&repo)).late.clone()));
            }
        }
        "browser" => {
            if let Some(b) = cat.browser.iter().find(|b| b.id == id) {
                let page = b.url.clone();
                out.push(Box::new(move || og_image(&page).into_iter().collect()));
                if let Some(console) = b.original.clone() {
                    let title = b.title.clone();
                    out.push(Box::new(move || libretro(root, cat, &[console], &title, "Named_Boxarts")));
                }
                {
                    // "PC (1993)": the year picks the right article ("Doom (1993 video game)").
                    let title = b.title.clone();
                    let year = b.original.as_deref().and_then(|o| YEAR.find(o)).map(|m| m.as_str().to_string());
                    out.push(Box::new(move || wikipedia_cover(&title, year.as_deref())));
                }
                if let Some(repo) = b.source.as_deref().and_then(|s| s.strip_prefix("https://github.com/")) {
                    let repo: String = repo.split('/').take(2).collect::<Vec<_>>().join("/");
                    let readme: Rc<OnceCell<Readme>> = Rc::default();
                    {
                        let (readme, repo) = (readme.clone(), repo.clone());
                        out.push(Box::new(move || readme.get_or_init(|| readme_images(&repo)).own.clone()));
                    }
                    {
                        let repo = repo.clone();
                        out.push(Box::new(move || repo_preview(&repo)));
                    }
                    out.push(Box::new(move || readme.get_or_init(|| readme_images(&repo)).late.clone()));
                }
                if let Some(console) = b.original.clone() {
                    let title = b.title.clone();
                    out.push(Box::new(move || libretro(root, cat, &[console], &title, "Named_Snaps")));
                }
            }
        }
        _ => {}
    }
    out
}

/// The game a port or browser game shows, so two versions of one game aren't taken as a
/// generic image.
fn subject(cat: &Catalog, kind: &str, id: &str) -> String {
    let name = match kind {
        "port" => cat.port(id).map(|p| p.game.clone()),
        "browser" => cat.browser.iter().find(|b| b.id == id).map(|b| b.title.clone()),
        _ => None,
    };
    name.map(|n| norm_key(&fold(&n))).unwrap_or_else(|| key(kind, id))
}

/// Every candidate for one thing, in order, and what would happen to it. Changes nothing.
pub fn explain(root: &DataRoot, cat: &Catalog, kind: &str, id: &str) -> Vec<String> {
    let dir = cache_dir(root);
    let k = key(kind, id);
    let about = subject(cat, kind, id);
    let ix = peek_index(&dir);
    let mut out = vec![format!("{k} (shows: {about})")];
    let mut tried: HashSet<String> = HashSet::new();
    for (i, source) in sources(root, cat, kind, id).into_iter().enumerate() {
        let urls: Vec<String> = source().into_iter().filter(|u| tried.insert(u.clone())).collect();
        if urls.is_empty() {
            out.push(format!("  source {}: nothing new", i + 1));
        }
        for url in urls {
            let verdict = match fetch(&url) {
                Err(e) => format!("refused: {e:#}"),
                Ok(_) if !UNIQUE.contains(&kind) => "usable".into(),
                Ok(f) => match ix.judge(&k, &about, &f) {
                    Verdict::Keep => "usable".into(),
                    Verdict::Banned => "refused: generic (banned)".into(),
                    Verdict::Taken(u) => format!("refused: same game, looks like {}", u.join(", ")),
                    Verdict::Generic(u) => format!("refused: generic, also used by {}", u.join(", ")),
                },
            };
            out.push(format!("  source {}: {url}\n      {verdict}", i + 1));
        }
    }
    out
}

/// Find (or fetch and cache) a picture. `None` when nothing usable exists.
pub fn picture(root: &DataRoot, cat: &Catalog, kind: &str, id: &str) -> Result<Option<PathBuf>> {
    let dir = cache_dir(root);
    std::fs::create_dir_all(&dir)?;
    let unique = UNIQUE.contains(&kind);
    if unique {
        ensure_current(&dir);
    }
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
    let about = subject(cat, kind, id);
    let mut tried: HashSet<String> = HashSet::new();
    for source in sources(root, cat, kind, id) {
        for url in source() {
            if !tried.insert(url.clone()) {
                continue;
            }
            let Ok(f) = fetch(&url) else { continue };
            let kept = if unique {
                claim(&dir, &k, &about, &f, &dest)?
            } else {
                std::fs::write(&dest, &f.jpeg)?;
                true
            };
            if kept {
                return Ok(Some(dest));
            }
        }
    }
    std::fs::write(&miss, "")?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequel_numbers() {
        assert_eq!(numbers(&norm_key("Mega Man X4")), vec![4]);
        assert_eq!(numbers(&norm_key("Caesar III")), vec![3]);
        assert_eq!(numbers(&norm_key("Mega Man X (USA)")), Vec::<u32>::new());
        assert_eq!(numbers(&norm_key("Dino Crisis 2 (USA) (Disc 1)")), vec![2]);
    }

    #[test]
    fn matches_the_right_entry_in_a_series() {
        let names: Vec<String> = [
            "Dino Crisis (USA)",
            "Dino Crisis 2 (USA)",
            "Mega Man X4 (USA)",
            "Mega Man X5 (USA)",
            "Pokemon - Red Version (USA, Europe) (SGB Enhanced)",
            "Einhander (USA)",
            "Caesar II (USA)",
            "Crash Bandicoot - Warped (USA)",
            "Doom (USA)",
            "Street Fighter III - 3rd Strike - Fight for the Future (Japan)",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(match_game(&names, "Dino Crisis 2").as_deref(), Some("Dino Crisis 2 (USA)"));
        assert_eq!(match_game(&names, "Crash Bandicoot 3: Warped").as_deref(), Some("Crash Bandicoot - Warped (USA)"));
        assert_eq!(match_game(&names, "Doom 64"), None);
        assert_eq!(match_game(&names, "Street Fighter III: 3rd Strike [3SX]").as_deref(), Some("Street Fighter III - 3rd Strike - Fight for the Future (Japan)"));
        assert_eq!(match_game(&names[2..], "Mega Man X6"), None);
        assert_eq!(match_game(&names, "Dino Crisis").as_deref(), Some("Dino Crisis (USA)"));
        assert_eq!(match_game(&names, "Mega Man X5").as_deref(), Some("Mega Man X5 (USA)"));
        assert_eq!(match_game(&names, "Pokemon Red").as_deref(), Some("Pokemon - Red Version (USA, Europe) (SGB Enhanced)"));
        assert_eq!(match_game(&names, "Einhänder").as_deref(), Some("Einhander (USA)"));
        assert_eq!(match_game(&names, "Caesar III"), None);
    }

    #[test]
    fn readme_urls() {
        let base = "https://raw.githubusercontent.com/o/r/HEAD/";
        assert_eq!(absolute("./docs/shot 1.png", base).as_deref(), Some("https://raw.githubusercontent.com/o/r/HEAD/docs/shot%201.png"));
        assert_eq!(absolute("https://github.com/o/r/blob/main/a.png?raw=true", base).as_deref(), Some("https://raw.githubusercontent.com/o/r/main/a.png?raw=true"));
        assert!(NOT_PICTURE.is_match("https://img.shields.io/github/v/release/o/r"));
        assert!(NOT_PICTURE.is_match("https://raw.githubusercontent.com/o/r/HEAD/anim.gif"));
        assert!(!NOT_PICTURE.is_match("https://github.com/user-attachments/assets/0b6c1f1e"));
        assert!(NOT_PICTURE.is_match("https://hosted.weblate.org/widget/uzdoom/open-graph.png"));

        let repo = "alexbeavs-ps1-ports/alien-resurrection-recomp";
        let shot = "https://raw.githubusercontent.com/alexbeavs-ps1-ports/alien-resurrection-recomp/HEAD/docs/screenshot1.png";
        let attachment = "https://github.com/user-attachments/assets/0b6c1f1e";
        let launcher = "https://raw.githubusercontent.com/TechnicallyComputers/RetComM-Launcher/main/docs/screenshots/hub-and-game-launcher.png";
        let group_banner = "https://raw.githubusercontent.com/alexbeavs-ps1-ports/psxrecomp-ports/main/docs/assets/alexbeav-ps1-recomps-banner.png";
        let own_logo = "https://raw.githubusercontent.com/alexbeavs-ps1-ports/alien-resurrection-recomp/HEAD/logo.png";
        let gyro = "https://raw.githubusercontent.com/alexbeavs-ps1-ports/alien-resurrection-recomp/HEAD/docs/deck_gyro_1.jpg";
        let own_gameplay = "https://raw.githubusercontent.com/alexbeavs-ps1-ports/alien-resurrection-recomp/HEAD/docs/launcher-gameplay.jpg";
        let ranks: Vec<u8> = [shot, own_gameplay, attachment, own_logo, gyro, group_banner, launcher].iter().map(|u| readme_rank(u, repo)).collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "{ranks:?}");
        // Settings shots and other repositories' art wait until every other source is tried.
        assert!([shot, own_gameplay, attachment, own_logo].iter().all(|u| readme_rank(u, repo) < LATE));
        assert!([gyro, group_banner, launcher].iter().all(|u| readme_rank(u, repo) >= LATE));
    }

    fn fetched(seed: u32) -> Fetched {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(320, 240, |x, y| image::Rgb([((x * seed + y) % 256) as u8, ((y * seed) % 256) as u8, (x % 256) as u8])));
        Fetched { sha: format!("sha{seed}"), look: hex::encode(look(&img)), jpeg: vec![seed as u8; 4], url: format!("https://example.test/{seed}.png") }
    }

    #[test]
    fn generic_images_are_banned_and_taken_back() {
        let dir = std::env::temp_dir().join(format!("onionring-pictures-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let jpg = |k: &str| dir.join(format!("{k}.jpg"));
        let banner = fetched(3);

        // First port to see the banner keeps it.
        assert!(claim(&dir, "port-a", "game a", &banner, &jpg("port-a")).unwrap());
        // Another port of the same game can't have it too, but it isn't generic.
        assert!(!claim(&dir, "port-a2", "game a", &banner, &jpg("port-a2")).unwrap());
        assert!(jpg("port-a").is_file());
        // A different game with the same image: generic. Banned, and port-a loses it.
        assert!(!claim(&dir, "port-b", "game b", &banner, &jpg("port-b")).unwrap());
        assert!(!jpg("port-a").is_file());
        let ix = load_index(&dir).unwrap();
        assert!(ix.banned.contains("sha3") && ix.looks.is_empty());
        // Nobody can use it again, even re-encoded (different bytes, same look).
        let again = Fetched { sha: "other-bytes".into(), ..fetched(3) };
        assert!(!claim(&dir, "port-a", "game a", &again, &jpg("port-a")).unwrap());
        // Its own picture is fine.
        assert!(claim(&dir, "port-a", "game a", &fetched(7), &jpg("port-a")).unwrap());

        // The same cover from another source (20 bits off): too alike for another version of
        // game a, but not close enough to call it generic for anyone else.
        let mut bytes = hex::decode(&fetched(7).look).unwrap();
        bytes.iter_mut().take(5).for_each(|b| *b ^= 0x0f);
        let scan = Fetched { sha: "scan".into(), look: hex::encode(bytes), ..fetched(7) };
        assert!(!claim(&dir, "port-a3", "game a", &scan, &jpg("port-a3")).unwrap());
        assert!(claim(&dir, "port-c", "game c", &scan, &jpg("port-c")).unwrap());
        assert!(jpg("port-a").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_picture_same_look() {
        let mut img = image::RgbImage::new(640, 480);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x / 3) as u8, (y / 2) as u8, ((x + y) % 256) as u8]);
        }
        let a = DynamicImage::ImageRgb8(img);
        let b = a.resize(320, 240, FilterType::Triangle);
        let c = a.fliph();
        assert!(same_look(&hex::encode(look(&a)), &hex::encode(look(&b))));
        assert!(!same_look(&hex::encode(look(&a)), &hex::encode(look(&c))));
    }
}
