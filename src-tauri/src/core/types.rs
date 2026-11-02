//! IPC types shared with the frontend (see `docs/contract.md`).
//!
//! specta forbids exporting `i64`, so ids and counts are `i32` and timestamps and durations
//! (`i64` in Rust) are exported as `number` through `#[specta(type = specta_typescript::Number)]`. Seconds-since-1970
//! and durations stay exact in a JS number.

use serde::{Deserialize, Serialize};
use specta::Type;

/// A unix-seconds timestamp or a duration passed as a command argument. Exported as `number`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ts(pub i64);

impl Type for Ts {
    fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
        <specta_typescript::Number as Type>::definition(types)
    }
}

macro_rules! str_enum {
    ($name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum GameSource {
    Manual,
    Steam,
    Epic,
    Gog,
    Amazon,
}
str_enum!(GameSource {
    Manual => "manual",
    Steam => "steam",
    Epic => "epic",
    Gog => "gog",
    Amazon => "amazon",
});

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Auto,
    Manual,
    Imported,
}
str_enum!(SessionSource { Auto => "auto", Manual => "manual", Imported => "imported" });

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    ExactPath,
    InstallDirPrefix,
    ExeName,
}
str_enum!(MatchMode {
    ExactPath => "exact_path",
    InstallDirPrefix => "install_dir_prefix",
    ExeName => "exe_name",
});

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
    Light,
    System,
}

