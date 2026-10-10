//! Starting games. Arguments are passed as a real argv (one element per token) straight to
//! CreateProcess, never through `cmd /c`, so paths with spaces, `&` or quotes are safe.

use crate::catalog::Catalog;
use crate::db::Db;
use crate::model::{Emulator, Game};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

#[derive(Debug, Clone)]
pub struct Plan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

pub fn emulators(db: &Db, cat: &Catalog, scan_cutoff: i64) -> Result<Vec<Emulator>> {
    Ok(db
        .emulator_rows()?
        .into_iter()
        .map(|(id, name, exe, version, args, managed, seen_at)| {
            let def = cat.emulator_for_instance(&id);
            let default_args = def.map(|d| d.args.clone()).unwrap_or_else(|| vec!["{rom}".into()]);
            let args = args.and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_else(|| default_args.clone());
            Emulator {
                missing: seen_at < scan_cutoff || !Path::new(&exe).is_file(),
                platforms: def.map(|d| d.platforms.clone()).unwrap_or_default(),
                notes: def.and_then(|d| d.notes.clone()),
                id,
                name,
                exe,
                version,
                args,
                default_args,
                managed,
            }
        })
        .collect())
}

pub fn plan(db: &Db, cat: &Catalog, game: &Game) -> Result<Plan> {
    if game.kind == "port" {
        let exe = PathBuf::from(&game.launch_path);
        if !exe.is_file() {
            bail!("The program for {} is missing. Reinstall it from Ports.", game.title);
        }
        let cwd = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let dir = cwd.to_string_lossy();
        let args = game.user_args.as_deref().map(|a| fill(a, &game.launch_path, &dir, &dir)).unwrap_or_default();
        return Ok(Plan { program: exe, args, cwd });
    }
    // A PS5 game converted with AnyPS5 runs as its own Windows program.
    if let Some(exe) = crate::anyps5::converted(db, game.id) {
        let cwd = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let args = game.user_args.clone().unwrap_or_default();
        return Ok(Plan { program: exe, args, cwd });
    }
    if game.packed {
        bail!("This game is still zipped. Use Unpack first.");
    }
    if game.missing {
        bail!("The game files are no longer where they were found. Rescan your library.");
    }
    let emu_id = game.emulator.as_deref().context("No emulator for this platform yet. Add one in Emulators.")?;
    let emu = emulators(db, cat, 0)?.into_iter().find(|e| e.id == emu_id).context("That emulator is no longer registered.")?;
    let exe = PathBuf::from(&emu.exe);
    if !exe.is_file() {
        bail!("{} is no longer where it was found. Rescan, or set where it is in Emulators.", emu.name);
    }
    let rom = PathBuf::from(&game.launch_path);
    let rom_dir = rom.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let emu_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    // The game's own launch options win over the emulator's.
    let template = game.user_args.as_deref().unwrap_or(&emu.args);
    let args = fill(template, &game.launch_path, &rom_dir, &emu_dir.to_string_lossy());
    Ok(Plan { program: exe, args, cwd: emu_dir })
}

/// Fill `{rom}`, `{rom_dir}` and `{emu_dir}` into each argv token, in one pass, so a path that
/// happens to contain a placeholder is never expanded again.
fn fill(template: &[String], rom: &str, rom_dir: &str, emu_dir: &str) -> Vec<String> {
    static PLACEHOLDER: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\{(rom|rom_dir|emu_dir)\}").unwrap());
    template
        .iter()
        .map(|a| {
            PLACEHOLDER
                .replace_all(a, |c: &regex::Captures| match &c[1] {
                    "rom" => rom.to_string(),
                    "rom_dir" => rom_dir.to_string(),
                    _ => emu_dir.to_string(),
                })
                .into_owned()
        })
        .collect()
}

pub fn spawn(plan: &Plan) -> Result<Child> {
    Command::new(&plan.program)
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .spawn()
        .with_context(|| format!("starting {}", plan.program.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::GameUpsert;
    use crate::model::Copy;
    use crate::util::{TempDir, path_str};

    #[test]
    fn game_options_replace_the_emulators() {
        let t = TempDir::new("launch");
        let exe = t.file("PCSX2/pcsx2-qt.exe", 10);
        let iso = t.file("Games/{rom} Okami (USA).iso", 10);
        let cat = Catalog::load(None).unwrap();
        let db = Db::open(Path::new(":memory:")).unwrap();
        db.upsert_emulator("pcsx2", "PCSX2", &path_str(&exe), Some("2.2.0"), false, 1).unwrap();
        let up = GameUpsert {
            key: "ps2:okami".into(),
            title: "Okami".into(),
            platform: "ps2".into(),
            kind: "rom".into(),
            launch_path: path_str(&iso),
            emulator: Some("pcsx2".into()),
            app_id: None,
            serial: None,
            region: None,
            size: 10,
            packed: false,
        };
        let (id, _) = db.upsert_game(&up, 1).unwrap();
        let copy = Copy { path: path_str(&iso), format: "iso".into(), size: 10, archive: false, primary: true, registered: false };
        db.replace_copies("ps2:okami", &[copy]).unwrap();

        let p = plan(&db, &cat, &db.game(id).unwrap()).unwrap();
        assert_eq!(p.program, exe);
        assert_eq!(p.args, vec!["-batch".to_string(), "-fullscreen".into(), "--".into(), path_str(&iso)]);

        db.set_game_args(id, Some(&["-nogui".into(), "{rom}".into(), "--dir={rom_dir}".into(), "{emu_dir}".into()])).unwrap();
        let p = plan(&db, &cat, &db.game(id).unwrap()).unwrap();
        let rom_dir = path_str(iso.parent().unwrap());
        let emu_dir = path_str(exe.parent().unwrap());
        assert_eq!(p.args, vec!["-nogui".to_string(), path_str(&iso), format!("--dir={rom_dir}"), emu_dir.clone()]);
        assert_eq!(path_str(&p.cwd), emu_dir);
    }
}
