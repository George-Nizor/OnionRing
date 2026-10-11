//! AnyPS5 (github.com/boykopovar/AnyPS5): turns a PS5 game's executable into a native Windows
//! program, with no emulator. OnionRing installs its relinker and Windows system libraries under
//! `Apps/anyps5/<version>/`, converts a game you dumped yourself into `Apps/ps5-<title>/`, and
//! from then on Play starts that program directly.
//!
//! Layout of a converted game (from AnyPS5's docs/user/USAGE.md):
//! ```text
//! app.exe
//! libs/        AnyPS5's Windows system libraries (*.prx plus the mingw runtime DLLs)
//! app0/        the game's files (linked to the dump, never copied or changed)
//!   sce_module/  modules converted by the relinker
//! ```

use crate::{github, install, paths::DataRoot, util};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const REPO: &str = "boykopovar/AnyPS5";
const KV_TOOL: &str = "anyps5:version";
const MODULE_DIRS: [&str; 3] = ["sce_module", "sce_modules", "prx"];

/// Where a converted game's program is recorded, per game.
pub fn kv_key(game_id: i64) -> String {
    format!("anyps5:game:{game_id}")
}

fn tool_dir(root: &DataRoot) -> PathBuf {
    root.apps().join("anyps5").join("current")
}

pub fn relinker(root: &DataRoot) -> Option<PathBuf> {
    let p = tool_dir(root).join("relinker.exe");
    p.is_file().then_some(p)
}

/// The installed AnyPS5 version, if any.
pub fn installed(db: &crate::db::Db, root: &DataRoot) -> Option<String> {
    relinker(root)?;
    db.kv_get(KV_TOOL).ok().flatten()
}

fn fetch(rel: &github::Release, pattern: &str, dir: &Path, progress: &dyn Fn(&str)) -> Result<PathBuf> {
    let asset = github::pick_asset(rel, pattern)?;
    let dest = dir.join(&asset.name);
    progress(&format!("Downloading {}", asset.name));
    github::download(&asset.browser_download_url, &dest, &|_, _| {})?;
    if let Some(want) = rel.sha256_of(asset)
        && util::sha256_file(&dest)? != want
    {
        let _ = std::fs::remove_file(&dest);
        bail!("{} did not match the checksum GitHub lists. Try again.", asset.name);
    }
    Ok(dest)
}

