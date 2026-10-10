//! Setup checks: files an emulator needs that only the player can supply (BIOS, firmware,
//! keys). Each check is described in `data/emulators.json`; this module looks for the file
//! where the emulator would, including folders that the emulator's own settings point at.

use crate::catalog::{Catalog, CheckDef, EmulatorDef, SettingRef};
use crate::model::Emulator;
use crate::paths;
use crate::util::path_str;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Text files bigger than this are never read for `contains` rules.
const MAX_TEXT: u64 = 1 << 20;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// What the file is, in plain words ("PS2 BIOS").
    pub what: String,
    pub ok: bool,
    /// The emulator works without it.
    pub optional: bool,
    /// One plain sentence on how to get it.
    pub help: String,
    /// The folder the file was found in or belongs in, when that folder exists. Meant for an
    /// "open folder" button, not for showing as text.
    pub folder: Option<String>,
}

/// Run every check of an emulator found at `exe_path`. Emulators with no requirements return
/// an empty list.
pub fn run_checks(def: &EmulatorDef, exe_path: &Path) -> Vec<CheckResult> {
    run_checks_with(def, exe_path, &|k| std::env::var(k).ok())
}

/// The checks of one emulator on the PC.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorChecks {
    /// Emulator id as in `launch::emulators` ("pcsx2", "pcsx2-2").
    pub id: String,
    pub checks: Vec<CheckResult>,
}

/// Checks for every emulator in `emus` that has any (missing emulators are left out).
pub fn emulator_checks(cat: &Catalog, emus: &[Emulator]) -> Vec<EmulatorChecks> {
    emus.iter()
        .filter(|e| !e.missing)
        .filter_map(|e| {
            let def = cat.emulator_for_instance(&e.id)?;
            let checks = run_checks(def, Path::new(&e.exe));
            (!checks.is_empty()).then(|| EmulatorChecks { id: e.id.clone(), checks })
        })
        .collect()
}

/// True when nothing required is missing.
pub fn ready(results: &[CheckResult]) -> bool {
    results.iter().all(|r| r.ok || r.optional)
}

fn run_checks_with(def: &EmulatorDef, exe_path: &Path, env: &dyn Fn(&str) -> Option<String>) -> Vec<CheckResult> {
    let emu_dir = exe_path.parent().filter(|p| !p.as_os_str().is_empty());
    def.checks.iter().map(|c| run_one(c, emu_dir, env)).collect()
}

fn run_one(c: &CheckDef, emu_dir: Option<&Path>, env: &dyn Fn(&str) -> Option<String>) -> CheckResult {
    let rule = Rule::new(c);
    // Candidate folders, best first: where the emulator's settings point, then its usual folders.
    let mut folders: Vec<PathBuf> = Vec::new();
    let mut found: Option<PathBuf> = None;
    let consider = |files: Vec<PathBuf>, found: &mut Option<PathBuf>| {
        if found.is_none() {
            *found = files.into_iter().find(|f| rule.accepts(f));
        }
    };

    for s in &c.settings {
        for cfg in paths::expand_with(&s.file, emu_dir, env) {
            let Some(target) = setting_target(&cfg, s) else { continue };
            match &s.glob {
                Some(pattern) => {
                    let files = glob(&target, pattern);
                    folders.push(target);
                    consider(files, &mut found);
                }
                None => {
                    folders.extend(target.parent().map(Path::to_path_buf));
                    consider(vec![target], &mut found);
                }
            }
        }
    }
    for template in &c.paths {
        for path in paths::expand_with(template, emu_dir, env) {
            let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else { continue };
            let name = name.to_string_lossy();
            let files = if name.contains(['*', '?']) { glob(dir, &name) } else { vec![path.clone()] };
            folders.push(dir.to_path_buf());
            consider(files, &mut found);
        }
    }

    let folder = match &found {
        Some(f) => f.parent().map(Path::to_path_buf),
        None => folders.into_iter().find(|f| f.is_dir()),
    };
    CheckResult {
        what: c.what.clone(),
        ok: found.is_some(),
        optional: c.optional,
        help: c.help.clone(),
        folder: folder.map(|f| path_str(&f)),
    }
}

/// What a file must look like to count.
struct Rule {
    min_size: u64,
    /// `Some(None)`: the catalog's regex is invalid, so nothing can pass.
    contains: Option<Option<Regex>>,
    ignore: Option<Regex>,
}

impl Rule {
    fn new(c: &CheckDef) -> Rule {
        let re = |s: &str| RegexBuilder::new(s).case_insensitive(true).build().ok();
        Rule { min_size: c.min_size.max(1), contains: c.contains.as_deref().map(re), ignore: c.ignore.as_deref().and_then(re) }
    }

