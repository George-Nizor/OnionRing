use serde::{Deserialize, Serialize};

/// A game as the UI sees it.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub key: String,
    pub title: String,
    pub platform: String,
    /// "rom" (played through an emulator) or "port" (a native app installed by OnionRing).
    pub kind: String,
    pub launch_path: String,
    pub emulator: Option<String>,
    pub app_id: Option<String>,
    pub serial: Option<String>,
    pub region: Option<String>,
    pub size: i64,
    pub favorite: bool,
    pub hidden: bool,
    pub missing: bool,
    /// Only an archive exists; it must be unpacked before it can be played.
    pub packed: bool,
    pub added_at: i64,
    pub last_played: Option<i64>,
    pub playtime: i64,
    pub play_count: i64,
    pub cover: Option<String>,
    pub cover_thumb: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub copies: i64,
}

/// One file or folder on disk that holds a game. A game can have several (an ISO, a folder,
/// the archive it came in), which is how duplicates are found.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    pub path: String,
    pub format: String,
    pub size: i64,
    pub archive: bool,
    pub primary: bool,
    /// Registered in the emulator's own config (e.g. RPCS3 games.yml).
    pub registered: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Emulator {
    pub id: String,
    pub name: String,
    pub exe: String,
    pub version: Option<String>,
    pub platforms: Vec<String>,
    pub args: Vec<String>,
    pub default_args: Vec<String>,
    pub managed: bool,
    pub missing: bool,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub exists: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub games: usize,
    pub new_games: usize,
    pub emulators: usize,
    pub copies: usize,
    pub missing: usize,
    pub millis: u128,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub detail: String,
    pub done: usize,
    pub total: usize,
}

impl Progress {
    pub fn new(stage: &str, detail: impl Into<String>, done: usize, total: usize) -> Self {
        Progress { stage: stage.into(), detail: detail.into(), done, total }
    }
}