/// Install (or update to) the latest AnyPS5 release: the relinker and the Windows libraries.
pub fn install_latest(db: &crate::db::Db, root: &DataRoot, progress: &dyn Fn(&str)) -> Result<String> {
    progress("Looking up the latest AnyPS5");
    let rel = github::latest(root, REPO, false)?;
    let version = rel.tag_name.clone();
    let vdir = root.apps().join("anyps5").join(install::safe_version(&version));
    let dl = root.downloads().join("anyps5").join(install::safe_version(&version));
    std::fs::create_dir_all(&dl)?;
    let exe = fetch(&rel, r"^relinker.*\.exe$", &dl, progress)?;
    let zip = fetch(&rel, r"^prx-windows.*\.zip$", &dl, progress)?;

    progress("Unpacking");
    let unpacked = dl.join("prx");
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked)?;
    install::extract(&zip, &unpacked)?;
    // The archive may hold the libraries at its top level or inside a folder.
    let libs_src = walkdir::WalkDir::new(&unpacked)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("prx")))
        .and_then(|e| e.path().parent().map(Path::to_path_buf))
        .context("The AnyPS5 download has no system libraries in it.")?;

    let _ = std::fs::remove_dir_all(&vdir);
    std::fs::create_dir_all(&vdir)?;
    std::fs::copy(&exe, vdir.join("relinker.exe"))?;
    std::fs::rename(&libs_src, vdir.join("libs")).or_else(|_| copy_dir(&libs_src, &vdir.join("libs")))?;
    let current = tool_dir(root);
    if current.exists() || junction::exists(&current).unwrap_or(false) {
        let _ = junction::delete(&current);
        let _ = std::fs::remove_dir(&current);
    }
    junction::create(&vdir, &current).context("pointing current at the new AnyPS5")?;
    let _ = std::fs::remove_dir_all(&dl);
    db.kv_set(KV_TOOL, &version)?;
    Ok(version)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for e in walkdir::WalkDir::new(from).into_iter().filter_map(|e| e.ok()) {
        let rel = e.path().strip_prefix(from)?;
        let dest = to.join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            std::fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

/// Convert a dumped PS5 game (the folder holding `eboot.bin` and `sce_sys/`) into a native
/// Windows program. Returns the program's path. The dump itself is never modified.
pub fn convert(db: &crate::db::Db, root: &DataRoot, game: &crate::model::Game, progress: &dyn Fn(&str)) -> Result<PathBuf> {
    if game.platform != "ps5" {
        bail!("Only PS5 games can be converted with AnyPS5.");
    }
    if relinker(root).is_none() {
        install_latest(db, root, progress)?;
    }
    let relinker = relinker(root).context("AnyPS5 did not install.")?;
    let eboot = PathBuf::from(&game.launch_path);
    let dump = eboot.parent().context("The game's folder is missing.")?.to_path_buf();
    if !eboot.is_file() {
        bail!("The game's files are no longer where they were found. Rescan your library.");
    }

    let name = game.serial.clone().unwrap_or_else(|| util::short_hash(&game.key));
    let dest = root.apps().join(format!("ps5-{}", name.replace(|c: char| !c.is_ascii_alphanumeric(), "-")));
    // Build next to the current program and swap it in only when the conversion worked, so a
    // failed reconversion (after an AnyPS5 update) leaves the game playable.
    let out = dest.with_file_name(format!("{}.new", dest.file_name().unwrap_or_default().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&out);
    let app0 = out.join("app0");
    std::fs::create_dir_all(&app0)?;

    // app0 mirrors the dump through links, except the module folder the relinker writes into.
    progress("Linking the game's files");
    for e in std::fs::read_dir(&dump)? {
        let e = e?;
        let fname = e.file_name();
        let lower = fname.to_string_lossy().to_ascii_lowercase();
        if MODULE_DIRS.contains(&lower.as_str()) || lower == "eboot.bin" {
            continue;
        }
        let dest = app0.join(&fname);
        if e.file_type()?.is_dir() {
            junction::create(e.path(), &dest).with_context(|| format!("linking {}", fname.to_string_lossy()))?;
        } else if std::fs::hard_link(e.path(), &dest).is_err() {
            std::fs::copy(e.path(), &dest)?;
        }
    }
    junction::create(tool_dir(root).join("libs"), out.join("libs")).context("linking AnyPS5's libraries")?;

    progress("Converting the game (this can take a few minutes)");
    let exe = out.join("app.exe");
    let mut cmd = Command::new(&relinker);
    cmd.arg("--windows").arg("--windows-gui").arg(&eboot).arg(&exe).current_dir(&out);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let res = cmd.output().context("starting AnyPS5's relinker")?;
    std::fs::write(out.join("anyps5-convert.log"), [res.stdout.as_slice(), res.stderr.as_slice()].concat())?;
    if !res.status.success() || !exe.is_file() {
        let err = String::from_utf8_lossy(&res.stderr);
        let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no details").trim();
        let msg = format!("AnyPS5 could not convert {}: {line}", game.title);
        // Keep the log where it can be read; drop the half-built folder.
        let _ = std::fs::copy(out.join("anyps5-convert.log"), root.apps().join(format!("{}-failed.log", dest.file_name().unwrap_or_default().to_string_lossy())));
        let _ = std::fs::remove_dir_all(&out);
        bail!(msg);
    }
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&out, &dest).context("replacing the converted game")?;
    let exe = dest.join("app.exe");
    db.kv_set(&kv_key(game.id), &exe.to_string_lossy())?;
    if let Some(v) = installed(db, root) {
        db.kv_set(&built_key(game.id), &v)?;
    }
    Ok(exe)
}

fn built_key(game_id: i64) -> String {
    format!("anyps5:built:{game_id}")
}

/// Install the newest AnyPS5 release when it is newer than the installed one. Returns the new
/// version when it updated. Does nothing when AnyPS5 was never installed.
pub fn update_if_newer(db: &crate::db::Db, root: &DataRoot, progress: &dyn Fn(&str)) -> Result<Option<String>> {
    let Some(have) = installed(db, root) else { return Ok(None) };
    let latest = github::latest(root, REPO, false)?;
    if latest.tag_name == have {
        return Ok(None);
    }
    install_latest(db, root, progress).map(Some)
}

/// A converted game was made with an older AnyPS5 than the one installed now, so it should be
/// converted again before it is played (each AnyPS5 release fixes and adds a lot).
pub fn stale(db: &crate::db::Db, root: &DataRoot, game_id: i64) -> bool {
    let (Some(_), Some(now)) = (converted(db, game_id), installed(db, root)) else { return false };
    db.kv_get(&built_key(game_id)).ok().flatten().as_deref() != Some(now.as_str())
}

/// The converted program for a game, when one exists.
pub fn converted(db: &crate::db::Db, game_id: i64) -> Option<PathBuf> {
    let p = PathBuf::from(db.kv_get(&kv_key(game_id)).ok().flatten()?);
    p.is_file().then_some(p)
}