    fn accepts(&self, f: &Path) -> bool {
        let Ok(meta) = std::fs::metadata(f) else { return false };
        if !meta.is_file() || meta.len() < self.min_size {
            return false;
        }
        match &self.contains {
            None => true,
            Some(None) => false,
            Some(Some(re)) => {
                if meta.len() > MAX_TEXT {
                    return false;
                }
                let Ok(bytes) = std::fs::read(f) else { return false };
                String::from_utf8_lossy(&bytes)
                    .lines()
                    .any(|l| re.is_match(l) && !self.ignore.as_ref().is_some_and(|i| i.is_match(l)))
            }
        }
    }
}

/// The file or folder a setting names, resolved to a full path. `None` when the config file is
/// missing or the setting is not there or empty.
fn setting_target(cfg: &Path, s: &SettingRef) -> Option<PathBuf> {
    let meta = std::fs::metadata(cfg).ok()?;
    if !meta.is_file() || meta.len() > MAX_TEXT {
        return None;
    }
    let text = String::from_utf8_lossy(&std::fs::read(cfg).ok()?).to_string();
    let value = read_setting(&text, s.section.as_deref(), &s.key)?;
    let value = PathBuf::from(value);
    let full = if value.is_absolute() {
        value
    } else {
        cfg.parent()?.join(s.base.as_deref().unwrap_or(".")).join(value)
    };
    Some(paths::clean(&full))
}

/// Find `key = value` in INI or TOML text, optionally only inside `[section]`. Quoted TOML
/// strings are unquoted ('literal' as is, "basic" with `\\` and `\"` unescaped).
fn read_setting(text: &str, section: Option<&str>, key: &str) -> Option<String> {
    let mut current = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            current = line.trim_matches(['[', ']']).trim().trim_matches('"').to_string();
            continue;
        }
        if section.is_some_and(|s| !s.eq_ignore_ascii_case(&current)) {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        if !k.trim().trim_matches('"').eq_ignore_ascii_case(key) {
            continue;
        }
        let v = unquote(v.trim());
        if !v.is_empty() {
            return Some(v);
        }
    }
    None
}

fn unquote(v: &str) -> String {
    if let Some(rest) = v.strip_prefix('\'') {
        return rest.split('\'').next().unwrap_or_default().to_string();
    }
    let Some(rest) = v.strip_prefix('"') else { return v.to_string() };
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            },
            c => out.push(c),
        }
    }
    out
}

/// Files directly in `dir` whose names match `pattern` (`*` and `?`, any case), sorted.
fn glob(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| wildcard(&pattern, &e.file_name().to_string_lossy().to_lowercase().chars().collect::<Vec<_>>()))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

