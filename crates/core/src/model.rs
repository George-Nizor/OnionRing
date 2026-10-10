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
    /// Launch options for this game only, one argv token per entry (same placeholders as the
    /// emulator's: `{rom}`, `{rom_dir}`, `{emu_dir}`). `None` uses the emulator's.
    pub user_args: Option<Vec<String>>,
}

/// A list of games the player put together. Games are remembered by key, so a game that
/// disappears and comes back (an unplugged drive) is still in its collections.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub created: i64,
    /// Games currently in the library, in the order they were added.
    pub game_ids: Vec<i64>,
}

/// One time a game was played.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: i64,
    pub game_id: i64,
    pub started: i64,
    pub ended: i64,
    pub seconds: i64,
}

/// Time played in one week, Monday 00:00 to the next Monday.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WeekTotal {
    /// Monday 00:00 local time, as a Unix time.
    pub start: i64,
    pub seconds: i64,
    pub sessions: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayTotals {
    pub seconds: i64,
    pub sessions: i64,
    /// Different games played at least once.
    pub games: i64,
}

/// A game ranked by time played.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TopGame {
    pub game_id: i64,
    pub title: String,
    pub platform: String,
    pub seconds: i64,
    pub sessions: i64,
    pub last_played: i64,
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
