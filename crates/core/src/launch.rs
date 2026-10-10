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
            let def = cat.emulators.iter().find(|d| id == d.id || id.starts_with(&format!("{}-", d.id)));
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
            bail!("{} is missing. Reinstall it from Ports.", exe.display());
        }
        let cwd = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        return Ok(Plan { program: exe, args: vec![], cwd });
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
        bail!("{} was not found. Rescan or set its location in Emulators.", emu.exe);
    }
    let rom = PathBuf::from(&game.launch_path);
    let rom_dir = rom.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let emu_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let args = emu
        .args
        .iter()
        .map(|a| {
            a.replace("{rom}", &game.launch_path)
                .replace("{rom_dir}", &rom_dir)
                .replace("{emu_dir}", &emu_dir.to_string_lossy())
        })
        .collect();
    Ok(Plan { program: exe, args, cwd: emu_dir })
}

pub fn spawn(plan: &Plan) -> Result<Child> {
    Command::new(&plan.program)
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .spawn()
        .with_context(|| format!("starting {}", plan.program.display()))
}
