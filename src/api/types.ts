// Hand-mirrored from docs/contract.md until the generated bindings replace it.

export type GameSource = "manual" | "steam" | "epic" | "gog";
export type SessionSource = "auto" | "manual" | "imported";
export type MatchMode = "exact_path" | "install_dir_prefix" | "exe_name";
export type Theme = "dark" | "light" | "system";

export interface AppInfo {
  version: string;
  dataDir: string;
  dbPath: string;
  logDir: string;
}

export interface Game {
  id: number;
  name: string;
  sortName: string;
  source: GameSource;
  sourceId: string | null;
  installDir: string | null;
  iconPath: string | null;
  coverPath: string | null;
  hidden: boolean;
  afkDetectionEnabled: boolean;
  createdAt: number;
}

export interface GameExecutable {
  id: number;
  gameId: number;
  exePath: string;
  matchMode: MatchMode;
}

export interface Session {
  id: number;
  gameId: number;
  startedAt: number;
  /** null while the session is active */
  endedAt: number | null;
  durationSecs: number;
  afkSecs: number;
  source: SessionSource;
  note: string | null;
}

export interface SessionWithGame extends Session {
  gameName: string;
}

export interface ActiveSession {
  sessionId: number;
  gameId: number;
  gameName: string;
  startedAt: number;
  elapsedSecs: number;
  afkSecs: number;
  isAfk: boolean;
  source: "auto" | "manual";
}

export interface TrackerStatus {
  trackingEnabled: boolean;
  active: ActiveSession[];
  lastPollAt: number | null;
}

export interface Tag {
  id: number;
  name: string;
}

export interface MergeResult {
  survivorId: number;
  mergedSessions: number;
  mergedExecutables: number;
}

export type ExportFormat = "csv" | "json";

export interface ExportOptions {
  format: ExportFormat;
  /** inclusive lower bound on startedAt */
  from: number | null;
  /** exclusive upper bound on startedAt */
  to: number | null;
  gameId: number | null;
}

export interface ExportResult {
  path: string;
  sessions: number;
  games: number;
}

export interface BackupInfo {
  path: string;
  createdAt: number;
  sizeBytes: number;
}

export interface GameSummary {
  game: Game;
  /** Sorted by name */
  tags: Tag[];
  totalSecs: number;
  todaySecs: number;
  weekSecs: number;
  sessionCount: number;
  lastPlayedAt: number | null;
  importedMinutes: number;
}

export interface GameTotal {
  gameId: number;
  gameName: string;
  totalSecs: number;
}

export interface DashboardStats {
  todaySecs: number;
  weekSecs: number;
  allTimeSecs: number;
  gamesPlayedCount: number;
  topGames: GameTotal[];
  recentSessions: SessionWithGame[];
}

export interface DailyTotal {
  /** Local date, "YYYY-MM-DD" */
  date: string;
  totalSecs: number;
}

export interface HourlyTotal {
  hour: number;
  totalSecs: number;
}

export interface Settings {
  pollIntervalSecs: number;
  afkThresholdSecs: number;
  sessionMergeGapSecs: number;
  requireForeground: boolean;
  trackingEnabled: boolean;
  theme: Theme;
  importOnStartup: boolean;
  notificationsEnabled: boolean;
  /** 0 means off; otherwise 300 to 14400 */
  reminderIntervalSecs: number;
  startupHintDismissed: boolean;
}

export type LimitScope = "global" | "game";
/** notify: once per day when reached. notify_repeat: again every 15 minutes while still playing past it. */
export type LimitAction = "notify" | "notify_repeat";

export interface Limit {
  id: number;
  scope: LimitScope;
  /** null for global */
  gameId: number | null;
  gameName: string | null;
  dailySecs: number;
  action: LimitAction;
}

export interface LimitInput {
  scope: LimitScope;
  gameId: number | null;
  dailySecs: number;
  action: LimitAction;
}

export interface LimitStatus {
  limit: Limit;
  /** Today's play counted against this limit, active sessions included, AFK excluded */
  usedTodaySecs: number;
  remainingSecs: number;
  reached: boolean;
}

export interface Reminder {
  sessionId: number;
  gameId: number;
  gameName: string;
  elapsedSecs: number;
}

export type ImportSource = "steam" | "epic" | "gog";

export interface ImportSourceStatus {
  source: ImportSource;
  /** The launcher was found on this machine */
  available: boolean;
  /** Install path when available, otherwise why not */
  detail: string | null;
  lastScanAt: number | null;
}

export interface DiscoveredGame {
  source: ImportSource;
  sourceId: string;
  name: string;
  installDir: string | null;
  /** Normalised exact executable paths known from the launcher; may be empty */
  executables: string[];
  /** Launcher-reported playtime, when the launcher tracks it */
  playtimeMinutes: number | null;
  /** A game with this source and sourceId already exists */
  alreadyImported: boolean;
}

export interface ImportResult {
  source: ImportSource;
  discovered: number;
  added: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface SteamApiSettings {
  apiKey: string | null;
  steamId64: string | null;
}

export interface SettingsUpdate {
  pollIntervalSecs: number | null;
  afkThresholdSecs: number | null;
  sessionMergeGapSecs: number | null;
  requireForeground: boolean | null;
  trackingEnabled: boolean | null;
  theme: Theme | null;
  importOnStartup: boolean | null;
  notificationsEnabled: boolean | null;
  reminderIntervalSecs: number | null;
  startupHintDismissed: boolean | null;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  /** Release date as an ISO string, when the feed provides it */
  date: string | null;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface GameUpdate {
  name: string | null;
  hidden: boolean | null;
  afkDetectionEnabled: boolean | null;
}

export interface SessionUpdate {
  startedAt: number | null;
  endedAt: number | null;
  /** null keeps, "" clears */
  note: string | null;
}

export interface SessionQuery {
  gameId: number | null;
  from: number | null;
  to: number | null;
  limit: number;
  offset: number;
}

export type ApiEventMap = {
  "session-started": ActiveSession;
  "session-updated": ActiveSession;
  "session-ended": Session;
  "tracker-status-changed": TrackerStatus;
  "library-changed": null;
  "sessions-changed": null;
  "limit-reached": LimitStatus;
  reminder: Reminder;
  "update-available": UpdateInfo;
  "update-progress": UpdateProgress;
};

export type ApiEventName = keyof ApiEventMap;
