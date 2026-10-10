//! GitHub Releases with ETag caching, so routine update checks cost nothing against the
//! 60-requests/hour anonymous limit (a 304 does not count). Set `GITHUB_TOKEN` to lift it.

use crate::paths::DataRoot;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn sha256_of(&self, asset: &Asset) -> Option<String> {
        let _ = self;
        asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(|s| s.to_lowercase())
    }
}

#[derive(Serialize, Deserialize)]
struct Cached {
    etag: Option<String>,
    body: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(60))
        .user_agent(concat!("OnionRing/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn token() -> Option<String> {
    std::env::var("ONIONRING_GITHUB_TOKEN").or_else(|_| std::env::var("GITHUB_TOKEN")).ok().filter(|t| !t.is_empty())
}

fn get_cached(root: &DataRoot, url: &str) -> Result<String> {
    let dir = root.cache().join("github");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("{}.json", crate::util::short_hash(url)));
    let cached: Option<Cached> = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok());
    let mut req = agent().get(url).set("Accept", "application/vnd.github+json").set("X-GitHub-Api-Version", "2022-11-28");
    if let Some(t) = token() {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    if let Some(etag) = cached.as_ref().and_then(|c| c.etag.as_deref()) {
        req = req.set("If-None-Match", etag);
    }
    match req.call() {
        Ok(resp) if resp.status() == 304 => Ok(cached.map(|c| c.body).unwrap_or_default()),
        Ok(resp) => {
            let etag = resp.header("ETag").map(|s| s.to_string());
            let body = resp.into_string()?;
            std::fs::write(&file, serde_json::to_string(&Cached { etag, body: body.clone() })?)?;
            Ok(body)
        }
        Err(ureq::Error::Status(403, _)) | Err(ureq::Error::Status(429, _)) => {
            // Rate limited: fall back to the last good answer if we have one.
            match cached {
                Some(c) => Ok(c.body),
                None => bail!("GitHub rate limit reached. Try again later, or set GITHUB_TOKEN."),
            }
        }
        Err(ureq::Error::Status(404, _)) => bail!("{url} not found (repository moved or removed?)"),
        Err(e) => match cached {
            Some(c) => Ok(c.body),
            None => Err(e).context(format!("fetching {url}")),
        },
    }
}

pub fn releases(root: &DataRoot, repo: &str, per_page: u32) -> Result<Vec<Release>> {
    let body = get_cached(root, &format!("https://api.github.com/repos/{repo}/releases?per_page={per_page}"))?;
    Ok(serde_json::from_str(&body)?)
}

/// Newest release. Stable unless `prerelease` is set (some projects only ship nightlies).
pub fn latest(root: &DataRoot, repo: &str, prerelease: bool) -> Result<Release> {
    if !prerelease {
        let body = get_cached(root, &format!("https://api.github.com/repos/{repo}/releases/latest"));
        if let Ok(b) = body
            && let Ok(r) = serde_json::from_str::<Release>(&b) {
                return Ok(r);
            }
    }
    releases(root, repo, 5)?.into_iter().next().context("no releases published")
}

/// Score a release asset as "the Windows x64 portable archive". `None` = not a candidate.
/// Mirrors `scripts/build-ports.mjs`, so the catalog only lists ports this will find.
pub fn windows_score(name: &str) -> Option<i32> {
    let n = name.to_lowercase();
    if !(n.ends_with(".zip") || n.ends_with(".7z")) {
        return None;
    }
    const BAD: &[&str] = &[
        "arm64", "aarch64", "linux", "macos", "mac-", "-mac", "osx", "darwin", "android", "ios", "source", "-src", "debug",
        "pdb", "symbols", "dbg", "appimage", "flatpak", "win32", "i686", "x86-", "-x86.", "libretro",
    ];
    if BAD.iter().any(|b| n.contains(b)) {
        return None;
    }
    let windows = ["win", "msvc", "mingw", "w64"].iter().any(|k| n.contains(k));
    if !windows {
        return None;
    }
    let mut s = 10;
    if n.contains("x64") || n.contains("x86_64") || n.contains("win64") || n.contains("amd64") {
        s += 3;
    }
    if n.contains("msvc") {
        s += 1;
    }
    if n.contains("portable") {
        s += 1;
    }
    if n.contains("installer") || n.contains("setup") {
        s -= 4;
    }
    Some(s)
}

pub fn pick_asset<'r>(release: &'r Release, pattern: &str) -> Result<&'r Asset> {
    if pattern == "auto" {
        return release
            .assets
            .iter()
            .filter_map(|a| windows_score(&a.name).map(|s| (s, a)))
            .max_by_key(|(s, _)| *s)
            .map(|(_, a)| a)
            .with_context(|| format!("release {} has no Windows download", release.tag_name));
    }
    let re = regex::RegexBuilder::new(pattern).case_insensitive(true).build()?;
    release
        .assets
        .iter()
        .find(|a| re.is_match(&a.name))
        .with_context(|| format!("release {} has no Windows download matching {pattern}", release.tag_name))
}

/// Stream a download to `dest`, reporting (bytes so far, total).
pub fn download(url: &str, dest: &Path, progress: &dyn Fn(u64, u64)) -> Result<()> {
    let resp = agent().get(url).call().with_context(|| format!("downloading {url}"))?;
    let total: u64 = resp.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);
    let tmp = dest.with_extension("part");
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut out = std::fs::File::create(&tmp)?;
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    let mut last = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        if done - last > (1 << 20) {
            progress(done, total);
            last = done;
        }
    }
    out.flush()?;
    drop(out);
    if total > 0 && done != total {
        bail!("download ended early ({done} of {total} bytes)");
    }
    std::fs::rename(&tmp, dest)?;
    progress(done, total.max(done));
    Ok(())
}