/// The source of an [`ActiveSession`]; the contract restricts it to `auto` or `manual`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ActiveSource {
    Auto,
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub log_dir: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i32,
    pub name: String,
    pub sort_name: String,
    pub source: GameSource,
    pub source_id: Option<String>,
    pub install_dir: Option<String>,
    pub icon_path: Option<String>,
    pub cover_path: Option<String>,
    pub hidden: bool,
    pub afk_detection_enabled: bool,
    #[specta(type = specta_typescript::Number)]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct GameExecutable {
    pub id: i32,
    pub game_id: i32,
    pub exe_path: String,
    pub match_mode: MatchMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: i32,
    pub game_id: i32,
    #[specta(type = specta_typescript::Number)]
    pub started_at: i64,
    #[specta(type = Option<specta_typescript::Number>)]
    pub ended_at: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub duration_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub afk_secs: i64,
    pub source: SessionSource,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionWithGame {
    #[serde(flatten)]
    pub session: Session,
    pub game_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    pub session_id: i32,
    pub game_id: i32,
    pub game_name: String,
    #[specta(type = specta_typescript::Number)]
    pub started_at: i64,
    #[specta(type = specta_typescript::Number)]
    pub elapsed_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub afk_secs: i64,
    pub is_afk: bool,
    pub source: ActiveSource,
    /// The game is hidden: tracked, but left out of default views.
    pub hidden: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackerStatus {
    pub tracking_enabled: bool,
    pub active: Vec<ActiveSession>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_poll_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub game: Game,
    #[specta(type = specta_typescript::Number)]
    pub total_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub today_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub week_secs: i64,
    pub session_count: i32,
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_played_at: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub imported_minutes: i64,
    /// Sorted by name.
    pub tags: Vec<Tag>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct GameTotal {
    pub game_id: i32,
    pub game_name: String,
    #[specta(type = specta_typescript::Number)]
    pub total_secs: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    #[specta(type = specta_typescript::Number)]
    pub today_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub week_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub all_time_secs: i64,
    pub games_played_count: i32,
    pub top_games: Vec<GameTotal>,
    pub recent_sessions: Vec<SessionWithGame>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyTotal {
    pub date: String,
    #[specta(type = specta_typescript::Number)]
    pub total_secs: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct HourlyTotal {
    pub hour: i32,
    #[specta(type = specta_typescript::Number)]
    pub total_secs: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub poll_interval_secs: u32,
    #[specta(type = specta_typescript::Number)]
    pub afk_threshold_secs: i64,
    #[specta(type = specta_typescript::Number)]
    pub session_merge_gap_secs: i64,
    pub require_foreground: bool,
    pub tracking_enabled: bool,
    pub theme: Theme,
    pub import_on_startup: bool,
    pub notifications_enabled: bool,
    /// 0 turns reminders off; otherwise 300 to 14400.
    pub reminder_interval_secs: u32,
    /// True once the user answered the "start with Windows" hint on the dashboard.
    pub startup_hint_dismissed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub poll_interval_secs: Option<u32>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub afk_threshold_secs: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub session_merge_gap_secs: Option<i64>,
    pub require_foreground: Option<bool>,
    pub tracking_enabled: Option<bool>,
    pub theme: Option<Theme>,
    pub import_on_startup: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub reminder_interval_secs: Option<u32>,
    pub startup_hint_dismissed: Option<bool>,
}

/// Release found by the updater feed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Release date as an ISO string, when the feed provides it.
    pub date: Option<String>,
    pub notes: Option<String>,
}

/// Payload of the `update-progress` event.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    #[specta(type = specta_typescript::Number)]
    pub downloaded: u64,
    #[specta(type = Option<specta_typescript::Number>)]
    pub total: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct GameUpdate {
    pub name: Option<String>,
    pub hidden: Option<bool>,
    pub afk_detection_enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
    #[specta(type = Option<specta_typescript::Number>)]
    pub started_at: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub ended_at: Option<i64>,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionQuery {
    pub game_id: Option<i32>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub from: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub to: Option<i64>,
    pub limit: u32,
    pub offset: u32,
    /// Include sessions of hidden games (default false).
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    Steam,
    Epic,
    Gog,
    Heroic,
}
str_enum!(ImportSource { Steam => "steam", Epic => "epic", Gog => "gog", Heroic => "heroic" });

impl ImportSource {
    pub const ALL: [ImportSource; 4] = [Self::Steam, Self::Epic, Self::Gog, Self::Heroic];

    /// The library source of this launcher's games. Heroic manages games of several stores;
    /// each of its games carries its own [`DiscoveredGame::game_source`].
    pub fn game_source(self) -> GameSource {
        match self {
            Self::Steam => GameSource::Steam,
            Self::Epic => GameSource::Epic,
            Self::Gog => GameSource::Gog,
            Self::Heroic => GameSource::Manual,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceStatus {
    pub source: ImportSource,
    /// The launcher was found on this machine.
    pub available: bool,
    /// Install path when available, otherwise why not.
    pub detail: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_scan_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredGame {
    pub source: ImportSource,
    pub source_id: String,
    pub name: String,
    pub install_dir: Option<String>,
    /// Normalised exact executable paths known from the launcher; may be empty.
    pub executables: Vec<String>,
    /// Launcher-reported playtime, when the launcher tracks it.
    #[specta(type = Option<specta_typescript::Number>)]
    pub playtime_minutes: Option<i64>,
    /// A game with this source and sourceId already exists.
    pub already_imported: bool,
    /// Library source when it differs from the launcher's own (Heroic: epic, gog, amazon or
    /// manual). Internal; not part of the IPC type.
    #[serde(skip)]
    pub game_source: Option<GameSource>,
}

impl DiscoveredGame {
    /// The `games.source` value this game is stored under.
    pub fn library_source(&self) -> GameSource {
        self.game_source
            .unwrap_or_else(|| self.source.game_source())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub source: ImportSource,
    pub discovered: i32,
    pub added: i32,
    pub updated: i32,
    pub skipped: i32,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct SteamApiSettings {
    pub api_key: Option<String>,
    pub steam_id64: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum LimitScope {
    Global,
    Game,
}
str_enum!(LimitScope { Global => "global", Game => "game" });

/// `Notify` fires once per day; `NotifyRepeat` fires again every 15 minutes while still playing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum LimitAction {
    Notify,
    NotifyRepeat,
}
str_enum!(LimitAction { Notify => "notify", NotifyRepeat => "notify_repeat" });

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct Limit {
    pub id: i32,
    pub scope: LimitScope,
    /// `None` for a global limit.
    pub game_id: Option<i32>,
    pub game_name: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub daily_secs: i64,
    pub action: LimitAction,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct LimitInput {
    pub scope: LimitScope,
    pub game_id: Option<i32>,
    #[specta(type = specta_typescript::Number)]
    pub daily_secs: i64,
    pub action: LimitAction,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct LimitStatus {
    pub limit: Limit,
    /// Today's play counted against this limit, active sessions included, AFK excluded.
    #[specta(type = specta_typescript::Number)]
    pub used_today_secs: i64,
    /// `max(0, dailySecs - usedTodaySecs)`.
    #[specta(type = specta_typescript::Number)]
    pub remaining_secs: i64,
    pub reached: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub session_id: i32,
    pub game_id: i32,
    pub game_name: String,
    #[specta(type = specta_typescript::Number)]
    pub elapsed_secs: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeResult {
    pub survivor_id: i32,
    pub merged_sessions: i32,
    pub merged_executables: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Json,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub format: ExportFormat,
    /// Inclusive lower bound on startedAt.
    #[specta(type = Option<specta_typescript::Number>)]
    pub from: Option<i64>,
    /// Exclusive upper bound on startedAt.
    #[specta(type = Option<specta_typescript::Number>)]
    pub to: Option<i64>,
    pub game_id: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub sessions: i32,
    pub games: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    #[specta(type = specta_typescript::Number)]
    pub created_at: i64,
    #[specta(type = specta_typescript::Number)]
    pub size_bytes: i64,
}

// ------------------------------------------------------------ Phase 7

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanRoot {
    pub id: i32,
    pub path: String,
    #[specta(type = specta_typescript::Number)]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExeCandidate {
    pub path: String,
    #[specta(type = specta_typescript::Number)]
    pub size_bytes: i64,
    /// Higher is more likely the main game executable.
    pub score: i32,
    /// Short human-readable reasons, e.g. "name matches folder".
    pub reasons: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum ScanConfidence {
    High,
    Medium,
    Low,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanCandidate {
    pub root_id: i32,
    /// Absolute path of the top-level folder under the root.
    pub folder: String,
    /// Cleaned from the folder name.
    pub suggested_name: String,
    /// Sorted by score, descending; junk excluded; at most 8.
    pub executables: Vec<ExeCandidate>,
    pub confidence: ScanConfidence,
    /// A library game already has an executable rule under this folder.
    pub existing_game_id: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanSelection {
    pub folder: String,
    pub name: String,
    pub exe_path: String,
    /// Optional cover to download, from a metadata match.
    pub cover_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataMatch {
    pub steam_app_id: u32,
    pub name: String,
    pub cover_url: String,
}