fn wildcard(p: &[char], s: &[char]) -> bool {
    // Iterative matcher with backtracking to the last `*`.
    let (mut pi, mut si) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, si));
            pi += 1;
        } else if let Some((sp, ss)) = star {
            pi = sp + 1;
            si = ss + 1;
            star = Some((sp, ss + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::util::TempDir;

    /// Environment pointing every user folder into the temp dir, so the real PC never leaks in.
    fn fake_env(home: PathBuf) -> impl Fn(&str) -> Option<String> {
        move |k| match k {
            "USERPROFILE" => Some(path_str(&home)),
            "APPDATA" => Some(path_str(&home.join("AppData").join("Roaming"))),
            "LOCALAPPDATA" => Some(path_str(&home.join("AppData").join("Local"))),
            _ => None,
        }
    }

    fn def(id: &str) -> EmulatorDef {
        Catalog::load(None).unwrap().emulator(id).unwrap().clone()
    }

    #[test]
    fn wildcards() {
        let m = |p: &str, s: &str| wildcard(&p.chars().collect::<Vec<_>>(), &s.chars().collect::<Vec<_>>());
        assert!(m("*.bin", "scph1001.bin"));
        assert!(m("*.bin", ".bin"));
        assert!(!m("*.bin", "scph1001.bin.zip"));
        assert!(m("bios?.bin", "bios7.bin"));
        assert!(m("*", "anything"));
        assert!(m("a*b*c", "aXXbYYc"));
        assert!(!m("a*b*c", "aXXbYY"));
    }

    #[test]
    fn ps2_bios_beside_the_emulator() {
        let t = TempDir::new("checks-ps2");
        let env = fake_env(t.path().join("home"));
        let exe = t.file("PCSX2/pcsx2-qt.exe", 10);
        let pcsx2 = def("pcsx2");

        let r = run_checks_with(&pcsx2, &exe, &env);
        assert_eq!(r.len(), 1);
        assert!(!r[0].ok && !r[0].optional && !ready(&r));
        assert_eq!(r[0].folder, None, "no bios folder exists yet");

        // Too small to be a BIOS.
        t.file("PCSX2/bios/fake.bin", 1024);
        let r = run_checks_with(&pcsx2, &exe, &env);
        assert!(!r[0].ok);
        assert_eq!(r[0].folder.as_deref(), Some(path_str(&t.path().join("PCSX2").join("bios")).as_str()));

        t.file("PCSX2/bios/SCPH-70012.BIN", 4 << 20);
        let r = run_checks_with(&pcsx2, &exe, &env);
        assert!(r[0].ok && ready(&r));
    }

    #[test]
    fn ps2_bios_in_the_folder_pcsx2_settings_name() {
        // A real portable layout: `Bios = bios\PS2_BIOS`, relative to the folder above inis\.
        let t = TempDir::new("checks-ps2-ini");
        let env = fake_env(t.path().join("home"));
        let exe = t.file("PCSX2/pcsx2-qt.exe", 10);
        t.write("PCSX2/inis/PCSX2.ini", b"[UI]\r\nBios = nowhere\r\n[Folders]\r\nBios = bios\\PS2_BIOS\r\nMemoryCards = memcards\r\n");
        t.file("PCSX2/bios/PS2_BIOS/ps2-0230a-20080220.bin", 4 << 20);
        let r = run_checks_with(&def("pcsx2"), &exe, &env);
        assert!(r[0].ok);
        let want = t.path().join("PCSX2").join("bios").join("PS2_BIOS");
        assert_eq!(r[0].folder.as_deref(), Some(path_str(&want).as_str()));
    }

    #[test]
    fn ps1_bios_in_documents_or_local_appdata() {
        let t = TempDir::new("checks-ps1");
        let home = t.path().join("home");
        let env = fake_env(home.clone());
        let exe = t.file("DuckStation/duckstation-qt-x64-ReleaseLTCG.exe", 10);
        assert!(!run_checks_with(&def("duckstation"), &exe, &env)[0].ok);
        t.file("home/AppData/Local/DuckStation/bios/scph5501.bin", 512 * 1024);
        assert!(run_checks_with(&def("duckstation"), &exe, &env)[0].ok);
    }

    #[test]
    fn ps3_firmware() {
        let t = TempDir::new("checks-ps3");
        let env = fake_env(t.path().join("home"));
        let exe = t.file("RPCS3/rpcs3.exe", 10);
        assert!(!run_checks_with(&def("rpcs3"), &exe, &env)[0].ok);
        t.file("RPCS3/dev_flash/vsh/module/vsh.self", 4096);
        assert!(run_checks_with(&def("rpcs3"), &exe, &env)[0].ok);
    }

    #[test]
    fn checks_for_every_emulator_found() {
        let t = TempDir::new("checks-all");
        let emu = |id: &str, exe: &Path, missing: bool| Emulator {
            id: id.into(),
            name: id.into(),
            exe: path_str(exe),
            version: None,
            platforms: vec![],
            args: vec![],
            default_args: vec![],
            managed: false,
            missing,
            notes: None,
        };
        let rpcs3 = t.file("RPCS3/rpcs3.exe", 10);
        t.file("RPCS3/dev_flash/vsh/module/vsh.self", 4096);
        let second = t.file("RPCS3 old/rpcs3.exe", 10);
        let ppsspp = t.file("PPSSPP/PPSSPPWindows64.exe", 10);
        let all = [emu("rpcs3", &rpcs3, false), emu("rpcs3-2", &second, false), emu("ppsspp", &ppsspp, false), emu("pcsx2", &ppsspp, true)];
        let r = emulator_checks(&Catalog::load(None).unwrap(), &all);
        let summary: Vec<(&str, bool)> = r.iter().map(|e| (e.id.as_str(), e.checks[0].ok)).collect();
        assert_eq!(summary, [("rpcs3", true), ("rpcs3-2", false)], "no checks for PPSSPP, none for missing emulators");
    }

    #[test]
    fn cemu_keys_need_a_real_key() {
        let t = TempDir::new("checks-cemu");
        let env = fake_env(t.path().join("home"));
        let exe = t.file("Cemu/Cemu.exe", 10);
        // The file Cemu writes on first start only holds its example key.
        let template = "# this file contains keys needed for decryption of disc file system data (WUD/WUX)\r\n\
                        # 1 key per line, any text after a '#' character is considered a comment\r\n\
                        541b9889519b27d363cd21604b97c67a # example key (can be deleted)\r\n";
        t.write("home/AppData/Roaming/Cemu/keys.txt", template.as_bytes());
        let r = run_checks_with(&def("cemu"), &exe, &env);
        assert!(!r[0].ok && r[0].optional && ready(&r));
        t.write("home/AppData/Roaming/Cemu/keys.txt", format!("{template}0123456789ABCDEF0123456789abcdef # Mario Kart 8\r\n").as_bytes());
        assert!(run_checks_with(&def("cemu"), &exe, &env)[0].ok);
    }

    #[test]
    fn xemu_files_come_from_its_settings() {
        let t = TempDir::new("checks-xemu");
        let env = fake_env(t.path().join("home"));
        let exe = t.file("xemu/xemu.exe", 10);
        let mcpx = t.file("files/mcpx_1.0.bin", 512);
        let flash = t.file("files/Complex_4627.bin", 1 << 20);
        let toml = format!(
            "[general]\nbootrom_path = 'C:\\nope.bin'\n\n[sys.files]\nbootrom_path = '{}'\nflashrom_path = \"{}\"\nhdd_path = ''\n",
            path_str(&mcpx),
            path_str(&flash).replace('\\', "\\\\"),
        );
        t.write("home/AppData/Roaming/xemu/xemu/xemu.toml", toml.as_bytes());
        let r = run_checks_with(&def("xemu"), &exe, &env);
        let ok: Vec<bool> = r.iter().map(|c| c.ok).collect();
        assert_eq!(ok, vec![true, true, false], "boot ROM and BIOS found, no hard drive image set");
        assert!(!ready(&r));
    }

    #[test]
    fn optional_files_never_block() {
        let t = TempDir::new("checks-ds");
        let env = fake_env(t.path().join("home"));
        let exe = t.file("melonDS/melonDS.exe", 10);
        let r = run_checks_with(&def("melonds"), &exe, &env);
        assert!(!r.is_empty() && r.iter().all(|c| c.optional && !c.ok));
        assert!(ready(&r));
        t.file("melonDS/bios7.bin", 16 * 1024);
        assert!(run_checks_with(&def("melonds"), &exe, &env).iter().any(|c| c.ok));
        // Emulators without requirements have no checks at all.
        assert!(run_checks_with(&def("ppsspp"), &exe, &env).is_empty());
    }

    #[test]
    fn settings_parsing() {
        let ini = "[BIOS]\nSearchDirectory = bios\n[Main]\nSearchDirectory = other\n";
        assert_eq!(read_setting(ini, Some("bios"), "SearchDirectory").as_deref(), Some("bios"));
        assert_eq!(read_setting(ini, Some("Main"), "searchdirectory").as_deref(), Some("other"));
        assert_eq!(read_setting(ini, Some("Missing"), "SearchDirectory"), None);
        let toml = "[DS]\nBIOS9Path = \"C:\\\\DS\\\\bios9.bin\"\n[DSi]\nBIOS7Path = 'x'\n";
        assert_eq!(read_setting(toml, Some("DS"), "BIOS9Path").as_deref(), Some("C:\\DS\\bios9.bin"));
        assert_eq!(read_setting(toml, Some("DS"), "BIOS7Path"), None);
        assert_eq!(read_setting("gba.bios=C:/GBA/gba_bios.bin\n", None, "gba.bios").as_deref(), Some("C:/GBA/gba_bios.bin"));
    }

    #[test]
    fn catalog_checks_are_well_formed() {
        let cat = Catalog::load(None).unwrap();
        let env = |k: &str| Some(format!("C:\\{k}"));
        let emu = Path::new("C:\\Emu");
        let mut seen = 0;
        for e in &cat.emulators {
            for c in &e.checks {
                seen += 1;
                assert!(!c.what.is_empty() && !c.help.is_empty(), "{}: every check needs a name and help", e.id);
                assert!(!c.paths.is_empty() || !c.settings.is_empty(), "{}: {} has nowhere to look", e.id, c.what);
                for p in c.paths.iter().chain(c.settings.iter().map(|s| &s.file)) {
                    assert!(!paths::expand_with(p, Some(emu), &env).is_empty(), "{}: bad template {p}", e.id);
                    let parent = Path::new(p).parent().map(path_str).unwrap_or_default();
                    assert!(!parent.contains(['*', '?']), "{}: wildcards only in the last part of {p}", e.id);
                }
                for re in [&c.contains, &c.ignore].into_iter().flatten() {
                    assert!(Regex::new(re).is_ok(), "{}: bad regex {re}", e.id);
                }
            }
            for p in &e.saves {
                assert!(!paths::expand_with(p, Some(emu), &env).is_empty(), "{}: bad save template {p}", e.id);
            }
        }
        assert!(seen >= 8);
    }
}
