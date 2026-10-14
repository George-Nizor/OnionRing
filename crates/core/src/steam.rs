//! Finding games installed through Steam, so mashups can be added to games the player already
//! owns. Read only: Steam's own files say where each library is and which folder a game is in.
//!
//! ```text
//! <Steam>\steamapps\libraryfolders.vdf        "path" of every library
//! <library>\steamapps\appmanifest_<id>.acf    "installdir" of one game
//! <library>\steamapps\common\<installdir>\    the game
//! ```

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Where Steam is installed: from the registry, else its default folder.
pub fn steam_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let user = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").and_then(|k| k.get_value::<String, _>("SteamPath"));
        let machine = || RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam").and_then(|k| k.get_value::<String, _>("InstallPath"));
        if let Some(p) = user.or_else(|_| machine()).ok().map(PathBuf::from).filter(|p| p.is_dir()) {
            return Some(p);
        }
    }
    let default = PathBuf::from(r"C:\Program Files (x86)\Steam");
    default.is_dir().then_some(default)
}

/// `"key"  "value"` pairs from a Valve KeyValues file, values unescaped (`\\` -> `\`).
fn values(text: &str, key: &str) -> Vec<String> {
    static PAIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]+)"\s+"((?:[^"\\]|\\.)*)""#).unwrap());
    PAIR.captures_iter(text)
        .filter(|c| c[1].eq_ignore_ascii_case(key))
        .map(|c| c[2].replace(r"\\", r"\").replace(r#"\""#, "\""))
        .collect()
}

/// Every Steam library folder (the Steam folder itself is always one).
pub fn libraries(steam: &Path) -> Vec<PathBuf> {
    let mut out = vec![steam.to_path_buf()];
    if let Ok(text) = std::fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        for p in values(&text, "path").into_iter().map(PathBuf::from) {
            if !out.iter().any(|o| o.to_string_lossy().eq_ignore_ascii_case(&p.to_string_lossy())) {
                out.push(p);
            }
        }
    }
    out
}

/// The folder of an installed game in any of these libraries.
pub fn app_dir_in(libraries: &[PathBuf], app_id: u32) -> Option<PathBuf> {
    libraries.iter().find_map(|lib| {
        let apps = lib.join("steamapps");
        let text = std::fs::read_to_string(apps.join(format!("appmanifest_{app_id}.acf"))).ok()?;
        let dir = apps.join("common").join(values(&text, "installdir").into_iter().next()?);
        dir.is_dir().then_some(dir)
    })
}

/// The folder of an installed Steam game, by its app id.
pub fn app_dir(app_id: u32) -> Option<PathBuf> {
    app_dir_in(&libraries(&steam_dir()?), app_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::TempDir;

    #[test]
    fn finds_games_in_any_library() {
        let t = TempDir::new("steam");
        let steam = t.path().join("Steam");
        let lib2 = t.path().join("Games Lib");
        let vdf = format!(
            "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"489830\"\t\t\"1\"\n\t\t}}\n\t}}\n}}\n",
            steam.display().to_string().replace('\\', r"\\"),
            lib2.display().to_string().replace('\\', r"\\"),
        );
        t.write("Steam/steamapps/libraryfolders.vdf", vdf.as_bytes());
        t.write("Games Lib/steamapps/appmanifest_489830.acf", b"\"AppState\"\n{\n\t\"appid\"\t\t\"489830\"\n\t\"installdir\"\t\t\"Skyrim Special Edition\"\n}\n");
        t.file("Games Lib/steamapps/common/Skyrim Special Edition/SkyrimSE.exe", 10);

        let libs = libraries(&steam);
        assert_eq!(libs.len(), 2, "{libs:?}");
        assert_eq!(app_dir_in(&libs, 489830), Some(lib2.join("steamapps").join("common").join("Skyrim Special Edition")));
        // Not installed: no manifest.
        assert_eq!(app_dir_in(&libs, 1245620), None);
    }
}
